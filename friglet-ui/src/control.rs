//! friglet-ui's own control socket (`<data>/friglet-ui.sock`, owner only): the CLI hands
//! `setup`, `password set|reset` and `status` over to a running daemon through it.
//! Newline-delimited JSON, one request and one response per connection.

use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use crate::setup::{self, Outcome};
use crate::supervisor::Shared;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum CtlRequest {
    Setup {
        descriptor: String,
        birthday: u64,
        replace: bool,
        network: String,
    },
    PasswordSet {
        password: String,
    },
    Status,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CtlResponse {
    Outcome(Outcome),
    State(Box<crate::view::StateResponse>),
    Done { ok: bool, message: String },
}

pub fn bind(path: &Path) -> io::Result<UnixListener> {
    // The caller holds the data-dir lock, so any existing socket file is stale.
    let _ = std::fs::remove_file(path);
    let l = UnixListener::bind(path)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(l)
}

pub async fn serve(listener: UnixListener, shared: Arc<Shared>) {
    // SAFETY: getuid has no preconditions.
    let me = unsafe { libc::getuid() };
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        if let Ok(cred) = stream.peer_cred()
            && cred.uid() != me
            && cred.uid() != 0
        {
            continue;
        }
        let shared = shared.clone();
        tokio::spawn(async move {
            let _ = handle(stream, shared).await;
        });
    }
}

async fn handle(stream: UnixStream, shared: Arc<Shared>) -> io::Result<()> {
    let (r, mut w) = stream.into_split();
    let mut line = String::new();
    BufReader::new(r).read_line(&mut line).await?;
    let resp = match serde_json::from_str::<CtlRequest>(line.trim()) {
        Err(e) => CtlResponse::Done {
            ok: false,
            message: format!("bad request: {e}"),
        },
        Ok(CtlRequest::Status) => CtlResponse::State(Box::new(shared.state())),
        Ok(CtlRequest::PasswordSet { password }) => match shared.auth.set_password(&password) {
            Ok(()) => {
                tracing::info!("UI password changed via the CLI; all sessions ended");
                CtlResponse::Done {
                    ok: true,
                    message: "password changed; all sessions ended".into(),
                }
            }
            Err(e) => CtlResponse::Done {
                ok: false,
                message: format!("cannot store the password: {e}"),
            },
        },
        Ok(CtlRequest::Setup {
            descriptor,
            birthday,
            replace,
            network,
        }) => CtlResponse::Outcome(
            setup_via_daemon(&shared, &descriptor, birthday, replace, &network).await,
        ),
    };
    let mut out = serde_json::to_vec(&resp)?;
    out.push(b'\n');
    w.write_all(&out).await
}

async fn setup_via_daemon(
    shared: &Shared,
    descriptor: &str,
    birthday: u64,
    replace: bool,
    network: &str,
) -> Outcome {
    if let Some(m) = &shared.cfg.unsupported_network {
        return Outcome::err(setup::EXIT_NETWORK, "unsupported-network", None, m.clone());
    }
    if network != shared.cfg.network.as_str() {
        return Outcome::err(
            setup::EXIT_NETWORK,
            "network-mismatch",
            None,
            format!(
                "--network {network} differs from the running friglet-ui's network {}",
                shared.cfg.network
            ),
        );
    }
    let (keys, _) =
        match setup::validate(descriptor, birthday, shared.cfg.network, shared.node_tip()) {
            Ok(v) => v,
            Err(o) => return *o,
        };
    crate::web::submit_setup(shared, keys, birthday, replace).await
}

/// Client side: send one request, read one response.
pub async fn call(path: &Path, req: &CtlRequest, timeout: Duration) -> io::Result<CtlResponse> {
    tokio::time::timeout(timeout, async {
        let stream = UnixStream::connect(path).await?;
        let (r, mut w) = stream.into_split();
        let mut out = serde_json::to_vec(req)?;
        out.push(b'\n');
        w.write_all(&out).await?;
        let mut line = String::new();
        BufReader::new(r).read_line(&mut line).await?;
        serde_json::from_str(line.trim()).map_err(io::Error::other)
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "friglet-ui did not answer in time"))?
}
