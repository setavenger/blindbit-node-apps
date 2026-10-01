# friglet on StartOS

StartOS package for [friglet](https://github.com/setavenger/blindbit-rs), BlindBit's
single-wallet Silent Payments scanner and Electrum server, run under `friglet-ui`. Built
with start-sdk 2.0.x for StartOS 0.4.0 and later. Operator guide:
[`docs/platforms/startos.md`](../docs/platforms/startos.md). Shared contract for the image
and `friglet-ui`: [`docs/contract.md`](../docs/contract.md).

## Table of Contents

- [Image and Container Runtime](#image-and-container-runtime)
- [Volume and Data Layout](#volume-and-data-layout)
- [File Models](#file-models)
- [Dependencies](#dependencies)
- [Network Access and Interfaces](#network-access-and-interfaces)
- [Installation and First-Run Flow](#installation-and-first-run-flow)
- [Actions](#actions)
- [Tasks](#tasks)
- [Health Checks](#health-checks)
- [Backups and Restore](#backups-and-restore)
- [Limitations and Differences](#limitations-and-differences)
- [Quick Reference for AI Consumers](#quick-reference-for-ai-consumers)

## Image and Container Runtime

- One image, `main`: the `friglet-node` image (`/usr/local/bin/friglet`,
  `/usr/local/bin/friglet-ui`, `tini`), x86_64 and aarch64, runs as uid:gid 1000:1000.
- The daemon runs the image entrypoint `tini -- friglet-ui`; friglet-ui supervises friglet.
  SIGTERM timeout 150 s (friglet-ui waits up to 120 s for friglet).
- Configuration is environment only, all `FRIGLET_UI_*`: platform `startos`, network
  `bitcoin`, data dir `/data`, UI on `0.0.0.0:8097`, auth `gateway` (the StartOS proxy
  authenticates), plaintext Electrum on `0.0.0.0:50031`, Electrum TLS `off` (StartOS
  terminates TLS), and `FRIGLET_UI_NODE` = bitcoind's `peer-local` bridge address.
- Before the daemon starts, the package hands `/data` to uid 1000 and sets it to 0700
  (StartOS mounts volumes root-owned).

## Volume and Data Layout

| Volume | Mounted at | Contents |
|---|---|---|
| `main` | `/data` | friglet-ui's data dir: `config.toml`, `scan.key` (0600), friglet's state file, `prev/`, control sockets |
| `startos` | not mounted | `store.json` (package state) |

## File Models

`store.json` on `startos`: `uiPassword` (proxy basic-auth password), `walletConfigured`
(switches the setup action to "Change wallet"), `spAddress` (public address of the wallet).
The package never models or writes friglet-ui's own files; `friglet-ui setup` does.

## Dependencies

- **Bitcoin** (`bitcoind`), required, running, version
  `(>=28.4:17 <29) || (>=29.4:4 <30) || (>=30.3:4 <31) || >=31.1:4`: the first release on
  each line that binds `peer-local` (whitelisted P2P, bridge only). Health checks
  `bitcoind`, `sync-progress`.
- A critical task on Bitcoin's **Auto-Configure** action sets `prune = 0`.
- Outbound HTTPS to the BlindBit oracle (`oracle.setor.dev`, third party).

## Network Access and Interfaces

| Interface | Container port | Off-box | Notes |
|---|---|---|---|
| `ui` (Web UI) | 8097 http | HTTPS via the StartOS proxy, basic auth `friglet` / `uiPassword` | plaintext only on the bridge |
| `electrum` (Electrum (SSL)) | 50031 tcp | `ssl://` via StartOS TLS, preferred external port 50032 | plaintext 50031 only on the bridge; addresses marked `ssl`/`tcp` |

## Installation and First-Run Flow

1. Install seeds `uiPassword` and raises the critical **Set up wallet** task.
2. The user runs **Set up wallet** (descriptor + birthday). The service cannot start before.
3. On start, friglet-ui verifies the wallet identity and opens Electrum.

## Actions

| Action | When | What it does | Repeat safety |
|---|---|---|---|
| `setup-wallet` (Set up wallet / Change wallet) | first run; to replace the wallet | runs `friglet-ui setup --network bitcoin --birthday N --json [--node peer-local] [--replace]` in a one-off subcontainer, descriptor on stdin; friglet-ui hands it to a running supervisor or stages files under a `flock`; 60 s timeout; returns the SP address | same descriptor and birthday is a no-op; a different wallet needs Change wallet (`--replace`) |
| `show-ui-password` | to log in | shows `friglet` / `uiPassword` | read-only |
| `reset-ui-password` | password leaked or lost | new random password; the proxy picks it up, friglet-ui keeps running | each run replaces the password |

## Tasks

- **Set up wallet** (own, critical): raised while `walletConfigured` is false; cleared by a
  successful run.
- **Auto-Configure** on Bitcoin (critical): raised while Bitcoin's `prune` is not 0; appears
  on the Bitcoin service page.

## Health Checks

| Id | Shows | Source |
|---|---|---|
| `primary` (Web UI) | friglet-ui is listening | port 8097 |
| `scan` (Scan) | state and heights; failure names the problem | `friglet-ui health` + `friglet-ui status --json`, every 15 s |
| `electrum` (Electrum) | Electrum port open (only after identity is verified) | port 50031 |
| `bitcoin-peer` | only when Bitcoin has no `peer-local` binding; friglet-ui does not run then | dependency check |

## Backups and Restore

Both volumes, sockets excluded. The state file is kept so a restore does not rescan.
Backups contain the scan key (`scan.key`, state file), encrypted with the StartOS master
password.

## Limitations and Differences

- One wallet. Electrum serves only that wallet (BR1, BR2 in `docs/plan.md`).
- Mainnet only (StartOS Bitcoin is mainnet).
- Inside the server, other services on the bridge reach the UI's plaintext port without the
  proxy's basic auth.

## Quick Reference for AI Consumers

```yaml
package_id: blindbit-friglet   # permanent
image: friglet-node (dockerTag; FRIGLET_NODE_IMAGE overrides at pack time)
arch: [x86_64, aarch64]
volumes: {main: /data, startos: unmounted store.json}
interfaces: {ui: 8097 http + basic auth, electrum: 50031 tcp, ssl external (pref 50032)}
dependencies: {bitcoind: running, peer-local P2P, prune=0 task}
actions: [setup-wallet, show-ui-password, reset-ui-password]
health_checks: [primary, scan, electrum]
secrets: descriptor via stdin only; never in argv, env, logs or action results
```
