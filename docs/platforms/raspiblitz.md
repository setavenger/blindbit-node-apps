# friglet on RaspiBlitz

RaspiBlitz v1.12 runs apps as native binaries under systemd, installed by a "bonus script".
Docker was removed in 2024 (`upstream/raspiblitz/NOTES.md` §11). friglet ships the same way:
`raspiblitz/home.admin/config.scripts/bonus.friglet.sh` installs the `friglet-node` release
tarball (`friglet` + `friglet-ui`, `docs/contract.md`) and runs `friglet-ui` as `friglet.service`.

**Status (2026-10-01): not installable yet.** The script pins no release checksum, so
`install`/`on` refuse (see [Releasing](#releasing-a-version-into-the-script)). It has been
verified only in a Debian bookworm container with stub binaries ([Tests](#tests)).

## Requirements

- RaspiBlitz v1.12 or `dev`, on arm64 (Raspberry Pi) or amd64.
- An **unpruned** Bitcoin node: friglet fetches every block from the wallet birthday from the
  local bitcoind. `on` refuses on a pruned node.
- `chain=main` or `chain=sig` in `raspiblitz.conf`. RaspiBlitz `chain=test` is testnet3, which
  friglet does not support, so `on` refuses it.
- Sparrow with a Silent Payments wallet, run as a dedicated instance (see the web UI Connect
  page).

## Using it

1. SSH menu → SERVICES → `fg` *BTC friglet Silent Payments Scanner*, or
   `/home/admin/config.scripts/bonus.friglet.sh on`.
2. The FRIGLET menu (main menu, or CONNECT) shows:
   - the web UI address `https://<ip>:8098` and the nginx certificate's SHA-256 fingerprint
   - the Tor Browser address, if Tor is on
   - the generated web UI password
   - the current status
3. Log in to the web UI. Paste the descriptor Sparrow shows and the wallet birthday, and
   compare the address. The Connect page shows the Electrum endpoints.

The SSH menu shows only the URL, password, status, password reset and Electrum exposure.
Wallet setup and connection details live in the web UI (plan: "RaspiBlitz SSH menu shows URL,
password, status and password reset only").

## Script actions

| Action | What it does |
|---|---|
| `status [showAddress]` | `key=value` lines, nothing on stderr, fast. blitz_api-compatible keys (`installed`, `configured`, `version`, `localIP`, `httpsPort`, `httpsForced`, `httpsSelfsigned`, `authMethod`) plus `state`, `scannedHeight`, `tipHeight`, `infoSync`, `electrumExposure`. Read from friglet-ui's `/api/widget`. The onion address appears only with `showAddress`. |
| `status-sync` | Cheap scan state for the login screen (`00infoBlitz.sh`). |
| `menu` | SSH dialog (see above). Refuses without a terminal, so the password never lands in a log. |
| `install` | Code only: downloads, verifies and unpacks the pinned tarball. Never touches the HDD. |
| `on` | Checks, installs, configures and enables (details below). Starts the service only when RaspiBlitz `state=ready`. Safe to repeat. |
| `off [--delete-data]` | Removes unit, nginx site, ufw rules, onion service, whitebind line, user and code. Keeps `/mnt/hdd/app-data/friglet` unless `--delete-data` (or `delete-data`) is given. |
| `update` | Installs the version pinned in the script and restarts. No-op if it is already active. Keeps the previous version. |
| `password-reset` | New web UI password, handed to `friglet-ui password set` on stdin (ends all sessions). Restarts the service only if that fails. |
| `electrum-exposure <lan\|lan-tcp\|onion\|local>` | Rewrites the listeners, ufw rules and onion ports, saves `frigletElectrum` in `raspiblitz.conf` and restarts the service. Used by the menu. |
| `prestart` | systemd `ExecStartPre` as user friglet. Writes `FRIGLET_UI_PUBLIC_HOST` (current LAN IP) and `FRIGLET_UI_ONION_HOST`. |

What `on` does, in order:

1. Refuses on a pruned node or `chain=test`.
2. Installs the pinned release (refuses if no checksum is pinned or the checksum differs).
3. Creates `/mnt/hdd/app-data/friglet` (0700) and the web UI password (only if none exists).
4. Adds the bitcoind whitebind.
5. Writes `friglet.service` and the nginx site (`nginx -t` is checked; on failure the site is
   removed again).
6. Sets the ufw rules and the onion service, writes the host environment, and sets
   `friglet=on`.
7. Enables the service and, if the node is ready, restarts bitcoind once (only if the
   whitebind was added and the port is not bound yet), then starts or restarts friglet.

friglet is restarted only when the unit or the onion address changed, so repeated `on` runs
(provisioning after an SD card update) keep the wallet, the password and the running scan.

## Files

| Path | Owner, mode | Content |
|---|---|---|
| `/home/friglet/friglet-node-<version>/` | root, 0755 | `friglet`, `friglet-ui`, licenses. Current and previous version are kept. |
| `/home/friglet/current` | symlink | Active version |
| `/home/friglet/friglet-ui.env` | friglet, 0600 | `FRIGLET_UI_PUBLIC_HOST`, `FRIGLET_UI_ONION_HOST` |
| `/home/friglet/onion-hostname` | root:friglet, 0640 | Copy of the Tor hostname (the Tor dir is not readable by friglet) |
| `/mnt/hdd/app-data/friglet/` | friglet, 0700 | `FRIGLET_UI_DATA_DIR`: config, scan key, state, certificates, `prev/` (all written by friglet-ui) |
| `/mnt/hdd/app-data/friglet/ui-password` | friglet, 0600 | Web UI password (`FRIGLET_UI_PASSWORD_FILE`) |
| `/etc/systemd/system/friglet.service` | root | Unit (below) |
| `/etc/nginx/sites-available/friglet_ssl.conf` | root | HTTPS 8098 → 127.0.0.1:8097 with `ssl-params`, `ssl-certificate-app-data` and `ssl-proxy-params` |
| `/mnt/hdd/app-data/raspiblitz.conf` | | `friglet=on\|off`, `frigletElectrum=<mode>` |

The unit sets the contract environment:

- `FRIGLET_UI_PLATFORM=raspiblitz`, `AUTH=password`, `PASSWORD_FILE`, `DATA_DIR`
- `NETWORK` (`bitcoin` or `signet`, from `chain`), `NODE=127.0.0.1:8337`,
  `LISTEN=127.0.0.1:8097`
- `ELECTRUM_TCP`/`ELECTRUM_TLS` per exposure mode
- `EnvironmentFile=-/home/friglet/friglet-ui.env`

Service settings: `User=friglet`, `UMask=0077`, `KillMode=mixed` (SIGTERM to friglet-ui only,
which stops friglet), `TimeoutStopSec=150` (friglet-ui's 120 s grace),
`ProtectSystem=strict` with write access only to the data dir and `/home/friglet`,
`PrivateTmp`, `PrivateDevices`, `NoNewPrivileges`.

The bonus script never reads or writes the scan key or `config.toml`. It never prints the UI
password and never uses `tee`. The password is generated by the friglet user inside the data
dir, and reaches friglet-ui only through the file and stdin.

## Ports and Electrum exposure

| Port | Bind | Exposure |
|---|---|---|
| 8097 | 127.0.0.1 | friglet-ui; reached through nginx and Tor only |
| 8098 | nginx `0.0.0.0` / `[::]` | web UI over HTTPS; ufw allows the LAN only |
| 50031 | per mode | Electrum plaintext; onion port 50031 |
| 50032 | per mode | Electrum TLS (friglet-ui's self-signed certificate; fingerprint on its Connect page) |
| 8095 | 127.0.0.1 | friglet HTTP API, started by friglet-ui, never exposed |
| 8337 | 127.0.0.1 | bitcoind `download` whitebind for friglet |

"LAN" in ufw means 10.0.0.0/8, 172.16.0.0/12 and 192.168.0.0/16, the ranges `_provision_.sh`
uses for HTTPS 443.

| Mode (`frigletElectrum`) | `ELECTRUM_TCP` | `ELECTRUM_TLS` | ufw LAN | Onion ports |
|---|---|---|---|---|
| `lan` (default) | 127.0.0.1:50031 | 0.0.0.0:50032 | 8098, 50032 | 50031, 80→8097 |
| `lan-tcp` | 0.0.0.0:50031 | 0.0.0.0:50032 | 8098, 50031, 50032 | 50031, 80→8097 |
| `onion` (needs Tor) | 127.0.0.1:50031 | off | 8098 | 50031, 80→8097 |
| `local` | 127.0.0.1:50031 | 127.0.0.1:50032 | 8098 | 80→8097 |

- Electrum on RaspiBlitz is TLS by friglet-ui itself, not an nginx `stream {}` block. That
  avoids editing the shared `nginx.conf` (NOTES §7, §12.8). Listeners also open only after
  the wallet identity is verified.
- Anyone who reaches an Electrum port can read the wallet's history. The menu says so.

## bitcoind connection

`on` adds `<chain>.whitebind=download@127.0.0.1:8337` (prefix `main`/`signet`, like electrs
and BTCPay). It restarts bitcoind once, only when the line was added, nothing listens on
8337 yet, and the node is ready. `off` removes the line without restarting bitcoind. `on`
refuses if another whitebind already uses 8337.

**Deviation from `docs/contract.md`:** the contract names 8336. On RaspiBlitz, BTCPay Server
writes `<chain>.whitebind=127.0.0.1:8336` (`bonus.btcpayserver.sh` on `dev`). A second bind on
the same address stops bitcoind from starting, and BTCPay's own check would add its line
next to ours. 8337 is unused in the RaspiBlitz tree.

## Pruned node refusal

`on` reads `bitcoin.conf`: plain `prune=`, `main.prune=`/`signet.prune=` and `[main]`
sections. Any positive value for the active chain refuses. If `bitcoin-cli` is available it
also asks bitcoind (`getblockchaininfo.pruned`). The exact text (setting and value vary):

```
# REFUSED: friglet needs an unpruned Bitcoin node.
# Problem: this node is pruned (prune=550 in /mnt/hdd/app-data/bitcoin/bitcoin.conf).
# Cause: friglet downloads every block from your wallet birthday onward from this node, and a pruned node has deleted old blocks.
# Fix: remove the prune setting (or set prune=0), restart bitcoind and let it download the full chain again, then switch friglet on again.
# Help: blindbit-node-apps docs/troubleshooting.md#node-pruned
error='node is pruned (prune=550), friglet needs an unpruned node'
```

Nothing is installed or changed when it refuses.

## Updates and rollback

- `update` (also in the UPDATE menu) installs the version pinned in the script. A new pinned
  version arrives with a RaspiBlitz `patch` or release.
- friglet-ui backs up the state file before running a different friglet version (plan,
  "Upgrades").
- Rollback: `sudo ln -sfn friglet-node-<previous> /home/friglet/current &&
  sudo systemctl restart friglet`. The next `update` or `on` switches to the pinned version
  again.

## Off, uninstall and data

- `off` keeps `/mnt/hdd/app-data/friglet`: wallet config, scan key, state and web UI password.
  The next `on` reuses them.
- `off --delete-data` deletes that directory, the only local copy of the scan key. Sparrow
  still holds the wallet.
- RaspiBlitz data migration (`blitz.data.sh`) carries `app-data`, scan key included. Whether
  the migration export file does too was not checked.

## Logs

`sudo journalctl -u friglet`. Until blindbit-rs BR8 these logs contain txids, amounts and
client IPs, so `blitz.debug.sh` (shared for support, also served over an onion) includes only
`systemctl status friglet -n0` and `bonus.friglet.sh status`, not the journal.

## Upstream PR

The app PR to `raspiblitz/raspiblitz` `dev` is:

- `bonus.friglet.sh`
- `raspiblitz/upstream-pr/0001-*.patch`: provisioning, SERVICES, main, CONNECT and UPDATE
  menus, login status line, debug, shutdown, Tor re-creation
- `0002-*.patch`: `CHANGES.md`

The patches are generated with `git format-patch` against `c421693`. The draft description
and the conditions for opening it are in `raspiblitz/upstream-pr/PR_DESCRIPTION.md`.
`raspiblitz/upstream-pr/check.sh` assembles the branch and runs shellcheck at upstream CI
level. It also reports whether the PR applies to the `rootzoll/raspiblitz` fork.

`raspiblitz/raspiblitz` is the canonical repo. `rootzoll/raspiblitz` is a fork whose `dev`
(`eb7b2f3`) is 28 commits behind it and 1 ahead: the code patch applies there, `CHANGES.md`
does not.

The WebUI (blitz_api `AppId`, raspiblitz-web app list) needs separate PRs in those repos and is
not done.

## Releasing a version into the script

1. Publish `friglet-node-<version>-linux-{arm64,amd64}.tar.gz` and `SHA256SUMS` (+ `.asc`).
2. In `bonus.friglet.sh`, set:
   - `VERSION`
   - `RELEASE_BASE_URL` (marked `TODO(release-location)`)
   - `SHA256_ARM64`, `SHA256_AMD64`
   - `PGP_PUBKEY_URL`, `PGP_FINGERPRINT` (marked `TODO(signing-key)`)
3. Once a fingerprint is set, install also requires a `VALIDSIG` by that key over `SHA256SUMS`,
   and `SHA256SUMS` must list the tarball with the pinned hash.
4. Run `raspiblitz/test/run.sh` and `raspiblitz/upstream-pr/check.sh`.

## Tests

Run `raspiblitz/test/run.sh` (needs Docker; no `--privileged`, no added capabilities). It runs
shellcheck, then a Debian bookworm container with:

- **Stub binaries:** `friglet-node` tarballs whose `friglet`/`friglet-ui` are stubs
  (`raspiblitz/test/stub-tarball/`). They serve `/api/widget`, bind the Electrum listeners
  and record their environment.
- **Real RaspiBlitz parts:** nginx with the RaspiBlitz snippets, plus the real `blitz.conf.sh`
  and `tor.onion-service.sh` from the PR base commit, fetched with pinned SHA256.
- **Fakes:** a fake bitcoind that binds the whitebinds it finds, a fake Tor reload that
  creates onion hostnames, and a ufw stub that records rules.
- **systemd stand-in:** `docker-systemctl-replacement`. systemd cannot be PID 1 in an
  unprivileged container here (read-only cgroup tree).

Covered:

- refusals: no pinned checksum, checksum mismatch, pruned node (3 forms with exact text),
  testnet3
- the first `on`: user, modes, unit environment, whitebind, a single bitcoind restart, the
  HTTPS proxy, ufw, onion, listeners
- `status`: key=value only, nothing on stderr
- repeated `on`: nothing restarts or changes
- SD card update: `on` before the node is ready keeps the wallet and password
- every exposure mode, password reset, the menu (no terminal, and with a terminal via
  `script`)
- update and keeping the previous version, `off`, `on` again, `off --delete-data`
- a scan of all output and unit logs for the passwords and a test scan key generated at run
  time

Not covered:

- real friglet/friglet-ui binaries
- real systemd: sandboxing and the timing of `EnvironmentFile=` after `ExecStartPre`
- real ufw and Tor
- arm64
- RaspiBlitz hardware, and the menus patched into RaspiBlitz scripts (checked by shellcheck
  only)
