# Integration contract

The one interface every track builds against: the `friglet-ui` binary, the `friglet-node`
image, and the native tarball. `docs/plan.md` is the why; this file is the what. A change
here is a change for every platform package, so change it in one commit with them.

## Artifacts

| Artifact | Name | Contents |
|---|---|---|
| Image | `friglet-node:<version>` (local tag `friglet-node:dev`; registry TBD, placeholder `ghcr.io/setavenger/friglet-node`) | `/usr/local/bin/friglet`, `/usr/local/bin/friglet-ui`, `tini`; entrypoint `tini -- friglet-ui`; runs as uid:gid 1000:1000; `VOLUME /data` |
| Native tarball | `friglet-node-<version>-linux-<amd64\|arm64>.tar.gz` + `SHA256SUMS` | `friglet`, `friglet-ui`, `LICENSE`s; glibc ≤ 2.36 (Debian bookworm) |

`<version>` is `<friglet short commit>-<package revision>` until blindbit-rs tags releases
(BR11), e.g. `93f6a92-1`.

## friglet-ui daemon (`friglet-ui` with no subcommand, or `friglet-ui run`)

Configuration comes only from `FRIGLET_UI_*` environment variables (or matching flags). Any
other `FRIGLET_*` variable in the environment is a fatal startup error naming the variable.

| Variable | Default | Meaning |
|---|---|---|
| `FRIGLET_UI_PLATFORM` | `compose` | `umbrel`, `startos`, `raspiblitz`, `mynode`, `compose`; selects copy, endpoints and auth defaults |
| `FRIGLET_UI_DATA_DIR` | `/data` | config, key, state, certs, `prev/`, UI secrets; created 0700 |
| `FRIGLET_UI_LISTEN` | `0.0.0.0:8097` (image), `127.0.0.1:8097` (native) | web UI + JSON API |
| `FRIGLET_UI_NETWORK` | required | `bitcoin`, `signet`, `testnet4`, `regtest`; anything else (e.g. Umbrel's `testnet` for testnet3) is the named state `unsupported-network`, not a restart loop |
| `FRIGLET_UI_NODE` | required | Bitcoin P2P peer `host:port`; resolved to a literal IP on every friglet start |
| `FRIGLET_UI_ORACLE_URL` | per network (`https://oracle.setor.dev`, `https://signet.oracle.setor.dev`) | gRPC oracle; overridable later under Advanced |
| `FRIGLET_UI_AUTH` | `password` (`gateway` on umbrel) | `password`, `gateway` (trust the platform's login; Umbrel only), `none` (compose only; warning + banner). StartOS uses `password` with a package-written `FRIGLET_UI_PASSWORD_FILE`, because port 8097 is reachable from other services without the StartOS proxy |
| `FRIGLET_UI_PASSWORD` | — | plaintext initial password (compose); hashed into the data dir on first start |
| `FRIGLET_UI_PASSWORD_FILE` | — | file holding the password (native installs, myNode and StartOS write it). Authoritative: re-read on every start, one trailing newline trimmed; when it differs from the stored hash the hash is replaced and all sessions end (this is how platform password resets work) |
| `FRIGLET_UI_TRUSTED_PROXIES` | `127.0.0.1,::1` | comma-separated IPs/CIDRs whose `X-Forwarded-For` is trusted for the per-client login limit (myNode: `172.17.0.1`) |
| `FRIGLET_UI_ELECTRUM_TCP` | `0.0.0.0:50031` | plaintext Electrum listener owned by friglet-ui; `off` disables |
| `FRIGLET_UI_ELECTRUM_TLS` | `0.0.0.0:50032` | TLS Electrum listener (self-signed cert in data dir); `off` disables (StartOS) |
| `FRIGLET_UI_PUBLIC_HOST` | request `Host` | LAN host shown on Connect (e.g. `umbrel.local`) |
| `FRIGLET_UI_ONION_HOST` | — | onion hostname shown on Connect for the plaintext port; empty string = unset (Umbrel exports `""` until Tor has written it) |
| `FRIGLET_UI_ALLOWED_HOSTS` | derived | comma-separated extra `host[:port]` values accepted for state-changing requests. Default policy is same-origin: `Origin` (or `Referer`) must match the request's own `Host`, which covers Umbrel's gateway (forwards the browser Host unchanged: `umbrel.local:2104`, LAN IP, web onion) |
| `FRIGLET_UI_FRIGLET_BIN` | `/usr/local/bin/friglet` (image), next to `friglet-ui` (native) | daemon binary |
| `FRIGLET_UI_LOG` | `info` | friglet-ui log level |

friglet itself is always started by friglet-ui with a cleared environment (PATH, HOME, TZ only),
umask 077, `--config <data>/config.toml`, Electrum on `127.0.0.1:50001`, HTTP on
`127.0.0.1:8095`, control socket `<data>/friglet.sock`, state file `<data>/scanner_state.json`.

## friglet-ui setup CLI

`friglet-ui setup [--network N] [--node host:port] --birthday <height> [--descriptor-file F]
[--replace] [--dry-run] [--json]`; descriptor from stdin or `--descriptor-file` (0600) only.
Exit codes: 0 ok, 2 usage, 3 descriptor, 4 birthday, 5 network mismatch, 6 node check,
7 wallet exists (needs `--replace`), 8 I/O. With a running supervisor it hands the request
over (through `<data>/friglet-ui.sock`; the daemon holds the data-dir `flock` for its life);
otherwise it stages the files under the `flock` and reports "staged, verification pending".
`--node` is checked only when passed. `--json` prints
`{"ok":bool,"code":n,"state":str|null,"sp_address":str|null,"key_fingerprint":str,"network":str,"message":str}`
plus `error_code` (and `field` for codes 3/4/5) on failure. `state` is `"staged"` or
`"unchanged"` offline, the current state id when handed over, `null` for `--dry-run`/errors.

UI credentials (`ui-password`, the password hash, sessions) and the TLS certificate are not
wallet data: `forget` and wallet replacement never touch them.

Other subcommands: `friglet-ui password set|reset` (reads stdin, ends all sessions, works while
the daemon runs),
`friglet-ui status --json` (daemon running: the full `/api/state` document, exit 0 ok/info,
1 warn/error, 2 needs-setup; daemon down: `{"configured":bool,"running":false,"state":null}`, exit 1, or 2 when unconfigured),
`friglet-ui health` (exit 0 in `scanning`, `up-to-date`, `rescanning-after-reset`; for container
healthchecks), `password reset` with empty stdin generates a password and prints it once,
`friglet-ui --version`, `friglet-ui self-test` (no network; checks the embedded assets and
crypto, used to prove arm64 binaries execute).

## HTTP surface

- `/` web UI (static, embedded). All `/api/*` JSON. State-changing requests need the CSRF
  token and a Host/Origin allowlist match on every platform.
- `GET /api/widget` — unauthenticated, read-only, fixed schema. It must be a valid Umbrel
  `text-with-progress` widget payload (umbreld parses `refresh` with `ms()` and throws otherwise)
  and carries our three fields alongside:
  `{"type":"text-with-progress","refresh":"30s","link":"","title":"Silent Payments scan",
  "text":"42%","subtext":"Block 850,000 of 900,000","progressLabel":"Scanning","progress":0.42,
  "state":id,"scanned_height":n|null,"tip_height":n|null}`. Error states use
  `text:"Needs attention"`, `progress:0`. No wallet data in it.
- `GET /healthz` — unauthenticated liveness of friglet-ui itself (200 always while serving).

## Ports (host side, every platform unless its package says otherwise)

| Port | What |
|---|---|
| 50031 | Electrum plaintext (friglet-ui) |
| 50032 | Electrum TLS (friglet-ui; RaspiBlitz may front with nginx stream instead) |
| 8097 | friglet-ui (container port; loopback on native installs) |
| 8098 | friglet-ui over HTTPS via nginx on RaspiBlitz and myNode |
| 2104 | Umbrel app proxy port |
| 8337 | RaspiBlitz bitcoind `whitebind=download@127.0.0.1:8337` for friglet (8336 is BTCPay's whitebind on RaspiBlitz; a second bind on it stops bitcoind) |
| 8095 | friglet HTTP API, loopback only, never published |
