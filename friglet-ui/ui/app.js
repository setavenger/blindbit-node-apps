// friglet-ui web assets. No build step: this file is served as-is by friglet-ui.
// Talks only to the JSON API in docs/api.md. Never stores or logs the descriptor.
"use strict";

(function () {
  const POLL_MS = 2000;
  const ROUTES = ["setup", "status", "connect", "wallet", "diagnostics", "login"];
  const TITLES = {
    loading: "Loading", login: "Sign in", setup: "Setup", status: "Status",
    connect: "Connect", wallet: "Wallet", diagnostics: "Diagnostics",
  };
  const SEVERITY_LABEL = { ok: "All good", info: "In progress", warn: "Check this", error: "Needs attention" };
  // Used only when the backend sends no headline/severity for a state id.
  const FALLBACK = {
    "needs-setup": ["info", "Set up your wallet"],
    "starting": ["info", "Starting friglet"],
    "verifying-identity": ["info", "Checking the wallet address"],
    "identity-mismatch": ["error", "The wallet address does not match"],
    "scanning": ["info", "Scanning for payments"],
    "up-to-date": ["ok", "Caught up with your node"],
    "scan-stopped": ["error", "Scanning stopped"],
    "paused": ["info", "Scanning is paused"],
    "oracle-unreachable": ["warn", "Cannot reach the oracle"],
    "oracle-dropped-resuming": ["warn", "The oracle connection dropped; resuming"],
    "peer-unreachable": ["warn", "Cannot reach your Bitcoin node"],
    "node-pruned": ["error", "Your Bitcoin node is pruned"],
    "node-syncing": ["warn", "Your Bitcoin node is still syncing"],
    "port-in-use": ["error", "A port friglet needs is already in use"],
    "rescanning-after-reset": ["warn", "Rescanning from the birthday"],
    "crash-loop": ["error", "friglet keeps stopping"],
    "unknown-exit": ["error", "friglet stopped unexpectedly"],
    "unsupported-network": ["error", "This network is not supported"],
    "ui-disconnected": ["error", "Lost contact with this box"],
  };
  const NETWORK_NAME = { bitcoin: "Bitcoin mainnet", signet: "Signet", testnet4: "Testnet4", testnet: "Testnet", regtest: "Regtest" };
  const PASSWORD_WHERE = {
    raspiblitz: "The password is shown in the RaspiBlitz SSH menu, under friglet.",
    mynode: "The password is shown on the friglet app page in the myNode dashboard.",
    compose: "The password is the FRIGLET_UI_PASSWORD value in the .env file next to your compose file.",
    startos: "The password is shown by the friglet actions in StartOS.",
  };

  const nf = new Intl.NumberFormat("en-US");
  const $ = (id) => document.getElementById(id);

  const app = {
    session: null,       // GET /api/session
    state: null,         // GET /api/state
    online: true,        // false = ui-disconnected
    lastOk: null,        // Date of the last successful /api/state
    view: null,
    focusNext: false,    // move focus to the view heading on the next view change
    polling: false,
    timer: null,
    lastAnnounced: null,
    connect: null,
    connectError: null,
    connectLoading: false,
    connectFor: undefined,
    pendingChange: null, // validated wallet change awaiting confirmation (kept in memory only)
    loginBlockedUntil: 0,
  };

  // ---------------------------------------------------------------------------
  // API
  // ---------------------------------------------------------------------------

  class ApiError extends Error {
    constructor(status, body, retryAfterHeader) {
      const e = body && body.error ? body.error : {};
      super(e.message || (status === 0 ? "Could not reach this box." : `The request failed (HTTP ${status}).`));
      this.status = status;
      this.code = e.code || null;
      this.field = e.field || null;
      const ra = body ? (body.retry_after_seconds ?? e.retry_after_seconds) : null;
      const hdr = retryAfterHeader ? parseInt(retryAfterHeader, 10) : NaN;
      this.retryAfter = Number.isFinite(ra) ? ra : (Number.isFinite(hdr) ? hdr : null);
    }
  }

  async function request(method, path, body) {
    const headers = { Accept: "application/json" };
    // "same-origin" keeps Origin/Referer on our own requests: the backend's
    // same-origin check rejects state-changing requests without them.
    const init = { method, headers, credentials: "same-origin", cache: "no-store", referrerPolicy: "same-origin" };
    if (method !== "GET") headers["X-CSRF-Token"] = (app.session && app.session.csrf_token) || "";
    if (body !== undefined) {
      headers["Content-Type"] = "application/json";
      init.body = JSON.stringify(body);
    }
    let res;
    try {
      res = await fetch(path, init);
    } catch (_) {
      throw new ApiError(0, null);
    }
    let data = null;
    const text = await res.text().catch(() => "");
    if (text) {
      try { data = JSON.parse(text); } catch (_) { data = null; }
    }
    if (!res.ok) throw new ApiError(res.status, data, res.headers.get("Retry-After"));
    return data;
  }

  // Every POST carries the CSRF token; a stale token is refreshed once.
  async function post(path, body, opts) {
    try {
      return await request("POST", path, body);
    } catch (err) {
      if (err.status === 403 && /csrf/i.test(err.code || "")) {
        await loadSession();
        return await request("POST", path, body);
      }
      if (err.status === 401 && !(opts && opts.login)) {
        loadSession().then(render, () => {});
      }
      throw err;
    }
  }

  async function loadSession() {
    app.session = await request("GET", "/api/session");
    return app.session;
  }

  // ---------------------------------------------------------------------------
  // Polling: /api/state every 2 s while the page is visible
  // ---------------------------------------------------------------------------

  async function tick() {
    if (app.polling) return;
    app.polling = true;
    clearTimeout(app.timer);
    try {
      if (!app.session || !app.session.authenticated) {
        await loadSession();
        setOnline(true);
      }
      if (app.session && app.session.authenticated) {
        app.state = await request("GET", "/api/state");
        app.lastOk = new Date();
        setOnline(true);
      }
    } catch (err) {
      if (err.status === 401) {
        try { await loadSession(); setOnline(true); } catch (_) { setOnline(false); }
      } else {
        setOnline(false);
      }
    } finally {
      app.polling = false;
    }
    render();
    schedule();
  }

  function schedule() {
    clearTimeout(app.timer);
    if (document.visibilityState === "visible") app.timer = setTimeout(tick, POLL_MS);
  }

  function setOnline(flag) {
    app.online = flag;
  }

  // ---------------------------------------------------------------------------
  // Helpers
  // ---------------------------------------------------------------------------

  function el(tag, attrs, ...children) {
    const node = document.createElement(tag);
    if (attrs) {
      for (const [k, v] of Object.entries(attrs)) {
        if (v === null || v === undefined || v === false) continue;
        if (k === "class") node.className = v;
        else if (k === "text") node.textContent = v;
        else node.setAttribute(k, v === true ? "" : v);
      }
    }
    for (const c of children) if (c !== null && c !== undefined) node.append(c);
    return node;
  }

  function setText(id, value, fallback) {
    const node = $(id);
    const text = value === null || value === undefined || value === "" ? (fallback ?? "—") : String(value);
    if (node.textContent !== text) node.textContent = text;
  }

  function showText(id, value) {
    const node = $(id);
    const has = value !== null && value !== undefined && value !== "";
    node.hidden = !has;
    if (has && node.textContent !== String(value)) node.textContent = String(value);
  }

  const num = (n) => (typeof n === "number" && Number.isFinite(n) ? nf.format(n) : null);
  const block = (n) => (num(n) === null ? null : `Block ${num(n)}`);
  const isObj = (v) => v !== null && typeof v === "object";

  function fmtTime(d) {
    return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  }
  function parseTime(iso) {
    if (!iso) return null;
    const d = new Date(iso);
    return Number.isNaN(d.getTime()) ? null : d;
  }
  function fmtDateTime(iso) {
    const d = parseTime(iso);
    return d ? d.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" }) : null;
  }
  function fmtDuration(sec) {
    if (sec < 60) return "less than a minute";
    const m = Math.round(sec / 60);
    if (m < 60) return `${m} minute${m === 1 ? "" : "s"}`;
    const h = Math.floor(m / 60);
    const mm = m % 60;
    if (h < 48) return mm && h < 10 ? `${h} h ${mm} min` : `${h} hour${h === 1 ? "" : "s"}`;
    const days = Math.round(h / 24);
    return `${days} days`;
  }
  function humanize(step) {
    if (!step) return "";
    const s = String(step).replace(/[-_]+/g, " ").trim();
    return s.charAt(0).toUpperCase() + s.slice(1);
  }
  function networkName(n) {
    return n ? NETWORK_NAME[n] || n : null;
  }
  function platform() {
    return (app.session && app.session.platform) || "compose";
  }
  function isConfigured() {
    const s = app.state;
    if (!s) return false;
    if (s.configured === true) return true;
    if (s.configured === false) return false;
    return s.wallet !== null && s.wallet !== undefined;
  }
  function shortAddr(a) {
    return a && a.length > 30 ? `${a.slice(0, 16)}…${a.slice(-10)}` : a || "—";
  }

  // The state shown on Status: the backend's, or the browser-side ui-disconnected.
  function currentState() {
    if (!app.online) {
      return {
        id: "ui-disconnected",
        severity: "error",
        headline: FALLBACK["ui-disconnected"][1],
        detail: app.lastOk
          ? `The details below are from ${fmtTime(app.lastOk)}. The page keeps trying every 2 seconds and updates on its own.`
          : "The page keeps trying every 2 seconds and updates on its own.",
        action: null,
        next_retry_at: null,
      };
    }
    const s = app.state && isObj(app.state.state) ? app.state.state : null;
    return s || { id: null, severity: null, headline: null, detail: null, action: null, next_retry_at: null };
  }
  function severityOf(st) {
    if (st.severity && SEVERITY_LABEL[st.severity]) return st.severity;
    return (FALLBACK[st.id] && FALLBACK[st.id][0]) || "info";
  }
  function headlineOf(st) {
    if (st.headline) return st.headline;
    if (FALLBACK[st.id]) return FALLBACK[st.id][1];
    return st.id ? `State: ${st.id}` : "Status not known yet";
  }

  function announce(text) {
    const live = $("live");
    live.textContent = "";
    // A short delay makes screen readers treat repeated text as new.
    setTimeout(() => { live.textContent = text; }, 60);
  }

  function linkTarget(t) {
    if (!t) return "#/status";
    if (/^https?:\/\//i.test(t) || t.startsWith("#/")) return t;
    const m = t.match(/^\/([a-z-]+)\/?$/);
    if (m && ROUTES.includes(m[1])) return `#/${m[1]}`;
    return t;
  }

  // ---------------------------------------------------------------------------
  // Routing (hash routes, so any static file server works)
  // ---------------------------------------------------------------------------

  function routeFromHash() {
    const m = location.hash.match(/^#\/([a-z-]+)/);
    return m && ROUTES.includes(m[1]) ? m[1] : null;
  }

  function resolveRoute() {
    const requested = routeFromHash();
    if (!app.session) return app.state ? requested || "status" : "loading";
    if (!app.session.authenticated) return "login";
    if (!app.state) return requested === "diagnostics" ? "diagnostics" : "loading";
    if (!isConfigured()) return requested === "diagnostics" ? "diagnostics" : "setup";
    if (!requested || requested === "setup" || requested === "login") return "status";
    return requested;
  }

  function syncHash(route) {
    if (route === "loading" || route === "login") return;
    if (routeFromHash() !== route) history.replaceState(null, "", `#/${route}`);
  }

  function showView(route) {
    for (const v of document.querySelectorAll(".view")) {
      const on = v.id === `view-${route}`;
      if (v.hidden === on) v.hidden = !on;
    }
    if (app.view === route) return;
    app.view = route;
    document.title = `${TITLES[route] || "friglet"} · friglet`;
    if (route === "connect") loadConnect();
    if (app.focusNext) {
      const h = document.querySelector(`#view-${route} h1`);
      if (h) h.focus();
    }
    app.focusNext = false;
  }

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  function render() {
    const route = resolveRoute();
    syncHash(route);
    renderBanners();
    renderNav(route);
    renderFooter();
    if (route === "loading") renderLoading();
    else if (route === "login") renderLogin();
    else if (route === "setup") renderSetup();
    else if (route === "status") renderStatus();
    else if (route === "connect") renderConnect();
    else if (route === "wallet") renderWallet();
    else if (route === "diagnostics") renderDiagnostics();
    showView(route);
    announceIfChanged();
  }

  function renderBanners() {
    $("banner-disconnected").hidden = app.online;
    setText("banner-disconnected-text", app.lastOk
      ? `Last update at ${fmtTime(app.lastOk)}. The page keeps trying every 2 seconds and updates on its own.`
      : "The page keeps trying every 2 seconds and updates on its own.");
    const s = app.session;
    $("banner-auth-none").hidden = !(s && s.auth_mode === "none");
    const extra = s && typeof s.banner === "string" ? s.banner.trim() : "";
    $("banner-platform").hidden = !extra;
    if (extra) setText("banner-platform-text", extra);
  }

  function renderNav(route) {
    const authed = !!(app.session && app.session.authenticated);
    $("nav").hidden = !authed || !app.state;
    const configured = isConfigured();
    for (const li of $("nav").querySelectorAll("li")) {
      const when = li.dataset.when;
      li.hidden = when === "configured" ? !configured : when === "unconfigured" ? configured : false;
    }
    for (const a of $("nav").querySelectorAll("a[data-route]")) {
      if (a.dataset.route === route) a.setAttribute("aria-current", "page");
      else a.removeAttribute("aria-current");
    }
    $("brand").setAttribute("href", configured ? "#/status" : "#/setup");
    $("logout").hidden = !(authed && app.session.auth_mode === "password");
  }

  function renderFooter() {
    const v = app.state && app.state.versions;
    if (!v) { setText("footer-versions", "friglet"); return; }
    const parts = [];
    if (v.friglet_ui) parts.push(`friglet-ui ${v.friglet_ui}`);
    if (v.friglet) parts.push(`friglet ${v.friglet}`);
    if (v.dev_build) parts.push("development build");
    setText("footer-versions", parts.join(" · ") || "friglet");
  }

  function announceIfChanged() {
    if (!app.session || !app.session.authenticated) return;
    const st = currentState();
    if (!st.id) return;
    const tx = app.state && app.state.transaction;
    const key = `${st.id}|${tx ? tx.step : ""}`;
    if (app.lastAnnounced === null) { app.lastAnnounced = key; return; }
    if (key === app.lastAnnounced) return;
    app.lastAnnounced = key;
    let msg = `${SEVERITY_LABEL[severityOf(st)]}: ${headlineOf(st)}.`;
    if (tx && tx.step) msg += ` Wallet change step: ${humanize(tx.step)}.`;
    announce(msg);
  }

  function renderLoading() {
    setText("loading-text", app.online
      ? "Loading the status of this box…"
      : "Cannot reach friglet-ui on this box yet. The page keeps trying every 2 seconds.");
  }

  // ---- Login ----------------------------------------------------------------

  function renderLogin() {
    const gateway = app.session && app.session.auth_mode === "gateway";
    $("login-password-mode").hidden = gateway;
    $("login-gateway-mode").hidden = !gateway;
    showText("login-where", PASSWORD_WHERE[platform()] || "");
  }

  // ---- Setup ----------------------------------------------------------------

  function renderSetup() {
    const startos = platform() === "startos";
    $("setup-startos").hidden = !startos;
    $("setup-web").hidden = startos;
    const d = app.state || {};
    const net = (d.wallet && d.wallet.network) || app.setupNetwork;
    setText("setup-network", networkName(net), "Set by this box");
    const node = d.node || {};
    setText("setup-node", node.configured || node.peer);
    const oracle = d.oracle || {};
    const url = oracle.url || "a BlindBit oracle server";
    setText("setup-oracle-note",
      `Scanning uses the BlindBit oracle at ${url}${oracle.third_party === false ? "" : ", a third-party service"}. ` +
      "It sees this box's IP address and which blocks are scanned, never your keys.");
  }

  // ---- Status ---------------------------------------------------------------

  function renderStatus() {
    const st = currentState();
    const d = app.state || {};
    const sev = severityOf(st);

    const pillKey = `${sev}`;
    const sevEl = $("status-severity");
    if (sevEl.dataset.key !== pillKey) {
      sevEl.dataset.key = pillKey;
      sevEl.replaceChildren(el("span", { class: `sev-${sev}`, text: SEVERITY_LABEL[sev] }));
    }
    setText("status-headline", headlineOf(st));
    showText("status-detail", st.detail);

    const retryAt = parseTime(st.next_retry_at);
    if (retryAt) {
      const secs = Math.round((retryAt.getTime() - Date.now()) / 1000);
      const when = secs <= 5 ? "in a moment" : `in about ${fmtDuration(secs)}`;
      showText("status-retry", `friglet-ui tries again automatically ${when} (at ${fmtTime(retryAt)}).`);
    } else {
      showText("status-retry", null);
    }

    renderNothingFound(st, d);
    renderProgress(d);
    renderTransaction($("status-transaction"), false);

    const w = d.wallet || null;
    setText("status-address", w && w.sp_address);
    $("status-wallet").querySelector("button.copy").disabled = !(w && w.sp_address);
    setText("status-identity", identityText(st, w));
    setText("status-fingerprint", w && w.key_fingerprint);

    renderAction($("status-action"), st.action);

    const h = d.heights || {};
    setText("d-payments", num(d.payments_found));
    setText("d-birthday", block(h.birthday ?? (w && w.birthday_height)));
    const persisted = num(h.persisted);
    setText("d-scanned", block(h.scanned) && (persisted !== null && h.persisted !== h.scanned
      ? `${block(h.scanned)} (saved to disk up to ${persisted})` : block(h.scanned)));
    setText("d-node-tip", block(h.node_tip));
    setText("d-oracle-tip", block(h.oracle_tip));
    setText("d-network", networkName(w && w.network));
    setText("d-node", nodeText(d.node));
    renderOracle($("d-oracle"), d.oracle);
    setText("d-versions", versionsText(d.versions));
    setText("d-restarts", num(d.restarts));
    setText("d-updated", fmtDateTime(h.updated_at));
  }

  function identityText(st, w) {
    if (!w) return null;
    if (w.identity_verified === true) return "Verified: friglet reports this same address.";
    if (st.id === "identity-mismatch") return "Does not match: friglet reports a different address.";
    if (w.identity_verified === false) return "Not checked yet: waiting for friglet to report its address.";
    return "Not known yet.";
  }

  function nodeText(node) {
    if (!node) return null;
    const parts = [];
    if (node.peer) parts.push(node.peer);
    if (node.configured && node.configured !== node.peer) parts.push(`set as ${node.configured}`);
    if (node.user_agent) parts.push(node.user_agent);
    return parts.join(" · ") || null;
  }

  function versionsText(v) {
    if (!v) return null;
    const parts = [];
    if (v.friglet) parts.push(`friglet ${v.friglet}`);
    if (v.friglet_ui) parts.push(`friglet-ui ${v.friglet_ui}`);
    if (v.dev_build) parts.push("development build");
    return parts.join(" · ") || null;
  }

  function renderOracle(node, oracle) {
    const key = JSON.stringify(oracle || null);
    if (node.dataset.key === key) return;
    node.dataset.key = key;
    if (!oracle || !oracle.url) { node.textContent = "—"; return; }
    const children = [el("span", { class: "mono", text: oracle.url })];
    if (oracle.third_party) children.push(" ", el("span", { class: "third-party", text: "Third-party service" }));
    node.replaceChildren(...children);
  }

  function renderNothingFound(st, d) {
    const node = $("status-nothing-found");
    const h = d.heights || {};
    const show = app.online && st.id === "up-to-date" && d.payments_found === 0 && num(h.birthday) && num(h.scanned);
    const key = show ? `${h.birthday}-${h.scanned}` : "";
    if (node.dataset.key === key) return;
    node.dataset.key = key;
    node.hidden = !show;
    if (!show) { node.replaceChildren(); return; }
    node.replaceChildren(
      `Scanned ${num(h.birthday)}–${num(h.scanned)}, no payments found. If you expected payments, the birthday may be too late. `,
      el("a", { href: "#/wallet", text: "Change the birthday" }),
    );
  }

  function renderProgress(d) {
    const p = d.progress || {};
    const h = d.heights || {};
    const frac = typeof p.fraction === "number" && Number.isFinite(p.fraction)
      ? Math.min(1, Math.max(0, p.fraction)) : null;
    $("status-progress-block").hidden = frac === null;
    if (frac === null) return;
    const bar = $("status-progress");
    if (bar.value !== frac) bar.value = frac;
    const pct = frac >= 1 ? "100%" : `${(Math.floor(frac * 1000) / 10).toFixed(1)}%`;
    setText("status-progress-pct", pct);
    const tips = [h.node_tip, h.oracle_tip].filter((x) => typeof x === "number");
    const tip = tips.length ? Math.min(...tips) : null;
    let text = null;
    if (num(h.scanned) && num(tip)) text = `Scanned to block ${num(h.scanned)} of ${num(tip)}`;
    else if (num(h.scanned)) text = `Scanned to block ${num(h.scanned)}`;
    if (text && num(h.birthday)) text += `, starting from ${num(h.birthday)}`;
    showText("status-progress-text", text && `${text}.`);
    const eta = p.eta_seconds;
    showText("status-eta", app.online && typeof eta === "number" && eta > 0 ? `About ${fmtDuration(eta)} left.` : null);
  }

  // One next action, taken from the backend payload.
  function renderAction(container, action) {
    const ok = isObj(action) && action.label && (action.kind === "link" || action.kind === "post");
    const key = ok && app.online ? JSON.stringify(action) : "";
    if (container.dataset.key === key) return;
    container.dataset.key = key;
    container.replaceChildren();
    container.hidden = !key;
    if (!key) return;
    if (action.kind === "link") {
      const href = linkTarget(action.target);
      const a = el("a", { class: "button button-primary", href, text: action.label });
      if (/^https?:/i.test(href)) a.setAttribute("rel", "noopener noreferrer");
      a.addEventListener("click", (ev) => {
        // A link to the page already shown: move to where the action happens instead.
        if (href !== location.hash) return;
        ev.preventDefault();
        const view = document.querySelector(".view:not([hidden])");
        const target = view && (view.querySelector("[data-action-focus]:not(:disabled)") || view.querySelector("h1"));
        if (target) target.focus();
      });
      container.append(a);
      return;
    }
    if (!/^\/api\//.test(action.target || "")) { container.hidden = true; return; }
    const result = el("p", { class: "result bad", tabindex: "-1", hidden: true });
    const btn = el("button", { type: "button", class: "button-primary", text: action.label });
    btn.addEventListener("click", async () => {
      btn.disabled = true;
      result.hidden = true;
      try {
        await post(action.target);
        app.focusNext = false;
        await tick();
      } catch (err) {
        result.textContent = `That did not work: ${err.message}`;
        result.hidden = false;
        result.focus();
      } finally {
        btn.disabled = false;
      }
    });
    container.append(el("div", { class: "actions" }, btn), result);
  }

  // Durable wallet-change progress, read from state.transaction on every poll.
  function renderTransaction(node, withAction) {
    const tx = app.state && isObj(app.state.transaction) ? app.state.transaction : null;
    const st = currentState();
    const failed = !!tx && app.online && severityOf(st) === "error";
    const key = tx ? JSON.stringify([tx, failed, withAction ? st.action : null]) : "";
    if (node.dataset.key === key) return;
    node.dataset.key = key;
    node.hidden = !tx;
    node.classList.toggle("failed", failed);
    if (!tx) { node.replaceChildren(); return; }
    const forget = tx.kind === "forget";
    const title = failed
      ? (forget ? "Forgetting the wallet stopped" : "The wallet change stopped")
      : (forget ? "Forgetting the wallet" : "Changing the wallet");
    const started = parseTime(tx.started_at);
    const parts = [
      el("h2", { tabindex: "-1", text: title }),
      el("p", null, el("strong", { text: failed ? "Stopped at: " : "Current step: " }), humanize(tx.step) || "Not reported yet"),
    ];
    if (started) parts.push(el("p", { class: "hint", text: `Started ${started.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" })}.` }));
    if (!failed) {
      parts.push(el("p", { text: "This continues on the box. You can close this page; it shows the progress again when you come back." }));
    } else if (withAction) {
      parts.push(el("p", { text: "Enter the wallet again below to retry, or restore the previous wallet." }));
    } else {
      parts.push(el("p", null, "Use the action below to fix it, or ", el("a", { href: "#/wallet", text: "restore the previous wallet" }), " on the Wallet page."));
    }
    if (withAction && failed) {
      const holder = el("div");
      renderAction(holder, st.action);
      parts.push(holder);
    }
    node.replaceChildren(...parts);
  }

  // ---- Connect --------------------------------------------------------------

  async function loadConnect() {
    if (app.connectLoading) return;
    app.connectLoading = true;
    try {
      app.connect = await request("GET", "/api/connect");
      app.connectError = null;
      app.connectFor = currentState().id;
    } catch (err) {
      app.connectError = err;
      if (err.status === 401) loadSession().then(render, () => {});
    } finally {
      app.connectLoading = false;
    }
    if (app.view === "connect") renderConnect();
  }

  function sparrowCommands(network) {
    const net = network && network !== "bitcoin" ? ` --network ${network}` : "";
    return {
      linux: `/opt/sparrow/bin/Sparrow --dir ~/.sparrow-friglet${net}`,
      mac: `open -n -a Sparrow --args --dir ~/.sparrow-friglet${net}`,
      windows: `"%ProgramFiles%\\Sparrow\\Sparrow.exe" --dir "%USERPROFILE%\\.sparrow-friglet"${net}`,
    };
  }

  function renderConnect() {
    const st = currentState();
    if (app.connect && app.connectFor !== st.id && st.id !== "ui-disconnected") loadConnect();
    const c = app.connect;
    $("connect-loading").hidden = !!c || !!app.connectError;
    const errNode = $("connect-error");
    if (!c) {
      $("connect-steps").hidden = true;
      $("connect-gate").hidden = true;
      $("connect-partial").hidden = true;
      if (app.connectError) {
        errNode.textContent = `Could not load the connection details: ${app.connectError.message} The page tries again when the status changes.`;
        errNode.hidden = false;
      }
      return;
    }
    errNode.hidden = true;
    $("connect-steps").hidden = false;

    const d = app.state || {};
    const h = d.heights || {};
    const open = c.listeners_open !== false;
    showText("connect-gate", open ? null
      : "Sparrow cannot connect yet. The Electrum port opens once friglet has checked the wallet address. This page updates when it opens.");
    const scanning = open && ["scanning", "rescanning-after-reset", "oracle-dropped-resuming"].includes(st.id);
    showText("connect-partial", scanning && num(h.scanned)
      ? `Sparrow shows payments up to block ${num(h.scanned)}; more appear as scanning continues.` : null);

    const cmds = sparrowCommands(d.wallet && d.wallet.network);
    setText("cmd-linux", cmds.linux);
    setText("cmd-mac", cmds.mac);
    setText("cmd-windows", cmds.windows);

    const recNode = $("connect-recommended");
    const recKey = JSON.stringify(c.recommended || null);
    if (recNode.dataset.key !== recKey) {
      recNode.dataset.key = recKey;
      recNode.replaceChildren(c.recommended
        ? endpointView(c.recommended, "rec", true)
        : el("p", { class: "notice", text: "No endpoint is available for this box yet." }));
    }

    const fp = c.tls_fingerprint_sha256;
    $("connect-fingerprint").hidden = !fp;
    if (fp) setText("connect-fp", fp);

    const others = Array.isArray(c.others) ? c.others : [];
    $("connect-others-step").hidden = others.length === 0;
    const othKey = JSON.stringify(others);
    const list = $("connect-others-list");
    if (list.dataset.key !== othKey) {
      list.dataset.key = othKey;
      setText("connect-others-summary", `Show ${others.length} other endpoint${others.length === 1 ? "" : "s"}`);
      list.replaceChildren(...others.map((ep, i) => endpointView(ep, `oth${i}`, false)));
    }

    setText("connect-exposure", c.exposure_text || "Anyone who can reach this port can read this wallet's history.");
  }

  function copyRow(id, value, what) {
    return el("div", { class: "copy-row" },
      el("p", { class: "mono", id, text: value }),
      el("button", { type: "button", class: "button-secondary copy", "data-copy": id },
        "Copy", el("span", { class: "visually-hidden", text: ` ${what}` })));
  }

  function endpointView(ep, prefix, recommended) {
    const ssl = String(ep.protocol || "").toLowerCase() === "ssl";
    const rows = [];
    if (ep.host) rows.push(el("div", null, el("dt", { text: "URL" }), el("dd", null, copyRow(`${prefix}-host`, ep.host, `${ep.label || ""} URL`))));
    if (ep.port !== null && ep.port !== undefined) {
      rows.push(el("div", null, el("dt", { text: "Port" }), el("dd", null, copyRow(`${prefix}-port`, String(ep.port), `${ep.label || ""} port`))));
    }
    rows.push(el("div", null, el("dt", { text: "Use SSL" }),
      el("dd", { text: ssl ? "On: tick Use SSL." : "Off: leave Use SSL unticked." })));
    if (ep.string) {
      rows.push(el("div", null, el("dt", { text: "Server string" }),
        el("dd", null, copyRow(`${prefix}-string`, ep.string, `${ep.label || ""} server string`),
          el("p", { class: "hint", text: "For wallets that take host:port:protocol in one field." }))));
    }
    const title = recommended ? `Recommended: ${ep.label || "this box"}` : (ep.label || "Endpoint");
    return el("div", { class: "endpoint" },
      el(recommended ? "p" : "h3", { class: "endpoint-title", text: title }),
      el("dl", { class: "field-rows" }, ...rows),
      ep.note ? el("p", { class: "hint", text: ep.note }) : null);
  }

  // ---- Wallet ---------------------------------------------------------------

  function renderWallet() {
    const d = app.state || {};
    const w = d.wallet || {};
    setText("wallet-address", w.sp_address);
    $("view-wallet").querySelector("button.copy").disabled = !w.sp_address;
    setText("wallet-fingerprint", w.key_fingerprint);
    setText("wallet-network", networkName(w.network));
    setText("wallet-birthday", block(w.birthday_height ?? (d.heights && d.heights.birthday)));
    renderTransaction($("wallet-transaction"), true);

    const failed = !!d.transaction && severityOf(currentState()) === "error";
    const busy = (!!d.transaction && !failed) || !app.online;
    // A restore message stays until the restore it started has finished.
    if (app.restoreWatch) {
      if (d.transaction) app.restoreWatch = "running";
      else if (app.restoreWatch === "running") {
        app.restoreWatch = null;
        setText("restore-result", "The previous wallet is active again.");
      }
    }
    const startos = platform() === "startos";
    $("change-startos").hidden = !startos;
    $("change-form").hidden = startos || !!app.pendingChange;
    if (startos) $("change-confirm").hidden = true;
    $("change-fieldset").disabled = busy;
    $("change-confirm-yes").disabled = busy;
    $("restore-open").disabled = busy;
    $("restore-yes").disabled = busy;
    $("forget-open").disabled = busy;
    $("forget-yes").disabled = busy;
  }

  // ---- Diagnostics ----------------------------------------------------------

  function renderDiagnostics() {
    const st = currentState();
    const d = app.state || {};
    setText("diag-state", st.id);
    setText("diag-platform", app.session && app.session.platform);
    setText("diag-versions", versionsText(d.versions));
    setText("diag-restarts", num(d.restarts));
    const list = $("logs-list");
    const p = platform();
    const mine = list.querySelector(`[data-platform="${p}"]`);
    if (mine && list.firstElementChild !== mine) list.prepend(mine);
    for (const item of list.children) item.dataset.current = String(item === mine);
  }

  // ---------------------------------------------------------------------------
  // Forms: field errors tied with aria-describedby, focus on the first error
  // ---------------------------------------------------------------------------

  function errorNodeFor(input) {
    return $(`${input.id}-error`);
  }

  function setFieldError(input, message) {
    const node = errorNodeFor(input);
    node.textContent = message;
    node.hidden = false;
    input.setAttribute("aria-invalid", "true");
    const ids = (input.getAttribute("aria-describedby") || "").split(/\s+/).filter(Boolean);
    if (!ids.includes(node.id)) ids.unshift(node.id);
    input.setAttribute("aria-describedby", ids.join(" "));
    const details = input.closest("details");
    if (details) details.open = true;
  }

  function clearFieldError(input) {
    const node = errorNodeFor(input);
    if (!node) return;
    node.hidden = true;
    node.textContent = "";
    input.removeAttribute("aria-invalid");
    const ids = (input.getAttribute("aria-describedby") || "").split(/\s+/).filter((x) => x && x !== node.id);
    if (ids.length) input.setAttribute("aria-describedby", ids.join(" "));
    else input.removeAttribute("aria-describedby");
  }

  function setFormError(id, content) {
    const node = $(id);
    if (typeof content === "string") node.textContent = content;
    else node.replaceChildren(...content);
    node.hidden = false;
  }

  // A wallet form (setup or change) with descriptor, hex keys and birthday.
  function walletForm(prefix) {
    const f = {
      descriptor: $(`${prefix}-descriptor`),
      scan_secret: $(`${prefix}-scan-secret`),
      spend_pubkey: $(`${prefix}-spend-pubkey`),
      birthday_height: $(`${prefix}-birthday`),
    };
    const formError = `${prefix}-form-error`;
    const hexMode = () => !f.descriptor.value.trim() && !!(f.scan_secret.value.trim() || f.spend_pubkey.value.trim());

    function inputFor(field) {
      if (field === "descriptor") return hexMode() ? f.scan_secret : f.descriptor;
      if (field === "birthday_height" || field === "birthday" || field === "start_height") return f.birthday_height;
      if (field === "scan_secret" || field === "scan_key") return f.scan_secret;
      if (field === "spend_pubkey") return f.spend_pubkey;
      return null;
    }

    function clear() {
      for (const input of Object.values(f)) clearFieldError(input);
      $(formError).hidden = true;
    }

    function show(errors, focus) {
      let first = null;
      for (const [field, message] of errors) {
        const input = inputFor(field);
        if (input) {
          setFieldError(input, message);
          first = first || input;
        } else {
          setFormError(formError, message);
          first = first || $(formError);
        }
      }
      if (focus !== false && first) first.focus();
    }

    function check() {
      const errs = [];
      const d = f.descriptor.value.trim();
      const s = f.scan_secret.value.trim();
      const p = f.spend_pubkey.value.trim();
      const b = f.birthday_height.value.trim();
      let descriptor = null;
      if (d && (s || p)) {
        errs.push(["descriptor", "Use either the descriptor or the two hex keys, not both. Clear one of them."]);
      } else if (d) {
        descriptor = d;
      } else if (s || p) {
        if (!/^[0-9a-fA-F]{64}$/.test(s)) errs.push(["scan_secret", "Enter exactly 64 hex characters (0–9, a–f)."]);
        if (!/^0[23][0-9a-fA-F]{64}$/.test(p)) errs.push(["spend_pubkey", "Enter exactly 66 hex characters, starting with 02 or 03."]);
        descriptor = `hex:${s.toLowerCase()}:${p.toLowerCase()}`;
      } else {
        errs.push(["descriptor", "Paste the descriptor from Sparrow."]);
      }
      if (!/^\d{1,9}$/.test(b)) {
        errs.push(["birthday_height", b ? "Enter the birthday as a block height, digits only." : "Enter the birthday height Sparrow shows for this wallet."]);
      }
      return { errs, value: errs.length ? null : { descriptor, birthday: parseInt(b, 10) } };
    }

    function collect() {
      clear();
      const r = check();
      if (r.errs.length) { show(r.errs, true); return null; }
      return r.value;
    }

    function showApiError(err, focus) {
      if (err.status === 0) {
        show([[null, "Could not reach this box. Check the connection and try again."]], focus);
      } else {
        show([[err.field, err.message]], focus);
      }
    }

    function reset() {
      for (const input of Object.values(f)) {
        input.value = "";
        clearFieldError(input);
      }
      for (const btn of document.querySelectorAll(`[data-reveal^="${prefix}-"]`)) {
        $(btn.dataset.reveal).type = "password";
        setRevealLabel(btn, false);
      }
      $(`${prefix}-hex`).open = false;
      $(formError).hidden = true;
    }

    // Clear a field's error as soon as the user edits it.
    for (const input of Object.values(f)) {
      input.addEventListener("input", () => clearFieldError(input));
    }

    return { f, clear, show, check, collect, showApiError, reset, formError };
  }

  // ---------------------------------------------------------------------------
  // Events
  // ---------------------------------------------------------------------------

  function setRevealLabel(btn, shown) {
    btn.textContent = shown ? "Hide" : "Show";
    const what = (btn.getAttribute("aria-label") || "").replace(/^(Show|Hide) /, "");
    btn.setAttribute("aria-label", `${shown ? "Hide" : "Show"} ${what}`);
  }

  async function copyText(text, btn) {
    if (navigator.clipboard && window.isSecureContext) {
      try {
        await navigator.clipboard.writeText(text);
        return true;
      } catch (_) { /* fall through */ }
    }
    // Plain-HTTP LAN pages have no async clipboard; try the legacy path.
    const ta = el("textarea", { readonly: true, class: "visually-hidden", "aria-hidden": "true", tabindex: "-1" });
    ta.value = text;
    document.body.append(ta);
    ta.select();
    let ok = false;
    try { ok = document.execCommand("copy"); } catch (_) { ok = false; }
    ta.remove();
    btn.focus();
    return ok;
  }

  function selectContents(node) {
    const range = document.createRange();
    range.selectNodeContents(node);
    const sel = window.getSelection();
    sel.removeAllRanges();
    sel.addRange(range);
  }

  function setCopyState(btn, state) {
    const label = btn.firstChild;
    if (!btn.dataset.label) btn.dataset.label = label.nodeValue;
    const mac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
    label.nodeValue = state === "done" ? "Copied" : `Selected: press ${mac ? "Cmd" : "Ctrl"}+C`;
    btn.dataset.state = state;
    clearTimeout(btn._copyTimer);
    btn._copyTimer = setTimeout(() => {
      label.nodeValue = btn.dataset.label;
      delete btn.dataset.state;
    }, state === "done" ? 2000 : 6000);
  }

  function bindEvents() {
    window.addEventListener("hashchange", () => {
      if (!location.hash.startsWith("#/")) return;
      app.focusNext = true;
      render();
      app.focusNext = false;
    });

    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") tick();
      else clearTimeout(app.timer);
    });

    $("skip-link").addEventListener("click", (ev) => {
      ev.preventDefault();
      const h = document.querySelector(".view:not([hidden]) h1");
      (h || $("main")).focus();
    });

    document.addEventListener("click", async (ev) => {
      const reveal = ev.target.closest("button[data-reveal]");
      if (reveal) {
        const input = $(reveal.dataset.reveal);
        const show = input.type === "password";
        input.type = show ? "text" : "password";
        setRevealLabel(reveal, show);
        return;
      }
      const copy = ev.target.closest("button[data-copy]");
      if (copy) {
        const src = $(copy.dataset.copy);
        const text = src ? src.textContent.trim() : "";
        if (!text || text === "—") return;
        if (await copyText(text, copy)) {
          setCopyState(copy, "done");
        } else {
          selectContents(src);
          setCopyState(copy, "manual");
        }
      }
    });

    $("logout").addEventListener("click", async () => {
      try { await post("/api/logout"); } catch (_) { /* session may already be gone */ }
      app.state = null;
      app.lastAnnounced = null;
      try { await loadSession(); } catch (_) { setOnline(false); }
      app.focusNext = true;
      render();
    });

    // ---- Login
    $("login-form").addEventListener("submit", async (ev) => {
      ev.preventDefault();
      const input = $("login-password");
      clearFieldError(input);
      $("login-form-error").hidden = true;
      if (Date.now() < app.loginBlockedUntil) {
        const left = Math.ceil((app.loginBlockedUntil - Date.now()) / 1000);
        setFormError("login-form-error", `Too many attempts. Try again in ${left} seconds.`);
        $("login-form-error").focus();
        return;
      }
      if (!input.value) {
        setFieldError(input, "Enter the password.");
        input.focus();
        return;
      }
      const btn = $("login-submit");
      btn.disabled = true;
      try {
        await post("/api/login", { password: input.value }, { login: true });
        input.value = "";
        await loadSession();
        app.focusNext = true;
        await tick();
      } catch (err) {
        if (err.status === 401) {
          setFieldError(input, err.field === "password" || !err.message || /HTTP/.test(err.message)
            ? "That password is not right." : err.message);
          input.focus();
        } else if (err.status === 429) {
          const secs = err.retryAfter || 60;
          app.loginBlockedUntil = Date.now() + secs * 1000;
          setFormError("login-form-error", `Too many attempts. Try again in ${secs} seconds.`);
          $("login-form-error").focus();
        } else {
          setFormError("login-form-error", err.status === 0 ? "Could not reach this box. Try again." : err.message);
          $("login-form-error").focus();
        }
      } finally {
        btn.disabled = false;
      }
    });
    $("login-retry").addEventListener("click", () => tick());

    // ---- Setup
    const setup = walletForm("setup");
    let validateSeq = 0;
    const inlineValidate = async () => {
      const r = setup.check();
      if (r.errs.length) { $("setup-preview").hidden = true; return; }
      const mine = ++validateSeq;
      try {
        const id = await post("/api/setup/validate", { descriptor: r.value.descriptor, birthday_height: r.value.birthday });
        if (mine !== validateSeq) return;
        setup.clear();
        showPreview(id);
      } catch (err) {
        if (mine !== validateSeq) return;
        $("setup-preview").hidden = true;
        if (err.field) setup.showApiError(err, false);
      }
    };
    for (const input of Object.values(setup.f)) input.addEventListener("change", inlineValidate);

    function showPreview(id) {
      if (!id || !id.sp_address) { $("setup-preview").hidden = true; return; }
      setText("setup-preview-address", id.sp_address);
      setText("setup-preview-network", networkName(id.network));
      setText("setup-preview-fingerprint", id.key_fingerprint);
      if (id.network) {
        app.setupNetwork = id.network;
        setText("setup-network", networkName(id.network));
      }
      $("setup-preview").hidden = false;
    }

    $("setup-form").addEventListener("submit", async (ev) => {
      ev.preventDefault();
      const value = setup.collect();
      if (!value) return;
      validateSeq++;
      const btn = $("setup-submit");
      btn.disabled = true;
      btn.textContent = "Saving…";
      try {
        const res = await post("/api/setup", { descriptor: value.descriptor, birthday_height: value.birthday, replace: false });
        setup.reset();
        $("setup-preview").hidden = true;
        adoptAcceptedState(res);
        app.focusNext = true;
        history.replaceState(null, "", "#/status");
        render();
        tick();
      } catch (err) {
        if (err.status === 409 || err.code === "wallet-exists") {
          setFormError("setup-form-error", [
            "A different wallet is already set up on this box. To replace it, use ",
            el("a", { href: "#/wallet", text: "Change wallet" }),
            " on the Wallet page.",
          ]);
          $("setup-form-error").focus();
        } else {
          setup.showApiError(err, true);
        }
      } finally {
        btn.disabled = false;
        btn.textContent = "Save and start scanning";
      }
    });

    // ---- Change wallet
    const change = walletForm("change");
    $("change-form").addEventListener("submit", async (ev) => {
      ev.preventDefault();
      const value = change.collect();
      if (!value) return;
      const btn = $("change-review");
      btn.disabled = true;
      btn.textContent = "Checking…";
      try {
        const id = await post("/api/setup/validate", { descriptor: value.descriptor, birthday_height: value.birthday });
        app.pendingChange = { descriptor: value.descriptor, birthday: value.birthday, identity: id || {} };
        showChangeConfirm();
      } catch (err) {
        change.showApiError(err, true);
      } finally {
        btn.disabled = false;
        btn.textContent = "Review change";
      }
    });

    function showChangeConfirm() {
      const pc = app.pendingChange;
      const w = (app.state && app.state.wallet) || {};
      const cur = w.sp_address || "";
      const curBirthday = w.birthday_height ?? (app.state && app.state.heights && app.state.heights.birthday);
      const id = pc.identity;
      setText("confirm-old-address", cur || "Unknown");
      setText("confirm-old-meta", [w.key_fingerprint && `key ${w.key_fingerprint}`, num(curBirthday) && `birthday ${num(curBirthday)}`].filter(Boolean).join(", ") || "");
      setText("confirm-new-address", id.sp_address || "Unknown");
      setText("confirm-new-meta", [id.key_fingerprint && `key ${id.key_fingerprint}`, `birthday ${num(pc.birthday)}`, networkName(id.network)].filter(Boolean).join(", "));
      const same = cur && id.sp_address === cur;
      let rescan;
      $("change-confirm-yes").hidden = false;
      if (same && pc.birthday === curBirthday) {
        rescan = "This is the wallet already set up, with the same birthday. Nothing changes.";
        $("change-confirm-yes").hidden = true;
      } else if (same) {
        rescan = `Same wallet, new birthday. friglet starts a fresh scan from block ${num(pc.birthday)}. Set the birthday in Sparrow to ${num(pc.birthday)} as well.`;
      } else {
        rescan = `friglet stops, then scans for the new wallet from block ${num(pc.birthday)}. Set the birthday in Sparrow to ${num(pc.birthday)} as well.`;
      }
      setText("confirm-rescan", rescan);
      $("change-confirm-title").textContent = same ? "Rescan this wallet?" : "Replace the wallet?";
      $("change-confirm-error").hidden = true;
      $("change-form").hidden = true;
      $("change-confirm").hidden = false;
      $("change-confirm-title").focus();
    }

    function closeChangeConfirm(focusId) {
      app.pendingChange = null;
      $("change-confirm").hidden = true;
      $("change-form").hidden = platform() === "startos";
      if (focusId) $(focusId).focus();
    }

    $("change-confirm-no").addEventListener("click", () => closeChangeConfirm("change-review"));
    $("change-confirm-yes").addEventListener("click", async () => {
      const pc = app.pendingChange;
      if (!pc) return;
      const btn = $("change-confirm-yes");
      btn.disabled = true;
      $("restore-result").hidden = true;
      app.restoreWatch = null;
      try {
        const res = await post("/api/setup", { descriptor: pc.descriptor, birthday_height: pc.birthday, replace: true });
        change.reset();
        closeChangeConfirm(null);
        adoptAcceptedState(res);
        await tick();
        const h = $("wallet-transaction").querySelector("h2");
        (h && !$("wallet-transaction").hidden ? h : $("wallet-title")).focus();
      } catch (err) {
        if (err.field) {
          closeChangeConfirm(null);
          change.showApiError(err, true);
        } else {
          setFormError("change-confirm-error", err.status === 0 ? "Could not reach this box. Try again." : err.message);
          $("change-confirm-error").focus();
        }
      } finally {
        btn.disabled = false;
      }
    });

    // ---- Restore previous
    $("restore-open").addEventListener("click", () => {
      $("restore-start").hidden = true;
      $("restore-confirm").hidden = false;
      $("restore-result").hidden = true;
      $("restore-confirm-text").focus();
    });
    $("restore-no").addEventListener("click", () => {
      $("restore-confirm").hidden = true;
      $("restore-start").hidden = false;
      $("restore-open").focus();
    });
    $("restore-yes").addEventListener("click", async () => {
      const result = $("restore-result");
      $("restore-yes").disabled = true;
      try {
        await post("/api/wallet/restore-previous");
        result.className = "result ok";
        result.textContent = "Restoring the previous wallet. The progress shows at the top of this page.";
        app.restoreWatch = "started";
      } catch (err) {
        result.className = "result bad";
        result.textContent = err.status === 404
          ? "There is no previous wallet on this box to restore."
          : `The restore did not start: ${err.message}`;
      } finally {
        $("restore-yes").disabled = false;
      }
      $("restore-confirm").hidden = true;
      $("restore-start").hidden = false;
      result.hidden = false;
      result.focus();
      tick();
    });

    // ---- Forget
    $("forget-open").addEventListener("click", () => {
      const w = (app.state && app.state.wallet) || {};
      setText("forget-confirm-text",
        `Forget the wallet ${shortAddr(w.sp_address)}? This deletes its configuration, key, scan progress and the previous-wallet backup from this box. ` +
        "You can set it up again later; scanning then starts over from the birthday.");
      $("forget-start").hidden = true;
      $("forget-confirm").hidden = false;
      $("forget-error").hidden = true;
      $("forget-confirm-text").focus();
    });
    $("forget-no").addEventListener("click", () => {
      $("forget-confirm").hidden = true;
      $("forget-start").hidden = false;
      $("forget-open").focus();
    });
    $("forget-yes").addEventListener("click", async () => {
      $("forget-yes").disabled = true;
      try {
        await post("/api/wallet/forget");
        $("forget-confirm").hidden = true;
        $("forget-start").hidden = false;
        app.focusNext = true;
        await tick();
      } catch (err) {
        setFormError("forget-error", `The wallet was not forgotten: ${err.message}`);
        $("forget-error").focus();
      } finally {
        $("forget-yes").disabled = false;
      }
    });

    // ---- Connect test
    $("connect-test").addEventListener("click", async () => {
      const btn = $("connect-test");
      const result = $("connect-test-result");
      btn.disabled = true;
      btn.textContent = "Testing…";
      result.hidden = true;
      try {
        const r = (await post("/api/connect/test")) || {};
        result.className = `result ${r.ok ? "ok" : "bad"}`;
        result.textContent = r.ok
          ? `Electrum answers on this node.${r.detail ? ` ${r.detail}` : ""}`
          : `Electrum did not answer on this node.${r.detail ? ` ${r.detail}` : ""}`;
      } catch (err) {
        result.className = "result bad";
        result.textContent = `The test could not run: ${err.message}`;
      } finally {
        btn.disabled = false;
        btn.textContent = "Test from this node";
      }
      result.hidden = false;
      result.focus();
    });
  }

  // POST /api/setup answers 202 {"state": …}; use it so the page does not
  // bounce back to Setup before the next poll.
  function adoptAcceptedState(res) {
    if (!res || !isObj(res.state)) return;
    if ("configured" in res.state) app.state = res.state;
    else if ("id" in res.state && app.state) app.state = Object.assign({}, app.state, { configured: true, state: res.state });
  }

  // ---------------------------------------------------------------------------
  // Boot
  // ---------------------------------------------------------------------------

  bindEvents();
  render();
  tick();
})();
