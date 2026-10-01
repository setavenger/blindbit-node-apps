# Packaging friglet for Umbrel: requirements digest

Written 2026-10-01. The baseline is umbrelOS **2.0.0** (released 2026-09-22) and
`getumbrel/umbrel-apps@01be6c3`. Commits and licenses for each source are in `SOURCES.md`.

Every rule below cites a pulled file as `path:line`, with paths relative to this folder.
Statements marked **PROPOSAL** are suggestions and have not been decided.
Statements marked **UNVERIFIED** are inferences that have not been tested.

Short aliases used in citations:

| Alias | File |
|---|---|
| PKG | `umbrel-apps/.claude.upstream/skills/umbrel-package-app/SKILL.md` |
| DEV | `umbrel-apps/.claude.upstream/skills/umbrel-develop-app/SKILL.md` |
| TEST | `umbrel-apps/.claude.upstream/skills/umbrel-test-app/SKILL.md` |
| UPD | `umbrel-apps/.claude.upstream/skills/umbrel-update-app/SKILL.md` |
| LINT | `umbrel-apps/.tools/lint-apps.mjs` |
| SCRIPT | `umbrel/packages/umbreld/source/modules/apps/legacy-compat/app-script` |
| SCHEMA | `umbrel/packages/umbreld/source/modules/apps/schema.ts` |
| GW | `umbrel/packages/umbreld/source/modules/app-gateway/app-gateway.ts` |
| REPO | `umbrel/packages/umbreld/source/modules/apps/app-repository.ts` |

---

## 0. What friglet needs, mapped to Umbrel

Facts about friglet itself come from the local `blindbit-rs` checkout (branch
`fix/electrum-headers-subscribe` @ `611a877`). Those are not upstream Umbrel files, so they are cited
by their repo path.

| friglet fact | Umbrel consequence |
|---|---|
| Needs a Bitcoin **P2P** peer as a literal `IP:port`. A hostname is rejected (`blindbit-rs/friglet/src/config.rs:172`, uses `SocketAddr::from_str`). It needs **no** bitcoind RPC. | Add `dependencies: [bitcoin]`. Set `FRIGLET_P2P_NODE_ADDR="${APP_BITCOIN_NODE_IP}:${APP_BITCOIN_P2P_WHITEBIND_PORT-${APP_BITCOIN_P2P_PORT}}"`. This is the same expression electrs uses (`umbrel-apps/electrs/docker-compose.yml:33`). `APP_BITCOIN_NODE_IP` is a static IP (`umbrel-apps/bitcoin/exports.sh:2`), so a literal IP is available. |
| `network` accepts `bitcoin\|testnet\|testnet4\|signet\|regtest` | Use `${APP_BITCOIN_NETWORK_ELECTRS}`. It is already `bitcoin` on mainnet and passes the other networks through unchanged (`umbrel-apps/bitcoin/exports.sh:104-111`). |
| Needs a BlindBit **oracle** over gRPC. Default is `https://oracle.setor.dev`. | Either rely on the hosted oracle or ship an oracle sidecar (see §4.3). |
| Required first-run inputs: `spend_pubkey`, `start_height`, scan secret | Must be enterable **in the browser**, not through env/SSH (DEV:21, DEV:96-100, PKG:14). Needs a setup UI (§9). |
| HTTP API on `:8080`, Electrum TCP on `:50001`. The image presets both to bind `0.0.0.0` (`blindbit-rs/Dockerfile:33-35`). | Web UI goes through `app_proxy` (§5). Electrum is a raw published port (§6). |
| Runs as UID 1000 with state in `/data` (`blindbit-rs/Dockerfile:22,27-32`) | Matches Umbrel's 1000:1000 ownership of `${APP_DATA_DIR}` (PKG:195). |
| No published image, no multi-arch build, no release tags | Blocks submission (§8). |

---

## 1. Required file layout

The package is one top-level directory named exactly after the app ID (PKG:33-42, PKG:80):

```
<app-id>/
  umbrel-app.yml          # manifest (required)
  docker-compose.yml      # services (required)
  exports.sh              # only if needed: computed/shared env, Tor hostname, port constants
  torrc.template          # only if a raw-port hidden service is wanted (electrs pattern)
  hooks/pre-start         # only if needed; must be executable
  data/<dir>/.gitkeep     # every bind-mount source dir that starts empty
```

Rules:
- Top-level `*.template` files are rendered with `envsubst` next to the template on install, start and
  update (PKG:313-317; SCRIPT:527). Templates under `data/` are **not** rendered.
- Updates copy only `docker-compose.yml *.template exports.sh torrc hooks` into installed app data
  (SCRIPT:831, SCRIPT:880; PKG:327). Anything under `data/` is laid down on **fresh install only**.
  - mosquitto works around this by having `hooks/pre-start` rewrite its nginx config
    (`umbrel-apps/mosquitto/hooks/pre-start`).
- `.gitkeep` files are stripped before runtime (PKG:211; TEST:112).
- Lint the package with the linter that ships with the repo (§11).

---

## 2. `umbrel-app.yml`: every field

Required by the official-store linter: `manifestVersion id category name version tagline description
developer website dependencies repo support port gallery path submitter submission`
(LINT:26-44 `REQUIRED_MANIFEST_FIELDS`). Recommended field order: PKG:52-77.
The full runtime schema is SCHEMA:89-137. Note that umbreld does **not** schema-validate at runtime
(SCHEMA:161-182, TODO at 177-180), so the linter is the real gate.

| Field | Meaning and rule | friglet PROPOSAL |
|---|---|---|
| `manifestVersion` | The umbrelOS feature level, not the app version. Default `1`. Installs are refused on older OSes (PKG:79; `umbrel/.../manifest-compatibility.ts:11-18`). In the store at this commit: `1` = 201 apps, `1.1` = 18 (electrs, fulcrum, bitcoin), `2.0` = 1 (vaultwarden). | `1.1`, like electrs and fulcrum. Use `2.0` only if 2.0-only features are needed (`environment:`). |
| `id` | Lowercase kebab-case. Must equal the directory name. Becomes the data path and the container-name prefix. **Never change it after release** (PKG:80; UPD:36). Community stores require the store-ID prefix (§10). | `friglet` (official) or `<storeid>-friglet` (community). Decide before the first release. |
| `implements` | Only for a real drop-in provider of another app's export contract (PKG:81, PKG:123-127). | **Omit**, unless friglet serves the full Electrum protocol for ordinary wallets (open question). |
| `category` | One of `ai automation bitcoin crypto developer files finance media networking social` (LINT:24) | `bitcoin` |
| `name`, `tagline` | Store copy (PKG:82-83) | `Friglet` / "Silent Payments Electrum server" |
| `version` | Upstream release string. Never `latest` or a digest. Short SHA if there is no release (PKG:99) | Needs a friglet release tag |
| `description`, `releaseNotes` | Folded `>-` YAML. Paragraphs are separated by **two** blank lines. Put setup and security notes first (PKG:84-98). `releaseNotes: ""` for a new app (PKG:103). | Say that friglet needs the user's SP scan key and spend pubkey. |
| `developer`, `website`, `repo`, `support` | Accurate links. `developer` links to `website` (PKG:104). `website` must be a URL (SCHEMA:101). | BlindBit / blindbit-rs links |
| `dependencies` | Umbrel app IDs needed at runtime. Never list same-compose services (PKG:105, PKG:119-130). | `[bitcoin]` |
| `permissions` | Only `GPU` and `STORAGE_DOWNLOADS` are recognised (PKG:134-138; LINT:25) | omit |
| `port` | **Host** port of the app_proxy/gateway (the browser URL). Must be unique across the store and must avoid 80, 443 and 2000 (PKG:100, PKG:222; LINT:56-60). | **2104**. It lint-checked clean against the store at this commit; 2110 is taken by core-lightning. |
| `gallery` | `[]` for official submissions; Umbrel makes the images (PKG:107; LINT:346). Community stores use full URLs (REPO:189-192). | `[]` |
| `icon` | Omit for the official store. Allowed (a URL) in community stores (PKG:108; LINT:350; REPO:194). | per store |
| `path` | `""`, or a `/subpath` appended to the URL (PKG:102) | `""` |
| `defaultUsername`, `defaultPassword` | Display-only, so must be real or `""` (PKG:109-110) | `""` |
| `deterministicPassword` | Shows `APP_PASSWORD`. Only if the app login really uses it (PKG:111) | omit |
| `widgets` | Home-screen cards backed by a real JSON endpoint `service:port/path`, fetched server-side **without auth** (PKG:153-162). Example: `umbrel-apps/bitcoin/umbrel-app.yml` `widgets:` | Later: a scan-progress `text-with-progress` widget |
| `backupIgnore` | Paths relative to `${APP_DATA_DIR}` excluded from Umbrel backups. Never exclude keys or seeds (PKG:142-149). electrs excludes its index (`umbrel-apps/electrs/umbrel-app.yml`, last lines). | `data/friglet/scanner_state.headers.json` (rebuildable) |
| `submitter`, `submission` | Contributor name; PR URL (PKG:114-115) | |
| `storage: {dataRoot: data}` | umbrelOS 2.0 relocatable data root. Mounts stay written as `${APP_DATA_DIR}/data/...` and get rewritten to `${APP_DATA_ROOT}`. Mounts may not escape `data/` (SCHEMA:128-134; the rewrite lives in `umbrel/.../apps/app.ts`, which was not pulled). Used by bitcoin (`umbrel-apps/bitcoin/umbrel-app.yml:3-4`) and 221 other apps. Not described in PKG. | Add it. Keep every mount under `${APP_DATA_DIR}/data/`. |
| Other schema-only fields | `disabled`, `optimizedForUmbrelHome`, `torOnly`, `requiresHttps`, `nativeTlsHostnameSuffixes`, `installSize`, `defaultShell`, `folderAccess`, `environment` (SCHEMA:92, 117-136). `environment:` lets users override env vars in the UI (SCHEMA:62-85); no store app uses it yet. | Not needed |

---

## 3. Dependencies and the env contract

### 3.1 How dependencies work
- Declare the dependency with `dependencies: [bitcoin]`. The dependency (or an app that `implements` it,
  e.g. Bitcoin Knots) must already be installed. It can't be uninstalled while friglet depends on it
  (PKG:121-129).
- On every lifecycle command, umbrelOS sources `exports.sh` of all **transitive** dependencies first,
  then the app's own (SCRIPT:235-279; PKG:128, PKG:275).
- Consume only the exact `APP_<DEP>_*` names the dependency exports (PKG:275).

### 3.2 `bitcoin` exports (`umbrel-apps/bitcoin/exports.sh`)

| Variable | Value / meaning | Line |
|---|---|---|
| `APP_BITCOIN_NODE_IP` | `10.21.21.8` (static) | 2 |
| `APP_BITCOIN_DATA_DIR` | `<bitcoin data root>/bitcoin` (electrs mounts it `:ro`) | 6-8 |
| `APP_BITCOIN_RPC_PORT` / `APP_BITCOIN_P2P_PORT` | `8332` / `8333` | 11-12 |
| `APP_BITCOIN_P2P_WHITEBIND_PORT` | `8335`, the whitelisted P2P listener for trusted internal apps. Not published. Generated as `whitebind=0.0.0.0:8335` (`umbrel-bitcoin/apps/backend/src/modules/config/config.ts:290-296`). | 13-14 |
| `APP_BITCOIN_ZMQ_*_PORT` | 28332-28336 | 18-22 |
| `APP_BITCOIN_NETWORK` | `mainnet\|testnet\|testnet4\|signet\|regtest`, taken from the user's settings | 25, 37-65 |
| `APP_BITCOIN_NETWORK_ELECTRS` | Same, except `bitcoin` for mainnet. This matches friglet's naming. | 104-111 |
| `APP_BITCOIN_RPC_USER` / `APP_BITCOIN_RPC_PASS` | Persisted in the bitcoin app's `.env`. The user is `umbrel`. | 81-97 |
| `APP_BITCOIN_RPC_HIDDEN_SERVICE` / `APP_BITCOIN_P2P_HIDDEN_SERVICE` | Onion hostnames | 99-103 |
| `APP_BITCOIN_IPC_*` | IPC socket path (off by default) | 27-79 |

- RPC is allowed from the whole app subnet: `rpcallowip=${NETWORK_IP}/16`
  (`umbrel-apps/bitcoin/docker-compose.yml` `APPS_SUBNET`; `umbrel-bitcoin/.../config.ts:264-269`).
- **REST is OFF by default.** It is a user toggle, "Public REST API" (`umbrel-bitcoin/libs/settings/settings.meta.ts:542-550`).
  This matters for an oracle sidecar (§4.3).
- Pruning is a user option, off by default (`settings.meta.ts:349-363`). An oracle needs an unpruned node.
- Bitcoin alternatives that `implement` bitcoin may not export `APP_BITCOIN_P2P_WHITEBIND_PORT`. That's why
  electrs falls back with `${APP_BITCOIN_P2P_WHITEBIND_PORT-${APP_BITCOIN_P2P_PORT}}` (`umbrel-apps/electrs/docker-compose.yml:33`).

### 3.3 `electrs` contract (only relevant if friglet ever `implements: electrs`)
- `APP_ELECTRS_IP`, `APP_ELECTRS_NODE_IP`, `APP_ELECTRS_NODE_PORT=50001`, `APP_ELECTRS_RPC_HIDDEN_SERVICE`
  (`umbrel-apps/electrs/exports.sh:1-7`).
- Implementers re-export their own values under the `APP_ELECTRS_*` names
  (`umbrel-apps/fulcrum/exports.sh:7-19`; `umbrel-apps/electrumx/exports.sh:15-27`; PKG:126).

### 3.4 Platform env available to compose, templates and hooks
(PKG:243-262; SCRIPT:200-312)

- **Paths:** `APP_ID`, `APP_VERSION`, `APP_DATA_DIR` (`${UMBREL_ROOT}/app-data/<id>`),
  `APP_DATA_ROOT` (2.0), `UMBREL_ROOT`.
- **Hostnames and the app URL:**
  - `APP_DOMAIN` and `DEVICE_DOMAIN_NAME`, both `<hostname>.local`; also `DEVICE_HOSTNAME`.
  - `APP_PROXY_PORT`, which equals the manifest `port`.
  - `APP_HIDDEN_SERVICE`, the web-UI onion. It is `not-enabled.onion` unless remote Tor access is on (SCRIPT:292-295).
- **Network and Tor:** `NETWORK_IP` (`10.21.0.0`), `TOR_PROXY_IP` / `TOR_PROXY_PORT`, `TOR_DATA_DIR`.
- **Secrets:** `APP_SEED` and `APP_PASSWORD`, see §7.
- **Only inside `exports.sh`:** `EXPORTS_APP_ID`, `EXPORTS_APP_DIR`, `EXPORTS_APP_DATA_DIR`,
  `EXPORTS_TOR_DATA_DIR`, `app_entropy_identifier`.
  - Do not use `APP_ID` or `APP_DATA_DIR` there (PKG:277-286).

---

## 4. Proposed service topology for friglet (PROPOSAL)

### 4.1 Services
| Service | Purpose | Exposure |
|---|---|---|
| `friglet` | Daemon. HTTP on 8080 (internal), Electrum on 50001 | Raw host port `${APP_FRIGLET_ELECTRUM_PORT}:50001` (§6) |
| `web` | Connection-info and setup UI, plus a small backend. Talks to `friglet:8080` and to the control socket on the shared `/data` volume. | Through `app_proxy` only (§5) |
| `tor` | Hidden service for the Electrum port (electrs pattern) | none |
| `oracle` (optional) | Self-hosted BlindBit oracle | internal only |

### 4.2 Wiring friglet
- `FRIGLET_P2P_NODE_ADDR="${APP_BITCOIN_NODE_IP}:${APP_BITCOIN_P2P_WHITEBIND_PORT-${APP_BITCOIN_P2P_PORT}}"`
- `FRIGLET_NETWORK="${APP_BITCOIN_NETWORK_ELECTRS}"`
- `FRIGLET_STATE_FILE=/data/scanner_state.json`. Without this, the default lands outside `/data`; see the
  bug note in §12.
- Volume `${APP_DATA_DIR}/data/friglet:/data`. Commit `data/friglet/.gitkeep`.

### 4.3 The oracle
- **Option A:** default to the hosted `https://oracle.setor.dev`. This is simple, but the app then depends
  on a third-party service, which runs against the Umbrel "use your own node" ethos.
- **Option B:** ship an oracle sidecar.
  - It needs bitcoind data. REST is off by default on Umbrel (§3.2), so use RPC with
    `${APP_BITCOIN_NODE_IP}:${APP_BITCOIN_RPC_PORT}` plus `APP_BITCOIN_RPC_USER` / `APP_BITCOIN_RPC_PASS`,
    as fulcrum does (`umbrel-apps/fulcrum/docker-compose.yml:40-42`).
  - It needs an unpruned node.
  - It must have gRPC enabled (`grpc_host`).
  - Its network names differ from friglet's (`main|signet|regtest|testnet`), so map them in `exports.sh`.
  - It needs its own multi-arch image.

---

## 5. Web UI: `app_proxy` (umbrelOS 2.0: the in-process app gateway)

- Declare an `app_proxy` service with **environment only** (PKG:170-174):
  ```yaml
  app_proxy:
    environment:
      APP_HOST: <app-id>_web_1     # Umbrel-injected container name <app-id>_<service>_1
      APP_PORT: <web listen port>  # internal port; need not be unique (PKG:224)
  ```
- The manifest `port` (e.g. 2104) is the browser URL `http://umbrel.local:2104`. `APP_PORT` is the
  container port (PKG:100-101, PKG:173). Do not also publish the web UI with `ports:` (PKG:176; LINT:625).
- **Auth.** Umbrel login (with 2FA) is enforced by default; don't add `PROXY_AUTH_ADD` (PKG:177-181; GW:93-95).
  - For wallet or companion clients that can't send Umbrel cookies, whitelist only the needed paths with
    `PROXY_AUTH_WHITELIST`, and use `PROXY_AUTH_BLACKLIST` to carve sensitive paths back out (PKG:182-185).
  - Whitelisted paths are public (PKG:184).
- **umbrelOS 2.0 change.** `app_proxy` is no longer a container.
  - app-script renders its env to `app-gateway.json`, then deletes the service from the runtime compose
    (SCRIPT:424-468, comment at 464).
  - umbreld proxies directly, with WebSockets supported (GW:64-100, GW:196).
  - It resolves `APP_HOST` via the container IP or the compose service name (`lan-ingress.ts`, not pulled).
  - New optional keys: `APP_PROTOCOL` (`https` upstream), `PROXY_TRUST_UPSTREAM`, `PROXY_TIMEOUT`
    (SCRIPT:447-456; GW:90-99).
  - Packages must still work on 1.x, where `app_proxy` *is* a container.
- **Web-UI Tor.** If the user enables remote Tor access, umbrelOS adds a `tor_server` sidecar exposing
  `80 → gateway:${APP_PROXY_PORT}`. This only happens for apps that have `app_proxy` (SCRIPT:303, SCRIPT:473-480;
  `umbrel/.../legacy-compat/docker-compose.tor.yml`; `tor-entrypoint.sh`). This does **not** cover raw ports.
- The web server must bind `0.0.0.0` (DEV:65).
- Serve the UI at `/` and keep `path: ""` (PKG:102). Add base-URL or proxy settings only if the UI breaks behind the gateway (PKG:198).

---

## 6. Raw TCP Electrum port: LAN and Tor (how electrs does it)

### 6.1 LAN
- Publish an explicit host mapping. Raw ports are only for non-HTTP protocols (PKG:220-222):
  `ports: ["${APP_FRIGLET_ELECTRUM_PORT}:50001"]`.
  - electrs uses `"${APP_ELECTRS_NODE_PORT}:${APP_ELECTRS_NODE_PORT}"` (`umbrel-apps/electrs/docker-compose.yml:40-41`).
- **Host 50001 is taken.** electrs has 50001, fulcrum 50002 (`umbrel-apps/fulcrum/exports.sh:4`), and
  electrumx publishes 50003→50001 (`umbrel-apps/electrumx/exports.sh:4-5`, `docker-compose.yml:40`).
  - Keep 50001 inside the container and publish a **different host port**.
  - **PROPOSAL: 50004.** It lint-checked clean at this commit. Re-check before submitting
    (`npm run lint:apps -- friglet`, which reports `ports.unique` and `manifest.port_unique`).
- Put the port in `exports.sh` (e.g. `APP_FRIGLET_ELECTRUM_PORT`) so the compose file, `torrc.template`
  and the UI share one value. The linter resolves simple static exports (PKG:223, PKG:309).
- The UI must show the **host** port. fulcrum and electrumx pass it as `ELECTRUM_PUBLIC_CONNECTION_PORT`
  (`umbrel-apps/fulcrum/docker-compose.yml:18`; `umbrel-apps/electrumx/docker-compose.yml:18`).
  - The electrs UI hardcodes 50001 (`umbrel-electrs/apps/backend/utils/const.js:11`).

### 6.2 Tor
This is independent of the user's "remote Tor access" setting, because the app runs its own Tor sidecar.

1. `torrc.template` declares the hidden service (`umbrel-apps/electrs/torrc.template:1-3`). It gets
   rendered to `${APP_DATA_DIR}/torrc`:
   ```
   HiddenServiceDir /data/app-$APP_ID-rpc
   HiddenServicePort <virtual port> <static container IP>:<container port>
   ```
2. A `tor` service runs `ghcr.io/getumbrel/tor:0.4.9.11@sha256:e382…`.
   - It has `user: "1000:1000"`.
   - It mounts `${APP_DATA_DIR}/torrc:/etc/tor/torrc:ro` and `${TOR_DATA_DIR}:/data`.
   - It sets `HOME=/tmp` (`umbrel-apps/electrs/docker-compose.yml:46-54`).
3. `exports.sh` reads the onion name, falling back to `notyetset.onion` (`umbrel-apps/electrs/exports.sh:6-7`):
   `${EXPORTS_TOR_DATA_DIR}/app-${EXPORTS_APP_ID}-rpc/hostname`
4. `hooks/pre-start` brings up the daemon and `tor` first, then polls up to about 10 s for the hostname file
   (`umbrel-apps/electrs/hooks/pre-start`). Each `compose` call re-sources the exports (SCRIPT:400-408),
   so the UI container gets the real onion on its first start.
5. The UI shows `<onion>:<port>:t`.
   - The virtual port equals the container port in electrs and fulcrum.
   - For friglet, use virtual port 50001 or the host port; just keep it consistent in the UI.

- **Static IPs.** electrs, fulcrum and electrumx pin `networks.default.ipv4_address` from `exports.sh`
  values (`10.21.21.x` / `10.21.22.x`) and use those IPs as the Tor target and as `APP_HOST`.
  - New guidance prefers Docker DNS names (PKG:216-218).
  - **UNVERIFIED:** whether a DNS name works reliably as a `HiddenServicePort` target. The reference apps
    don't use one. If a static IP is used, pick an unused one manually; the linter does not check IP
    uniqueness (no `ipv4` rule in LINT).

---

## 7. Persistence, secrets, user and permissions

### 7.1 Persistence
- All durable state goes in `${APP_DATA_DIR}/data/...` bind mounts. Named volumes are not allowed
  (PKG:203-212).
- Container filesystems are recreated on restart and update (PKG:203, PKG:326).
- File bind mounts: the host file must exist before compose starts, or Docker creates a directory instead
  (PKG:212). Relevant if `config.toml` is ever mounted as a file.
- For friglet, mount the directory `data/friglet:/data`.
  - `config.toml` and `scan.key` are precious.
  - `scanner_state.json` can be rebuilt by a slow rescan, but it embeds the scan secret in plaintext.
  - `scanner_state.headers.json` is rebuildable.
- **Backups.** Umbrel backs up app data by default (PKG:142). The scan key will therefore be in Umbrel
  backups. Only use `backupIgnore` for rebuildable data (PKG:145-148).

### 7.2 User and permissions
- Package files are installed as UID/GID 1000:1000 (PKG:195).
- The friglet image already runs as UID 1000, so `user: "1000:1000"` is fine.
- Don't force it on images that need root (PKG:196). The tor sidecar runs as 1000:1000 (electrs compose).
- Forbidden or discouraged: Docker socket, `privileged`, host networking, device mounts, `cap_add`
  (PKG:17, PKG:225-227; LINT `access.*` rules).

### 7.3 Secrets
- `derive_entropy <label>` gives an HMAC-SHA256 of the device seed (SCRIPT:182-199; PKG:265-273).
- `APP_SEED` and `APP_PASSWORD` are derived from it.
- **Never** use derived values as wallet keys, seeds or private keys (PKG:271). The SP scan secret must be
  user-supplied.
  - Do not pass it via `FRIGLET_SCAN_SECRET` in compose, as the testkit does. The repo's `AGENTS.md`
    forbids the scan key in compose files, logs and images.
  - Set it through the UI and the control socket (`SetScanKey`) instead.
- Treat the Docker network as untrusted (PKG:197).
  - friglet's HTTP API and control socket have no auth. Keep 8080 unpublished and only reachable from `web`.
  - The control socket is protected only by file permissions. Share it with the `web` sidecar alone.

---

## 8. Image requirements

- Prebuilt only. No `build:` (PKG:231; LINT `compose.build`).
- **linux/amd64 and linux/arm64**, using the index (manifest-list) digest (PKG:231, PKG:234; LINT:1015).
- Publicly pullable without credentials (PKG:232).
  - Registries seen: ghcr.io (216 refs), docker.io, quay.io, codeberg.org.
  - The linter fetches manifests anonymously using a bearer token (LINT:1348, LINT:1383-1460).
- Pin as `registry/repo:<version-tag>@sha256:<index digest>`. Tag and digest go together. Never use
  `latest` or a bare digest (PKG:233-235).
  - Verify with `docker buildx imagetools inspect`, then `npm run lint:apps -- <id> --check-images` (PKG:234, PKG:373).
- Pin **every** image: the web UI, oracle and tor too (PKG:236).
- Prefer official upstream images; use a wrapper only for real Umbrel glue (PKG:237).
- What friglet needs:
  - A published multi-arch image with release tags.
  - The same for the `web` UI image (and the oracle image, if shipped).
  - Example CI: `umbrel-electrs/.github/workflows/on-tag.yml` (buildx `linux/amd64,linux/arm64` on `v*.*.*` tags).

---

## 9. Connection-info UI: how electrs does it, and what friglet's must do

### 9.1 electrs reference
- The compose `app` service (image `getumbrel/umbrel-electrs`, port 3006) gets env vars
  (`umbrel-apps/electrs/docker-compose.yml:9-24`):
  - `ELECTRUM_HIDDEN_SERVICE` (the onion from `exports.sh`)
  - `ELECTRUM_LOCAL_SERVICE=${DEVICE_DOMAIN_NAME}`
  - `ELECTRS_HOST`
  - `BITCOIN_HOST`, `RPC_USER`, `RPC_PASSWORD`, `RPC_PORT`
- Backend routes (`umbrel-electrs/apps/backend/routes/v1/electrs.js`):
  - `GET /v1/electrs/electrum-connection-details` returns `{tor:{address,port,connectionString}, local:{…}}`.
    The connection string is `host:port:t` (`umbrel-electrs/apps/backend/logic/system.js:4-24`).
  - `/version` and `/syncPercent`. Sync is `electrum headers.subscribe` height divided by
    `getblockchaininfo.blocks`. It returns -1 while bitcoind is in IBD and -2 when it can't connect
    (`umbrel-electrs/apps/backend/services/electrs.js`).
- Frontend: a Local Network / Tor toggle, a QR code, and copy-able host, port and connection string
  (`umbrel-electrs/apps/frontend/src/components/ConnectionInformation.vue`; `src/views/Home.vue`).
- A minimal alternative with no backend is a static page served by nginx
  (`umbrel-apps/mosquitto/docker-compose.yml`, `data/www/index.html`, `data/nginx/default.conf`).
  It cannot show per-install values, though.

### 9.2 What friglet's UI must do (PROPOSAL)
- **Status.** friglet's `/height` and `/subscribe` block during scans.
  - Use the control socket's `GetStatus` (scan progress, tip, oracle connected, SP address, version).
  - Or add a non-blocking HTTP `/status` upstream. DEV:71 asks for a lightweight status endpoint.
- **First-run setup form.** Collect spend pubkey, birthday height and scan key. Apply them with `SetConfig`
  and `SetScanKey`, with no restart and no SSH (DEV:21).
  - Show degraded/retrying states rather than crash-looping (DEV:73).
- **Connection details.** Show `${DEVICE_DOMAIN_NAME}:<host port>:t` and `<onion>:<port>:t` with QR codes.
  - State plainly that the port is plaintext and unauthenticated, as electrs's `:t` also is.
- A widget needs an **unauthenticated** internal JSON endpoint on the web service (PKG:158-162).

---

## 10. Official store vs community store

| | Official (`getumbrel/umbrel-apps`) | Community store (your own repo) |
|---|---|---|
| How | PR to `master`, titled `Add <app name>`, using `.github/PULL_REQUEST_TEMPLATE.md` | Repo made from `umbrel-community-app-store`. Users paste its URL in App Store → ⋯ → Community App Stores |
| Gate | Lint CI on `pull_request_target` with `--check-images` (`umbrel-apps/.github/workflows/lint-apps.yml`); Umbrel review | No review. Unvetted, shown with a trust warning (umbrel.com support page, see SOURCES) |
| Bar | Must open to a useful UI. amd64+arm64. Data under `${APP_DATA_DIR}`. Minimal host access (PKG:12-17; `umbrel-apps/README.md` "App Store Standard") | Same runtime, but nothing enforced |
| Metadata | `gallery: []`, no `icon`. Put screenshots and logo in the PR body, not the repo (PKG:107-108, PKG:395; LINT:867) | `icon:` URL and full-URL `gallery` allowed (`umbrel-community-app-store/sparkles-hello-world/umbrel-app.yml`; REPO:189-194) |
| App ID | Free choice; must match the directory | **Must start with the store `id`** from `umbrel-app-store.yml`. Non-matching apps are silently dropped (REPO:157, REPO:184; `umbrel-community-app-store/README.md:9`) |
| PR body | App/version, upstream URL, image source, tests run, environment and architecture tested, permissions, credentials (PKG:392-395; TEST:151-166) | n/a |
| Updates | New PR bumping `version`, `releaseNotes`, tag and digest (UPD:32-46) | Push to the default branch. Stores are depth-1 cloned and polled every 5 min (REPO:80; `umbrel/.../apps/app-store.ts:78`) |

`umbrel-app-store.yml` holds `id` (letters and dashes) and `name`, shown in the UI as "<name> App Store"
(`umbrel-community-app-store/umbrel-app-store.yml`).

---

## 11. Local testing workflow

1. **Static checks**, run in an `umbrel-apps` checkout with the package copied in (TEST:19-28; PKG:370-384):
   ```sh
   npm ci && npm run lint:apps -- <app-id> --check-images && git diff --check
   ```
2. **Environment.** Use a real device (best), or a containerized umbrelOS (TEST:30-51):
   `docker run --privileged --network host -v umbrelos-test:/data ghcr.io/getumbrel/umbrelos:2.0.0 /sbin/init`
   - Documented as working on OrbStack (macOS). WSL may not work.
   - Alternatives: `npm run dev` from `getumbrel/umbrel` (`umbrel-apps-legacy-readme/README.md:306-364`),
     or a Linux VM (`umbrel.wiki/Install-umbrelOS-on-a-Linux-VM.md`).
3. **Sync the package** into the official store directory (TEST:102-117):
   `rsync -av --delete --exclude=.gitkeep <pkg>/ umbrel@umbrel.local:/home/umbrel/umbrel/app-stores/getumbrel-umbrel-apps-github-53f74447/<app-id>/`
4. **Drive it** with `umbreld client`, over SSH or the web terminal (TEST:53-100):
   - `appStore.registry.query`
   - `apps.install.mutate --appId bitcoin`, then `--appId <id>` (direct calls do **not** enforce dependencies)
   - `apps.state.query`, `apps.logs.query`, `apps.restart.mutate`
5. **Fresh-install checklist** (TEST:119-133):
   - Open the app from the Umbrel home screen and do a real workflow. For friglet: set the keys, see scan
     progress, connect an SP wallet over LAN and over Tor.
   - Restart, confirm persistence, and read the logs.
   - Test amd64 **and** arm64 if possible (TEST:36).
6. **Not accepted as proof:** a raw `docker compose up`, an image pull, or `state==ready` without opening
   the app (TEST:168-175).

---

## 12. Gotchas

- **README moved.** The `umbrel-apps` README no longer documents the framework. The current spec is
  the `.claude.upstream/skills/*/SKILL.md` files plus the linter (`umbrel-apps/README.md` "Contributing";
  `umbrel-apps/AGENTS.md.upstream`). Older blog and community guides predate umbrelOS 2.0.
- **app_proxy in 2.0.** It is now a gateway inside umbreld; the container is gone (§5). Don't depend on an
  `app_proxy` container hostname. `APP_PROXY_HOSTNAME` is still listed in PKG:251.
- **Port collisions.** Host 50001 belongs to electrs; also avoid 50002 and 50003. The manifest port must be
  unique across *all* manifest and compose host ports (LINT:323, LINT:621). The linter checks only the
  official store, not community stores the user has installed.
- **friglet image bug.** The Dockerfile comment says the state file lands in `/data`, but the default
  resolves to `$HOME/.config/friglet/scanner_state.json`. Set `FRIGLET_STATE_FILE=/data/scanner_state.json`.
  The default CMD also needs `/data/config.toml` to exist (`blindbit-rs/Dockerfile:28-31,41`; survey of
  `friglet/src/config.rs`).
- **Literal IP for P2P.** friglet's `p2p_node_addr` must be a literal IP. `APP_BITCOIN_NODE_IP` is one for
  the official bitcoin app; check the IPs exported by alternatives that `implements: bitcoin`.
- **Files under `data/` don't update.** They are not re-copied on update (SCRIPT:831). Ship config changes
  via templates or a `pre-start` hook (see the mosquitto hook).
- **Hooks.**
  - They must be executable and use exact names (PKG:336-347).
  - Failures are swallowed (`|| true`, SCRIPT:395-396).
  - `pre-install`, `pre-stop` and `pre-update` run before env is sourced (PKG:365).
  - The literal string `"${UMBREL_ROOT}/scripts/app"` is rewritten by sed in hooks (SCRIPT:392-393), so
    write it exactly like that.
- **`exports.sh` rules.**
  - It is sourced under `set -euo pipefail` (PKG:308).
  - No `exit`, `cd`, shell options, Docker calls or mkdir (PKG:302-307; LINT `exports.*`).
  - It runs for **every dependent app** too, so keep it cheap and quiet.
- **Templates.** `envsubst` also replaces `$VAR` inside shell or nginx templates (PKG:317). Also,
  `*.template` files must be top-level (LINT `templates.location`).
- **Restart policy.** Use `restart: on-failure`. The linter warns on `always`; electrs itself triggers that
  warning (PKG:193; lint run in this research).
- **Version fields.** `manifestVersion` is parsed as a semver (`1.1` → `1.1.0`). `version` is a free
  string, so quote it (SCHEMA:147-160; PKG:99).
- **Privacy.** friglet's Electrum port has no TLS or auth. Anyone on the LAN who can reach it can read
  the wallet's SP history. electrs has the same exposure for xpub-derived queries. Say so in the UI and
  the description.
- **Agent files in this folder.** The vendored `umbrel-apps/CLAUDE.md`, `AGENTS.md` and `.claude/skills/`
  were renamed with an `.upstream` suffix on import so coding agents do not auto-load them (see `SOURCES.md`).
