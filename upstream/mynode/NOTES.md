# friglet on myNode: distilled requirements

As of 2026-10-01, against mynodebtc/mynode `351432c` (see `SOURCES.md`).
`M/` means `upstream/mynode/mynodebtc-mynode/`. `file:L12` gives a line in that verbatim copy.
Items marked **(proposal)** are recommendations only. Nothing below is decided.

## 0. Summary

- myNode has one app model for third-party apps: **"dynamic apps"** (`sdk_version` 2). Each app
  is a folder under `rootfs/standard/usr/share/mynode_apps/<short_name>/` **in the myNode repo itself**
  (`M/doc/applications.md:3`).
- **The app must be merged upstream.** No catalogue, no third-party repo, and since v0.3.48-2
  (2026-09-18) no sideloading through the UI (`M/CHANGELOG:18`, commit `58ee8241`).
- Docker apps pull a **registry image pinned by its multi-arch index digest**. No on-device build is
  needed (`M/doc/applications.md:23-28`).
- friglet needs a Bitcoin **P2P** peer and a BlindBit **oracle**, not bitcoind RPC. A *local* oracle
  is blocked on stock myNode: it ships Core 29.3 with no REST (see section 5).
- The project is active (last commit 2026-09-28) but has a single maintainer. Worth targeting as a
  second-tier platform **(proposal)** (see section 10).

## 1. App definition (`<short_name>.json`)

The schema is the defaults function `initialize_application_defaults()` (`M/rootfs/standard/var/pynode/application_info.py:196-284`)
plus the settings table (`M/doc/applications.md:102-163`). `app_sdk/create.py check_doc` keeps the two in step (`M/app_sdk/create.py:165-194`).
There is no JSON Schema file. Built-in apps such as electrs are defined in `M/rootfs/standard/usr/share/mynode/application_info.json`, which third parties do not edit.

Proposed friglet values **(proposal)**, each with its source:

| Field | Value | Why / source |
|---|---|---|
| `short_name` | `friglet` | Must equal the folder name (`M/doc/applications.md:111`; checked at `M/app_sdk/create.py:131-132`) |
| `category` | `bitcoin_app` | Allowed values: `M/doc/applications.md:114` |
| `linux_user` | `bitcoin` | Canary and jam use it (`M/.../canary/canary.service:17`). A dedicated user is added to the `bitcoin` and `docker` groups (`M/.../application_info.py:728-739`) |
| `latest_version` | friglet tag | Exported as `$VERSION` to install and uninstall (`M/.../application_info.py:1141`) |
| `download_skip` | `true` | Docker apps skip the tarball (`M/doc/applications.md:122`; `M/.../canary/canary.json:21`) |
| `requires_docker_image_installation` | `true` | Adds the docker group and the "installing images" status (`M/.../application_info.py:737-739, 508`) |
| `requires_bitcoin` | `true` | Status display only. The real gate is `wait_on_bitcoin.sh` in the unit (`M/.../application_info.py:516, 578`; `M/rootfs/standard/usr/bin/wait_on_bitcoin.sh`) |
| `requires_electrs` / `requires_lightning` | `false` / `false` | **Set explicitly**: the doc's sample column says `true` for lightning (`M/doc/applications.md:137`) |
| `supported_archs` | `["aarch64","x86_64"]` | Format from `M/doc/applications.md:120`. **Not enforced** (see section 8) |
| `http_port` / `https_port` | only if we ship our own web UI | Opened in ufw and given a Tor port of 80 or 443 (`M/.../application_info.py:778-785, 940-945`) |
| `extra_ports` | `[<electrum port>]`, **not 50001** | ufw allow plus a HiddenServicePort with the same number (`M/.../application_info.py:786-788, 946-949`) |
| `supports_testnet` | `false` (initially) | The app is disabled in testnet mode (`M/.../application_info.py:506`) |
| `show_on_status_page`, `show_on_homepage`, `can_uninstall` | `true` | Code defaults are `false` (`M/.../application_info.py:233, 239, 242`) |
| `journalctl_log_name` | `friglet` | Shows logs when there is no log file (`M/doc/applications.md:144`) |
| `app_tile_button_href` | `/app/friglet/info` | Gives the tile an Info page (`M/.../application_info.py:265-266`) |
| `data_manageable` | `false` **(proposal)** | "Reset Data" deletes `/mnt/hdd/mynode/friglet`, **including the scan key** (`M/doc/applications.md:157`) |
| `homepage_order` | ≥ 91 | `M/doc/applications.md:161`. The code default is 9999 (`M/.../application_info.py:250`) |
| `sdk_version` | `2` | `M/doc/applications.md:163` (Canary still says 1: `M/.../canary/canary.json:39`) |

## 2. Folder layout and lifecycle scripts

- Required: `<app>.png` and `scripts/{install,uninstall,pre,post}_<app>.sh` (`M/app_sdk/create.py:134-139`).
  Optional: `<app>.service`, `nginx/https_<app>.conf`, `www/python/<app>.py`, `www/templates/*.html`,
  `screenshots/*.png`, `app_data/` (`M/doc/applications.md:65-100`).
- `mynode-manage-apps init` copies the files into place: the unit goes to `/etc/systemd/system`, the scripts to
  `/usr/bin/service_scripts/`, nginx to `/etc/nginx/sites-enabled/`, and Flask/Jinja into the web UI
  (`M/.../application_info.py:983-1053`). It runs at every boot (`M/rootfs/standard/usr/bin/mynode_startup.sh:488`).
- **install**: runs as `linux_user` (`sudo -u`) with cwd `/opt/mynode/<app>` and env `VERSION`,
  `INSTALL_FOLDER`, `STORAGE_FOLDER` plus `install_env_vars` (`M/.../application_info.py:1136-1148`).
  The install folder is **wiped on every upgrade** (`M/.../application_info.py:741-755`). Persistent state belongs in
  `/mnt/hdd/mynode/<app>` (`M/.../application_info.py:215-216`).
- **uninstall**: runs as `linux_user` before `/opt/mynode/<app>` is deleted. The platform does **not**
  delete the storage folder (`M/.../application_info.py:1183-1215`). Canary removes it itself
  (`M/.../canary/scripts/uninstall_canary.sh`). Whether friglet keeps the scan key is a decision for us.
- **pre / post**: these are `ExecStartPre` / `ExecStartPost` in the unit (`M/app_sdk/sampleapp/sampleapp.service`).
  The `ExecStartPre=+…` prefix runs pre as **root** (`M/.../canary/canary.service:12`). Canary uses it to
  generate secrets with umask 077 and to refuse symlinks (`M/.../canary/scripts/pre_canary.sh`).
- `app_data/` is copied to `/opt/mynode/<app>/app_data` on init and install (`M/.../application_info.py:874-878, 1043-1048`).
- Test loop: scp the folder, then run `sudo mynode-manage-apps init && sudo mynode-manage-apps install <app>` (`M/doc/applications.md:30-46`).

## 3. Docker images: pulled from a registry, pinned by digest

- In the install script: `pull_app_docker_image <image> "$VERSION" <app> "$IMAGE_DIGEST"`, where
  `IMAGE_DIGEST="<tag> sha256:…"` (`M/app_sdk/sampleapp/scripts/install_sampleapp.sh`; helper at
  `M/rootfs/standard/usr/share/mynode/mynode_app_versions.sh:49-71`).
  It pulls `image@digest` and tags it `image:tag`. Canary example: `M/.../canary/scripts/install_canary.sh:62-68`.
- The pin is the **image index digest (all platforms)** from `docker buildx imagetools inspect`
  (`M/scripts/print_app_download_hashes.sh:60-63`). friglet therefore needs a **multi-arch manifest list
  (amd64 + arm64) on Docker Hub or GHCR**. Add a line to `MARKETPLACE_DOCKER_IMAGES` (`M/scripts/print_app_download_hashes.sh:34-40`).
- Users who choose a custom version get a pull **by tag, with no pin** (`M/.../mynode_app_versions.sh:62-66`).
- On-device builds still exist upstream (nostrrsrelay runs `docker build` in its install script), but the documented path is a
  pinned pull (`M/doc/applications.md:81-82`). A Rust build on a Pi would take a long time.
- Run the container with plain `docker run` (albyhub/jam style, as in the sampleapp unit `ExecStart`) or with Compose (Canary).
  `/usr/local/bin/docker-compose` became Compose v2 in the commit we pinned (`351432c`), so **(proposal)** use `docker run`.
- The unit should include `ExecStartPre=/usr/bin/wait_on_docker_image_install.sh` (`M/.../canary/canary.service:11`).

## 4. Running friglet's container on myNode

- **UID mismatch**: the friglet image runs as uid 1000 (`/work/blindbit/blindbit-rs/Dockerfile:22`, `611a877`).
  The host storage folder belongs to `linux_user`. Pass `--user $(id -u bitcoin):$(id -g bitcoin)` the way Canary does
  (`M/.../canary/app_data/docker-compose.yml:6`; `M/.../canary/scripts/install_canary.sh` `write_compose_identity`).
  `BITCOIN_UID` and `BITCOIN_GID` are also in `.btcrpc_environment` (`M/rootfs/standard/usr/bin/mynode_startup.sh:535-539`).
- Networking: Canary uses `network_mode: host` and reaches services on `127.0.0.1` (`M/.../canary/app_data/docker-compose.yml:7,16`).
  Bridge mode works too: bitcoind whitelists and allows RPC from `172.16.0.0/12` (`M/rootfs/standard/usr/share/mynode/bitcoin.conf:26,42`).
- Bind addresses: the image defaults to `0.0.0.0:8080` / `0.0.0.0:50001` (`/work/blindbit/blindbit-rs/Dockerfile:34-35`).
  Both must be remapped (see section 6).

## 5. Bitcoin backend and credentials

- friglet's inputs are `p2p_node_addr` and `oracle_url` (default `https://oracle.setor.dev`), plus the scan key file
  and `spend_pubkey` (`/work/blindbit/blindbit-rs/friglet/src/config.rs:37,47,54-57,87`). **It needs no bitcoind RPC.**
  On myNode the P2P peer is `127.0.0.1:8333` (allowed by `whitelist=127.0.0.1`, `M/.../bitcoin.conf:41`).
- **A local oracle is blocked by default.** BlindBit oracle v2 needs Core **v30+** with REST
  (`/rest/spenttxouts`, PR #32540) and an unpruned node (`/work/blindbit/blindbit-oracle/README.md:20-27,137`).
  myNode pins Core **29.3** (`M/.../mynode_app_versions.sh:73`) and does not set `rest=1` (`M/.../bitcoin.conf`).
- Users could raise the Core version (custom app versions) and add `rest=1` through
  `/mnt/hdd/mynode/settings/bitcoin_post_config.conf` (`M/rootfs/standard/usr/bin/mynode_gen_bitcoin_config.sh:163-178`).
  An app should not do this for them.
- If RPC is ever needed, for example for an oracle app:
  - user `mynode`; password in `/mnt/hdd/mynode/settings/.btcrpcpw` (0600, owned by bitcoin)
    (`M/.../mynode_startup.sh:313-319`; `M/.../mynode_gen_bitcoin_config.sh:181-183`).
  - `/mnt/hdd/mynode/settings/.btcrpc_environment` holds `BTC_RPC_PASSWORD`, `BITCOIN_UID` and `BITCOIN_GID` (`M/.../mynode_startup.sh:522-539`).
    Load it with systemd `EnvironmentFile=` (jam does, upstream `mynode_apps/jam/jam.service:12`, not pulled).
  - `rpcbind=0.0.0.0`, `rpcallowip` covers localhost, the LAN and docker (`M/.../bitcoin.conf:18-27`).
    ZMQ listens on 28332-28334 (`M/.../bitcoin.conf:53-55`).

## 6. Ports, firewall, nginx/HTTPS, Tor

- **Port 50001 is taken by electrs** (`M/rootfs/standard/usr/share/mynode/electrs.toml`, `electrum_rpc_addr = "0.0.0.0:50001"`).
  nginx's TLS stream uses **50002** (`M/rootfs/standard/etc/nginx/modules-enabled/https_electrs.conf`), and both are open in ufw
  (`M/rootfs/standard/usr/bin/mynode_firewall.sh:74-75`). friglet needs another port, e.g. 50011, which appears nowhere in the repo **(proposal)**.
- **8080 is risky**: JoinMarket on the host uses `onion_serving_port = 8080` on 127.0.0.1 (upstream
  `rootfs/standard/usr/share/mynode/joinmarket.cfg:66-67`, not pulled). Remap the HTTP API, or bind it to loopback only.
- Firewall: ports from `http_port`, `https_port` and `extra_ports` get `ufw allow <port>` **from any source**
  (`M/.../application_info.py:933-955`), applied at boot (`M/.../mynode_startup.sh:489`; `M/.../mynode_firewall.sh:106`).
  Don't list ports that should stay on loopback.
- HTTPS: an app's `nginx/https_<app>.conf` is copied into `sites-enabled`, which sits inside `http {}` (`M/.../application_info.py:1013-1014`;
  `M/rootfs/standard/usr/share/mynode/nginx.conf:65`). It works for a web UI (`M/app_sdk/sampleapp/nginx/https_sampleapp.conf`).
- **An app cannot add a TLS `stream {}` proxy** for Electrum-over-SSL. electrs does it from `modules-enabled`, at main context
  (`M/.../nginx.conf:7`). friglet would have to serve TLS itself or offer plain TCP only.
- Tor: `/etc/torrc.d/<app>` gets `HiddenServiceDir /var/lib/tor/mynode_<app>/` plus ports 80→http, 443→https and
  each extra port to itself (`M/.../application_info.py:774-797`). The electrs equivalent is `M/rootfs/standard/etc/torrc.d/mynode_electrs`.
- **Onion hostnames are premium-only.** `get_onion_url_for_service()` returns `not_available` on Community Edition
  or when Tor remote access is off (`M/rootfs/standard/var/pynode/device_info.py:807-808, 1637-1645`;
  `M/rootfs/standard/var/www/mynode/templates/electrum_server.html:78-82`).

## 7. Showing connection info in the web UI

- Option A, the generic page `M/rootfs/standard/var/www/mynode/templates/app/generic_app.html`, filled from
  `app_page_content`. It replaces only `{VERSION} {SHORT_NAME} {HTTP_PORT} {HTTPS_PORT} {APP_TOR_ADDRESS} {LOCAL_IP_ADDRESS}`
  (`M/.../application_info.py:168-178`; these variables are **not documented** in `doc/applications.md`). There is no
  `{EXTRA_PORT}`, so the Electrum port would be hard-coded in the text.
- Option B **(proposal)**, a custom blueprint and template like Canary's (`M/.../canary/www/python/canary.py`,
  `M/.../canary/www/templates/canary.html`). It would mirror electrs's info table: IP, TCP/SSL port, onion, CLI line,
  and Premium/Tor-off branches (`M/rootfs/standard/var/www/mynode/electrum_server.py`;
  `M/.../templates/electrum_server.html:58-95`). Helpers: `get_local_ip()` and `get_onion_url_for_service()` (`M/.../device_info.py:419, 1637`).
- The blueprint is imported into myNode's Flask process (`M/.../application_info.py:969-981`), which runs as **root**
  (upstream `etc/systemd/system/www.service:19`, not pulled). Keep it small and read files defensively, as Canary does
  with `O_NOFOLLOW` (`M/.../canary/www/python/canary.py:18`).
- On myNode, option B can replace our planned separate frontend container **(proposal)**. Only set `http_port`
  (and get the "Open" button, `generic_app.html:76-77`) if we still ship it.

## 8. arm64 + amd64

- Current rootfs targets are `raspi4`, `raspi5`, `debian` and `rockpro64` (`M/.github/copilot-instructions.md.upstream:5`). Arch comes from `uname -m`:
  `aarch64`, `x86_64` or `armv7l` (`M/rootfs/standard/usr/share/mynode/mynode_device_info.sh:17`). `IS_32_BIT=1` still exists for older Pis (`:34,38`).
- **`supported_archs` is never enforced**: the only code reference is the default (`M/.../application_info.py:208`).
  Its spelling also varies (`["amd64","arm64"]` in `M/.../canary/canary.json:20` vs `["aarch64","x86_64"]` in `M/doc/applications.md:120`).
  The install script should **exit early with a clear message** when `IS_32_BIT=1`.
- `download_binary_url` keys must match `uname -m` (`M/.../application_info.py:851-855`). This only matters if we ship binaries.

## 9. How to submit

- Open a PR to `mynodebtc/mynode` containing the app folder **and** its line in `scripts/print_app_download_hashes.sh`
  (`M/doc/applications.md:54-63`). Rules: no altcoins, a fitting category, pinned downloads and images, quality. Upstream may reject
  "for any reason". The PR template asks for the devices tested (`M/.github/pull_request_template.md`).
- Run `app_sdk/create.py check friglet` first. On 2026-10-01 upstream's own `canary` **fails** it (missing `post_canary.sh`).
- Precedent: Canary (outside author) went in as PR #984. Opened 2026-01-10, merged 2026-04-29 (about 3.5 months, 12 reviews, 11 comments).
  The follow-up #1020 merged within a day. The app folder will be **LGPL-3.0** (`M/rootfs/standard/usr/share/mynode_apps/LICENSE.md`; `M/README.md:175`).
- Out-of-date docs to ignore: `mynodebtc/mynode_docs` `docs/developer/add-app.md` (built-in app process) and the archived
  `mynode_sdk` / `sdk_app_template` repos, which moved into `app_sdk/` in commit `d2d1eab4` (`M/CHANGELOG:11`).

## 10. Maintenance status, and is myNode worth it?

- Active: last commit 2026-09-28. v0.3.49 shipped 2026-09-25 and there have been 12 releases since 2025-10-15 (`M/CHANGELOG`).
  The app SDK was reworked in September 2026: pinning, per-app credentials, SDK moved in-repo (`M/CHANGELOG:1-30`).
- Bus factor 1: 171 of 186 commits in the last 12 months are by `mynode-dev`. The repo has 739 stars and 138 open issues.
  No issue mentions silent payments or BlindBit.
- Verdict **(proposal)**: worth a low-cost, second-tier submission. It is Docker-friendly, outside apps get merged, and friglet fits as one container.
  Against it: one gatekeeper with slow review, Tor connection info locked behind Premium, no local oracle on stock Core 29.3, and an unknown user base.

## 11. Gotchas (friglet-specific)

1. Port clashes: 50001/50002 (electrs) and 8080 (JoinMarket on host). See section 6.
2. **The Electrum protocol has no auth, and friglet serves one private wallet.** Any LAN host, or anyone with the onion, can read
   its history. Electrs only serves public chain data. ufw opens `extra_ports` to all sources (`M/.../application_info.py:946-949`).
   A grep found no auth in `/work/blindbit/blindbit-rs/friglet/src/server/mod.rs`.
3. **Installing from the UI reboots the node.** The Marketplace runs `reinstall_app()`, which stops core services, runs
   `mynode_post_upgrade.sh`, then `reboot_device()` (`M/.../application_info.py:26-47`; `M/rootfs/standard/usr/bin/mynode_reinstall_app.sh`).
4. myNode has no generic per-app settings UI. The scan key and `spend_pubkey` must be entered through our own blueprint (a root
   Flask POST) or over SSH.
5. The "Type / Default" column in `doc/applications.md` holds sample values, not code defaults. Set every field explicitly (`M/.../application_info.py:232-250`).
6. Installs wipe `/opt/mynode/friglet`. Keep `config.toml`, `scan.key` and the scanner state in `/mnt/hdd/mynode/friglet`.
7. The image runs as uid 1000. Pass `--user` or the volume ends up owned by the wrong host user (section 4).
8. No Electrum-over-TLS through nginx for dynamic apps (section 6).
9. The onion address is shown only on Premium devices with Tor remote access on (section 6).
10. `supported_archs` is not enforced. Guard 32-bit in the install script (section 8).
11. The default `oracle_url` is a remote BlindBit-run service. Disclose it on the app page. A local oracle needs Core ≥ 30 with `rest=1` (section 5).
12. Compose moved v1→v2 in `351432c`. Prefer `docker run`.
