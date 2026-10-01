# NOTES — packaging friglet for RaspiBlitz

Distilled from the verbatim files in this folder (see `SOURCES.md`). Citations use the form
`<folder>/<path>:<lines>` and are relative to `upstream/raspiblitz/`. "friglet facts" come from
`blindbit-rs@611a877`, which is not pulled here. Status: 2026-10-01.

## 0. Which app model is current (Oct 2026)

| Model | Status | Relevance |
|---|---|---|
| **RaspiBlitz v1.12.x** bonus scripts (`home.admin/config.scripts/bonus.<app>.sh` + systemd) | Latest release is v1.12.1 (2026-03-01). The release branch `v1.12` is still patched (head 2026-08-23). | **Target this.** |
| WebUI = `raspiblitz-web` + `blitz_api` | Ships with v1.12. Apps are a hard-coded whitelist that calls the same bonus scripts. | Optional second step |
| **NixBlitz** (NixOS, plugin = `plugin.nix` + `plugin.json`) | "Highly experimental", no security review (`nixblitz/README.md:3-6`) | Watch only; it's not a RaspiBlitz release |

- There is no "RaspiBlitz v2" app model. The successor experiment is NixBlitz (issue #3332 "RESEARCH: nixBlitz").
- Release branches are `v1.x`. Development and PRs go to `dev` (`raspiblitz-docs/docs/faq/dev.md:150,159-165`).
- Canonical repo is `raspiblitz/raspiblitz`. `rootzoll/raspiblitz` is now a fork.

## 1. Script structure and required actions

Start from `raspiblitz/home.admin/config.scripts/bonus.template.sh`. That is the official instruction
(`raspiblitz-docs/docs/faq/dev.md:211-213`).

| Action | Purpose | Source |
|---|---|---|
| `status` | `key=value` lines only; comments start with `#`. The SSH menu, WebUI and debug all `source <(...)` it. | template `:81-99`; electrs `:24-104` |
| `status-sync` | Cheap, loop-safe sync/index progress (electrs, fulcrum) | electrs `:107-166`; fulcrum `:118-172` |
| `menu` | SSH dialog: connect info, status, logs, reindex | template `:114-138`; electrs `:168-284`; fulcrum `:174-365` |
| `install` | Code/binary only, with no HDD or config. `blitz.fatpack.sh` calls it at **image build** time. | electrs `:291-329`; albyhub `:151`; fatpack `:147-153,219-224` |
| `on` / `1` | Configure and enable. Re-run on **every SD update/recovery** by `_provision_.sh`, so it **must be idempotent** and must reuse existing app-data. | template `:147-347`; `_provision_.sh:423-437,743-750` |
| `off` / `0` | Remove the unit, nginx, ufw and onion, and set the flag to off. Keep data unless `delete-data` / `deleteindex` is passed. | template `:418-455`; electrs `:577-615` |
| `update` | Called from the SSH UPDATE menu | electrs `:618-655`; `99updateMenu.sh:582-588,659-663` |
| `prestart` | Optional `ExecStartPre=-…bonus.X.sh prestart`. Runs as the app user (no sudo). Regenerates config ad hoc. | template `:248,390-410` |
| `uninstall` | Optional: remove the user and code after `off` | electrs `:563-574` |

- The app flag lives in `/mnt/hdd/app-data/raspiblitz.conf` and is set via `blitz.conf.sh set <appid> on|off`
  (template `:335,446`; `blitz.conf.sh:1-40`). The key is the app id, e.g. `friglet=on`.
- Only start the service when the node is ready: `source <(/home/admin/_cache.sh get state)`, then check `state == ready`
  (template `:342-346`; electrs `:541-552`).
- The template's `status` emits `isInstalled`/`isRunning`, but blitz_api reads **`installed`** (§10). Follow
  albyhub (`bonus.albyhub.sh:93-110`), not the template, for status keys.

## 2. Installing the binary (three patterns in the tree)

| Pattern | Example | Notes |
|---|---|---|
| Build from source with rustup + `cargo build --locked --release`, verifying the tag via `blitz.git-verify.sh` | electrs `:291-329` (Rust, ~40 min on a Pi, needs clang/cmake); lndk `:34-43` | Removes `.cargo`/`.rustup` afterwards (electrs `:537-539`). |
| Download the release, `gpg --verify` the signed SHA256SUMS, `sha256sum -c` | fulcrum `:367-395` (arch map `:432-437`); lit `:153-210` (signed manifest) | **Preferred pattern for friglet.** |
| Download the binary per arch with **no verification** | albyhub `:166-180` | Don't copy this. |
| Docker | none in v1.12 | Removed; see §11 |

- Arch: only `aarch64` and `x86_64` are supported (`build_sdcard.sh:270-278`). The base is Debian 12 bookworm /
  RaspiOS bookworm arm64 (`build_sdcard.sh:4-17`). `dev` adds Debian 13 trixie handling (SOURCES §1).
- **glibc:** bookworm ships glibc 2.36. Release binaries must target glibc <= 2.36, or be static musl.
  friglet's Docker image builds on `rust:1.97-slim` and runs on `debian:trixie-slim` (blindbit-rs `Dockerfile`), so
  a binary lifted from it probably won't run on bookworm (not verified).
- blindbit-rs has **no GitHub releases** today (`gh release list` is empty), so the fulcrum pattern needs us to publish
  signed per-arch tarballs first. Until then, only the electrs-style source build works. That build also needs the
  `protobuf-compiler` + `libprotobuf-dev` apt packages (blindbit-rs `Dockerfile`).

## 3. systemd conventions

- The bonus script writes `/etc/systemd/system/<app>.service` inline; there are no packaged units (template `:233-266`).
- Typical settings: `After=bitcoind.service`, `User=`/`Group=` set to the app user, `Restart=always` or `on-failure`,
  `RestartSec`, `TimeoutSec`, hardening (`PrivateTmp`, `ProtectSystem=full`, `NoNewPrivileges`,
  `PrivateDevices`) (template `:240-262`; electrs `:478-500`; fulcrum `:397-427`).
- `KillSignal=SIGINT` + `TimeoutStopSec=300` for DB-heavy daemons (fulcrum `:419-424`). The app also belongs in
  `blitz.shutdown.sh:45` `servicesToStop` so the DB closes cleanly.
- `systemctl enable` happens in `on`; `start` happens only when `state == ready` (template `:337-346`).

## 4. Users and directories

- Dedicated system user: `adduser --system --group --home /home/<app> <app>` (template `:166`; electrs `:302`).
- **DATA**, `/mnt/hdd/app-data/<app>`: critical data and config. It survives updates and is migrated
  (template `:177-194`; `blitz.data.sh:33-41`; `raspiblitz-docs/docs/faq/dev.md:156-157`).
- **STORAGE**, `/mnt/hdd/app-storage/<app>`: regenerable indexes/DBs. electrs and fulcrum keep `db/` and an
  `initial-sync.done` flag there (electrs `:141-146,393`; fulcrum `:148-153,464-469`).
- friglet mapping:
  - `scan.key` (0600, owner friglet) and `config.toml` go in `app-data/friglet/`.
  - `scanner_state.json` can be regenerated by rescanning from the birthday, so it can live in `app-storage/friglet/`.
  - Set `--config`, `--key-file` and `state_file` explicitly. friglet's defaults use the platform config dir (`friglet-ipc`).
- Code/binary lives in `/home/<app>/` or `/usr/local/bin` (lndk `:118`). It's reinstalled after an SD update.

## 5. Bitcoin node access

- `bitcoin.conf` is `/mnt/hdd/app-data/bitcoin/bitcoin.conf`. Apps scrape RPC credentials from it with grep/cut
  (electrs `:376-379`; fulcrum `:473-474`). The template's defaults: `rpcuser=raspibolt`, RPC on 127.0.0.1:8332
  (`assets/bitcoin.conf:19-25`).
- If an app stores the RPC password, it needs a hook in `blitz.passwords.sh` for password-B changes
  (`blitz.passwords.sh:352-358`). friglet uses **no RPC**, so it doesn't need one.
- **friglet uses P2P** to fetch blocks and broadcast transactions, plus an oracle over gRPC.
  - bitcoind P2P listens on `main.bind=127.0.0.1` (Tor-only by default: `onlynet=onion`) (`assets/bitcoin.conf:36-43`).
  - `maxuploadtarget=5000` (`:34`) can cut off historical-block downloads. electrs therefore adds a dedicated
    **`<chain>.whitebind=download@127.0.0.1:8335`** and restarts bitcoind (electrs `:394,517-546`).
  - friglet should do the same on its own port (for example 8336, unverified as free). Point `p2p_node_addr` at that port.
- Chain mapping: `raspiblitz.conf` `chain=main|test|sig` maps to prefix `main|test|signet` (electrs `:517-526`). For
  friglet's `network`, map to `bitcoin|testnet|signet`.
- Bitcoin Core version: a fresh install gets **29.2** (`bitcoin.install.sh:5`); the recommended update is **30.2**
  (`bitcoin.update.sh:31`). A **local blindbit-oracle needs Core >= 30 + REST** (`rest=1`, which is not in the default
  `bitcoin.conf`), per blindbit-oracle `README.md`. Knots is 29.2.2 (`bonus.knots.sh`, not pulled).

## 6. Ports and firewall

- There is no central port registry. Ports are hard-coded per script. Known conflicts with friglet's defaults:
  - **8080** is the LND REST port, opened by `_provision_.sh:143` and used by `lnd.install.sh`. friglet's default `127.0.0.1:8080` collides.
  - **50001/50002** belong to electrs (electrs `:470-471`). **50021/50022/8021** belong to fulcrum (fulcrum `:6-8`).
- Proposal (unverified beyond grep):
  - Electrum TCP **50031**, SSL **50032** (nginx stream).
  - HTTP API **8095**, bound to 127.0.0.1 with **no** ufw rule. friglet's HTTP API has no auth.
- ufw: `sudo ufw allow <port> comment '<app> TCP'` in `on`; `ufw delete allow <port>` in `off` (electrs
  `:470-471,608-609`). Note: the template's `off` uses `ufw deny` instead (template `:436-437`).

## 7. nginx and TLS

- Self-signed certs live in `/mnt/hdd/app-data/nginx/tls.cert|tls.key` (and `tor_tls.*` for Tor), via snippets
  (`assets/nginx/snippets/ssl-certificate-app-data*.conf`). `blitz.web.sh:81,138-144` creates them.
- Web apps: `sites-available/<app>_ssl.conf`, `_tor.conf`, `_tor_ssl.conf` proxying to `127.0.0.1:<port>` (template `:273-332`).
- **Electrum servers use an nginx `stream {}` block** appended to `/etc/nginx/nginx.conf`. It does TLS termination
  `listen <ssl> ssl;` and `proxy_pass` to the plain TCP port (electrs `:415-466`; fulcrum `:514-561`). friglet has no
  native TLS, so do the same. It's fragile: a `truncate -s-2` hack, and fulcrum's `off` leaves the block behind
  (fulcrum `:639-648`). friglet's `off` should remove its own block.

## 8. Tor

- `tor.onion-service.sh <name> <onionPort> <localPort> [<onionPort2> <localPort2>]` writes
  `HiddenServiceDir /mnt/hdd/app-data/tor/<name>` and reloads Tor. `off <name>` removes it
  (`tor.onion-service.sh:1-12,79-142`). The onion hostname is `/mnt/hdd/app-data/tor/<name>/hostname`.
- Guard the call with `runBehindTor=on` (template `:269-272`; electrs `:512-515`).
- electrs maps 50002→50002 (SSL via nginx) and 50001→50001.
- Also add the app to **`tor.network.sh:120-135`**, which re-creates every onion when the user toggles Tor
  (fulcrum is missing there).
- **friglet has no SOCKS5/proxy support** (no `socks`/`proxy` hits in blindbit-rs). On a Tor-by-default node, the
  default oracle `https://oracle.setor.dev` is therefore reached over clearnet. See §12.

## 9. How electrs shows connection info

- SSH:
  - A main-menu entry appears when the flag is on (`00mainMenu.sh:92-97,278-281`). Also `99connectMenu.sh:20-21,58`.
  - `menu` → `CONNECT` prints `localIP:50001:t`, the SSL variant `:50002:s`, the Tor address and Electrum CLI
    examples. It shows the onion as a QR on the LCD via `blitz.display.sh qr` (electrs `:227-254`).
  - Before that, it guards on "service not running", "index not ready" and "nginx broken" (electrs `:168-214`).
- Login status: `00infoBlitz.sh:369-387` shows `infoSync` from `status-sync` until `initial-sync.done` exists.
- WebUI: `Electrs.tsx:60-64,125,142` shows QR codes and copy snippets of `<localIP>:<portSSL>:s` and
  `<TORaddress>:<portSSL>:s`. They come from blitz_api's electrs-only advanced status, which merges `status showAddress`
  and `status-sync` (keys `localIP, publicIP, portTCP, portSSL, TORaddress, initialSynced, blockheight, infoSync`)
  (`blitz_api/app/apps/impl/raspiblitz.py:485-600`).
- friglet: reuse these exact keys and the `host:port:s` format. Electrum wallets (and Sparrow) accept that format.

## 10. WebUI integration (separate PRs in two more repos)

- blitz_api only runs `bonus.<id>.sh` for ids in the `AppId` enum (`blitz_api/app/apps/models.py:18-38`;
  `raspiblitz.py:349-362`). Adding friglet needs an enum entry there.
- The WebUI needs `AppId` + `isAppId` (`raspiblitz-web/src/models/app-status.ts:38-64`), an `availableApps` entry
  (`src/utils/availableApps.ts:5-67`), i18n text (`src/i18n/langs/en.json:37-45`) and a logo in `public/assets/apps/logos`.
  A custom component like `Electrs.tsx` is optional.
- Status keys blitz_api parses: `installed`(=1 means shown as installed), `configured`, `version`, `localIP`,
  `httpPort`, `httpsPort`, `httpsForced`, `httpsSelfsigned`, `toraddress`, `authMethod`, `error`
  (`raspiblitz.py:126-183`). Any stderr output fails the call (`:89-106`). Only electrs gets the 30 s timeout (`:78`),
  so `status` must be fast.
- Install/uninstall from the WebUI runs `bash bonus.<id>.sh on|off` and streams stdout
  (`raspiblitz.py:349-440`). The WebUI has **no input form for app config**, so the scan key / spend pubkey would need
  SSH or a new UI.
- Release deploys use blitz_api branch `blitz-v1.12` and web branch `release/v1.12` (`blitz.web.api.sh:182-201`;
  `blitz.web.ui.sh:56-66`). Fatpack images pre-install "WEBUI-APP"s via `install` (`blitz.fatpack.sh:147-153`).

## 11. Docker on RaspiBlitz

- **Not supported in v1.12.** `blitz.docker.sh` was removed on 2024-08-06 (commit c73c5f0, closing #2074). The
  historical copy is in `raspiblitz-removed-c73c5f0/home.admin/config.scripts/blitz.docker.sh:3-5`. No v1.12 bonus
  script runs a container (grep over `config.scripts`).
- Maintainer stance (#2074, rootzoll):
  - Keep dependencies minimal.
  - Verify PGP signatures directly instead of pulling images.
  - Possibly move to Debian packaging long-term.
  - Contributors are pointed to `bonus.template.sh`.
- The open issues #4283, #4761 and #5198 are about running *RaspiBlitz itself* in a container or as a Proxmox OCI image,
  not about apps.

## 12. friglet-specific gotchas

1. **Scan key secrecy.**
   - Scripts routinely `sudo tee` configs to stdout (electrs `:399`; fulcrum `:501`).
   - `_provision_.sh` appends all bonus-script output to its log (`:426`).
   - `blitz.debug.sh` output is the shared support artifact, and `tor.network.sh:124` publishes debug logs over an onion.
   - So write `scan.key` with `install -m 600` / redirect to `/dev/null`, and never `cat` config into status or debug output.
2. **Non-interactive `on`.** `_provision_.sh` reruns `on` with no TTY. Key, spend pubkey and birthday must already
   be in `app-data/friglet/`. Prompt only from `menu` or an `on`-time dialog guarded by a TTY.
3. **Port clashes**: 8080 (LND REST) and 50001 (electrs). See §6.
4. **Unauthenticated wallet data.**
   - friglet's Electrum server serves *one wallet's* history and its HTTP API has no auth.
   - LAN-exposing it (like electrs on `0.0.0.0`) leaks that wallet to the LAN.
   - Prefer 127.0.0.1 + nginx SSL + onion, and decide explicitly on the LAN default.
5. **Oracle over clearnet.** There is no proxy support, so a Tor-only node leaks its IP to `oracle.setor.dev`.
   - Option A: add SOCKS5 to friglet (a blindbit-rs PR).
   - Option B: ship a `bonus.blindbit-oracle.sh`. That needs Core >= 30 (the default install is 29.2) plus `rest=1`.
6. **P2P throttling.** Add a `download@` whitebind (see §5) and restart bitcoind only when `state == ready`.
7. **glibc**: bookworm is 2.36. Don't reuse trixie-built artifacts.
8. **nginx stream** edits to `nginx.conf` are shared and fragile. Test with `nginx -t` and remove our block in `off`.
9. **Integration checklist** for the upstream PR:
   - `_provision_.sh`
   - `00mainMenu.sh`
   - `00settingsMenuServices.sh` (flag default + option + on/off flow, `:15-16,46-47,209-305`)
   - `99connectMenu.sh`
   - `99updateMenu.sh`
   - `blitz.debug.sh:309-318`
   - `blitz.shutdown.sh:45`
   - `tor.network.sh`
   - `00infoBlitz.sh`
   - optional: `blitz.fatpack.sh`

## 13. Getting it merged upstream

- Fork `raspiblitz/raspiblitz`, branch from **`dev`**, and open the PR against `dev`
  (`CONTRIBUTING.md:55-60`; `pull_request_template.md:1-2`; template `:463-465`).
- Test on a device with the `github` command: switch the repo/branch, then `patch` to sync
  (`raspiblitz-docs/docs/faq/dev.md:197-201`; `CONTRIBUTING.md:43-45`).
- Hotfixes land on the release branch via squash-merge and are cherry-picked to `dev`
  (`raspiblitz-docs/docs/faq/dev.md:159-165`). New apps normally ship in the next release.
- Review culture is Concept/Approach ACK/NACK, with security first (`CONTRIBUTING.md:61-79`).
- WebUI visibility needs extra PRs to `fusion44/blitz_api` (`dev`) and `raspiblitz/raspiblitz-web` (`master`). See §10.

## 14. Recommendation: native binary + systemd, not Docker

**Ship a native `bonus.friglet.sh` that installs a verified prebuilt binary under systemd.**

- Docker isn't available on RaspiBlitz v1.12 and was deliberately removed (§11). A Docker-based script would have to
  install Docker Engine itself, which would almost certainly be NACKed.
- Every integration point assumes a bonus script + systemd unit + `raspiblitz.conf` flag: provisioning, menus, WebUI,
  debug, Tor and fatpack (§1, §9-§10).
- The required networking is all host-local: P2P whitebind on 127.0.0.1, onion → 127.0.0.1, nginx stream → 127.0.0.1.
  A container would need host networking anyway.
- Pi resource budget: no extra daemon and no image layers on SD/NVMe.
- The fulcrum pattern (signed SHA256SUMS + per-arch tarball) matches maintainer expectations, and it avoids the
  electrs-style ~40-min Rust build on a Pi.
  - **Prerequisite:** blindbit-rs publishes signed `aarch64`/`x86_64` Linux release artifacts compatible with glibc 2.36 (or musl).
  - **Interim:** a source build pinned to a signed tag, verified via `blitz.git-verify.sh` (electrs pattern), adding `protobuf-compiler`.
- Keep the Docker image for Umbrel/StartOS-style platforms. Don't use it on RaspiBlitz.

## 15. NixBlitz (if it ever matters)

- A plugin is `plugin.json` (manifest, config_schema, actions, permissions, dashboard) plus a two-stage `plugin.nix`
  NixOS module (`nixblitz/docs/plugin-authoring.md:21-48,101-150`). Example:
  `nixblitz_official_plugins/electrs/plugin.json`, `plugin.nix`.
- Secrets in plugin config end up in the Nix store (`plugin-authoring.md:853-858`; also `:144-145,231`). That conflicts with the scan-key
  rule, so the key would need out-of-band staging.
- Official-plugin bar: broad use, stable across one TUI release, and an active maintainer (`plugin-authoring.md:890-907`).
