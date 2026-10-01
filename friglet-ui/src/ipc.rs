//! friglet control-socket client with timeouts (friglet-ipc protocol at the pin).

use std::io;
use std::path::Path;
use std::time::Duration;

use friglet_ipc::{Request, Response};

/// One request/response on a fresh connection.
pub async fn request(socket: &Path, req: &Request, timeout: Duration) -> io::Result<Response> {
    let path = socket.to_string_lossy().into_owned();
    tokio::time::timeout(timeout, async move {
        let mut client = friglet_ipc::Client::connect(&path).await?;
        client.request(req).await
    })
    .await
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            "friglet control socket did not answer in time",
        )
    })?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn talks_the_friglet_ipc_protocol() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("f.sock");
        let listener = friglet_ipc::listen(sock.to_str().unwrap()).unwrap();
        tokio::spawn(async move {
            let mut conn = friglet_ipc::accept(&listener).await.unwrap();
            while let Ok(Some(req)) = conn.next_request().await {
                let resp = match req {
                    Request::Shutdown => Response::Ok,
                    _ => Response::Error("unsupported".into()),
                };
                conn.respond(&resp).await.unwrap();
            }
        });
        let r = request(&sock, &Request::Shutdown, Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(r, Response::Ok);
        assert!(
            request(
                &dir.path().join("missing.sock"),
                &Request::GetStatus,
                Duration::from_secs(1)
            )
            .await
            .is_err()
        );
    }
}
