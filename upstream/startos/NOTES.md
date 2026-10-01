# NOTES — packaging friglet for StartOS

Distilled from the verbatim files in this folder (see `SOURCES.md`). Researched 2026-10-01.
Citations are `→ <path>:<line>` relative to `upstream/startos/`. Abbreviations:
`LD` = `start-technologies-live-docs/projects`, `MS` = `start-technologies-master`.

## 0. Which packaging model is current

| Question | Answer (2026-10-01) | Evidence |
| --- | --- | --- |
| Current OS | StartOS **0.4.0.x**. Latest release `v0.4.0.1` (2026-07-27); 0.4.0.2 has release notes on `master` but no release yet. | → `MS/projects/start-os/release-notes/0.4.0.1.md:1`, `0.4.0.2.md:1` |
| Current model | TypeScript **start-sdk** (`@start9labs/start-sdk`), `startos/*.ts`, `Makefile` including the SDK's `s9pk.mk`, s9pk **v2**. | → `LD/start-sdk/README.md:1`, `MS/projects/start-os/release-notes/0.4.0.md:19-20` |
| SDK to pin | **2.0.9** = npm `latest` (2026-07-25). It writes `osVersion: 0.4.0-beta.10` (checked on a local build). | → `npm-start-sdk-2.0.9/package/package.json`, `hello-world-startos/package.json:9` |
| Docs version | docs.start9.com serves `live-docs`, which already documents SDK **2.0.10** (not on npm yet; `osVersion` floor raised to 0.4.0). | → `LD/start-sdk/CHANGELOG.md:3-10`, `MS/projects/start-sdk/docs/src/environment-setup.md:240` |
| Next SDK | **3.0.0** on `master`, unreleased. It targets StartOS 0.4.0.2 and has breaking edits (`z.looseObject`, toolchain bundled, `mountDependency` loses `type`, `alpn` shape). | → `MS/projects/start-sdk/release-notes/3.0.0.md:1-19` |
| Registry accepts | The Community Registry takes submissions by email, forks them into `Start9-Community`, and builds them with the shared SDK CI. Inference: only SDK (0.4) packages. | → `LD/start-sdk/docs/src/publishing.md:11-35`, `frigate-startos/.github/workflows/build.yml` |
| 0.3.5 packages | They still **install**: a v1 s9pk is auto-converted to v2 on load and runs as a **Legacy** service. 0.3.5 is maintenance-only. Don't target it. | → `LD/../shared-libs/crates/start-core/src/s9pk/mod.rs:20-61`, `MS/projects/start-os/docs/src/update-040.md:196-198` |
| 0.3.5 model (for contrast) | `manifest.yaml`, `scripts/*.ts`, Makefile, "Properties", submitted by email to the Debian build box. | → `documentation-0.3.5.x/site/source/developer-docs/submission.rst` |

**Decision for friglet (proposed):** target StartOS ≥ 0.4.0 with SDK 2.0.9. Plan a 3.0.0 bump once
it and StartOS 0.4.0.2 ship. No 0.3.5 build.

## 1. friglet facts that drive the design (from blindbit-rs @ 611a877, not from this folder)

- Ports: HTTP API on 8080 (`/height`, `/subscribe`) and plaintext Electrum TCP on 50001. friglet does no TLS.
- All state is under `/data`: `config.toml`, `scan.key` (0600), `scanner_state.json`, and the control socket `friglet.sock`.
- Required settings: `network`, `spend_pubkey`, `start_height`, `p2p_node_addr` (parsed as `SocketAddr`, so it must be IP:port), and `oracle_url` (defaults to `https://oracle.setor.dev`). `FRIGLET_*` env vars take precedence over the config file.
- The image runs as `USER friglet` (uid 1000). `/height` blocks while a scan runs.
- It implements its own `blockchain.scripthash.*`, `transaction.*` and `silentpayments.*` methods. It has no backend Electrum server setting (Frigate does).

## 2. File layout (copy the template)

- Copy `hello-world-startos/` as the starting point. The layout is `startos/{manifest/,actions/,fileModels/,i18n/,init/,versions/}`,
  plus `main.ts`, `interfaces.ts`, `dependencies.ts`, `backups.ts`, `sdk.ts`, `utils.ts` and `index.ts`.
  → `LD/start-sdk/docs/src/project-structure.md:5-50`
- Repo root holds `Makefile`, `package.json`, `tsconfig.json`, `Dockerfile`, `icon.svg` (≤ 40 KiB), `LICENSE`, `README.md`,
  `instructions.md`, `UPDATING.md`, `AGENTS.md` and `assets/` (required, can be empty). → same file, `manifest.md:81-87`
- `sdk.ts` and `index.ts` are boilerplate. Init order: `restoreInit → versionGraph → setInterfaces → setDependencies → actions`.
  → `hello-world-startos/startos/sdk.ts`, `startos/init/index.ts`, `LD/start-sdk/docs/src/dependencies.md:167-175`
- Docs carry weight in review: README per `writing-readmes.md`, and `instructions.md` is packed into the s9pk.
  → `LD/start-sdk/docs/src/new-package-checklist.md`
- Our blindbit-node-apps rule: packaging lives outside `upstream/`. friglet comes from a pinned commit (submodule) or an image digest.

## 3. Manifest / SDK fields

- Required fields: `id`, `title`, `license` (SPDX; blindbit-rs is MIT), `packageRepo`, `upstreamRepo`, `marketingUrl`,
  `donationUrl|null`, `description.{short,long}` (i18n objects), `volumes`, `images`, `dependencies`. → `LD/start-sdk/docs/src/manifest.md:53-68`
- Image, option A: `source.dockerBuild` with our Dockerfile and blindbit-rs as a git submodule (the electrs pattern).
  → `electrs-startos/startos/manifest/index.ts`, `electrs-startos/Dockerfile`, `electrs-startos/.gitmodules`
- Image, option B: `source.dockerTag` pinned **by digest** to a published multi-arch friglet image (the Frigate pattern).
  → `frigate-startos/startos/manifest/index.ts:9-19`
- Set `arch: ['x86_64','aarch64']` and `ARCHES := x86 arm`. Without that line s9pk.mk defaults to `x86 arm riscv`.
  → `LD/start-sdk/docs/src/manifest.md:161-171`, `npm-start-sdk-2.0.9/package/s9pk.mk:15`
- `hardwareRequirements.ram` is in **bytes** (`8 * 1024 ** 3`). Leave it unset unless there's a real floor. → `LD/start-sdk/docs/src/manifest.md:219-232`
- Version is ExVer `<upstream>:<rev>` (for example `0.1.0:0`). Git tag is `v0.1.0_0`. → `LD/start-sdk/docs/src/versions.md:5-35`, `:312-331`
- Write every user-facing string with `i18n()`. Descriptions ship in en_US, es_ES, de_DE, pl_PL and fr_FR.
  → `LD/start-sdk/docs/src/actions.md:246-256`, `manifest.md:8-28`

## 4. Dependency on bitcoind (and electrs)

- Declare `bitcoind` in `manifest.dependencies` with `optional: false` and metadata title **"Bitcoin"** (generic name, not Core/Knots).
  → `electrs-startos/startos/manifest/index.ts`, `LD/start-sdk/docs/src/dependencies.md:41-46`
- Add `bitcoin-core-startos` as an npm git dependency so its constants, manifest and actions are typed. The `overrides` entry is mandatory.
  → `electrs-startos/package.json`, `LD/start-sdk/docs/src/dependencies.md:47-68`
- **friglet needs P2P, not RPC.** Resolve `peer-local`, bitcoind's whitelisted P2P bind that is bridge-only, like this:
  `sdk.host.getBridgeAddress(effects,{packageId:'bitcoind',hostId:peerLocalHostId,internalPort:peerPortLocal}).const()`.
  It returns `10.0.3.1:<port>`, which fits friglet's `SocketAddr`. → `bitcoin-core-startos/startos/utils.ts:12-20,48`, `bitcoin-core-startos/README.md:150`
- If the address is `null` (bitcoind absent), write nothing and never invent an address. Run a health-check-only `main`, as electrs does.
  → `LD/start-sdk/docs/src/service-to-service.md:60`, `electrs-startos/startos/main.ts:28-50`
- Forbidden: `bitcoind.startos` DNS, another package's container IP, and reading `assignedPort` directly. → `LD/start-sdk/docs/src/service-to-service.md:138-144`
- Fixing bitcoind's config: `sdk.action.createTask(effects,'bitcoind',autoconfig,'critical',{input:{kind:'partial',accept:[{prune:0}],set:{prune:0}},when:{condition:'input-not-matches',once:false}})`.
  Use it only if friglet needs historical blocks below the prune horizon. → `electrs-startos/startos/dependencies.ts:6-14`, `bitcoin-core-startos/startos/actions/config/autoconfig.ts:8-16`
- Return value: `{bitcoind:{kind:'running',versionRange:'>=31.1:…',healthChecks:['bitcoind','sync-progress']}}`. This drives warnings only and does **not** gate startup.
  → `LD/start-sdk/docs/src/dependencies.md:70-80`, `electrs-startos/startos/dependencies.ts:16-21`
- If RPC is ever needed (for example a co-packaged BlindBit oracle), there are two patterns:
  - Cookie: `mountDependency` bitcoind `main` read-only and read `/.cookie`. → `electrs-startos/startos/main.ts:63-82`, `fileModels/electrs.toml.ts:5-12`, `frigate-startos/startos/main.ts:95-101`
  - rpcauth: a task on bitcoind's hidden `generate-rpc-dependent` action. → `bitcoin-core-startos/startos/actions/generateRpcUserDependent.ts:45-60`
  - RPC hosts: `rpc` is 8332 through the proxy, JSON-RPC only. `rpc-local` is 58332, bitcoind itself with REST, bridge-only. → `bitcoin-core-startos/startos/utils.ts:22-53`
- electrs/fulcrum: friglet appears not to need an Electrum backend (§1). If one were needed, Frigate shows the pattern: an optional dependency
  selected in `store.json` and resolved over the bridge. → `frigate-startos/startos/dependencies.ts:20-48`, `frigate-startos/startos/main.ts:28-63`
- BlindBit oracle: no StartOS package exists. Outbound internet works by default, but DNS must use the container resolver.
  → `LD/start-sdk/docs/src/outbound-networking.md`

## 5. Interfaces (web UI vs raw TCP 50001)

- Electrum 50001: copy electrs/fulcrum/frigate exactly. Use `bindPort(50001,{protocol:null,addSsl:{preferredExternalPort:50002,alpn:null,addXForwardedHeaders:false,auth:null},preferredExternalPort:50001,secure:null})`,
  then `createInterface({type:'api',schemeOverride:{ssl:'ssl',noSsl:'tcp'},...})`. → `electrs-startos/startos/interfaces.ts:7-35`
- With that setup, StartOS terminates TLS with the server's root-CA certificate. Off-box wallets only ever see `ssl://`.
  Plaintext 50001 is reachable only on the bridge (other services). → `LD/start-sdk/docs/src/interfaces.md:438-446`, `LD/start-docs/bitcoin-guides/src/connecting-wallets.md:41-57`
- Wallets must trust the server root CA, or use an ACME domain. Electrum desktop fails on the chain (spesmilo/electrum#7459), so document the `certs/<host>` fix.
  → `LD/start-docs/bitcoin-guides/src/connecting-wallets.md:59-104`
- The external port is assigned at install. 50002 is a preference, so never hardcode it in docs. → `connecting-wallets.md:30`, `LD/start-sdk/docs/src/service-to-service.md:18-20`
- LAN/Tor/clearnet are the user's choice. LAN is on by default. Tor is a separate marketplace service, and the user adds onions per interface.
  Never claim "exposed on Tor". → `LD/start-sdk/docs/src/interfaces.md:5-17`, `LD/start-os/docs/src/tor.md`
- StartOS 0.4.0.2 adds "mark gateway secure", which offers plaintext (`secure:null`) ports on that LAN. → `MS/projects/start-os/release-notes/0.4.0.2.md:12`
- HTTP API 8080: bind `protocol:'http'` with `type:'api'` (TLS terminated by StartOS), or bind without exporting to keep it bridge-only.
  It has no auth, so if exported gate it with `addSsl.auth` (bearer/basic) or leave it unexported. → `LD/start-sdk/docs/src/interfaces.md:93-137`, `:448-486`, `service-to-service.md:103-118`
- "Connection info" page: 0.4 has **no Properties**. Wallet URLs come from the Interfaces tab (copy + QR). An action can return
  `group` results with `masked/copyable/qr`. → `LD/start-os/docs/src/interfaces.md:30`, `LD/start-sdk/docs/src/actions.md:94-122`
- A custom web UI is optional: one more `type:'ui'` HTTP binding. → `LD/start-sdk/docs/src/recipe-web-ui.md`

## 6. Config / actions for user input (scan key etc.)

- Pattern: a `config` action (`sdk.Action.withInput`) that prefills from the file model and saves with `merge`.
  The init raises a **critical own task** on install, so the service can't start until it's answered.
  → `frigate-startos/startos/init/index.ts:15-24`, `frigate-startos/startos/actions/config.ts`, `LD/start-sdk/docs/src/tasks.md:5-30`
- Fields: `spend_pubkey`, `start_height` (`Value.number`) and `network` (fixed to mainnet; bitcoind is mainnet). The scan secret is
  `Value.text({masked:true,required:true,patterns:[hex64]})`. → `LD/start-sdk/docs/src/actions.md:176-240`
- The scan secret goes to `/data/scan.key` (0600) from the action. Never return it in an action result, write it to logs, or put it in `env`.
  `FRIGLET_SCAN_SECRET` would expose it in process env. Prefill must not echo it, so leave the field blank and keep the stored value.
- Model `config.toml` with a `FileHelper.toml` file model. Enforce owned keys with `.literal(..).catch(..)`, as electrs does with `electrum_rpc_addr`.
  The package rewrites `p2p_node_addr` every start. → `electrs-startos/startos/fileModels/electrs.toml.ts`, `LD/start-sdk/docs/src/file-models.md`
- Keep StartOS-only state (for example a backend selection) in `store.json`, never as invented keys in friglet's config.
  → `LD/start-sdk/docs/src/service-to-service.md:87-101`
- `main` must `.const()` the user-controlled keys so a Configure save restarts friglet (it reads its config once).
  → `frigate-startos/startos/main.ts:24-27`, `fulcrum-startos/startos/main.ts:56-61`

## 7. Health checks

- Daemon `ready`: `sdk.healthCheck.checkPortListening(effects,50001,…)`. It reads `/proc/net` with no I/O. Don't use HTTP `/height`, which blocks during scans.
  → `LD/start-sdk/docs/src/main.md:394-400`, `frigate-startos/startos/main.ts:138-162`
- Add a separate `sync-progress` check (`addHealthCheck`). Parse friglet's scan-progress stdout via `onStdout` and return `loading` with
  the height until caught up. → `frigate-startos/startos/main.ts:110-136,164-194`, `fulcrum-startos/startos/main.ts`
- Result states are `success|loading|disabled|starting|failure`. Use a slower `trigger` for expensive probes. → `LD/start-sdk/docs/src/main.md:380-426`
- Health-check ids are a public contract: dependents list them in `healthChecks`. → `LD/start-sdk/docs/src/recipe-health-checks.md`

## 8. Backups

- `sdk.setupBackups(async () => sdk.Backups.ofVolumes('main').setOptions({exclude:[...]}))`. Exclude rebuildable data and the socket
  (`/friglet.sock`; `scanner_state.json` is a choice, since keeping it avoids a rescan). → `frigate-startos/startos/backups.ts`, `electrs-startos/startos/backups.ts`
- The service is stopped during backup and restarted only if it was running. → `LD/start-sdk/docs/src/recipe-backups.md`
- Backups are encrypted with the master password, but they do contain `scan.key`. Document that in the README.
  → `LD/start-os/docs/src/backup-create.md:16`
- 0.3.5 and 0.4 backups are mutually incompatible. → `LD/start-os/docs/src/update-040.md:18`, `MS/projects/start-os/docs/src/update-040.md:18`

## 9. Volumes

- Use a single `volumes: ['main']` mounted at `/data`. Optionally add a `startos` volume for `store.json`.
  → `LD/start-sdk/docs/src/manifest.md:272-290`, `service-to-service.md:91-99`
- Mounts are immutable builders, so reassign on conditional mounts. → `LD/start-sdk/docs/src/main.md:428-474`
- The friglet image runs as uid 1000. Either add `idmap` on the mount or run a root oneshot `chown -R friglet:friglet /data` before the daemon.
  → `LD/start-sdk/docs/src/main.md:476-488`, `:184-200`, `:222`
- Generated, non-persistent files go to the subcontainer rootfs, not the volume. → `LD/start-sdk/docs/src/main.md:490-508`

## 10. Multi-arch build and toolchain

- Host tools: Docker (running, user in `docker` group), make, Node ≥ 22, squashfs tools (**`tar2sqfs`** from squashfs-tools-ng on Linux),
  git, jq, and **start-cli** (latest 2.1.0, signed by Start9 + Matt Hill). → `LD/start-sdk/docs/src/environment-setup.md:12-210`
- `dockerBuild` images are built with `docker buildx build --platform=linux/<arch> --build-arg ARCH=<x86_64|aarch64> -o type=docker | docker load`,
  so **buildx is required** for our Dockerfile. Cross-arch needs QEMU/binfmt. → `LD/../shared-libs/crates/start-core/src/s9pk/v2/pack.rs:525-578`
- `dockerTag` images are pulled per platform and exported. No buildx or QEMU is needed (aarch64 built fine on an x86 host).
  → `pack.rs:594-700`
- In CI, arm targets run **natively** on `ubuntu-24.04-arm` and x86 on `ubuntu-latest`. The setup installs QEMU, buildx, squashfs-tools-ng, Rust and start-cli.
  → `MS/.github/workflows/build.yml:54-61`, `MS/.github/actions/setup-build-env/action.yml`
- Rust under QEMU is slow, so friglet's arm build should run natively or cross-compile. Use `ARG TARGETARCH` in the Dockerfile for arch-specific downloads.
  → `electrs-startos/Dockerfile`
- Package CI is four thin workflows calling `Start9Labs/start-technologies/.github/workflows/*.yml@master`. Signing uses the `DEV_KEY` secret.
  → `hello-world-startos/.github/workflows/*.yml`, `MS/.github/workflows/release.yml`

## 11. Build and verify a .s9pk without a StartOS device (verified 2026-10-01)

1. `npm ci`, then provide a signing key. Normal path: `start-cli s9pk init-workspace`, which clones the monorepo.
   CI shortcut: `start-cli init-key`, then `mkdir .startos && cp ~/.startos/id.key.pem .startos/build.key.pem`. → `MS/.github/workflows/build.yml:85-103`
2. Run `make x86` / `make arm` (one arch), `make` (all `ARCHES`) or `make universal`. → `LD/start-sdk/docs/src/makefile.md:16-35`
3. Inspect offline: `start-cli s9pk inspect <f>.s9pk manifest|file-tree|commitment`. Check `osVersion`, `images.*.arch` and `hardwareRequirements.arch`.
   → `LD/start-os/docs/src/cli-reference.md` (s9pk section)
4. Result of the local run: `hello-world_x86_64.s9pk` and `hello-world_aarch64.s9pk` built on x86_64 with start-cli 2.1.0, SDK 2.0.9 and Docker 29
   without buildx. Manifest reads `osVersion 0.4.0-beta.10`.
5. Runtime testing still needs StartOS on hardware or a VM (`make x86 install` or the Sideload UI). There is no local emulator.
   → `LD/start-sdk/docs/src/environment-setup.md:8-10`, `LD/start-os/docs/src/sideloading.md`

## 12. Submission to the Community Registry

- Email **submissions@start9.com** with the public repo link. Start9 forks it into `Start9-Community`, reviews via a PR on the fork,
  and **the fork becomes upstream** from then on. → `LD/start-sdk/docs/src/publishing.md:20-24`
- Merging a PR into the fork auto-builds, tags and deploys to **community-beta**. You then email or open an issue to promote to **community**.
  → `publishing.md:26-35`
- Pre-publish: tag convention, `tsc` and pack green, README matches the code (no version numbers), and end-to-end tested on StartOS,
  including uninstall/reinstall and backup/restore. → `publishing.md:37-44`, `LD/start-sdk/docs/src/new-package-checklist.md`
- Community listing means "meets technical criteria", not endorsed or supported. → `LD/start-os/docs/src/default-registries.md`
- Self-hosted registry (`startos-registry` service) or GitHub-release sideloads work in parallel. → `publishing.md:5-9`, `LD/start-sdk/docs/src/host-registry.md`

## 13. Gotchas

- **SDK drift:** the docs site (2.0.10) is ahead of npm (2.0.9), and `master` is at 3.0.0. Read docs at the tag you pin. → `LD/start-sdk/CHANGELOG.md`
- **`make` needs a workspace key.** Without `.startos/build.key.pem` the pack refuses. → `LD/start-sdk/docs/src/makefile.md:106-110`
- **`tar2sqfs` missing** fails pack with `Docker Error: tar2sqfs: No such file or directory`. A failed pack can leave a stopped container behind (seen locally).
- **Never hardcode** `50002`, `10.0.3.1:<port>` or `bitcoind.startos`. Resolve via `getBridgeAddress(...).const()`, not `.once()` or `.waitFor()`.
  → `LD/start-sdk/docs/src/service-to-service.md:47-58`
- **bitcoind peer-local** needs a free inbound slot. bitcoind's `maxconnections` floor is 40 for that reason. → `bitcoin-core-startos/README.md:150`
- **Pin the bitcoind dependency branch.** Start9's own wrappers mix `#next/31.x` and `#next/28.x`. Pick one and set `versionRange` to the first
  bitcoind version exposing `peer-local`. → `electrs-startos/package.json`, `fulcrum-startos/package.json`
- **DNS:** containers must use `10.0.3.1`. Direct queries to `1.1.1.1:53` are silently dropped. DNSSEC is not passed through.
  → `LD/start-sdk/docs/src/outbound-networking.md`
- **TLS:** wallets that pin certs were unstable across routes before 0.4.0.2. → `MS/projects/start-os/release-notes/0.4.0.2.md:15`
- **Secrets:** the scan key is in volume backups and must stay out of logs, images, env and action results (blindbit-node-apps AGENTS.md).
- **Licenses:** don't copy code from `fulcrum-startos` (GPL-3.0 text). Delete `documentation-0.3.5.x/` before going public (no license). See `SOURCES.md`.
