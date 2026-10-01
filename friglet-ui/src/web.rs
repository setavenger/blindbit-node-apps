//! Web server: the embedded UI and the JSON API of docs/api.md.
//!
//! Every state-changing request (any method but GET/HEAD) needs the CSRF token (double
//! submit: `X-CSRF-Token` equal to the `friglet_csrf` cookie from `GET /api/session`) and
//! passes the origin policy (same-origin: `Origin`, else `Referer`, must match the request's
//! own `Host` or a `FRIGLET_UI_ALLOWED_HOSTS` entry), in every auth mode. `/api/*` needs a
//! session in password mode, except `/api/session`, `/api/login`, `/api/logout` and the
//! unauthenticated `/api/widget`; `/healthz` is always open.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use tokio::sync::oneshot;

use crate::auth::{CSRF_COOKIE, LoginError, SESSION_COOKIE, SESSION_TTL};
use crate::config::{AuthMode, Platform};
use crate::setup::{self, Outcome};
use crate::supervisor::{Command, Shared};
use crate::util::{ct_eq, random_hex};

pub type AppState = Arc<Shared>;

pub fn router(shared: Arc<Shared>) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/api/widget", get(widget))
        .route("/api/session", get(session))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/state", get(state))
        .route("/api/setup/validate", post(setup_validate))
        .route("/api/setup", post(setup_submit))
        .route("/api/wallet/forget", post(wallet_forget))
        .route("/api/wallet/restore-previous", post(wallet_restore))
        .route(
            "/api/wallet/discard-previous",
            post(wallet_discard_previous),
        )
        .route("/api/scan/pause", post(scan_pause))
        .route("/api/scan/resume", post(scan_resume))
        .route("/api/connect", get(connect))
        .route("/api/connect/test", post(connect_test))
        .fallback(static_or_404)
        .layer(middleware::from_fn_with_state(shared.clone(), guard))
        .with_state(shared)
}

// ---------------------------------------------------------------------------------------------
// Errors and helpers
// ---------------------------------------------------------------------------------------------

pub fn api_error(status: StatusCode, code: &str, message: &str, field: Option<&str>) -> Response {
    let mut resp = (
        status,
        Json(json!({"error": {"code": code, "message": message, "field": field}})),
    )
        .into_response();
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

#[allow(clippy::result_large_err)] // the error is the ready HTTP response
fn parse_body<T: DeserializeOwned>(body: &Bytes) -> Result<T, Response> {
    serde_json::from_slice(body).map_err(|e| {
        api_error(
            StatusCode::BAD_REQUEST,
            "bad-request",
            &format!("invalid JSON body: {e}"),
            None,
        )
    })
}

pub fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
}

fn is_https(headers: &HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("https"))
}

/// `scheme://authority/...` → lower-case authority.
fn authority_of(url: &str) -> Option<String> {
    let rest = url.split_once("://").map(|(_, r)| r)?;
    let auth = rest.split(['/', '?', '#']).next()?;
    if auth.is_empty() {
        None
    } else {
        Some(auth.to_ascii_lowercase())
    }
}

fn host_part(authority: &str) -> &str {
    if authority.starts_with('[') {
        return authority
            .split(']')
            .next()
            .map(|h| &authority[..h.len() + 1])
            .unwrap_or(authority);
    }
    authority
        .rsplit_once(':')
        .map(|(h, _)| h)
        .unwrap_or(authority)
}

/// Same-origin policy for state-changing requests.
pub fn origin_allowed(headers: &HeaderMap, allowed: &[String]) -> Result<(), &'static str> {
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(str::to_ascii_lowercase)
        .ok_or("missing Host header")?;
    let source = if let Some(origin) = headers.get(header::ORIGIN) {
        let o = origin.to_str().map_err(|_| "invalid Origin header")?;
        if o == "null" {
            return Err("opaque Origin");
        }
        Some(authority_of(o).ok_or("invalid Origin header")?)
    } else if let Some(referer) = headers.get(header::REFERER) {
        let r = referer.to_str().map_err(|_| "invalid Referer header")?;
        Some(authority_of(r).ok_or("invalid Referer header")?)
    } else {
        // Not sent by a browser page; the CSRF token is still required.
        None
    };
    let Some(src) = source else { return Ok(()) };
    if src == host {
        return Ok(());
    }
    let matches_entry = |entry: &String| {
        let e = entry.to_ascii_lowercase();
        if e.contains(':') && !e.starts_with('[') || e.contains("]:") {
            e == src
        } else {
            e == host_part(&src)
        }
    };
    if allowed.iter().any(matches_entry) {
        Ok(())
    } else {
        Err("Origin does not match Host")
    }
}

fn client_key(shared: &Shared, peer: Option<SocketAddr>, headers: &HeaderMap) -> String {
    let Some(peer) = peer else {
        return "unknown".into();
    };
    let ip = peer.ip().to_canonical();
    if shared.cfg.trusted_proxies.iter().any(|n| n.contains(ip))
        && let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok())
        && let Some(last) = xff.rsplit(',').next().map(str::trim)
        && let Ok(client) = last.parse::<IpAddr>()
    {
        return client.to_canonical().to_string();
    }
    ip.to_string()
}

async fn guard(State(shared): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let method = req.method().clone();
    let headers = req.headers().clone();
    let state_changing = !(method == Method::GET || method == Method::HEAD);

    if state_changing {
        if let Err(why) = origin_allowed(&headers, &shared.cfg.allowed_hosts) {
            return api_error(StatusCode::FORBIDDEN, "origin-rejected", why, None);
        }
        let cookie_token = cookie(&headers, CSRF_COOKIE);
        let header_token = headers.get("x-csrf-token").and_then(|v| v.to_str().ok());
        let ok = matches!((cookie_token.as_deref(), header_token), (Some(c), Some(h)) if !c.is_empty() && ct_eq(c.as_bytes(), h.as_bytes()));
        if !ok {
            return api_error(
                StatusCode::FORBIDDEN,
                "csrf-invalid",
                "missing or stale CSRF token; reload the page",
                None,
            );
        }
    }

    let public = matches!(
        path.as_str(),
        "/api/session" | "/api/login" | "/api/logout" | "/api/widget" | "/healthz"
    );
    if path.starts_with("/api/") && !public {
        let session = cookie(&headers, SESSION_COOKIE);
        if !shared.auth.is_authenticated(session.as_deref()) {
            return api_error(
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "log in first",
                None,
            );
        }
    }

    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert("referrer-policy", HeaderValue::from_static("same-origin"));
    if path.starts_with("/api/") {
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    resp
}

// ---------------------------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------------------------

async fn widget(State(shared): State<AppState>) -> Response {
    Json(shared.widget()).into_response()
}

fn banner(shared: &Shared) -> Option<String> {
    match shared.cfg.auth {
        AuthMode::None => Some(
            "No login: FRIGLET_UI_AUTH=none. Anyone who can reach this page can change the wallet. \
             Put your own authentication in front of it."
                .to_string(),
        ),
        _ => None,
    }
}

async fn session(State(shared): State<AppState>, headers: HeaderMap) -> Response {
    let existing = cookie(&headers, CSRF_COOKIE).filter(|c| c.len() == 64);
    let token = existing.clone().unwrap_or_else(|| random_hex(32));
    let authenticated = shared
        .auth
        .is_authenticated(cookie(&headers, SESSION_COOKIE).as_deref());
    let body = json!({
        "authenticated": authenticated,
        "auth_mode": shared.cfg.auth.as_str(),
        "csrf_token": token,
        "platform": shared.cfg.platform.as_str(),
        "banner": banner(&shared),
    });
    let mut resp = Json(body).into_response();
    if existing.is_none() {
        let secure = if is_https(&headers) { "; Secure" } else { "" };
        if let Ok(v) = HeaderValue::from_str(&format!(
            "{CSRF_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict{secure}"
        )) {
            resp.headers_mut().append(header::SET_COOKIE, v);
        }
    }
    resp
}

#[derive(Deserialize)]
struct LoginBody {
    password: String,
}

async fn login(
    State(shared): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let body: LoginBody = match parse_body(&body) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let client = client_key(&shared, Some(peer), &headers);
    match shared.auth.login(&client, &body.password) {
        Ok(token) => {
            let secure = if is_https(&headers) { "; Secure" } else { "" };
            let mut resp = StatusCode::NO_CONTENT.into_response();
            if let Ok(v) = HeaderValue::from_str(&format!(
                "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{secure}",
                SESSION_TTL.as_secs()
            )) {
                resp.headers_mut().append(header::SET_COOKIE, v);
            }
            resp
        }
        Err(LoginError::BadPassword) => {
            tracing::warn!(client = %crate::redact::redact(&client), "failed UI login");
            api_error(
                StatusCode::UNAUTHORIZED,
                "bad-password",
                "wrong password",
                Some("password"),
            )
        }
        Err(LoginError::RateLimited {
            retry_after_seconds,
        }) => {
            let mut resp = (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({
                    "error": {
                        "code": "rate-limited",
                        "message": format!("too many attempts; try again in {retry_after_seconds} s"),
                        "field": null,
                        "retry_after_seconds": retry_after_seconds,
                    },
                })),
            )
                .into_response();
            if let Ok(v) = HeaderValue::from_str(&retry_after_seconds.to_string()) {
                resp.headers_mut().insert(header::RETRY_AFTER, v);
            }
            resp
        }
        Err(LoginError::NotPasswordMode) => api_error(
            StatusCode::BAD_REQUEST,
            "no-login",
            "this installation does not use a friglet password",
            None,
        ),
    }
}

async fn logout(State(shared): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = cookie(&headers, SESSION_COOKIE) {
        shared.auth.logout(&token);
    }
    let mut resp = StatusCode::NO_CONTENT.into_response();
    resp.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_static("friglet_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"),
    );
    resp
}

async fn state(State(shared): State<AppState>) -> Response {
    Json(shared.state()).into_response()
}

#[derive(Deserialize)]
struct ValidateBody {
    descriptor: String,
    birthday_height: u64,
}

fn outcome_error(o: &Outcome) -> Response {
    let status = match o.code {
        setup::EXIT_EXISTS => StatusCode::CONFLICT,
        setup::EXIT_IO => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::BAD_REQUEST,
    };
    api_error(
        status,
        o.error_code.as_deref().unwrap_or("invalid"),
        &o.message,
        o.field.as_deref(),
    )
}

async fn setup_validate(State(shared): State<AppState>, body: Bytes) -> Response {
    let body: ValidateBody = match parse_body(&body) {
        Ok(b) => b,
        Err(r) => return r,
    };
    if let Some(m) = &shared.cfg.unsupported_network {
        return api_error(StatusCode::BAD_REQUEST, "unsupported-network", m, None);
    }
    match setup::validate(
        &body.descriptor,
        body.birthday_height,
        shared.cfg.network,
        shared.node_tip(),
    ) {
        Ok((_, identity)) => Json(json!({
            "sp_address": identity.sp_address,
            "network": identity.network.as_str(),
            "key_fingerprint": identity.key_fingerprint,
        }))
        .into_response(),
        Err(o) => outcome_error(&o),
    }
}

#[derive(Deserialize)]
struct SetupBody {
    descriptor: String,
    birthday_height: u64,
    #[serde(default)]
    replace: bool,
}

async fn setup_submit(State(shared): State<AppState>, body: Bytes) -> Response {
    let body: SetupBody = match parse_body(&body) {
        Ok(b) => b,
        Err(r) => return r,
    };
    if let Some(m) = &shared.cfg.unsupported_network {
        return api_error(StatusCode::BAD_REQUEST, "unsupported-network", m, None);
    }
    let (keys, _identity) = match setup::validate(
        &body.descriptor,
        body.birthday_height,
        shared.cfg.network,
        shared.node_tip(),
    ) {
        Ok(v) => v,
        Err(o) => return outcome_error(&o),
    };
    let outcome = submit_setup(&shared, keys, body.birthday_height, body.replace).await;
    if outcome.ok {
        accepted(&shared)
    } else {
        outcome_error(&outcome)
    }
}

/// Hand a validated setup to the supervisor and wait for its answer.
pub async fn submit_setup(
    shared: &Shared,
    keys: crate::descriptor::WalletKeys,
    birthday: u64,
    replace: bool,
) -> Outcome {
    let (tx, rx) = oneshot::channel();
    let cmd = Command::Setup {
        keys: Box::new(keys),
        birthday,
        replace,
        reply: tx,
    };
    if shared.cmd.send(cmd).await.is_err() {
        return Outcome::err(setup::EXIT_IO, "io", None, "the supervisor is not running");
    }
    match tokio::time::timeout(Duration::from_secs(180), rx).await {
        Ok(Ok(o)) => o,
        _ => Outcome::err(
            setup::EXIT_IO,
            "io",
            None,
            "the supervisor did not answer in time",
        ),
    }
}

async fn wallet_forget(State(shared): State<AppState>) -> Response {
    let (tx, rx) = oneshot::channel();
    if shared
        .cmd
        .send(Command::Forget { reply: tx })
        .await
        .is_err()
    {
        return api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "io",
            "supervisor not running",
            None,
        );
    }
    match rx.await {
        Ok(Ok(())) => accepted(&shared),
        Ok(Err(m)) => api_error(StatusCode::INTERNAL_SERVER_ERROR, "io", &m, None),
        Err(_) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "io",
            "supervisor did not answer",
            None,
        ),
    }
}

async fn wallet_restore(State(shared): State<AppState>) -> Response {
    let (tx, rx) = oneshot::channel();
    if shared
        .cmd
        .send(Command::RestorePrevious { reply: tx })
        .await
        .is_err()
    {
        return api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "io",
            "supervisor not running",
            None,
        );
    }
    match rx.await {
        Ok(Ok(true)) => accepted(&shared),
        Ok(Ok(false)) => api_error(
            StatusCode::NOT_FOUND,
            "no-previous-wallet",
            "there is no previous wallet to restore",
            None,
        ),
        Ok(Err(m)) => api_error(StatusCode::INTERNAL_SERVER_ERROR, "io", &m, None),
        Err(_) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "io",
            "supervisor did not answer",
            None,
        ),
    }
}

/// 202 with the full `GET /api/state` document.
fn accepted(shared: &Shared) -> Response {
    (StatusCode::ACCEPTED, Json(shared.state())).into_response()
}

async fn wallet_discard_previous(State(shared): State<AppState>) -> Response {
    let (tx, rx) = oneshot::channel();
    if shared
        .cmd
        .send(Command::DiscardPrevious { reply: tx })
        .await
        .is_err()
    {
        return api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "io",
            "supervisor not running",
            None,
        );
    }
    match rx.await {
        Ok(Ok(true)) => StatusCode::NO_CONTENT.into_response(),
        Ok(Ok(false)) => api_error(
            StatusCode::NOT_FOUND,
            "no-previous-wallet",
            "there is no previous wallet on this box",
            None,
        ),
        Ok(Err(m)) => api_error(StatusCode::CONFLICT, "transaction-in-progress", &m, None),
        Err(_) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "io",
            "supervisor did not answer",
            None,
        ),
    }
}

async fn scan_pause(State(shared): State<AppState>) -> Response {
    let (tx, rx) = oneshot::channel();
    let _ = shared.cmd.send(Command::Pause { reply: tx }).await;
    let _ = rx.await;
    accepted(&shared)
}

async fn scan_resume(State(shared): State<AppState>) -> Response {
    let (tx, rx) = oneshot::channel();
    let _ = shared.cmd.send(Command::Resume { reply: tx }).await;
    let _ = rx.await;
    accepted(&shared)
}

// ---------------------------------------------------------------------------------------------
// Connect
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct Endpoint {
    pub label: String,
    pub host: String,
    pub port: u16,
    pub protocol: String,
    pub string: String,
    pub note: String,
}

fn endpoint(label: &str, host: &str, port: u16, tls: bool, note: &str) -> Endpoint {
    Endpoint {
        label: label.into(),
        host: host.into(),
        port,
        protocol: if tls { "ssl" } else { "tcp" }.into(),
        string: format!("{host}:{port}:{}", if tls { "s" } else { "t" }),
        note: note.into(),
    }
}

/// Endpoints for this platform, recommended first.
pub fn endpoints(shared: &Shared, request_host: Option<&str>) -> Vec<Endpoint> {
    let cfg = &shared.cfg;
    let host = cfg
        .public_host
        .clone()
        .or_else(|| request_host.map(|h| host_part(&h.to_ascii_lowercase()).to_string()))
        .unwrap_or_else(|| "localhost".into());
    let mut out = Vec::new();
    if let Some(tls) = cfg.electrum_tls {
        out.push(endpoint(
            "LAN, encrypted",
            &host,
            tls.port(),
            true,
            "Self-signed certificate: your wallet asks you to trust it on the first connection. Compare the fingerprint below.",
        ));
    }
    if let Some(tcp) = cfg.electrum_tcp {
        let note = if cfg.platform == Platform::Startos {
            "Use the ssl:// address StartOS shows for this service under Interfaces; StartOS encrypts it. Trust the StartOS root CA in your wallet's computer first."
        } else {
            "Unencrypted: anyone on your network can see this wallet's traffic. Prefer the encrypted endpoint on a LAN."
        };
        let label = if cfg.platform == Platform::Startos {
            "StartOS interface"
        } else {
            "LAN, unencrypted"
        };
        out.push(endpoint(label, &host, tcp.port(), false, note));
        if let Some(onion) = &cfg.onion_host {
            out.push(endpoint(
                "Tor",
                onion,
                tcp.port(),
                false,
                "Reachable over Tor from anywhere. Tor encrypts the connection; your wallet needs Tor enabled.",
            ));
        }
    }
    out
}

async fn connect(State(shared): State<AppState>, headers: HeaderMap) -> Response {
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let mut eps = endpoints(&shared, host).into_iter();
    let recommended = eps.next();
    let others: Vec<Endpoint> = eps.collect();
    Json(json!({
        "recommended": recommended,
        "others": others,
        "tls_fingerprint_sha256": shared.tls_fingerprint,
        "exposure_text": "Anyone who can reach this port can read this wallet's history. Use a dedicated Sparrow instance for this wallet: friglet serves only this one wallet.",
        "listeners_open": shared.electrum.listeners_open(),
    }))
    .into_response()
}

async fn connect_test(State(shared): State<AppState>, headers: HeaderMap) -> Response {
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let Some(ep) = endpoints(&shared, host).into_iter().next() else {
        return Json(json!({"ok": false, "detail": "Electrum listeners are switched off on this installation."})).into_response();
    };
    if !shared.electrum.listeners_open() {
        return Json(json!({"ok": false, "detail": "Electrum stays closed until the wallet identity is verified."})).into_response();
    }
    let tls = ep.protocol == "ssl";
    let target = format!("{}:{}", ep.host, ep.port);
    let mut tried = Vec::new();
    let mut addrs: Vec<SocketAddr> = tokio::net::lookup_host(target.as_str())
        .await
        .map(|a| a.collect())
        .unwrap_or_default();
    addrs.truncate(2);
    addrs.push(SocketAddr::from(([127, 0, 0, 1], ep.port)));
    for addr in addrs {
        match crate::electrum::server_version(addr, tls, Duration::from_secs(5)).await {
            Ok(v) => {
                let via = if addr.ip().is_loopback() && !tried.is_empty() {
                    format!(
                        " (via 127.0.0.1; {} did not answer from inside the box)",
                        ep.host
                    )
                } else {
                    String::new()
                };
                return Json(json!({"ok": true, "detail": format!("Electrum answers on this node at {}{via}: server.version {v}", ep.string)})).into_response();
            }
            Err(e) => tried.push(format!("{addr}: {e}")),
        }
    }
    Json(json!({"ok": false, "detail": format!("no answer: {}", crate::redact::redact(&tried.join("; ")))})).into_response()
}

// ---------------------------------------------------------------------------------------------
// Static assets
// ---------------------------------------------------------------------------------------------

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "txt" => "text/plain; charset=utf-8",
        "webmanifest" => "application/manifest+json",
        _ => "application/octet-stream",
    }
}

async fn static_or_404(req: Request) -> Response {
    let path = req.uri().path();
    if path.starts_with("/api/") || path == "/api" {
        return api_error(StatusCode::NOT_FOUND, "not-found", "no such endpoint", None);
    }
    if req.method() != Method::GET && req.method() != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    // The UI uses hash routes: no SPA fallback.
    let rel = match path.trim_start_matches('/') {
        "" => "index.html",
        r => r,
    };
    let Some(bytes) = crate::assets::get(rel) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut resp = Response::new(Body::from(bytes));
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type(rel)),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    resp
}

/// Content Security Policy from friglet-ui/DESIGN.md.
pub const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'";

#[cfg(test)]
mod tests {
    use super::*;

    fn h(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut m = HeaderMap::new();
        for (k, v) in pairs {
            m.append(
                axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                HeaderValue::from_str(v).unwrap(),
            );
        }
        m
    }

    #[test]
    fn same_origin_policy() {
        let none: Vec<String> = vec![];
        // Umbrel's gateway forwards the browser Host unchanged.
        assert!(
            origin_allowed(
                &h(&[
                    ("host", "umbrel.local:2104"),
                    ("origin", "http://umbrel.local:2104")
                ]),
                &none
            )
            .is_ok()
        );
        assert!(
            origin_allowed(
                &h(&[
                    ("host", "192.168.1.5:2104"),
                    ("referer", "http://192.168.1.5:2104/setup")
                ]),
                &none
            )
            .is_ok()
        );
        assert!(
            origin_allowed(
                &h(&[
                    ("host", "umbrel.local:2104"),
                    ("origin", "http://evil.example")
                ]),
                &none
            )
            .is_err()
        );
        assert!(
            origin_allowed(
                &h(&[("host", "umbrel.local:2104"), ("origin", "null")]),
                &none
            )
            .is_err()
        );
        assert!(
            origin_allowed(&h(&[("origin", "http://umbrel.local")]), &none).is_err(),
            "Host is required"
        );
        // No Origin and no Referer: a non-browser client (CSRF token still required).
        assert!(origin_allowed(&h(&[("host", "127.0.0.1:8097")]), &none).is_ok());
        // Extra allowed values, with and without port.
        let allowed = vec!["proxy.example:8443".to_string(), "mynode.local".to_string()];
        assert!(
            origin_allowed(
                &h(&[
                    ("host", "127.0.0.1:8097"),
                    ("origin", "https://proxy.example:8443")
                ]),
                &allowed
            )
            .is_ok()
        );
        assert!(
            origin_allowed(
                &h(&[
                    ("host", "127.0.0.1:8097"),
                    ("origin", "https://proxy.example:9999")
                ]),
                &allowed
            )
            .is_err()
        );
        assert!(
            origin_allowed(
                &h(&[
                    ("host", "127.0.0.1:8097"),
                    ("origin", "https://mynode.local:8098")
                ]),
                &allowed
            )
            .is_ok()
        );
    }

    #[test]
    fn cookie_parsing() {
        let m = h(&[("cookie", "a=1; friglet_csrf=abc; friglet_session=def")]);
        assert_eq!(cookie(&m, CSRF_COOKIE).as_deref(), Some("abc"));
        assert_eq!(cookie(&m, SESSION_COOKIE).as_deref(), Some("def"));
        assert_eq!(cookie(&m, "missing"), None);
    }

    #[test]
    fn authority_and_host_parts() {
        assert_eq!(
            authority_of("http://Umbrel.local:2104/x?y").as_deref(),
            Some("umbrel.local:2104")
        );
        assert_eq!(host_part("umbrel.local:2104"), "umbrel.local");
        assert_eq!(host_part("[::1]:8097"), "[::1]");
        assert_eq!(host_part("umbrel.local"), "umbrel.local");
    }
}
