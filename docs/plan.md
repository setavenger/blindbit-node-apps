<!-- /autoplan restore point: "/home/agent/.gstack/projects/blindbit-node-apps/integration-autoplan-restore-20261001-122532.md" -->
## Implementation plan
# Plan: friglet on node-in-a-box platforms

Status: revised by gstack `/autoplan` on 2026-10-01 (auto-decided by the review, not yet
confirmed by the maintainer). Findings, decisions and open questions are in the review record
below the plan.

## Taste decisions taken by the agent (2026-10-01, not maintainer-confirmed)

The review left six taste decisions. Under the standing rule that technical calls are the
agent's, they are settled as follows:

- **TD1** friglet-ui is Rust on blindbit-rs's `friglet-ipc` at the friglet pin. Accepted.
- **TD2** Electrum exposure: friglet binds `127.0.0.1:50001`; friglet-ui owns the external
  listeners, TLS 50032 (self-signed, fingerprint on Connect) and plaintext 50031. Where the
  platform terminates TLS (StartOS) friglet-ui only serves 50031 to the platform proxy. On LAN
  the Connect page recommends TLS; plaintext 50031 is for the onion and the platform proxy,
  and an Advanced opt-in for LAN (Umbrel publishes 50031 as its electrs convention, so the
  Umbrel Connect page names both and recommends TLS). Whether to hold the community-store
  announcement until BR1 is a release call for the maintainer.
- **TD3** Keep the tray's visual language, plus a light theme. Accepted, no mockup round.
- **TD4** No QR code. Accepted.
- **TD5** `FRIGLET_UI_AUTH=none` allowed on compose with a warning and banner. Accepted.
- **TD6** Umbrel relies on its gateway login. Accepted.
- **P5** No bundled oracle this round; local-oracle packaging is a follow-up ticket.
- Pin: blindbit-rs `93f6a92` (`origin/integration`, same as the testkit pin) until BR3 lands.

Tracker ids (project P-SNB-75): M0 SNB-648, M1 SNB-649, M2 SNB-650, M3 SNB-651, M4 SNB-652,
M5 SNB-653. BR1 SNB-630, BR2 SNB-631, BR3 SNB-632, BR4 SNB-633, BR5 SNB-634, BR6 SNB-635,
BR7 SNB-636, BR8 SNB-637, BR9 SNB-628, BR10 SNB-623, BR11 SNB-638, BR12 SNB-639, BR13 SNB-640,
BR14 SNB-641. Maintainer decisions: registry and signing SNB-642, permanent ids SNB-643,
`upstream/` licensing and visibility SNB-644, oracle operation SNB-645, local oracle SNB-646,
launch before BR2 SNB-647.

Scope of the first implementation round: M0 in full, and M1-M4 packages built and checked
with each platform's own tooling. Hardware install tests, Sparrow tests and the release gate
stay open until they can be run.

## Goal

A user with a Bitcoin node box (Umbrel, StartOS, RaspiBlitz, myNode, and later the
platforms in `upstream/others/SURVEY.md`) installs **friglet**, pastes the Silent Payments
descriptor that Sparrow shows (`sp(spscan1…)`) plus a birthday height into a web page, and
gets an Electrum endpoint that Sparrow connects to. friglet then scans for that one wallet
using the box's own Bitcoin node for blocks and broadcast.

Requirement surfaced by research: the node must hold every block from the wallet birthday
onward. friglet downloads candidate blocks from that height over P2P from one fixed peer, and a
pruned peer answers `notfound` (`blindbit-lib/src/scanner/p2p.rs:201-208`). The supported
configuration is an **unpruned** node. Each package says so; StartOS enforces it with a task;
RaspiBlitz and myNode check `prune=` in `bitcoin.conf` at install and refuse with a message;
friglet-ui reports a `notfound` from the node as a named error.

Non-goals for this round: multi-wallet, adopting Sparrow's subscribe keys
(see memory `friglet-onboarding-scope`), a bundled oracle, native Electrum TLS inside friglet
(platform TLS is used where the platform provides it: StartOS, RaspiBlitz nginx stream),
patching friglet. friglet defects found here become blindbit-rs tickets (BR list below).

## Facts the design rests on

From the friglet survey (blindbit-rs @ `611a877`, testkit pin `93f6a92`):

- friglet needs a BlindBit **oracle** over gRPC and a Bitcoin **P2P peer** given as a literal
  `IP:port` (`friglet/src/config.rs:172`). It needs **no** bitcoind RPC and has no SOCKS/Tor
  support, so the oracle is always reached over clearnet. friglet's default `oracle_url` is
  `https://oracle.setor.dev` for **every** network (`friglet-ipc/src/lib.rs:72`); signet needs
  `https://signet.oracle.setor.dev` set explicitly, and a network mismatch is not detected.
  The oracle must be reachable at startup (exit 1 otherwise), and a gRPC stream error mid-scan
  aborts the process (exit 134, `panic = "abort"`).
- It refuses to start without `p2p_node_addr`, `start_height`, `spend_pubkey` and a scan
  secret, taken from CLI flags, `FRIGLET_*` env or `config.toml`, in that precedence (env beats
  the file). The control socket exists only while friglet runs and offers `SetConfig` and
  `SetScanKey`, but Electrum keeps serving the old wallet until a restart, and `SetScanKey` is
  ignored when the state file was made with another key (`friglet/src/control/mod.rs:10-17,380-390`).
  A wallet change therefore means: stop, fresh state file, restart.
- It serves no web UI. Status comes from the control socket (`GetStatus`, newline-delimited
  JSON, unversioned), which never waits on the scanner; the HTTP API hangs for as long as
  scanning is enabled. `GetStatus.last_error` is filled when the scan task ends with an error
  (`friglet/src/supervisor.rs:60-64`), and the process then keeps serving stale Electrum data
  without rescanning; `Start` and `Stop` requests exist. `scan_progress` covers only the
  current range. Failures before the socket exists, and panics, are visible only in logs.
- Default state file lands outside `/data` in the upstream image (bug); we always set an
  absolute `state_file`. The state file embeds the scan secret in plaintext (mode 0600) and is
  rewritten non-atomically; a corrupt file silently triggers a full rescan.
- No published image, release tag or signed binary exists. The oracle image exists on Docker Hub.
- The Electrum server is single-wallet ("single-server mode"): unknown scripthashes get an empty
  history, and `blockchain.silentpayments.subscribe` ignores the client's keys and returns the
  configured wallet's address, history, tweaks and raw transactions to any caller
  (`friglet/src/electrum/mod.rs:651-700,773-812`). Sparrow uses one server for all its wallets.
- At the pinned commits, spend detection, reorg rollback and the oracle-stream panic are fixed
  only on unmerged blindbit-rs branches. friglet logs balances, txids and client IPs at `info`.

From the platform research (`upstream/*/NOTES.md`):

| Platform | Mechanism | Docker | Third-party path | Bitcoin P2P peer |
|---|---|---|---|---|
| Umbrel 2.0 | `umbrel-app.yml` + compose, `app_proxy` gateway with Umbrel auth | yes | community store (our repo) now, official store PR later | `${APP_BITCOIN_NODE_IP}:${APP_BITCOIN_P2P_WHITEBIND_PORT-${APP_BITCOIN_P2P_PORT}}` |
| StartOS 0.4 | TypeScript start-sdk 2.0.x, s9pk v2 built with `start-cli` | yes | email submissions@start9.com, lands in `Start9-Community` | bitcoind `peer-local` whitebind (IP:port) |
| RaspiBlitz v1.12 | `bonus.<app>.sh` + systemd; **Docker removed in 2024** | no | PR to `dev` | `127.0.0.1` dedicated whitebind port |
| myNode | dynamic app in `mynode_apps/`, digest-pinned compose | yes | PR upstream only (sideload removed 2026-09-18) | local bitcoind |

## Architecture

One repository, three kinds of artifact:

1. **`friglet` binaries** (linux amd64 + arm64) built from blindbit-rs at a pinned commit,
   targeting glibc 2.36 so they run on Debian bookworm (RaspiBlitz, MiniBolt, RaspiBolt).
   Cross-compiled with `cargo zigbuild`; no QEMU needed.
2. **`friglet-ui`**, a small Rust program in this repo's Cargo workspace (static binary with the
   static UI embedded, adapted from `friglet-tray/ui/`). It consumes blindbit-rs's `friglet-ipc`
   crate at the same pinned commit as friglet, so status and config types match the daemon:
   - first-run setup form: paste `sp(spscan1…)` descriptor (hex scan secret + spend pubkey
     only behind a disclosure), answer whether the wallet has received payments before, and set
     the birthday height; network and P2P peer come from the platform and are shown read-only;
     the same parser and validation are exposed as a `friglet-ui setup` CLI for StartOS;
   - writes `config.toml` and `scan.key` (0600) atomically, never logs the secret;
   - supervises the friglet process: starts it once configured, restarts it on config change
     and on **any** exit (including codes 0 and 134) with capped backoff, forwards SIGTERM and
     waits up to 120 s, forwards logs and classifies known error lines into UI states;
   - status page rendered from friglet-ui's own supervisor state plus the last `GetStatus`
     snapshot (named states, never an endless spinner);
   - connection page: dedicated-Sparrow step first, then one recommended endpoint per platform
     with copy, other endpoints collapsed, the exact transport and privacy statement for each,
     no QR code (taste TD4);
   - mandatory password on platforms without gateway auth: RaspiBlitz, myNode, compose
     (friglet-ui login) and StartOS (service interfaces are reachable on the LAN without a
     StartOS login, so the `ui` binding uses `addSsl.auth` basic auth with a generated
     password shown and rotated by an action); Umbrel relies on its gateway login (taste TD6);
   - Electrum front: friglet binds Electrum to loopback only; friglet-ui owns the external
     Electrum listeners (TLS on 50032 with a self-signed certificate whose fingerprint the
     Connect page shows, plaintext 50031 only for the onion, the platform proxy or an explicit
     opt-in), opens them only after the wallet identity is verified and closes client
     connections during a wallet change;
3. **One multi-arch image** `friglet-node` containing both binaries; the entrypoint is
   `tini -- friglet-ui` so signal forwarding and zombie reaping are not hand-rolled. One image
   keeps one digest to pin per platform. Native installs ship friglet and friglet-ui as one
   tarball with one checksum, since the IPC between them is unversioned.

Platform packages, each in its own top-level folder, all consuming the same image or
binaries:

- `umbrel` community store: `umbrel-app-store.yml` at repo root (store id `blindbit`) and
  app folder `blindbit-friglet/` at repo root, because Umbrel loads stores from the repo
  root. App id `blindbit-friglet` is also valid for the official store, so one id serves both.
- `startos/`: SDK 2.0.x package modelled on `Start9-Community/frigate-startos` (closest
  analogue). Setup is a StartOS-native masked-input action raised as a critical task on
  install; the action runs `friglet-ui setup` inside the subcontainer (same parser, birthday
  rules and identity check as the web form) and `friglet-ui` serves status and connection info
  as the `ui` interface, with "Change wallet" pointing back to the action. Electrum uses the
  electrs/fulcrum/frigate pattern (`type: 'api'` with `schemeOverride` ssl/tcp, `addSsl` on the
  bound port); StartOS terminates TLS, so wallets see `ssl://` and must trust the server root CA. Bitcoin dependency asks for `prune=0` via a task. Health check is the
  Electrum port, not HTTP `/height` (blocks during scans).
- `raspiblitz/`: `bonus.friglet.sh` (status/menu/install/on/off/update), systemd unit,
  installs the signed binary release; setup via `friglet-ui` behind the RaspiBlitz nginx;
  the SSH menu shows the UI URL, the generated UI password, status and a password reset.
- `mynode/`: app definition in `mynode_apps/` layout, ready for an upstream PR.
- `compose/`: plain Docker Compose for any Docker host; also the local test harness. The node
  peer and network come from `FRIGLET_UI_NODE` (`host:port`, resolved to a literal IP on every
  start and shown in the UI) and `FRIGLET_UI_NETWORK`; `FRIGLET_UI_PASSWORD` is required; a
  `.env.example` ships next to the compose file and CI runs the quickstart exactly as written.

Shared defaults across platforms: Electrum port **50031** inside and outside (free in every
pulled platform tree; 50001 belongs to electrs/Fulcrum, 50011 to Frigate), friglet HTTP API on
`127.0.0.1:8095` (8080 is LND REST on RaspiBlitz and JoinMarket on myNode; friglet-ui never
uses it), `oracle_url` written explicitly per network (mainnet `https://oracle.setor.dev`,
signet `https://signet.oracle.setor.dev`) and shown in the UI as a third-party service.

Pinned versions live in one file, `versions.env` (blindbit-rs commit, Rust toolchain,
cargo-zigbuild, zig, protoc, base image digests, blindbit-testkit commit, and the platform tools:
start-cli, the umbrel-apps commit used for linting, the myNode commit used for `create.py`).
The `friglet-ipc` git `rev` in `Cargo.toml` is generated from it and CI fails if they differ;
`BLINDBIT_RS_SRC=<path>` builds against a local or unmerged blindbit-rs checkout and tags the
build `dev` in the UI and diagnostics.

## Verification

- `friglet-ui`: Rust `cargo test` (descriptor vectors from the BIP, config writer, supervisor,
  state catalog, `setup` CLI contract).
- Image: multi-arch build; arm64 binaries are executed (`friglet --version`,
  `friglet-ui --self-test`) on an arm64 runner or under qemu-user, not only checked with `file`.
- End to end (CI): blindbit-testkit T2 (regtest bitcoind, real oracle, constructed payments)
  with the packaged image in place of its friglet; set up by pasting a descriptor through
  friglet-ui; assert every constructed payment is found over Electrum on 50031, that balance
  and history survive a restart, and that the status page shows heights.
- End to end (manual smoke): compose stack on signet with the signet oracle and a signet peer.
- Umbrel: upstream linter `.tools/lint-apps.mjs` against our app.
- StartOS: build `.s9pk` for x86_64 and aarch64 with `start-cli` (verified offline on
  this host for the template) and inspect the manifest.
- RaspiBlitz: shellcheck; run the binary in a Debian bookworm container.
- myNode: `app_sdk/create.py check` on our app.
- CI workflow committed for all of the above plus image publishing on tags.

## Open items (not decided)

- Publishing location of images and binaries (GHCR under `setavenger`?) and signing key.
- Store-facing app name and ids (Umbrel app ids and StartOS package ids can never change).
- Whether third-party material under restrictive or missing licenses stays in
  `upstream/` if the repo is public. This blocks the Umbrel community store: Umbrel clones the
  store repo's default branch onto every device and re-polls it every 5 minutes (11 MB today).
- Local oracle packaging (needs Bitcoin Core 30+ with REST and an unpruned node). Umbrel offers
  Core 31.1/30.3, StartOS ships 31.1 and RaspiBlitz recommends 30.2, so it is feasible there once
  REST is switched on; myNode pins 29.3. Deferred by the owner's non-goal.

<!-- autoplan-accepted:ceo -->
- Delivery order and milestones (CEO). Order: M0 compose + image + friglet-ui; M1 Umbrel
  community store, dogfooded on a real Umbrel; M2 StartOS sideloadable `.s9pk`; M3 RaspiBlitz
  bonus script PR to `dev`; M4 myNode app PR. Each platform tracks four separate states:
  artifact built, install-tested on the platform, submission-ready, installable by users.
  No platform waits on another platform's review queue. Nothing is cut.
- Release gate for anything offered beyond the maintainer (community store announcement or any
  catalog submission): blindbit-rs fixes for spend detection, reorg rollback and the
  oracle-stream panic are merged on its integration branch and re-pinned here (BR3), and a
  pinned Sparrow version completes descriptor setup, receives a signet payment, spends it and
  shows the same history after a friglet restart, on a real Umbrel and on compose.
- Official catalog submissions (Umbrel official store, StartOS registry, RaspiBlitz, myNode)
  also wait for a key-checked `blockchain.silentpayments.subscribe` in friglet (BR1).
- Sparrow server caveat: until friglet forwards non-Silent-Payments calls (BR2), every
  connection page and README tells the user to run the Silent Payments wallet in a dedicated
  Sparrow instance with its own data directory, because pointing the main Sparrow at friglet
  empties every other wallet. Verify the data-directory flag and the empty-history behaviour on
  the pinned Sparrow version during M0.
- Wallet identity check: friglet-ui shows the SP address derived from the pasted descriptor and
  the `sp_address` friglet reports after start, and does not mark setup complete unless they
  match; the page asks the user to compare it with Sparrow's receive address.
- Wallet change: stop friglet, move the old state file aside (kept as `.prev`), write the new
  config and key, start, verify the address. Moving the birthday earlier also starts from a
  fresh state. The UI states that this rescans from the new birthday.
- Electrum exposure policy (taste TD2): LAN plaintext on 50031 following each platform's electrs
  convention, plus an onion; platform TLS on StartOS (automatic) and RaspiBlitz (nginx stream on
  50032); ufw rules limited to the LAN where the platform helper allows it; the onion address is
  shown only inside the authenticated UI; the UI and store copy say the port reveals this
  wallet's history to anyone who can reach it.
- Accepted delights: Umbrel home-screen widget (unauthenticated JSON with only state, scanned
  height and tip height); birthday helper ("new wallet, use current tip" plus a date-to-height
  estimate labelled as an estimate), where the tip comes from the `start_height` field of the
  box node's P2P `version` message (friglet-ui does one handshake; no RPC) and the user can
  always type a height; "test connection" (`server.version` against the advertised LAN
  address, run from the box; the onion is not tested because friglet-ui has no SOCKS client);
  diagnostics download built only from structured data (versions, config without secret,
  friglet-ui state history, classifier output), never from free-text friglet logs; catch-up ETA.
- Positioning and disclosure: README and store copy say why friglet rather than Frigate using
  measured numbers only (time to first balance and peak RSS for a one-year mainnet birthday on
  arm64, oracle bytes), and say what the oracle operator sees (box IP, scanned height ranges,
  timing; never keys).
- blindbit-rs follow-up tickets, filed and referenced by id, not patched here:
  BR1 key-checked `silentpayments.subscribe`; BR2 forward non-SP Electrum calls to a backend
  (electrs/Fulcrum); BR3 merge spend detection, reorg rollback, oracle-stream panic and K_max
  fixes; BR4 start unconfigured, fill `GetStatus.last_error`, version the IPC protocol;
  BR5 SOCKS5/Tor for the oracle and an oracle network check; BR6 Docker `state_file` default and
  relative-path rewrite; BR7 bind failure exits 0, non-atomic state writes, SIGTERM delayed by
  blocking P2P reads; BR8 redact balances, txids, keys and IPs from `info` logs; BR9 fixed fee
  estimate stubs; BR10 descriptor parsing in blindbit-lib shared with friglet-tray; BR11 release
  workflow (tags, multi-arch image, signed binaries) in blindbit-rs; BR12 README `host:port`
  wording and Rust 1.88 minimum.
<!-- /autoplan-accepted:ceo -->

<!-- autoplan-accepted:design -->
- UI states (design): friglet-ui keeps one state machine whose ids drive the status page, the
  Umbrel widget and the StartOS "Scan" health check: `needs-setup`, `starting`,
  `verifying-identity`, `identity-mismatch`, `scanning`, `up-to-date`, `oracle-unreachable`,
  `oracle-dropped-resuming`, `peer-unreachable`, `node-pruned`, `node-syncing`, `port-in-use`,
  `rescanning-after-reset`, `crash-loop`, `unknown-exit`, plus a browser-side
  `ui-disconnected` banner. Each has a headline, an offered action and a widget/health value.
  The widget exposes only the state id, scanned height and tip height; error states read
  "Needs attention". Fault-injection tests render every state from real pinned-friglet output.
- Screens: Setup (first run only), then Status, Connect, Wallet, Diagnostics as plain links
  with `aria-current`. Status order: state headline with overall progress, wallet identity,
  one next action, then details (payments found, birthday with date, network, node peer,
  oracle marked third-party, versions, restarts). No balance until BR3 lands.
- Progress is (scanned − birthday) / (tip − birthday), computed by friglet-ui; the ETA appears
  only after 60 s of steady throughput, is labelled "about" and hides during stalls and restarts.
  A falling scanned height shows `rescanning-after-reset`.
- Birthday step: the user answers "has this wallet received payments before?" (never / yes, I
  know the height / yes, not sure). "Never" uses the node tip from the P2P `version`
  handshake; "not sure" uses taproot activation (mainnet 709,632); date estimates are rounded
  down by 2016 blocks; every height is shown with a date; copy says "If unsure, pick earlier";
  heights outside taproot activation .. node tip are rejected. When up to date with nothing
  found the page says "Scanned X–Y, no payments found" and links to changing the birthday.
- The same handshake reads service bits: NODE_NETWORK_LIMITED without NODE_NETWORK blocks
  Submit with `node-pruned` before any oracle contact; a `notfound` from a full node is shown
  as `node-syncing`.
- M0 verification: whether Sparrow requires friglet's `start_height` to equal the birthday
  Sparrow subscribes with (`friglet/src/types.rs:66-71`); if so, the birthday step asks for
  the birthday height shown in Sparrow and drops the date estimate.
- Descriptor field: masked with a reveal toggle, autocomplete and spellcheck off, inline
  checksum and network validation, never shown again after saving (address and a short key
  fingerprint are shown instead); hex entry sits behind a disclosure. An oracle disclosure line
  sits directly above Submit and in the StartOS action description.
- Identity mismatch: stop friglet, show both full addresses, offer re-entry or "start fresh";
  if friglet reports no address within 5 minutes, say so and link Diagnostics.
- Connect page order: (1) dedicated Sparrow instance, with tested per-OS launch commands for the
  pinned Sparrow version and the sentence "Other wallets are unaffected; switch Sparrow back to
  your old server to see them again"; (2) one recommended endpoint for this platform with copy;
  (3) other endpoints collapsed; (4) who can read this port, in plain words; (5) "Test from this
  node", worded as "Electrum answers on this node" with laptop-side troubleshooting below.
  Endpoints per platform: Umbrel LAN `host:50031:t` recommended, onion `:t`; StartOS the
  `ssl://` address from the Interfaces tab with root-CA steps; RaspiBlitz `:50032:s` recommended,
  `:50031:t`, onion; myNode `:50031:t`, onion only on Premium; compose `:50031:t`. No QR code
  (taste TD4). The dedicated-instance note also appears in store copy and READMEs before install.
- While scanning, Connect is allowed once identity is verified, with "Sparrow shows payments up
  to block X; more appear as scanning continues" (M0 verifies friglet serves partial history).
- Wallet change is its own flow: confirmation naming current and new wallet and the rescan,
  durable progress across reloads, named failure step with retry or restore. `.prev` is
  deleted once the new identity is verified and the UI says so. "Forget wallet" deletes config,
  key, state and `.prev` and returns to setup; each README says what uninstall removes.
- Platform setup paths: StartOS runs `friglet-ui setup` from its action (same parser, birthday
  rules and identity check) and the web UI's "Change wallet" points to that action; RaspiBlitz
  SSH menu shows URL, password, status and password reset only.
- Login on non-gateway platforms: compose refuses to start without `FRIGLET_UI_PASSWORD`;
  RaspiBlitz and myNode generate a password at install and show it on an already-authenticated
  platform surface (SSH menu; myNode app page); never set on first browser visit; HttpOnly,
  SameSite=Strict session cookie for 12 h; 5 login attempts per minute.
- Oracle URL: read-only by default, editable under Advanced on every platform, validated as an
  https URL and restarted on change.
- Visual system (taste TD3): CSS variables with the tray's dark palette plus a light theme via
  `prefers-color-scheme`; IBM Plex Sans and Mono embedded; 16 px body; spacing 4/8/12/16/24/32;
  radius 6 px; 2 px accent focus ring at ≥ 3:1; tabular numerals; one reading column, no stacked
  cards; recorded in `friglet-ui/DESIGN.md`.
- Responsive and accessibility: works from 320 px with no horizontal scroll at 200 % zoom; reading
  width capped at 760 px; addresses and hosts wrap; 44 px touch targets; nothing hover-only;
  landmarks; native `<progress>`; one polite live region for state changes only; field errors
  tied by `aria-describedby` with focus on the first error; contrast 4.5:1 text and 3:1 controls;
  reduced motion respected; clipboard failure falls back to selected text.
<!-- /autoplan-accepted:design -->

<!-- autoplan-accepted:dx -->
- Time-to-hello-world targets (DX): Umbrel install to "verified, connected, scanning" under 10
  minutes, timed on hardware at M1; first payment visible within minutes for a recent birthday;
  contributor fresh clone to the regtest UI showing a found payment under 10 minutes through
  `make dev-regtest` (testkit T2 with the local image; prints the UI URL and a regtest
  descriptor generated at run time). Prerequisites are stated before install: supported
  platform, synced unpruned node, pinned Sparrow version. Timings are recorded in
  `docs/performance.md`.
- Ports and paths (verified free only by grep of the pulled trees): Electrum 50031, TLS 50032
  (RaspiBlitz nginx stream), friglet HTTP `127.0.0.1:8095`, friglet-ui 8097 (container; loopback
  on native installs), RaspiBlitz and myNode HTTPS for friglet-ui 8098 through nginx, Umbrel
  manifest port 2104, RaspiBlitz P2P whitebind `download@127.0.0.1:8336`; control socket, config,
  key and state files under the platform data dir (`/data` in the image). Each README carries the
  table for its platform.
- Environment isolation: friglet-ui starts friglet with a cleared environment plus an allowlist
  (PATH, HOME, TZ) and an explicit `--config`, and refuses to start, naming the variable, when any
  `FRIGLET_*` variable other than `FRIGLET_UI_*` is set. Unit-tested.
- `friglet-ui setup` contract: descriptor only from stdin or `--descriptor-file` (mode 0600
  checked), never argv or env; `--network`, `--node host:port`, `--birthday <height|tip|taproot>`;
  `--dry-run` validates only; an existing different wallet needs `--replace`; the same
  descriptor and birthday is a no-op; `--json` result; exit codes 0 ok, 2 usage, 3 descriptor,
  4 birthday, 5 network mismatch, 6 node check, 7 wallet exists, 8 I/O. Used by StartOS and by
  headless compose.
- Compose: `FRIGLET_UI_NODE` (`host:port`, resolved to a literal IP on every start and shown in
  the UI; resolution failure is a named state), `FRIGLET_UI_NETWORK`, required
  `FRIGLET_UI_PASSWORD`; `FRIGLET_UI_AUTH=none` is allowed for users with their own auth in
  front, with a startup warning and a permanent UI banner (taste TD5).
- Escape hatches under Advanced: node peer override (checked by a `version` handshake before
  saving), oracle URL (must answer before saving; warning when it differs from the network
  default), Electrum exposure LAN / onion only / localhost where the platform allows it (not
  on Umbrel, where the published port is fixed; documented), pause scanning (`paused` state),
  friglet log level. `docker run --entrypoint friglet` is documented as the full bypass.
- One state catalog file (`friglet-ui/states.toml`): id, detection evidence, cause, fix per
  platform, docs anchor. UI copy, widget text, StartOS health text and `docs/troubleshooting.md`
  are generated from it and CI fails on drift. `notfound` maps to `node-syncing` only when the
  handshake height is below the requested block, otherwise to "node lacks block H".
  Permanent classes (port-in-use, node-pruned, identity-mismatch, config write errors) stop
  automatic retries until the user acts or the config changes; transient classes back off to
  5 minutes, and the page shows the next retry time. The last failure survives restarts.
- Exact refusal texts with problem, cause, fix and docs anchor for: pruned node (RaspiBlitz and
  myNode install, all platforms at Submit), missing UI password, stray `FRIGLET_*` variable,
  wallet exists without `--replace`. friglet's own "set FRIGLET_X or pass --flag" advice is
  rewritten for friglet-ui users.
- Birthday bounds per network come from the pinned bitcoin crate's taproot activation heights
  (mainnet 709,632); regtest "not sure" means 0; birthday equal to tip shows `up-to-date`
  immediately; wording is "caught up with your node".
- StartOS lifecycle: the critical task's action runs `friglet-ui setup` in a one-off subcontainer
  while the service is stopped; the result says identity is checked once the service starts;
  `main` runs friglet-ui as the daemon, and the "Scan" health check shows `identity-mismatch`
  and the other states. Action timeout 60 s; a failed action leaves the old config untouched.
- Wallet replacement is a durable transaction (supersedes the design block's `.prev` timing):
  friglet-ui writes an intent file first, then stops friglet, snapshots config, key and state
  into `prev/` (0700), writes the new set, starts, verifies identity and waits until the first
  scanned range is persisted. `prev/` is deleted when the user confirms or 24 h after verified
  progress, and the UI says so. On boot an unfinished intent is resumed or rolled back;
  "Restore previous wallet" restores the whole snapshot.
- Upgrades: before starting a different friglet version, friglet-ui stops friglet cleanly and
  copies the state file to a 0600 backup tagged with the old version; a height reset after
  upgrade shows "this update required a rescan". READMEs give rollback steps (re-pin the
  previous digest, restore the backup). Package versions are `<friglet version or short
  commit>` plus a package revision (StartOS ExVer `x.y.z:N`). CI tests upgrade from the previous
  release on compose. Stop deadlines cover the 120 s grace: compose `stop_grace_period: 150s`,
  systemd `TimeoutStopSec=150`, StartOS daemon sigterm timeout 150 s.
- Per-platform acceptance checks before "install-tested": first install, restart, upgrade,
  backup and restore, uninstall and reinstall; on RaspiBlitz, repeated non-interactive `on`
  keeps the wallet and the UI password.
- Docs deliverables at M0: root README routing operators and contributors; compose quickstart
  run verbatim by CI; generated troubleshooting page; CONTRIBUTING (bootstrap, build, test,
  `make dev-regtest`, pin update via one script that rewrites every reference and lockfile,
  reviewer-response flow for each platform); a "try on signet" path; where raw logs live per
  platform with a warning that they contain txids, balances and IPs until BR8; a comparison
  table against the public Frigate server, Frigate on StartOS and Frigate on MiniBolt covering
  setup effort, dependencies, time to useful history, wallet limits and who sees what.
- Connect page copy matches the pinned Sparrow server dialog (per-field copy for host, port and
  SSL, plus the combined string), verified in M0.
<!-- /autoplan-accepted:dx -->

<!-- autoplan-accepted:eng -->
- Scan-health supervision (eng): besides process exits, friglet-ui polls `GetStatus`; when
  `scanning` is false with `last_error` set and the user has not paused, it classifies the error
  and sends `Start` with jittered exponential backoff (cap 5 min); a `scan-stopped` state joins
  the catalog. "Pause scanning" uses `Stop`/`Start`. `Shutdown` is preferred over SIGTERM.
  Error classification uses `last_error` first and log lines only for failures before the
  socket exists and for panics (BR4 narrows to: report pre-socket failures, version the IPC).
- State model: four inputs (desired operation, process lifecycle, scan condition, wallet
  transaction) with explicit precedence produce the presentation state ids; events carry a
  daemon generation id so stale events are dropped; transition tests cover the precedence.
- Electrum front: friglet binds Electrum to `127.0.0.1:50001` inside its namespace;
  friglet-ui owns the external listeners: TLS on 50032 by default on every platform where the
  platform does not terminate TLS itself (self-signed certificate generated at install,
  fingerprint shown on the Connect page), plaintext 50031 only for the onion, the StartOS or
  nginx proxy, or an explicit Advanced opt-in. Listeners open only after identity is verified
  and client connections are closed during a wallet change. This revises the CEO exposure
  policy (taste TD2): the recommended LAN endpoint becomes TLS on every platform. M0 confirms
  by packet capture whether the pinned Sparrow sends the scan private key in
  `blockchain.silentpayments.subscribe`; the TLS default stays either way.
- BR1 (key-checked subscribe) now also gates the community-store announcement, not only
  official catalogs; no duplicate key check is built into friglet-ui.
- Birthday: the setup field takes the birthday height the user's Sparrow wallet uses, because
  friglet returns its configured `start_height` and Sparrow rejects a different one
  (`friglet/src/types.rs:66-69`). This supersedes the tip, taproot and date-estimate helpers in
  the CEO and design blocks for M0; M0 records where Sparrow shows that height and re-enables a
  helper only if Sparrow tolerates a different start. "Change wallet" also tells the user to
  change Sparrow's birthday. Taproot activation heights come from a tested table in friglet-ui
  (mainnet 709,632; signet and regtest active from genesis), not from rust-bitcoin.
- Heights: node tip (P2P handshake, refreshed every 10 minutes), oracle tip, scanned height and
  persisted height are tracked separately with timestamps; progress uses min(node tip, oracle
  tip); `node-syncing` shows when the node is behind the oracle; completion needs real scan
  coverage including the birthday block (supersedes "birthday equal to tip is up to date").
- Pruned and syncing detection does not rely on `notfound`: Bitcoin Core disconnects or stays
  silent, which friglet reports as a closed connection or a 90 s timeout; those signals plus
  handshake height and service bits (rechecked on every start) map to `node-pruned` and
  `node-syncing`. Tested on regtest with `-prune -fastprune` and with a lagging node.
- One transaction owner: a shared friglet-ui library performs setup and wallet replacement;
  the online CLI submits requests to the running supervisor; StartOS stages offline under an
  exclusive `flock` on the data dir and reports "staged, verification pending". Before
  `prev/` is deleted, friglet-ui performs a clean `Shutdown`, checks the state file parses and
  matches the new identity, fsyncs file and directory, and restarts. Kill -9 tests run at every
  step; concurrent writers are refused. BR14 asks friglet for a persisted-checkpoint
  acknowledgement.
- Last-known-good state: at each clean stop friglet-ui keeps one copy of the state file and
  restores it when a height reset after an unclean stop is detected; otherwise the page shows
  that a rescan started.
- Secret-bearing file inventory, owned by one module: `scan.key`, `config.toml`, state file,
  last-known-good copy, `prev/`, the transaction intent file, the single upgrade backup, plus
  platform backups (disclosed, not deleted). Forget and uninstall delete every local item; a
  test greps the data dir, logs, diagnostics and StartOS action results for the key afterwards.
  friglet is spawned with umask 077 and the data dir is 0700.
- Ingress: friglet-ui binds only where the platform's ingress reaches it; every state-changing
  request needs a Host/Origin allowlist match and a CSRF token on every platform, including
  `FRIGLET_UI_AUTH=none`; the widget endpoint is read-only with the fixed schema. StartOS uses
  `addSsl.auth` basic auth on the `ui` binding with a generated password in `store.json` and an
  action to show or rotate it. Umbrel relies on its gateway login (taste TD6; other apps on the
  Umbrel network can reach the backend, which matches Umbrel's existing trust in installed
  apps). Behind nginx the login limit is per client, trusting `X-Forwarded-For` only from local
  nginx; passwords are stored as argon2 hashes, compared in constant time, and a reset ends all
  sessions. Acceptance tests try direct backend access and unauthenticated mutations.
- Process hygiene: the image entrypoint is `tini -- friglet-ui`; friglet-ui never calls
  `SetConfig`; the node peer is re-resolved on every friglet start on all platforms and after
  repeated `peer-unreachable`; a CI test checks `docker stop` finishes inside the 150 s grace
  while friglet is blocked in a P2P read.
- Logs: forwarded friglet lines are redacted (txids, amounts, keys, IPs) on a best-effort basis
  until BR8; the classifier reads an in-memory ring of raw lines; golden-log fixtures per pin
  fail CI when a catalogued line disappears.
- Native installs: the Electrum exposure setting on RaspiBlitz and myNode is changed through
  the SSH menu or bonus script (ufw, nginx stream, Tor need root) and shown read-only in the web
  UI; friglet and friglet-ui ship as one tarball with one checksum.
- Release CI refuses `BLINDBIT_RS_SRC` builds. Behavioral tests run against the final re-pinned
  artifact: confirmed spend, reorg rollback, scanner death with a live process, disk full at
  config write and at checkpoint, kill -9 during wallet change, upgrade then rollback, pruned
  and lagging node, oracle down at boot and mid-scan with jitter, PID 1 stop timing, arm64
  execution. Worst-case scan from taproot activation is measured and published.
- Phasing of friglet-ui inside the milestones (nothing cut): M0 delivers setup, process and
  scan supervision, status, Connect, auth and the Electrum front; M1 adds the Umbrel widget,
  diagnostics, upgrade backup, last-known-good restore and the Advanced escape hatches.
- New blindbit-rs tickets: BR13 switch off the HTTP API (unauthenticated `/subscribe` on
  loopback); BR14 persisted-checkpoint acknowledgement with write-failure propagation.
<!-- /autoplan-accepted:eng -->
## Review record

All content below this heading is review output from gstack `/autoplan`, run non-interactively
on 2026-10-01. Every decision here was **auto-decided by the review** using autoplan's six
principles and is not confirmed by the maintainer. Items that need a human are collected in the
GSTACK REVIEW REPORT at the end.

### Phase 0: intake

- Restore point: `~/.gstack/projects/blindbit-node-apps/integration-autoplan-restore-20261001-122532.md`.
- Repo: `blindbit-node-apps`, branch `integration`, no remote; base branch `master` (local).
  The plan is untracked. No `CLAUDE.md` or `TODOS.md`; `AGENTS.md` read.
- UI scope: **yes** (setup form, status and connection pages; "layout" was a false positive).
  DX scope: **yes** (14 term matches; also a packaging repo other maintainers build on).
- No design doc. The `/office-hours` offer was auto-skipped (P6): the plan already rests on
  five research digests.
- Outside voice: Codex CLI `ready` (codex-cli 0.157.1) for every phase.
- Facts were re-verified against blindbit-rs @ `611a877` by a read-only subagent; the
  corrections are folded into "Facts the design rests on" above.

### Phase 1: CEO review (mode: SELECTIVE EXPANSION)

#### System audit

- Two commits on `integration` (init, pulled upstream material); `docs/` untracked. No TODO or
  FIXME outside `upstream/`. No prior reviews, CEO plans, learnings or design docs.
- Taste references: `upstream/startos/NOTES.md` and `upstream/umbrel/NOTES.md` cite every rule
  to a pulled file and mark PROPOSAL/UNVERIFIED; the plan should keep that discipline.
  Anti-pattern to avoid: friglet's own Dockerfile comment that drifted from the code (state file).
- Landscape (WebSearch; Aside not installed): Sparrow 2.5.0 ships Silent Payments receive
  wallets and auto-selects the public Frigate server `frigate.2140.dev`, which receives the scan
  key. Frigate is packaged for StartOS and MiniBolt and needs Fulcrum/electrs, `txindex`, a large
  index and hours of indexing.
  - Layer 1: Electrum servers on node boxes are packaged the electrs way (raw TCP port, Tor
    sidecar, connection page).
  - Layer 3: friglet's real edge is that the scan key never leaves the box and no Electrum index
    is needed; that edge only holds if the port does not leak the wallet and the numbers are
    measured.

#### 0A. Premise challenge

| # | Premise in the plan | Verdict | Evidence | Resolution |
|---|---|---|---|---|
| P1 | Sparrow points at friglet and nothing else breaks | **Challenged** | friglet is single-wallet; unknown scripthashes get empty history (`friglet/src/electrum/mod.rs:651-700`); Sparrow has one server for all wallets | Dedicated Sparrow instance documented; forwarding is ticket BR2; gate item |
| P2 | Node must be unpruned | Refined | Only candidate blocks are fetched, from one peer; pruned peer answers `notfound` (`p2p.rs:201-208`) | Keep unpruned as the supported config; enforce per platform; name the error |
| P3 | friglet needs `config.toml`, and config changes need a restart | Partly wrong | CLI/env/file precedence; `SetConfig`/`SetScanKey` exist but Electrum keeps the old wallet and `SetScanKey` is ignored for an existing state (`control/mod.rs:10-17,380-390`) | Wallet change = stop, fresh state, restart; supervisor still required |
| P4 | Public oracle default, shown in the UI | Accepted with conditions | Default URL is mainnet for every network (`friglet-ipc/src/lib.rs:72`); needed at startup; clearnet only | Explicit per-network `oracle_url`; disclosure; SOCKS ticket BR5 |
| P5 | Local oracle blocked because stock platforms ship Core 29.x | **Wrong** | Umbrel offers 31.1/30.3 (`umbrel-apps/bitcoin/umbrel-app.yml:47`), StartOS 31.1, RaspiBlitz recommends 30.2; only myNode is 29.3 | Open item corrected; stays deferred by owner's non-goal; gate question |
| P6 | friglet at the pinned commit is fit for users | **Wrong** for any public release | Spend detection broken (`scanner.rs:399-404`), no reorg handling, oracle-stream panic, non-atomic state writes; fixes on unmerged branches | Release gate on BR3 merged and re-pinned; gate item |
| P7 | "Plaintext and unauthenticated" disclosure is enough for the Electrum port | **Challenged** | Any caller gets address, history, tweaks, raw txs (`electrum/mod.rs:773-812`) | Packaging-side exposure policy plus ticket BR1 gating official catalogs (taste TD2) |
| P8 | One image; friglet-ui is PID 1 and supervisor | Accepted | friglet cannot run unconfigured; exits 1 / 0 / 134 on oracle, bind and stream failures | Supervisor restarts on any exit |
| P9 | friglet-ui in Go | Challenged | `friglet-ipc` is a light Rust crate; protocol is unversioned; tray UI is reusable | Rust (taste TD1) |
| P10 | Four platforms delivered together | Refined | myNode review took ~3.5 months for Canary; StartOS has no emulator | Per-platform milestones; nothing cut |

#### 0B. Existing code leverage

| Sub-problem | Existing code | Decision |
|---|---|---|
| Control-socket client, status and config types | `blindbit-rs/friglet-ipc` (NDJSON over a Unix socket, `lib.rs:96-162`) | Reuse as a crate at the pinned commit |
| Input validation, atomic `config.toml` and key write | `friglet-tray/src/setup.rs:38-162` | Port the logic (crate depends on Tauri), MIT attribution |
| Spawn, socket wait, `Shutdown` | `friglet-tray/src/lifecycle.rs:161-341` | Port; add auto-restart, which the tray lacks |
| Settings and status UI | `friglet-tray/ui/{index.html,main.js,style.css}` (only needs `window.__TAURI__.core.invoke`) | Reuse with a fetch shim, so desktop and node look the same |
| `sp(spscan1…)` descriptor parsing | None in blindbit-rs or go-bip352 | New in friglet-ui; ticket BR10 moves it to blindbit-lib for the tray |
| End-to-end harness with known payments | blindbit-testkit T2 (`compose/`, `scenarios/plain-payment.toml`) | Reuse as the CI end-to-end gate |
| Container recipe without the state-file bug | testkit `compose/friglet/Dockerfile` (sets `FRIGLET_STATE_FILE`, `KEY_FILE`, `CONTROL_SOCKET`) | Model the image on it |
| Health probe | testkit `compose/friglet/healthcheck.sh` (`blockchain.headers.subscribe`) | Reuse the idea |
| Platform patterns | Umbrel electrs (Tor sidecar, `exports.sh`, `torrc.template`, `pre-start`); StartOS frigate/electrs; RaspiBlitz fulcrum signed-release install; myNode Canary | Model, never import from `upstream/` |

#### 0C. Dream state

```
  CURRENT                         THIS PLAN                          12-MONTH IDEAL
  friglet runs by hand (CLI/env)  one image + glibc 2.36 binaries;   friglet in the official Umbrel and
  or via the desktop tray; no     friglet-ui setup/status; Umbrel    StartOS catalogs, RaspiBlitz and
  image, no release. Sparrow      community store, StartOS s9pk,     myNode; forwards non-SP calls to
  users hand their scan key to    RaspiBlitz bonus script, myNode    the box's electrs/Fulcrum so one
  frigate.2140.dev or run         app, compose; testkit e2e proof;   Sparrow serves every wallet;
  Frigate (Fulcrum + big index)   daemon gaps filed as BR tickets    key-checked subscribe; local or
                                                                     Tor-reached oracle; releases cut
                                                                     and signed in blindbit-rs
```

Delta: the plan builds the packaging foundation. What remains to the ideal is daemon-side
(BR1, BR2, BR5, BR11) plus catalog acceptance, which needs human submissions.

#### 0C-bis. Implementation alternatives

```
APPROACH A: Plan as written (Go friglet-ui)
  Summary: Go static binary with its own NDJSON client, TOML writer and descriptor parser.
  Effort: M   Risk: Med
  Pros: fast builds; tiny binary; stdlib HTTP server.
  Cons: hand-copied types against an unversioned protocol; a second parser language the
        tray cannot share; two toolchains in CI.
  Reuses: platform patterns only.

APPROACH B: Minimal viable (no friglet-ui)
  Summary: env-configured friglet; StartOS action, compose env and RaspiBlitz SSH dialog only.
  Effort: S   Risk: High
  Pros: least code; no supervisor.
  Cons: Umbrel requires browser setup and myNode has no settings UI, so two of four platforms
        fail; the scan secret would sit in env, which AGENTS.md forbids.
  Reuses: friglet env loader.

APPROACH C: Rust friglet-ui on friglet-ipc (ideal architecture)
  Summary: Rust binary in this repo's workspace; `friglet-ipc` crate at the pinned commit;
           tray UI reused through a fetch shim; parser later moved to blindbit-lib; thin
           supervisor that can be deleted once friglet starts unconfigured.
  Effort: M   Risk: Low-Med
  Pros: status/config types exact; one toolchain (cargo zigbuild) for both binaries; parser
        and UI shared with the tray roadmap.
  Cons: longer compiles; larger binary (est. 5-8 MB); Rust HTTP stack (axum, already in
        friglet's tree).
  Reuses: friglet-ipc, friglet-tray ui/setup/lifecycle, testkit T2.
```

RECOMMENDATION: C, because DRY and exact types matter more than build speed when the protocol
is unversioned and `SetConfig` fills missing fields with defaults. Auto-decided as **taste TD1**
(A stays viable).

#### 0F. Mode

SELECTIVE EXPANSION (autoplan override for CEO phase).

#### 0D. Selective expansion analysis

- Complexity: five platform folders plus friglet-ui, image, CI. That is inherent in the owner's
  four-platform scope; nothing can be cut without cutting a platform.
- Minimum set for the core outcome: image + friglet-ui + compose. Deferrable without blocking:
  RaspiBlitz WebUI PRs (blitz_api, raspiblitz-web), official Umbrel PR, myNode PR review.
- 10x version: "Silent Payments on every node box" where the box's one Electrum endpoint serves
  every Sparrow wallet and nothing leaves the box. Daemon-side; filed as BR2 and BR5.
- Delight candidates, each auto-decided (P2: inside friglet-ui, under a day each):

| # | Candidate | Effort | Decision |
|---|---|---|---|
| D1 | Umbrel home-screen widget (state, scanned height, tip) | S | ACCEPTED |
| D2 | Birthday helper: "new wallet, use current tip" plus a date-to-height estimate | S | ACCEPTED |
| D3 | "Test connection" button: `server.version` against the advertised address | S | ACCEPTED |
| D4 | Redacted diagnostics download | S | ACCEPTED |
| D5 | Show descriptor-derived SP address next to friglet's `sp_address`; refuse "done" on mismatch | S | ACCEPTED |
| D6 | Catch-up ETA from measured blocks per second | S | ACCEPTED |
| X1 | Local oracle companion app (Umbrel, StartOS) | L | DEFERRED (owner non-goal) |
| X2 | BigBear universal app, MiniBolt/RaspiBolt guides | M | DEFERRED (survey only) |
| X3 | Electrum forwarding, key-checked subscribe, SOCKS | L | DEFERRED to blindbit-rs tickets |

CEO plan written to `~/.gstack/projects/blindbit-node-apps/ceo-plans/2026-10-01-friglet-node-apps.md`.

#### 0E. Temporal interrogation

```
HOUR 1 (foundations): which blindbit-rs commit to pin. 611a877 lacks spend detection, reorg
  and stream fixes -> pin the integration head at build time, record it in versions.env, and
  gate release on BR3.
HOUR 2-3 (core logic): descriptor grammar (hrp spscan/tspscan, checksum, reject spspend);
  which config keys friglet-ui owns vs platform env (env beats file; the image must not preset
  FRIGLET_* keys the UI writes); wallet-change sequence (stop, fresh state, start, verify).
HOUR 4-5 (integration): Umbrel Tor sidecar IP; StartOS bridge address; RaspiBlitz whitebind
  port and bitcoind restart; myNode uid mapping; stop grace of 120 s everywhere.
HOUR 6+ (tests): testkit T2 with the packaged image; failure injection (oracle down, pruned
  peer, exit 134); arm64 execution, not just `file`.
```
(Human-team hours; with CC each block is roughly 5-10 minutes. Decisions are the same.)

#### Dual voices

Claude subagent (INPUT ceo `1cdf87c8…49ab`, matched): 1 critical, 6 high, 4 medium.
- C1 critical: single-server mode silently empties the user's other Sparrow wallets.
- H1 Frigate positioning missing; H2 Core-29.x premise wrong, local oracle feasible;
  H3 friglet-ui duplicates friglet-tray; H4 the port serves the wallet; H5 four platforms before
  the first real user; H6 public repo and `upstream/` licensing blocks the Umbrel store.
- M1 unpruned may be overbroad; M2 no success metric; M3 "friglet" reads as "Frigate lite",
  app ids are permanent; M4 cheaper reach (BigBear, guides) and registry/signing undecided.

Codex (completed, `Recommendation: Revise before implementation`): 1 critical, 6 high, 1 medium.
- Critical: defaults expose the wallet's history; UI auth does not protect Electrum.
- High: release checks can pass without Sparrow finding a payment; wallet replacement keeps the
  old scan secret in state; oracle has no operating plan and is needed at startup; unpruned is a
  note, not an eligibility gate; no demonstrated reason to choose friglet over Frigate; artifact
  completion is confused with user availability.
- Medium: release ownership, signing and rollback undecided.

```
CEO DUAL VOICES — CONSENSUS TABLE:
═══════════════════════════════════════════════════════════════
  Dimension                            Claude  Codex  Consensus
  ──────────────────────────────────── ─────── ─────── ─────────
  1. Premises valid?                   No      No     CONFIRMED (premises need fixing)
  2. Right problem to solve?           Partly  Partly CONFIRMED (differentiation unproven)
  3. Scope calibration correct?        No      No     CONFIRMED (sequence, do not cut)
  4. Alternatives sufficiently explored?No     n/a    DISAGREE (Codex silent on architecture)
  5. Competitive/market risks covered? No      No     CONFIRMED (Frigate)
  6. 6-month trajectory sound?         No      No     CONFIRMED (wallet-history exposure)
═══════════════════════════════════════════════════════════════
```
Single-voice critical: C1 (Sparrow single server), Claude only; verified in code, kept.
Cross-model tension: the Electrum exposure fix. Claude wants a friglet key-match guard; Codex
wants packaging-side access controls and says no friglet change is needed. Recorded as taste TD2.
No user challenge: Codex keeps the oracle deferred and friglet unpatched, so the two voices do not
agree on changing the owner's direction.

#### Section 1: Architecture

```
  browser ──(platform login, or friglet-ui password)──▶ friglet-ui (HTTP, UI port)
                                                        │ setup / status / connection
  ┌──────────── friglet-node image (Docker) or native install (RaspiBlitz) ────────────┐
  │ friglet-ui (PID 1, or systemd main process)                                         │
  │   setup: parse sp(spscan1…) ─▶ /data/config.toml + /data/scan.key (0600)            │
  │   supervisor: spawn friglet, restart on any exit, SIGTERM + 120 s grace             │
  │   status: GetStatus over /data/friglet.sock (NDJSON) + classified log lines         │
  │          │ spawn                       ▲ unix socket                                │
  │          ▼                             │                                            │
  │ friglet ──gRPC/TLS, clearnet──▶ oracle (oracle.setor.dev / signet.oracle.setor.dev) │
  │    ├──P2P IP:port──▶ box bitcoind (whitebind / peer-local)                         │
  │    └──Electrum TCP :50031──▶ Sparrow (LAN; onion; StartOS/RaspiBlitz TLS in front)  │
  │ /data: config.toml, scan.key, scanner_state.json (holds the secret),               │
  │        scanner_state.headers.json, friglet.sock                                     │
  └─────────────────────────────────────────────────────────────────────────────────────┘
```
- Coupling added: friglet-ui to the unversioned IPC protocol (pin the same commit; check
  `GetStatus.version`); to oracle availability; to each platform's env contract.
- Single points of failure: oracle (startup and scanning), the one P2P peer, friglet-ui itself.
- Scaling: one wallet per instance; 10x wallets means 10 instances, a non-goal.
- Rollback: re-pin the previous digest or binary; state compatibility across friglet versions is
  unknown, so a downgrade may force a rescan (documented, tested in Section 9).
- Findings, all auto-decided: 1.1 supervisor restarts on any exit (accepted); 1.2 the image
  must not preset `FRIGLET_*` env for keys friglet-ui writes, because env beats the file
  (accepted); 1.3 pin friglet-ipc to friglet's commit and refuse an unexpected `version`
  (accepted); 1.4 Electrum exposure policy (taste TD2).

#### Section 2: Error and rescue map

```
  CODEPATH                    | WHAT CAN GO WRONG                         | ERROR KIND
  ----------------------------|-------------------------------------------|---------------------------
  setup: parse descriptor     | bad checksum, bad hrp, spspend pasted,    | DescriptorError::{Checksum,
                              | network mismatch (tspscan on mainnet),    |  Hrp, SpendSecret, Network,
                              | empty / oversized input                   |  Malformed}
  setup: validate birthday    | not a number, in the future, below        | BirthdayError::{Parse,
                              | taproot activation (mainnet 709632)       |  Future, TooEarly}
  setup: write files          | disk full, permission denied, partial     | PersistError::{Io, Perm}
  supervisor: spawn           | binary missing, exec denied               | SpawnError
  supervisor: child exit      | 1 oracle down at start, 0 bind failure,   | ChildExit{code, class}
                              | 134 oracle stream panic, SIGKILL          |
  status: GetStatus           | socket absent (starting/crashed), EACCES, | IpcError::{Absent, Perm,
                              | malformed or closed reply, 2 s timeout    |  Protocol, Timeout}
  friglet log classifier      | oracle unreachable, peer notfound         | LogClass::{Oracle, Pruned,
                              | (pruned), peer refused, bind in use       |  Peer, Bind}
  wallet check                | descriptor SP address != sp_address       | WalletMismatch
  ----------------------------|-------------------------------------------|---------------------------

  ERROR KIND           | RESCUED? | RESCUE ACTION                          | USER SEES
  ---------------------|----------|----------------------------------------|----------------------------
  DescriptorError::*   | Y        | reject, keep form input except secrets | named reason + fix hint
  BirthdayError::*     | Y        | reject                                 | allowed range
  PersistError::*      | Y        | abort, old config untouched            | "could not save: <reason>"
  SpawnError           | Y        | backoff retry, state Failed            | "friglet binary missing"
  ChildExit{1,Oracle}  | Y        | backoff retry (cap 5 min)              | "oracle unreachable, retrying"
  ChildExit{0|134}     | Y        | restart with backoff; count restarts   | "restarted N times, last: …"
  IpcError::Absent     | Y        | Starting if < 60 s since spawn, else   | "starting…" / "not responding"
                       |          | NotResponding                          |
  IpcError::Perm       | Y        | fatal config error, logged once        | "socket permission denied"
  LogClass::Pruned     | Y        | state Blocked (no restart storm)       | "node lacks blocks from <h>"
  LogClass::Bind       | Y        | state Blocked                          | "port 50031 in use"
  WalletMismatch       | Y        | stop friglet, setup not "done"         | both addresses, re-paste
  state file corrupt   | N ← GAP  | friglet silently rescans (daemon)      | was silent; now "rescan
                       |          | -> friglet-ui detects height reset     |  from birthday started"
```
Catch-alls are not allowed in friglet-ui; every error kind above gets a test (Section 6).

#### Section 3: Security and threat model

| # | Threat | Likelihood | Impact | Mitigation in this plan |
|---|---|---|---|---|
| S1 | Electrum port returns the wallet's history, tweaks and raw txs to any LAN host or onion holder | High | High (privacy) | Exposure policy (taste TD2), in-UI disclosure, onion only shown behind auth; BR1 gates official catalogs |
| S2 | Unauthenticated setup form on RaspiBlitz, myNode, compose lets a LAN host replace the wallet | Med | Med | Mandatory password there; platform login on Umbrel/StartOS |
| S3 | CSRF: a web page POSTs to the setup endpoint with the user's Umbrel cookie | Med | Med | Same-origin check on `Origin`/`Host` plus a per-session token on every state-changing request |
| S4 | User pastes a spend secret (`spspend`) | Med | High | Rejected with a message; never written, never logged |
| S5 | Scan secret at rest: `scan.key`, `scanner_state.json`, platform backups | High (by design) | Med | 0600, friglet uid, absolute paths under the data dir; README states that backups contain the secret; never in env, compose, image or logs |
| S6 | friglet logs balances, txids, owned keys, client IPs; RaspiBlitz debug output is shared publicly | Med | Med | Keep friglet out of `blitz.debug.sh`; diagnostics bundle redacts; BR8 |
| S7 | Oracle operator sees box IP, scanned height ranges and timing | High | Low-Med | Disclosure before setup; BR5 (SOCKS) |
| S8 | Supply chain: image and binaries built here, signing key undecided | Med | High | CI builds from pinned commits, multi-arch digests, signed SHA256SUMS once the key exists (human item) |
| S9 | Control socket reachable by other users | Low | Med | Same uid for both processes; socket in a 0700 directory |
| S10 | Descriptor or log text injected into the status page | Low | Med | Escape all output; length and charset limits on input |

#### Section 4: Data flows and interaction edge cases

```
  SETUP:  INPUT ──▶ VALIDATE ──▶ TRANSFORM ──▶ PERSIST ──▶ APPLY ──▶ VERIFY
          paste      checksum,     scan secret   tmp+fsync+   stop friglet  sp_address ==
          + birthday hrp/network,  hex, spend    rename;      (SIGTERM,     descriptor
                     no spspend,   pubkey hex    old state    120 s), start address?
                     range                       moved aside
            │           │             │             │            │             │
          [empty]    [invalid ->   [n/a]        [disk full ->  [oracle down  [mismatch ->
          [huge]      named error] [ ]           old config     -> Blocked,    stop, not done]
          [spaces]                               kept]          retries]
```
- Ordering: setup requests are serialized through one state machine. A second submit while a
  restart is in flight is rejected with "busy"; the same descriptor submitted twice is a no-op.
  Reconfigure during a scan stops friglet first, then writes, then starts, so no schedule can
  start friglet on a half-written config.
- Interaction edge cases: double submit (no-op), navigate away mid-restart (state survives on the
  server), birthday in the future or before taproot (rejected), descriptor with whitespace or
  upper case (normalized), onion not created yet (`notyetset.onion` shown as "pending"),
  status while friglet is down (last known heights plus state), wallet switch (explicit
  "this rescans from the new birthday" confirmation).

#### Section 5: Code quality

- DRY: reuse `friglet-ipc`, the tray UI and testkit T2 (Section 0B). One `versions.env` feeds every
  platform file through a render step, with a CI check that rendered files are in sync.
- Naming: image `friglet-node`, app id `blindbit-friglet`, binary `friglet-ui`; the store-facing
  name is a human item (Umbrel ids are permanent).
- Under-engineering risks fixed: atomic writes with fsync, PID 1 signal handling and reaping,
  restart on exit code 0. No over-engineering found: no plugin layer, no multi-wallet scaffolding.

#### Section 6: Tests

```
  NEW UX FLOWS: paste descriptor; choose birthday; view status; view connection info; test
    connection; download diagnostics; change wallet; enter password (non-gateway platforms)
  NEW DATA FLOWS: descriptor -> config/key files; GetStatus -> status JSON -> page/widget;
    friglet stderr -> classifier -> UI state
  NEW CODEPATHS: supervisor states and backoff; log classifier; platform env -> config render
  NEW EXTERNAL CALLS: oracle (friglet), P2P peer (friglet), control socket (friglet-ui)
  NEW ERROR PATHS: every row of the Section 2 table
```
- Unit: descriptor vectors (valid mainnet/signet, bad checksum, `spspend`, wrong hrp, labels if
  the grammar has them), birthday bounds, atomic writer, classifier lines, state machine.
- Integration: supervisor against a fake friglet (`friglet-tray/examples/fake-daemon.rs`) that
  exits 0, 1, 134 and hangs on SIGTERM.
- End to end (the release gate): testkit T2 with the packaged image finds every constructed
  payment and keeps history across restart; Sparrow receive/spend/restart on a real Umbrel.
- Chaos: kill the oracle mid-scan; stop bitcoind; fill the disk; SIGKILL during a state write.
- Flakiness: the public signet oracle and peer are external, so they are a manual smoke only.

#### Section 7: Performance

- friglet-ui budget (to measure and record): RSS under 30 MB, idle CPU under 1 %, status poll
  every 2 s at most one in flight (GetStatus can take 2 s itself).
- Daemon numbers are unknown beyond about 100 blocks/s on signet (`blindbit-rs/AGENTS.md:28`);
  matching is single-threaded and every candidate block opens a new P2P connection.
- Merge bar: before any public release, measure on arm64 hardware the time to first balance and
  peak RSS for a one-year mainnet birthday, plus oracle bytes transferred, and publish them next
  to Frigate's documented requirements. Without numbers the "lighter than Frigate" claim is not
  made.

#### Section 8: Observability

- friglet-ui logs one structured line per state transition and per restart (exit code, class,
  backoff), never secrets or wallet data.
- Status page and widget show state, scanned and tip heights, restarts, last classified error.
- StartOS health checks: Electrum port listening (ready) and friglet-ui `/api/status` for sync
  progress, so no log scraping in the package.
- Runbook per platform in its README: oracle down, node pruned, port in use, wallet mismatch.

#### Section 9: Deployment and rollout

- Release flow: tag, CI builds binaries and a multi-arch image, pushes, records digests, renders
  platform files from `versions.env`, commits the pins, lints each platform, then install tests.
- The Umbrel community store goes live within 5 minutes of a push to the default branch, so
  `master` is the release channel and only moves after maintainer review (matches AGENTS.md).
- Rollback: re-pin the previous digest per platform; document that a friglet downgrade may force a
  rescan; rehearse upgrade then rollback once on compose before the first public release.
- Per-platform milestones (accepted): built, install-tested, submission-ready, user-installable.

#### Section 10: Long-term trajectory

- Debt introduced: the supervisor (delete when friglet starts unconfigured, BR4), the descriptor
  parser (move to blindbit-lib, BR10), release builds here (move to blindbit-rs, BR11).
- Reversibility: 4/5, except Umbrel app ids and StartOS package ids, which are one-way doors.
- After this ships: forwarding (BR2) turns friglet into the box's single Electrum endpoint; a local
  oracle (X1) removes the last external dependency.

#### Section 11: Design and UX (CEO level)

```
  [password?] ─▶ SETUP (paste descriptor, birthday) ─▶ STARTING ─▶ SCANNING (ETA) ─▶ CAUGHT UP
                      ▲   │ invalid -> inline error        │ oracle/peer error ─▶ BLOCKED (retrying)
                      │   ▼                                ▼
                      └── CHANGE WALLET (confirm rescan) ◀─ CONNECTION page (LAN/onion/QR, test)
```
- First thing the user sees after setup: "is it working" (state + heights), then how to connect.
- Interaction states (loading, empty, error, success, partial) are taken up by the design phase.

#### Required outputs

**NOT in scope** (with rationale):
- Multi-wallet; adopting Sparrow's subscribe keys (owner's onboarding scope).
- Native Electrum TLS inside friglet; platform TLS is used instead (owner's non-goal).
- Bundled or local oracle, even though Core 30+ is now available on Umbrel and StartOS (owner's
  non-goal; gate question).
- Patching friglet; every daemon gap is a BR ticket below.
- BigBear, MiniBolt, RaspiBolt, TrueNAS, Unraid and the other surveyed platforms (survey only).
- RaspiBlitz WebUI PRs to blitz_api and raspiblitz-web (optional second step after the bonus script).
- Official Umbrel PR, StartOS registry email, RaspiBlitz and myNode PRs: outward actions that need
  the maintainer.
- TODOS.md was not written (write access limited to this plan); deferred work is listed here.

**What already exists:** see 0B.

**Dream state delta:** see 0C.

**Error and rescue registry:** see Section 2 (13 error kinds, 1 daemon-side gap now surfaced).

**Failure modes registry:**
```
  CODEPATH              | FAILURE MODE                         | RESCUED? | TEST? | USER SEES?            | LOGGED?
  ----------------------|--------------------------------------|----------|-------|-----------------------|--------
  Sparrow server choice | other wallets go empty               | N        | N     | Silent                | N   ← CRITICAL GAP (mitigated: docs + connection-page warning; BR2)
  friglet scan (daemon) | spent coins counted in balance       | N        | N     | Silent (wrong balance)| N   ← CRITICAL GAP (release gate BR3)
  network config        | signet scans the mainnet oracle      | Y        | Y     | named error           | Y   (explicit oracle_url + check)
  state file            | corrupt -> silent full rescan        | Y        | Y     | "rescan started"      | Y
  oracle                | down at start / mid-scan panic       | Y        | Y     | "retrying"            | Y
  P2P peer              | pruned -> notfound                   | Y        | Y     | "node lacks blocks"   | Y
  Electrum port         | wallet history readable by LAN host  | partial  | N     | disclosed             | n/a (S1, taste TD2)
  setup                 | spspend pasted                       | Y        | Y     | rejection message     | Y (no secret)
  stop                  | SIGTERM ignored for up to 90 s       | Y        | Y     | nothing (grace 120 s) | Y
```

#### Implementation tasks (CEO)

- [ ] **T1 (P1, human: ~2h / CC: ~15min)** — plan/packaging — Pin friglet-ui to `friglet-ipc` at friglet's commit; record both in `versions.env`
  - Surfaced by: 0C-bis approach C; Section 1 finding 1.3
  - Files: `versions.env`, `friglet-ui/Cargo.toml`
  - Verify: build fails if the two commits differ
- [ ] **T2 (P1, human: ~1d / CC: ~1h)** — friglet-ui — Supervisor that restarts on any exit, SIGTERM + 120 s grace, log classifier
  - Surfaced by: P3, P8, Section 2
  - Files: `friglet-ui/src/supervisor.rs`, `friglet-ui/src/classify.rs`
  - Verify: fake-daemon integration tests for exits 0, 1, 134 and a hung stop
- [ ] **T3 (P1, human: ~4h / CC: ~30min)** — friglet-ui — Descriptor parser with spspend and network rejection; SP address cross-check
  - Surfaced by: S4, D5, P1
  - Files: `friglet-ui/src/descriptor.rs`
  - Verify: unit vectors; e2e wallet-mismatch case
- [ ] **T4 (P1, human: ~1d / CC: ~1h)** — CI — testkit T2 end-to-end with the packaged image as the release gate
  - Surfaced by: Codex finding 1; Section 6
  - Files: `.github/workflows/e2e.yml`, `compose/`
  - Verify: workflow green with every constructed payment found
- [ ] **T5 (P1, human: ~2h / CC: ~15min)** — docs — Sparrow dedicated-instance instructions and connection-page warning
  - Surfaced by: P1 / C1
  - Files: each platform README, `friglet-ui` connection page
  - Verify: manual check on the pinned Sparrow version
- [ ] **T6 (P2, human: ~4h / CC: ~30min)** — friglet-ui — Delights D1-D6
  - Surfaced by: 0D cherry-pick
  - Files: `friglet-ui/src/http.rs`, `friglet-ui/ui/*`, `umbrel/blindbit-friglet/umbrel-app.yml`
  - Verify: unit tests plus Umbrel widget render
- [ ] **T7 (P2, human: ~1d / CC: ~1h)** — perf — Measure time to first balance, RSS, oracle bytes on arm64
  - Surfaced by: Section 7, H1
  - Files: `docs/performance.md`
  - Verify: numbers recorded with hardware and commit
- [ ] **T8 (P2, human: ~1h / CC: ~10min)** — tickets — File BR1-BR12 in blindbit-rs
  - Surfaced by: Facts re-verification
  - Files: none in this repo
  - Verify: ticket ids listed in this plan

```
  +====================================================================+
  |            MEGA PLAN REVIEW — COMPLETION SUMMARY                   |
  +====================================================================+
  | Mode selected        | SELECTIVE EXPANSION                         |
  | System Audit         | 2 commits, plan untracked, facts re-verified|
  | Step 0               | 10 premises (3 wrong, 3 challenged); C chosen|
  | Section 1  (Arch)    | 4 issues found                              |
  | Section 2  (Errors)  | 13 error kinds mapped, 1 GAP (now surfaced) |
  | Section 3  (Security)| 10 issues found, 3 High severity            |
  | Section 4  (Data/UX) | 8 edge cases mapped, 0 unhandled            |
  | Section 5  (Quality) | 3 issues found                              |
  | Section 6  (Tests)   | Diagram produced, 4 gaps                    |
  | Section 7  (Perf)    | 2 issues found                              |
  | Section 8  (Observ)  | 3 gaps found                                |
  | Section 9  (Deploy)  | 3 risks flagged                             |
  | Section 10 (Future)  | Reversibility: 4/5, debt items: 3           |
  | Section 11 (Design)  | 2 issues (deep review in the design phase)  |
  +--------------------------------------------------------------------+
  | NOT in scope         | written (8 items)                           |
  | What already exists  | written                                     |
  | Dream state delta    | written                                     |
  | Error/rescue registry| 13 kinds, 0 CRITICAL GAPS                   |
  | Failure modes        | 9 total, 2 CRITICAL GAPS (daemon-side)      |
  | TODOS.md updates     | 0 written (no write access); 12 BR tickets  |
  | Scope proposals      | 9 proposed, 6 accepted                      |
  | CEO plan             | written                                     |
  | Outside voice        | codex completed                             |
  | Lake Score           | 4/4 recommendations chose complete option   |
  | Diagrams produced    | 5 (architecture, data flow, error, user flow, dream state) |
  | Stale diagrams found | 0                                           |
  | Unresolved decisions | 0 auto; 3 gate questions (P1, P5, P6)        |
  +====================================================================+
```

Baseline corrections applied to the Implementation plan by this phase (exact replacements):

<!-- autoplan-baseline-edits:ceo {"sourceSha256":"1cdf87c8f0cef7d871201389ef880d68eba4c97c3ae1da922f664caa910e49ab","replacements":[{"oldText":"Status: draft for gstack `/autoplan` review. Date: 2026-10-01.","newText":"Status: revised by gstack `/autoplan` on 2026-10-01 (auto-decided by the review, not yet\nconfirmed by the maintainer). Findings, decisions and open questions are in the review record\nbelow the plan."},{"oldText":"Requirement surfaced by research: the node must be **unpruned** (friglet fetches historical\nblocks from the birthday height over P2P). Each package says so and, where the platform\nallows (StartOS), asks for it.","newText":"Requirement surfaced by research: the node must hold every block from the wallet birthday\nonward. friglet downloads candidate blocks from that height over P2P from one fixed peer, and a\npruned peer answers `notfound` (`blindbit-lib/src/scanner/p2p.rs:201-208`). The supported\nconfiguration is an **unpruned** node. Each package says so; StartOS enforces it with a task;\nRaspiBlitz and myNode check `prune=` in `bitcoin.conf` at install and refuse with a message;\nfriglet-ui reports a `notfound` from the node as a named error."},{"oldText":"Non-goals for this round: multi-wallet, adopting Sparrow's subscribe keys\n(see memory `friglet-onboarding-scope`), a bundled oracle, Electrum TLS, patching friglet.","newText":"Non-goals for this round: multi-wallet, adopting Sparrow's subscribe keys\n(see memory `friglet-onboarding-scope`), a bundled oracle, native Electrum TLS inside friglet\n(platform TLS is used where the platform provides it: StartOS, RaspiBlitz nginx stream),\npatching friglet. friglet defects found here become blindbit-rs tickets (BR list below)."},{"oldText":"- friglet needs a BlindBit **oracle** over gRPC (public default `https://oracle.setor.dev`,\n  signet `https://signet.oracle.setor.dev`) and a Bitcoin **P2P peer** given as a literal\n  `IP:port`. It needs **no** bitcoind RPC and no Tor support of its own.","newText":"- friglet needs a BlindBit **oracle** over gRPC and a Bitcoin **P2P peer** given as a literal\n  `IP:port` (`friglet/src/config.rs:172`). It needs **no** bitcoind RPC and has no SOCKS/Tor\n  support, so the oracle is always reached over clearnet. friglet's default `oracle_url` is\n  `https://oracle.setor.dev` for **every** network (`friglet-ipc/src/lib.rs:72`); signet needs\n  `https://signet.oracle.setor.dev` set explicitly, and a network mismatch is not detected.\n  The oracle must be reachable at startup (exit 1 otherwise), and a gRPC stream error mid-scan\n  aborts the process (exit 134, `panic = \"abort\"`)."},{"oldText":"- It refuses to start without `config.toml` (spend pubkey, birthday, P2P peer) and\n  `scan.key` (32-byte hex scan secret). The control socket only exists once it runs, and\n  config changes need a restart before Electrum reflects them.","newText":"- It refuses to start without `p2p_node_addr`, `start_height`, `spend_pubkey` and a scan\n  secret, taken from CLI flags, `FRIGLET_*` env or `config.toml`, in that precedence (env beats\n  the file). The control socket exists only while friglet runs and offers `SetConfig` and\n  `SetScanKey`, but Electrum keeps serving the old wallet until a restart, and `SetScanKey` is\n  ignored when the state file was made with another key (`friglet/src/control/mod.rs:10-17,380-390`).\n  A wallet change therefore means: stop, fresh state file, restart."},{"oldText":"- It serves no web UI. Status comes from the control socket (`GetStatus`), which unlike\n  the HTTP API does not block during a scan.","newText":"- It serves no web UI. Status comes from the control socket (`GetStatus`, newline-delimited\n  JSON, unversioned), which never waits on the scanner; the HTTP API hangs for as long as\n  scanning is enabled. `GetStatus.last_error` is always null and `scan_progress` covers only\n  the current range, so error states must be derived from friglet's log lines."},{"oldText":"- Default state file lands outside `/data` in the upstream image (bug); we always set\n  `state_file` explicitly.","newText":"- Default state file lands outside `/data` in the upstream image (bug); we always set an\n  absolute `state_file`. The state file embeds the scan secret in plaintext (mode 0600) and is\n  rewritten non-atomically; a corrupt file silently triggers a full rescan."},{"oldText":"- No published image exists. The oracle image exists on Docker Hub.","newText":"- No published image, release tag or signed binary exists. The oracle image exists on Docker Hub.\n- The Electrum server is single-wallet (\"single-server mode\"): unknown scripthashes get an empty\n  history, and `blockchain.silentpayments.subscribe` ignores the client's keys and returns the\n  configured wallet's address, history, tweaks and raw transactions to any caller\n  (`friglet/src/electrum/mod.rs:651-700,773-812`). Sparrow uses one server for all its wallets.\n- At the pinned commits, spend detection, reorg rollback and the oracle-stream panic are fixed\n  only on unmerged blindbit-rs branches. friglet logs balances, txids and client IPs at `info`."},{"oldText":"2. **`friglet-ui`**, a small Go program (static binary, embedded HTML/JS, no build step):","newText":"2. **`friglet-ui`**, a small Rust program in this repo's Cargo workspace (static binary with the\n   static UI embedded, adapted from `friglet-tray/ui/`). It consumes blindbit-rs's `friglet-ipc`\n   crate at the same pinned commit as friglet, so status and config types match the daemon:"},{"oldText":"   - supervises the friglet process: starts it once configured, restarts on config change\n     or crash with backoff, forwards logs;","newText":"   - supervises the friglet process: starts it once configured, restarts it on config change\n     and on **any** exit (including codes 0 and 134) with capped backoff, forwards SIGTERM and\n     waits up to 120 s, forwards logs and classifies known error lines into UI states;"},{"oldText":"   - optional password (`FRIGLET_UI_PASSWORD`) for platforms without gateway auth.","newText":"   - mandatory password on platforms without gateway auth (RaspiBlitz, myNode, compose);\n     Umbrel and StartOS rely on the platform's own login."},{"oldText":"Shared defaults across platforms: Electrum host port **50031** (free in every pulled\nplatform tree; 50001 belongs to electrs/Fulcrum, 50011 to Frigate), friglet HTTP API on\nloopback only, oracle defaults to the public instance and is shown as such in the UI.","newText":"Shared defaults across platforms: Electrum port **50031** inside and outside (free in every\npulled platform tree; 50001 belongs to electrs/Fulcrum, 50011 to Frigate), friglet HTTP API on\n`127.0.0.1:8095` (8080 is LND REST on RaspiBlitz and JoinMarket on myNode; friglet-ui never\nuses it), `oracle_url` written explicitly per network (mainnet `https://oracle.setor.dev`,\nsignet `https://signet.oracle.setor.dev`) and shown in the UI as a third-party service."},{"oldText":"- End to end: compose stack on signet with the public oracle and a signet P2P peer; set up\n  via the UI's HTTP API; assert friglet starts and Electrum answers `server.version` and\n  `blockchain.headers.subscribe` on 50031; status page shows heights.","newText":"- End to end (CI): blindbit-testkit T2 (regtest bitcoind, real oracle, constructed payments)\n  with the packaged image in place of its friglet; set up by pasting a descriptor through\n  friglet-ui; assert every constructed payment is found over Electrum on 50031, that balance\n  and history survive a restart, and that the status page shows heights.\n- End to end (manual smoke): compose stack on signet with the signet oracle and a signet peer."},{"oldText":"- Publishing location of images and binaries (GHCR under `setavenger`?) and signing key.","newText":"- Publishing location of images and binaries (GHCR under `setavenger`?) and signing key.\n- Store-facing app name and ids (Umbrel app ids and StartOS package ids can never change)."},{"oldText":"- Whether third-party material under restrictive or missing licenses stays in\n  `upstream/` if the repo is public.","newText":"- Whether third-party material under restrictive or missing licenses stays in\n  `upstream/` if the repo is public. This blocks the Umbrel community store: Umbrel clones the\n  store repo's default branch onto every device and re-polls it every 5 minutes (11 MB today)."},{"oldText":"- Local oracle packaging (needs Bitcoin Core 30+ with REST; stock platforms ship 29.x).","newText":"- Local oracle packaging (needs Bitcoin Core 30+ with REST and an unpruned node). Umbrel offers\n  Core 31.1/30.3, StartOS ships 31.1 and RaspiBlitz recommends 30.2, so it is feasible there once\n  REST is switched on; myNode pins 29.3. Deferred by the owner's non-goal."}]} -->

<!-- autoplan-accepted:ceo -->
- Delivery order and milestones (CEO). Order: M0 compose + image + friglet-ui; M1 Umbrel
  community store, dogfooded on a real Umbrel; M2 StartOS sideloadable `.s9pk`; M3 RaspiBlitz
  bonus script PR to `dev`; M4 myNode app PR. Each platform tracks four separate states:
  artifact built, install-tested on the platform, submission-ready, installable by users.
  No platform waits on another platform's review queue. Nothing is cut.
- Release gate for anything offered beyond the maintainer (community store announcement or any
  catalog submission): blindbit-rs fixes for spend detection, reorg rollback and the
  oracle-stream panic are merged on its integration branch and re-pinned here (BR3), and a
  pinned Sparrow version completes descriptor setup, receives a signet payment, spends it and
  shows the same history after a friglet restart, on a real Umbrel and on compose.
- Official catalog submissions (Umbrel official store, StartOS registry, RaspiBlitz, myNode)
  also wait for a key-checked `blockchain.silentpayments.subscribe` in friglet (BR1).
- Sparrow server caveat: until friglet forwards non-Silent-Payments calls (BR2), every
  connection page and README tells the user to run the Silent Payments wallet in a dedicated
  Sparrow instance with its own data directory, because pointing the main Sparrow at friglet
  empties every other wallet. Verify the data-directory flag and the empty-history behaviour on
  the pinned Sparrow version during M0.
- Wallet identity check: friglet-ui shows the SP address derived from the pasted descriptor and
  the `sp_address` friglet reports after start, and does not mark setup complete unless they
  match; the page asks the user to compare it with Sparrow's receive address.
- Wallet change: stop friglet, move the old state file aside (kept as `.prev`), write the new
  config and key, start, verify the address. Moving the birthday earlier also starts from a
  fresh state. The UI states that this rescans from the new birthday.
- Electrum exposure policy (taste TD2): LAN plaintext on 50031 following each platform's electrs
  convention, plus an onion; platform TLS on StartOS (automatic) and RaspiBlitz (nginx stream on
  50032); ufw rules limited to the LAN where the platform helper allows it; the onion address is
  shown only inside the authenticated UI; the UI and store copy say the port reveals this
  wallet's history to anyone who can reach it.
- Accepted delights: Umbrel home-screen widget (unauthenticated JSON with only state, scanned
  height and tip height); birthday helper ("new wallet, use current tip" plus a date-to-height
  estimate labelled as an estimate), where the tip comes from the `start_height` field of the
  box node's P2P `version` message (friglet-ui does one handshake; no RPC) and the user can
  always type a height; "test connection" (`server.version` against the advertised LAN
  address, run from the box; the onion is not tested because friglet-ui has no SOCKS client);
  diagnostics download built only from structured data (versions, config without secret,
  friglet-ui state history, classifier output), never from free-text friglet logs; catch-up ETA.
- Positioning and disclosure: README and store copy say why friglet rather than Frigate using
  measured numbers only (time to first balance and peak RSS for a one-year mainnet birthday on
  arm64, oracle bytes), and say what the oracle operator sees (box IP, scanned height ranges,
  timing; never keys).
- blindbit-rs follow-up tickets, filed and referenced by id, not patched here:
  BR1 key-checked `silentpayments.subscribe`; BR2 forward non-SP Electrum calls to a backend
  (electrs/Fulcrum); BR3 merge spend detection, reorg rollback, oracle-stream panic and K_max
  fixes; BR4 start unconfigured, fill `GetStatus.last_error`, version the IPC protocol;
  BR5 SOCKS5/Tor for the oracle and an oracle network check; BR6 Docker `state_file` default and
  relative-path rewrite; BR7 bind failure exits 0, non-atomic state writes, SIGTERM delayed by
  blocking P2P reads; BR8 redact balances, txids, keys and IPs from `info` logs; BR9 fixed fee
  estimate stubs; BR10 descriptor parsing in blindbit-lib shared with friglet-tray; BR11 release
  workflow (tags, multi-arch image, signed binaries) in blindbit-rs; BR12 README `host:port`
  wording and Rust 1.88 minimum.
<!-- /autoplan-accepted:ceo -->

### Phase 2: Design review

#### Step 0: design scope

- Initial design completeness: **3/10**. The plan lists friglet-ui features (form, status page,
  connection page) but never says what the user sees first, which states exist, or how errors,
  waiting and wallet changes look. A 10 here names every screen's hierarchy, a state table that
  the status page, Umbrel widget and StartOS health checks share, the copy for the two traps
  (dedicated Sparrow, birthday too late), per-platform setup paths and responsive/a11y rules.
- DESIGN.md: none. No `/design-consultation` run: for a small Operate-mode utility the tray's
  existing tokens are the starting system (DRY); tokens are fixed below (taste TD3).
- Existing leverage: `friglet-tray/ui/{index.html,main.js,style.css}` (dark palette, orange accent,
  labelled inputs, reduced-motion rule). Its defects for the node: tabs without tab semantics,
  14 px `system-ui` body, stacked cards, Start/Stop buttons, raw path and bind fields, per-range
  progress shown as overall, a "connected" badge that means only "daemon reachable".
- Focus: all seven passes (autoplan P1).
- Mockups: **not generated**. The gstack designer is installed but has no OpenAI API key
  (`No OpenAI API key found`); a key is a human item. ASCII wireframes below replace them.

#### Dual voices

Claude subagent (INPUT design `4ca46e31…4083`, matched): 2 critical, 8 high, 10 medium.
Critical: no state model (the page has nothing defined to show while friglet is down); the
dedicated-Sparrow warning is passive text. High: pruned node found only after the wait on
Umbrel/compose (read service bits in the P2P handshake); birthday risk is lopsided; StartOS loses
helper and identity check; per-range progress; tray controls leak in; no per-platform endpoint
matrix; password lifecycle undefined; tests do not inject faults; "Go" wording left over.

Codex (first run timed out after 600 s exploring files; retry with a time budget completed,
`Recommendation: Revise before implementation`): 2 critical, 8 high, 2 medium.
Critical: dedicated Sparrow handoff is documentation, not design; birthday helpers can silently
exclude payments ("new wallet" undefined, no "I don't know" route). High: hierarchy serves the
daemon; no state contract; progress/ETA untrustworthy; identity mismatch undesigned; wallet
replacement is file operations, not a recoverable flow; connection details mix transport and
privacy; auth and StartOS return paths missing; a11y absent. Medium: responsive; browser tests.

```
DESIGN OUTSIDE VOICES — LITMUS SCORECARD:
═══════════════════════════════════════════════════════════════
  Check                                    Claude  Codex  Consensus
  ─────────────────────────────────────── ─────── ─────── ─────────
  1. Brand unmistakable in first screen?   NOT SPEC NOT SPEC NOT SPEC'D
  2. One strong visual anchor?             No      No     CONFIRMED (state headline missing)
  3. Scannable by headlines only?          No      No     CONFIRMED
  4. Each section has one job?             No      No     CONFIRMED (tray mixes jobs)
  5. Cards actually necessary?             No      No     CONFIRMED (stacked tray cards)
  6. Motion improves hierarchy?            n/a     n/a    NOT SPEC'D (none needed)
  7. Premium without decorative shadows?   NOT SPEC NOT SPEC NOT SPEC'D
  ─────────────────────────────────────── ─────── ─────── ─────────
  Hard rejections triggered:               #7      #7     CONFIRMED (app UI as stacked cards)
═══════════════════════════════════════════════════════════════
```
Disagreements: none of substance; both voices independently named the same two critical traps.

#### Pass 1: Information architecture — 2/10 → 8/10

Classifier: OPERATE (app UI). Navigation: `Status · Connect · Wallet · Diagnostics` as links with
`aria-current`, no tabs. First run shows Setup only (no nav until a wallet exists).

```
 SETUP (first run)                      STATUS (home)
 ┌───────────────────────────────┐      ┌──────────────────────────────────────────────┐
 │ friglet   ·  Umbrel           │      │ friglet · Umbrel     Status Connect Wallet Diag│
 │ 1 Paste descriptor  [masked]  │      │ ① SCANNING  812,400 / 866,102 · about 2 h     │
 │   from Sparrow: Settings ▸    │      │   ▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░  63 %   updated 4 s ago │
 │ 2 Has this wallet received    │      │ ② Wallet  sp1qq…(full, wraps, mono) [Copy]   │
 │   payments before?            │      │   ✓ matches the running scanner              │
 │   ( ) never  ( ) yes, I know  │      │   compare with your dedicated Sparrow        │
 │   the height ( ) yes, unsure  │      │ ③ Next: Connect Sparrow  [Connection details]│
 │ 3 Birthday 709,700 (≈Nov 2021)│      │ ─ details ─────────────────────────────────── │
 │ Node 10.21.21.8:8335 (Umbrel) │      │ Payments found 3 · Birthday 709,700 (Nov 21) │
 │ Network mainnet (from node)   │      │ Network mainnet · Node peer 10.21.21.8:8335  │
 │ ⓘ Scanning uses a third-party │      │ Oracle oracle.setor.dev (third party)        │
 │   oracle: it sees this box's  │      │ friglet 0.1.0 · restarts 0                    │
 │   IP and scanned heights.     │      └──────────────────────────────────────────────┘
 │ [ Start scanning ]            │
 └───────────────────────────────┘
 CONNECT                                 WALLET                    DIAGNOSTICS
 1 Use a separate Sparrow for this      Current wallet address    versions, state history,
   wallet (why + exact command per OS)  Birthday, payments found  classifier events, config
 2 Recommended: <host:port, TLS?> [Copy] [Change wallet]           without secret [Download]
 3 Other endpoints (collapsed)          [Forget wallet]
 4 Who can read this port (plain words)
 5 [Test from this node]
```
Hierarchy rule for Status: state headline, then wallet identity, then the one next action, then
details. Auto-decided (P5, structural).

#### Pass 2: Interaction state coverage — 1/10 → 8/10

The status page, Umbrel widget and StartOS "Scan" health check render the same state ids.
friglet-ui derives them from its supervisor, the P2P handshake, `GetStatus` and classified log
lines; the classifier is tested against real output of the pinned friglet.

| State id | Trigger | Headline (user sees) | Action offered | Widget / StartOS |
|---|---|---|---|---|
| `needs-setup` | no config | "Add your Silent Payments wallet" | Setup form | "Not set up" / disabled |
| `starting` | spawned, socket absent < 60 s | "Starting the scanner…" | none | "Starting" / starting |
| `verifying-identity` | running, address not yet compared | "Checking the wallet…" | none | "Starting" / starting |
| `identity-mismatch` | descriptor address != `sp_address` | "The scanner reports a different wallet" + both addresses | Re-enter descriptor, start fresh | "Needs attention" / failure |
| `scanning` | scanned < tip | "Scanning: X of Y, about T" | Connect (history partial) | progress / loading |
| `up-to-date` | scanned == tip | "Up to date at block Y" | Connect | "Up to date" / success |
| `oracle-unreachable` | exit 1 at start, oracle log class | "Can't reach the scanning service; retrying in N s" | Diagnostics | "Needs attention" / loading |
| `oracle-dropped-resuming` | exit 134 | "Connection to the scanning service dropped; resuming" | none | progress / loading |
| `peer-unreachable` | connect refused / timeout | "Can't reach your Bitcoin node at H:P" | platform hint | "Needs attention" / failure |
| `node-pruned` | handshake: NODE_NETWORK_LIMITED without NODE_NETWORK, or `notfound` below tip | "Your node doesn't keep blocks from your birthday" | how to unprune per platform | "Needs attention" / failure |
| `node-syncing` | `notfound` while node is NODE_NETWORK and behind | "Your node is still syncing (block N)" | wait | "Waiting for node" / loading |
| `port-in-use` | bind error class (exit 0) | "Port 50031 is in use" | platform hint | "Needs attention" / failure |
| `rescanning-after-reset` | scanned height went backwards | "Rescan from birthday started" | none | progress / loading |
| `crash-loop` | ≥ 5 restarts in 10 min | "The scanner keeps stopping" + last exit | Diagnostics | "Needs attention" / failure |
| `unknown-exit` | unclassified exit | "Unexpected restart (code N)" | Diagnostics | progress / loading |
| `ui-disconnected` | browser poll failed | banner "Lost connection to this page's server; last update at t" | retry | n/a |

| Feature | Loading | Empty | Error | Success | Partial |
|---|---|---|---|---|---|
| Setup submit | button shows "Checking…", inputs locked | n/a | inline per field, focus first error, secret field cleared | moves to `starting` | n/a |
| Status | skeleton of last snapshot with "updated t ago" | "No payments found yet" during scan; "Scanned X–Y, no payments found" when up to date (link: change birthday) | state headline from table | `up-to-date` | `scanning` with "history complete up to block X" |
| Connect | n/a | onion "pending" until the hostname exists | test failure names the step that failed | "Electrum answers on this node" | TLS endpoint shown with cert steps |
| Wallet change | durable "Changing wallet…" survives reload | n/a | names failed step, offers retry or restore previous | identity verified | n/a |
| Login (non-gateway) | n/a | n/a | "Wrong password", 5 tries/min | session 12 h | n/a |

Auto-decided (P5, P1: structural).

#### Pass 3: User journey — 3/10 → 8/10

| Step | User does | User feels | Plan now specifies |
|---|---|---|---|
| 1 | Reads store copy | "Is this for me?" | Unpruned node required; separate Sparrow; oracle disclosure (before install) |
| 2 | Opens app, pastes descriptor | Unsure where to find it | Hint naming Sparrow's menu path; inline validation |
| 3 | Answers "received payments before?" | Fear of picking wrong | "If unsure, pick earlier"; heights shown with dates; "I don't know" scans from taproot activation |
| 4 | Starts, waits | Anxious: is it working? | Named state, overall progress, ETA only when stable, "updated t ago" |
| 5 | Verifies address | Trust | Full wrapped address, "matches the running scanner", compare with Sparrow's base SP address |
| 6 | Connects Sparrow | Risk of breaking other wallets | Separate-instance step first, exact per-OS command, recovery sentence |
| 7 | Sees first payment | Relief | "Payments found N"; Sparrow shows history up to block X while scanning |
| 8 | Months later | Forgets details | Status headline answers "is it working" in one glance; widget on Umbrel |

Time horizons: 5 s (state headline), 5 min (setup to first progress), 5 years (wallet change,
forget wallet, uninstall says what it removes).

#### Pass 4: AI slop risk — 4/10 → 7/10

- Hard rejection #7 (app UI as stacked cards) would be inherited from the tray: replaced by one
  reading column with sections separated by spacing and hairlines; cards only for the
  dedicated-Sparrow step.
- Blacklist #11 (`system-ui` primary font) and 14 px body inherited from the tray: replaced (Pass 5).
- Generic phrases in the plan ("status page", "Sparrow steps", "QR code") replaced with the
  wireframes, state table and connection matrix. Capped at 7: no mockups to judge visually.

#### Pass 5: Design system alignment — 2/10 → 7/10

Tokens (taste TD3, continuity with the tray): CSS variables only; dark theme from the tray
(`--bg #12161d`, `--surface #1b2432`, `--text #e8eef7`, `--muted #8b98ab`, `--accent #f7931a`,
`--good #34c07a`, `--bad #e05555`) plus a light theme via `prefers-color-scheme`; IBM Plex Sans
for UI and IBM Plex Mono for heights, addresses and endpoints, embedded (OFL); body 16 px,
small 14 px, h1 20 px; spacing 4/8/12/16/24/32; radius 6 px; focus ring 2 px accent with ≥ 3:1
contrast; tabular numerals for heights; themed selection and scrollbars. A short
`friglet-ui/DESIGN.md` records them. Capped at 7: palette not checked against contrast tools yet.

#### Pass 6: Responsive and accessibility — 0/10 → 8/10

- One column from 320 px; reading width capped at 760 px on desktop; label/value tables stack
  below 640 px; addresses and hostnames wrap (`overflow-wrap: anywhere`); no horizontal scroll at
  200 % zoom; Copy and recovery actions never hover-only; touch targets 44 × 44 px.
- Landmarks `header/nav/main`; nav links with `aria-current`; native `<progress>` with value text;
  one polite live region announcing state changes only, not each poll; field errors tied with
  `aria-describedby`; focus moves to the first error; contrast 4.5:1 text, 3:1 controls; respects
  `prefers-reduced-motion`; clipboard failure falls back to selected text.

#### Pass 7: Unresolved design decisions (all auto-decided)

| Decision | Resolution | Class |
|---|---|---|
| QR code on Connect | Removed: desktop Sparrow does not scan QR codes and no payload consumer exists | Taste TD4 |
| Visual direction without mockups | Tray tokens + light theme + Plex fonts | Taste TD3 |
| Oracle URL editable? | Read-only by default; editable under Advanced on every platform, validated, restart on change | Mechanical (P5, operator-configurable endpoint) |
| Show balance? | No (wrong until BR3); show "Payments found N" | Mechanical (P1 trust) |
| StartOS setup path | StartOS action runs `friglet-ui setup` (one parser, same birthday rules); web UI shows status and points "Change wallet" to the action | Mechanical (P4) |
| Password lifecycle | Compose refuses to start without `FRIGLET_UI_PASSWORD`; RaspiBlitz and myNode generate one at install and show it on an already-authenticated platform surface (SSH menu; myNode app page); never set on first browser visit; HttpOnly SameSite=Strict cookie, 12 h; 5 tries/min | Mechanical (P1 security) |
| RaspiBlitz SSH menu | URL, password, status, password reset; no setup in SSH | Mechanical (P4) |
| `.prev` state retention | Deleted once the new wallet's identity is verified (it holds the old secret); UI says so | Mechanical (P1) |
| Forget wallet | Deletes config, key, state and `.prev`, returns to setup; each README says what uninstall removes | Mechanical (P1) |
| Birthday too late vs too early | "If unsure, pick earlier"; date estimates rounded down by 2016 blocks; "never received" → node tip from the handshake; "I don't know" → taproot activation height (mainnet 709,632) | Mechanical (P1) |
| Connect while scanning | Allowed after identity is verified, with "history complete up to block X" (verify friglet serves partial history in M0) | Mechanical |
| friglet `start_height` vs Sparrow's subscription | `friglet/src/types.rs:66-71` says Sparrow rejects notifications whose start height differs from its subscription key. M0 verifies whether Sparrow requires friglet's birthday to equal the wallet birthday Sparrow uses; if yes, the birthday step asks for the birthday height shown in Sparrow and drops the date estimate | Mechanical (verify first) |

#### Required outputs

- NOT in scope: mockups and a visual comparison board (no designer key; human item); a full
  `/design-consultation` design system; localization; native mobile layouts beyond responsive web;
  RaspiBlitz WebUI (blitz_api/raspiblitz-web) screens.
- What already exists: tray UI files and tokens (reused, adapted as listed); Umbrel electrs
  connection page pattern (Local/Tor switch, copyable strings); StartOS Interfaces tab (addresses
  and ports come from it on StartOS); myNode Canary blueprint pattern.
- TODOS.md: none written; deferred design work is the mockup pass above.

#### Implementation tasks (design)

- [ ] **D-T1 (P1, human: ~1d / CC: ~1h)** — friglet-ui — State machine and state table shared by page, widget and StartOS health
  - Surfaced by: Pass 2 — no state model
  - Files: `friglet-ui/src/state.rs`, `friglet-ui/src/classify.rs`
  - Verify: fault-injection tests render each state id
- [ ] **D-T2 (P1, human: ~1d / CC: ~1h)** — friglet-ui — Setup, Status, Connect, Wallet, Diagnostics screens per wireframes
  - Surfaced by: Pass 1
  - Files: `friglet-ui/ui/*`
  - Verify: browser checks at 320 px and 1280 px, keyboard-only run
- [ ] **D-T3 (P1, human: ~2h / CC: ~15min)** — friglet-ui — Birthday question, conservative estimate, handshake tip, pruned/syncing detection
  - Surfaced by: Pass 3, Pass 7
  - Files: `friglet-ui/src/p2p.rs`, `friglet-ui/src/birthday.rs`
  - Verify: unit tests for service bits and rounding; regtest pruned node case
- [ ] **D-T4 (P1, human: ~2h / CC: ~15min)** — docs/UI — Dedicated Sparrow step with tested per-OS commands
  - Surfaced by: dual voices, critical
  - Files: connection page, every platform README, store copy
  - Verify: run on Linux/macOS/Windows with the pinned Sparrow version
- [ ] **D-T5 (P2, human: ~2h / CC: ~15min)** — friglet-ui — Tokens, Plex fonts, light/dark, `friglet-ui/DESIGN.md`
  - Surfaced by: Pass 4, Pass 5
  - Files: `friglet-ui/ui/style.css`, `friglet-ui/DESIGN.md`
  - Verify: contrast check on both themes
- [ ] **D-T6 (P2, human: ~4h / CC: ~30min)** — packaging — Password lifecycle per platform and StartOS action calling `friglet-ui setup`
  - Surfaced by: Pass 7
  - Files: `compose/`, `raspiblitz/bonus.friglet.sh`, `mynode/`, `startos/startos/actions/`
  - Verify: install tests per platform

```
  +====================================================================+
  |         DESIGN PLAN REVIEW — COMPLETION SUMMARY                    |
  +====================================================================+
  | System Audit         | no DESIGN.md; UI = friglet-ui (5 screens)   |
  | Step 0               | 3/10 initial; all 7 passes                  |
  | Pass 1  (Info Arch)  | 2/10 → 8/10 after fixes                     |
  | Pass 2  (States)     | 1/10 → 8/10 after fixes                     |
  | Pass 3  (Journey)    | 3/10 → 8/10 after fixes                     |
  | Pass 4  (AI Slop)    | 4/10 → 7/10 after fixes                     |
  | Pass 5  (Design Sys) | 2/10 → 7/10 after fixes                     |
  | Pass 6  (Responsive) | 0/10 → 8/10 after fixes                     |
  | Pass 7  (Decisions)  | 12 resolved (2 taste), 0 deferred           |
  +--------------------------------------------------------------------+
  | NOT in scope         | written (5 items)                           |
  | What already exists  | written                                     |
  | TODOS.md updates     | 0 (deferred mockup pass noted)              |
  | Approved Mockups     | 0 generated (no API key), 0 approved        |
  | Decisions made       | 12 auto-decided (not user-approved)         |
  | Decisions deferred   | 0                                           |
  | Overall design score | 0/10 → 7/10                                 |
  +====================================================================+
```
Passes 4 and 5 stay below 8 until mockups and a contrast check exist.

<!-- autoplan-baseline-edits:design {"sourceSha256":"11ea8e0f00749746ecd8ddc85fb2b6e7a5d5fa1bc2d0895e95aa05305398a657","replacements":[{"oldText":"   - first-run setup form: paste `sp(spscan1…)` descriptor (or hex scan secret + spend\n     pubkey), birthday height; network and P2P peer come from the platform;","newText":"   - first-run setup form: paste `sp(spscan1…)` descriptor (hex scan secret + spend pubkey\n     only behind a disclosure), answer whether the wallet has received payments before, and set\n     the birthday height; network and P2P peer come from the platform and are shown read-only;\n     the same parser and validation are exposed as a `friglet-ui setup` CLI for StartOS;"},{"oldText":"   - status page from `GetStatus` over the control socket;\n   - connection page: Electrum LAN `host:port` and onion `host:port` from platform env,\n     QR code, Sparrow steps, plain statement that the port is plaintext and unauthenticated;","newText":"   - status page rendered from friglet-ui's own supervisor state plus the last `GetStatus`\n     snapshot (named states, never an endless spinner);\n   - connection page: dedicated-Sparrow step first, then one recommended endpoint per platform\n     with copy, other endpoints collapsed, the exact transport and privacy statement for each,\n     no QR code (taste TD4);"},{"oldText":"install; it writes `config.toml`/`scan.key` and `friglet-ui` runs with its setup form\n  disabled, serving status and connection info as the `ui` interface.","newText":"install; the action runs `friglet-ui setup` inside the subcontainer (same parser, birthday\n  rules and identity check as the web form) and `friglet-ui` serves status and connection info\n  as the `ui` interface, with \"Change wallet\" pointing back to the action."},{"oldText":"  installs the signed binary release; setup via `friglet-ui` behind the RaspiBlitz nginx,\n  or the SSH menu.","newText":"  installs the signed binary release; setup via `friglet-ui` behind the RaspiBlitz nginx;\n  the SSH menu shows the UI URL, the generated UI password, status and a password reset."}]} -->

<!-- autoplan-accepted:design -->
- UI states (design): friglet-ui keeps one state machine whose ids drive the status page, the
  Umbrel widget and the StartOS "Scan" health check: `needs-setup`, `starting`,
  `verifying-identity`, `identity-mismatch`, `scanning`, `up-to-date`, `oracle-unreachable`,
  `oracle-dropped-resuming`, `peer-unreachable`, `node-pruned`, `node-syncing`, `port-in-use`,
  `rescanning-after-reset`, `crash-loop`, `unknown-exit`, plus a browser-side
  `ui-disconnected` banner. Each has a headline, an offered action and a widget/health value.
  The widget exposes only the state id, scanned height and tip height; error states read
  "Needs attention". Fault-injection tests render every state from real pinned-friglet output.
- Screens: Setup (first run only), then Status, Connect, Wallet, Diagnostics as plain links
  with `aria-current`. Status order: state headline with overall progress, wallet identity,
  one next action, then details (payments found, birthday with date, network, node peer,
  oracle marked third-party, versions, restarts). No balance until BR3 lands.
- Progress is (scanned − birthday) / (tip − birthday), computed by friglet-ui; the ETA appears
  only after 60 s of steady throughput, is labelled "about" and hides during stalls and restarts.
  A falling scanned height shows `rescanning-after-reset`.
- Birthday step: the user answers "has this wallet received payments before?" (never / yes, I
  know the height / yes, not sure). "Never" uses the node tip from the P2P `version`
  handshake; "not sure" uses taproot activation (mainnet 709,632); date estimates are rounded
  down by 2016 blocks; every height is shown with a date; copy says "If unsure, pick earlier";
  heights outside taproot activation .. node tip are rejected. When up to date with nothing
  found the page says "Scanned X–Y, no payments found" and links to changing the birthday.
- The same handshake reads service bits: NODE_NETWORK_LIMITED without NODE_NETWORK blocks
  Submit with `node-pruned` before any oracle contact; a `notfound` from a full node is shown
  as `node-syncing`.
- M0 verification: whether Sparrow requires friglet's `start_height` to equal the birthday
  Sparrow subscribes with (`friglet/src/types.rs:66-71`); if so, the birthday step asks for
  the birthday height shown in Sparrow and drops the date estimate.
- Descriptor field: masked with a reveal toggle, autocomplete and spellcheck off, inline
  checksum and network validation, never shown again after saving (address and a short key
  fingerprint are shown instead); hex entry sits behind a disclosure. An oracle disclosure line
  sits directly above Submit and in the StartOS action description.
- Identity mismatch: stop friglet, show both full addresses, offer re-entry or "start fresh";
  if friglet reports no address within 5 minutes, say so and link Diagnostics.
- Connect page order: (1) dedicated Sparrow instance, with tested per-OS launch commands for the
  pinned Sparrow version and the sentence "Other wallets are unaffected; switch Sparrow back to
  your old server to see them again"; (2) one recommended endpoint for this platform with copy;
  (3) other endpoints collapsed; (4) who can read this port, in plain words; (5) "Test from this
  node", worded as "Electrum answers on this node" with laptop-side troubleshooting below.
  Endpoints per platform: Umbrel LAN `host:50031:t` recommended, onion `:t`; StartOS the
  `ssl://` address from the Interfaces tab with root-CA steps; RaspiBlitz `:50032:s` recommended,
  `:50031:t`, onion; myNode `:50031:t`, onion only on Premium; compose `:50031:t`. No QR code
  (taste TD4). The dedicated-instance note also appears in store copy and READMEs before install.
- While scanning, Connect is allowed once identity is verified, with "Sparrow shows payments up
  to block X; more appear as scanning continues" (M0 verifies friglet serves partial history).
- Wallet change is its own flow: confirmation naming current and new wallet and the rescan,
  durable progress across reloads, named failure step with retry or restore. `.prev` is
  deleted once the new identity is verified and the UI says so. "Forget wallet" deletes config,
  key, state and `.prev` and returns to setup; each README says what uninstall removes.
- Platform setup paths: StartOS runs `friglet-ui setup` from its action (same parser, birthday
  rules and identity check) and the web UI's "Change wallet" points to that action; RaspiBlitz
  SSH menu shows URL, password, status and password reset only.
- Login on non-gateway platforms: compose refuses to start without `FRIGLET_UI_PASSWORD`;
  RaspiBlitz and myNode generate a password at install and show it on an already-authenticated
  platform surface (SSH menu; myNode app page); never set on first browser visit; HttpOnly,
  SameSite=Strict session cookie for 12 h; 5 login attempts per minute.
- Oracle URL: read-only by default, editable under Advanced on every platform, validated as an
  https URL and restarted on change.
- Visual system (taste TD3): CSS variables with the tray's dark palette plus a light theme via
  `prefers-color-scheme`; IBM Plex Sans and Mono embedded; 16 px body; spacing 4/8/12/16/24/32;
  radius 6 px; 2 px accent focus ring at ≥ 3:1; tabular numerals; one reading column, no stacked
  cards; recorded in `friglet-ui/DESIGN.md`.
- Responsive and accessibility: works from 320 px with no horizontal scroll at 200 % zoom; reading
  width capped at 760 px; addresses and hosts wrap; 44 px touch targets; nothing hover-only;
  landmarks; native `<progress>`; one polite live region for state changes only; field errors
  tied by `aria-describedby` with focus on the first error; contrast 4.5:1 text and 3:1 controls;
  reduced motion respected; clipboard failure falls back to selected text.
<!-- /autoplan-accepted:design -->

### Phase 2.5: DX review (mode: DX POLISH)

#### Step 0: investigation

- Product type: **Platform** (deployable packages and images) with a small **CLI**
  (`friglet-ui setup`). Two audiences: operators and packaging contributors.
- No README, docs, CLI help or error strings exist yet in this repo; the audit covered the plan,
  `AGENTS.md`, the platform NOTES and friglet's config loader (`friglet/src/config.rs:126-185,263-300`).
- Initial DX completeness: **3/10**. TTHW today: Umbrel about 9 actions and an unstated scan
  time; compose 30+ minutes (no published image, build from source); contributor regtest only
  in CI.

```
TARGET DEVELOPER PERSONA (auto-selected, P6)
============================================
Who:       Node-box operator (Umbrel or StartOS first, RaspiBlitz SSH user second) who runs
           Sparrow, is technical but does not program. Secondary: a packaging contributor who
           bumps the blindbit-rs pin and answers platform reviewers.
Context:   Wants Silent Payments receipts without handing the scan key to a public server.
Tolerance: ~10 minutes to "Sparrow sees my wallet"; abandons on an opaque error or a hidden
           multi-hour index.
Expects:   One-click install from the store, a pasted descriptor, a clear "it's working".
```

Empathy narrative (operator, today's plan): "I add the BlindBit community store to my Umbrel and
install friglet. The page asks for my descriptor; I find it in Sparrow and paste it. Then it asks
whether the wallet has received payments before, which I don't remember, so I pick 'not sure'.
It starts scanning from 2021 and shows an ETA. The Connect page tells me to run a second
Sparrow with its own data folder. That surprises me, but it says my other wallets are safe. I
copy the host and port into Sparrow's server dialog, which wants host, port and an SSL toggle,
not the `host:port:t` string I was given. Twenty minutes later my one payment shows up. If my
node had been pruned, the page would have told me before it started, not after."

```
COMPETITIVE DX BENCHMARK
=========================
Tool                        | TTHW (to first history)        | Notable DX choice                    | Source
Sparrow + frigate.2140.dev  | ~0 min (auto-selected)         | zero setup; scan key goes to a       | Sparrow 2.5.0 release notes
                            |                                | public server                        |
Frigate on StartOS          | hours to days (full index from | critical "Configure" task; needs     | BitcoinMechanic/frigate-startos README
                            | taproot activation)            | bitcoind txindex + ZMQ               |
Frigate on MiniBolt         | hours (guide, Fulcrum backend) | ~18 GB index, most of the RAM        | upstream/others SURVEY (frigate.md)
friglet (this plan, target) | <10 min to connected and       | no index; one wallet from its        | this plan
                            | scanning; minutes to first     | birthday; scan key stays on the box  |
                            | payment for a recent birthday  |                                      |
```
Target tier: **Competitive** for operators (install to "verified, connected, scanning" under
10 minutes on Umbrel) and for contributors (clone to regtest UI with a found payment under 10
minutes). Champion (<2 min) is not reachable while install and descriptor export are manual.

Magical moment: the status page flips to "Payments found 1" and Sparrow shows the transaction
minutes after install, with no index build. Vehicle: the product itself for operators; a
copy-paste `make dev-regtest` for contributors (lowest effort, P5).

#### Dual voices

Claude subagent (INPUT dx `f27de21f…7874`, matched): 2 critical, 12 high, 8 medium.
Critical: compose has no way to give peer/network (and `bitcoind:8333` is rejected as not an
IP); inherited `FRIGLET_*` env silently overrides what friglet-ui wrote. High: no ports/paths
table, `setup` CLI undefined (secret in argv risk), Go leftovers, no local dev loop, two pins can
drift, no peer override, oracle override unchecked, states lack cause/fix/docs, Electrum
exposure has no off switch.

Codex (completed, `Recommendation: Revise before approving implementation`): 10 high, 2 medium.
High: no defined TTHW start/success; StartOS setup lifecycle unclear; `setup` CLI not a contract;
diagnosis and retry policy; birthday/progress rules across networks; no state-compatibility or
rollback contract on upgrade; wallet replacement not transactional; contributor build not
reproducible; verification can pass while install/recovery fails; distribution and reviewer
handoff unresolved. Medium: docs discoverability; comparison omits the public-server option.

```
DX DUAL VOICES — CONSENSUS TABLE:
═══════════════════════════════════════════════════════════════
  Dimension                           Claude  Codex  Consensus
  ──────────────────────────────────── ─────── ─────── ─────────
  1. Getting started < 5 min?          No      No     CONFIRMED (targets + dev loop missing)
  2. API/CLI naming guessable?         No      No     CONFIRMED (setup CLI undefined)
  3. Error messages actionable?        Partly  No     CONFIRMED (no cause/fix/retry policy)
  4. Docs findable & complete?         No      No     CONFIRMED
  5. Upgrade path safe?                n/a     No     DISAGREE (Claude silent) → taken up, single voice
  6. Dev environment friction-free?    No      No     CONFIRMED (no local loop, pin drift)
═══════════════════════════════════════════════════════════════
```
Single-voice items kept: env override (Claude, verified in `config.rs:132,263-267`); StartOS
lifecycle and upgrade compatibility (Codex). Cross-model tension: none on direction.

#### Passes 1-8

| Pass | Before | After | Main fixes (auto-decided) |
|---|---|---|---|
| 1 Getting started | 3 | 7 | TTHW targets per audience; compose inputs with hostname resolution; `make dev-regtest`; prerequisites stated up front |
| 2 API/CLI | 3 | 8 | `friglet-ui setup` contract; one ports/paths table; one naming table (human decides names); env isolation |
| 3 Errors | 4 | 8 | State catalog with cause, evidence, fix per platform and docs anchor; retry policy; exact refusal texts |
| 4 Docs | 2 | 7 | Root README routing, compose quickstart run by CI, troubleshooting generated from the catalog, CONTRIBUTING with pin-update flow |
| 5 Upgrade | 1 | 7 | State backup before a friglet version change; rollback steps; previous-release upgrade test |
| 6 Dev env | 2 | 7 | Single pin source with generated Cargo rev; `BLINDBIT_RS_SRC`; arm64 execution in CI; pinned platform tools |
| 7 Community | 2 | 5 | CONTRIBUTING, reviewer-response flow, comparison table; support channel is a human item |
| 8 Measurement | 0 | 6 | Timed first-run checks per platform recorded in `docs/performance.md`; boomerang `/devex-review` after M1 |

Error path traces (what the operator sees, before → after):
1. Pruned node on Umbrel: before, a long scan then a log line `notfound` → after, Submit blocked:
   "Your node keeps only recent blocks (pruned). friglet needs every block from your birthday.
   Fix: turn off pruning in the Bitcoin Node app settings (re-downloads the chain, ~N GB).
   Help: docs/troubleshooting.md#node-pruned".
2. Compose without password: before, undefined → after, friglet-ui exits 64 with
   "FRIGLET_UI_PASSWORD is not set. The setup page accepts your scan key, so it needs a
   password. Fix: set it in .env (see .env.example). Help: docs/troubleshooting.md#ui-password".
3. Stray `FRIGLET_ORACLE_URL` in compose: before, silently overrides the UI → after, refuses to
   start: "FRIGLET_ORACLE_URL is set in the environment. friglet-ui owns friglet's
   configuration; set the oracle under Advanced instead, or remove the variable."

#### Required outputs

```
STAGE           | DEVELOPER DOES                         | FRICTION POINTS                      | STATUS
----------------|----------------------------------------|--------------------------------------|--------
1. Discover     | reads store copy / README              | why not Frigate or the public server | fixed (comparison table)
2. Install      | store install / compose up / bonus on  | no image yet; compose peer input     | fixed (M0 image; FRIGLET_UI_NODE)
3. Hello World  | pastes descriptor, birthday, verifies  | birthday unknown; Sparrow dialog     | fixed (question; per-field copy)
4. Real Usage   | connects dedicated Sparrow, receives   | main-Sparrow trap                    | fixed (step 1 on Connect)
5. Debug        | reads status, diagnostics, raw logs    | no cause/fix; raw logs location      | fixed (catalog; README log section)
6. Upgrade      | store update / repin                   | state compatibility unknown          | fixed (backup + rollback + test)
7. Contribute   | clone, build, test, bump pin           | CI-only e2e; two pins                | fixed (make dev-regtest; single pin)
8. Submit       | platform reviewer loop                 | StartOS fork-and-build vs monorepo   | open (human: mirror repos?)
9. Measure      | timed checks                           | no instrumentation                   | fixed (recorded timings)
```

```
FIRST-TIME DEVELOPER REPORT (contributor, before fixes)
=======================================================
T+0:00  Clones the repo; README missing; opens docs/plan.md.
T+0:30  Finds "Go unit tests" and a Rust crate; unsure which toolchain.
T+1:00  Looks for a build command; none; sees versions.env and Cargo rev, unsure which wins.
T+2:00  Wants to see the UI; the only end-to-end path is the CI workflow.
T+3:00  Gives up on local and pushes a branch to see CI. → all four points addressed above.
```

- NOT in scope: a hosted sandbox or playground; video walkthroughs; localization; Windows-native
  packaging; telemetry or analytics (not wanted on a privacy product; timings are measured in
  tests instead).
- What already exists: testkit T2 compose stack and scenarios; `friglet-tray/examples/fake-daemon.rs`;
  friglet's own config errors (rewritten for friglet-ui users); platform linters (`lint-apps.mjs`,
  `start-cli s9pk inspect`, `create.py check`).

```
+====================================================================+
|              DX PLAN REVIEW — SCORECARD                             |
+====================================================================+
| Dimension            | Score  | Prior  | Trend  |
|----------------------|--------|--------|--------|
| Getting Started      |  7/10  |  3/10  |  ↑     |
| API/CLI/SDK          |  8/10  |  3/10  |  ↑     |
| Error Messages       |  8/10  |  4/10  |  ↑     |
| Documentation        |  7/10  |  2/10  |  ↑     |
| Upgrade Path         |  7/10  |  1/10  |  ↑     |
| Dev Environment      |  7/10  |  2/10  |  ↑     |
| Community            |  5/10  |  2/10  |  ↑     |
| DX Measurement       |  6/10  |  0/10  |  ↑     |
+--------------------------------------------------------------------+
| TTHW                 | <10 min target | ~15 min + scan (Umbrel) |   |
| Competitive Rank     | Competitive (target)                         |
| Magical Moment       | designed, via the product + make dev-regtest |
| Product Type         | Platform (+ CLI)                             |
| Mode                 | POLISH                                       |
| Overall DX           |  5/10  |  3/10  |  ↑  (lowest pass)          |
+====================================================================+
| DX PRINCIPLE COVERAGE                                               |
| Zero Friction      | gap (manual descriptor export, store add)      |
| Learn by Doing     | covered (make dev-regtest, signet path)        |
| Fight Uncertainty  | covered (state catalog)                        |
| Opinionated + Escape Hatches | covered (Advanced: oracle, peer, exposure, pause, log level) |
| Code in Context    | covered (CI-run compose quickstart)            |
| Magical Moments    | covered                                        |
+====================================================================+
```

```
DX IMPLEMENTATION CHECKLIST
============================
[ ] Umbrel install to verified + connected + scanning < 10 min (timed on hardware)
[ ] Contributor clone to regtest UI with a found payment < 10 min (`make dev-regtest`)
[ ] Compose quickstart is copy-paste complete and run by CI
[ ] Every state has problem + cause + fix + docs anchor (generated from one catalog)
[ ] `friglet-ui setup` contract: stdin/file secret, --dry-run, --replace, --json, exit codes
[ ] One pin source; CI fails on drift; BLINDBIT_RS_SRC override
[ ] Upgrade from previous release tested; state backup before friglet version change
[ ] arm64 binaries executed in CI
[ ] Root README, troubleshooting, CONTRIBUTING, comparison table exist
```

#### Implementation tasks (DX)

- [ ] **X-T1 (P1, human: ~4h / CC: ~30min)** — friglet-ui — `setup` CLI contract and env isolation
  - Surfaced by: Pass 2 — undefined CLI; inherited `FRIGLET_*`
  - Files: `friglet-ui/src/cli.rs`, `friglet-ui/src/supervisor.rs`
  - Verify: tests for stdin secret, `--replace`, exit codes, refusal on stray env
- [ ] **X-T2 (P1, human: ~4h / CC: ~30min)** — compose — `FRIGLET_UI_NODE`/`NETWORK`, `.env.example`, quickstart run in CI
  - Surfaced by: Pass 1 — compose has no peer input
  - Files: `compose/`, `.github/workflows/`
  - Verify: CI job runs README commands verbatim
- [ ] **X-T3 (P1, human: ~1d / CC: ~1h)** — friglet-ui/docs — State catalog generating UI copy, widget text, health text, troubleshooting page
  - Surfaced by: Pass 3
  - Files: `friglet-ui/states.toml`, `docs/troubleshooting.md`
  - Verify: generated docs diff-checked in CI
- [ ] **X-T4 (P1, human: ~4h / CC: ~30min)** — repo — `make dev-regtest`, single pin source, `BLINDBIT_RS_SRC`
  - Surfaced by: Pass 6
  - Files: `Makefile`, `versions.env`, `scripts/pin.sh`
  - Verify: fresh clone reaches the regtest UI in under 10 minutes
- [ ] **X-T5 (P2, human: ~4h / CC: ~30min)** — friglet-ui — Upgrade state backup, rollback doc, previous-release upgrade test
  - Surfaced by: Pass 5
  - Files: `friglet-ui/src/upgrade.rs`, `docs/upgrading.md`
  - Verify: compose upgrade test from the previous tag
- [ ] **X-T6 (P2, human: ~4h / CC: ~30min)** — docs — Root README routing, CONTRIBUTING, comparison table, raw-log locations
  - Surfaced by: Pass 4, Pass 7
  - Files: `README.md`, `CONTRIBUTING.md`
  - Verify: newcomer finds install/recover/contribute in under 2 minutes

<!-- autoplan-baseline-edits:dx {"sourceSha256":"80a85b23458d3045a65d2ea887067907d7b3cc32128642aa40cf7e112bd43782","replacements":[{"oldText":"- `compose/`: plain Docker Compose for any Docker host; also the local test harness.","newText":"- `compose/`: plain Docker Compose for any Docker host; also the local test harness. The node\n  peer and network come from `FRIGLET_UI_NODE` (`host:port`, resolved to a literal IP on every\n  start and shown in the UI) and `FRIGLET_UI_NETWORK`; `FRIGLET_UI_PASSWORD` is required; a\n  `.env.example` ships next to the compose file and CI runs the quickstart exactly as written."},{"oldText":"Pinned versions live in one file, `versions.env` (blindbit-rs commit, Go and Rust\ntoolchain, base image digests).","newText":"Pinned versions live in one file, `versions.env` (blindbit-rs commit, Rust toolchain,\ncargo-zigbuild, zig, protoc, base image digests, blindbit-testkit commit, and the platform tools:\nstart-cli, the umbrel-apps commit used for linting, the myNode commit used for `create.py`).\nThe `friglet-ipc` git `rev` in `Cargo.toml` is generated from it and CI fails if they differ;\n`BLINDBIT_RS_SRC=<path>` builds against a local or unmerged blindbit-rs checkout and tags the\nbuild `dev` in the UI and diagnostics."},{"oldText":"- `friglet-ui`: Go unit tests (descriptor vectors from the BIP, config writer, supervisor).\n- Image: build amd64 locally; arm64 binaries cross-compiled and checked with `file`.","newText":"- `friglet-ui`: Rust `cargo test` (descriptor vectors from the BIP, config writer, supervisor,\n  state catalog, `setup` CLI contract).\n- Image: multi-arch build; arm64 binaries are executed (`friglet --version`,\n  `friglet-ui --self-test`) on an arm64 runner or under qemu-user, not only checked with `file`."}]} -->

<!-- autoplan-accepted:dx -->
- Time-to-hello-world targets (DX): Umbrel install to "verified, connected, scanning" under 10
  minutes, timed on hardware at M1; first payment visible within minutes for a recent birthday;
  contributor fresh clone to the regtest UI showing a found payment under 10 minutes through
  `make dev-regtest` (testkit T2 with the local image; prints the UI URL and a regtest
  descriptor generated at run time). Prerequisites are stated before install: supported
  platform, synced unpruned node, pinned Sparrow version. Timings are recorded in
  `docs/performance.md`.
- Ports and paths (verified free only by grep of the pulled trees): Electrum 50031, TLS 50032
  (RaspiBlitz nginx stream), friglet HTTP `127.0.0.1:8095`, friglet-ui 8097 (container; loopback
  on native installs), RaspiBlitz and myNode HTTPS for friglet-ui 8098 through nginx, Umbrel
  manifest port 2104, RaspiBlitz P2P whitebind `download@127.0.0.1:8336`; control socket, config,
  key and state files under the platform data dir (`/data` in the image). Each README carries the
  table for its platform.
- Environment isolation: friglet-ui starts friglet with a cleared environment plus an allowlist
  (PATH, HOME, TZ) and an explicit `--config`, and refuses to start, naming the variable, when any
  `FRIGLET_*` variable other than `FRIGLET_UI_*` is set. Unit-tested.
- `friglet-ui setup` contract: descriptor only from stdin or `--descriptor-file` (mode 0600
  checked), never argv or env; `--network`, `--node host:port`, `--birthday <height|tip|taproot>`;
  `--dry-run` validates only; an existing different wallet needs `--replace`; the same
  descriptor and birthday is a no-op; `--json` result; exit codes 0 ok, 2 usage, 3 descriptor,
  4 birthday, 5 network mismatch, 6 node check, 7 wallet exists, 8 I/O. Used by StartOS and by
  headless compose.
- Compose: `FRIGLET_UI_NODE` (`host:port`, resolved to a literal IP on every start and shown in
  the UI; resolution failure is a named state), `FRIGLET_UI_NETWORK`, required
  `FRIGLET_UI_PASSWORD`; `FRIGLET_UI_AUTH=none` is allowed for users with their own auth in
  front, with a startup warning and a permanent UI banner (taste TD5).
- Escape hatches under Advanced: node peer override (checked by a `version` handshake before
  saving), oracle URL (must answer before saving; warning when it differs from the network
  default), Electrum exposure LAN / onion only / localhost where the platform allows it (not
  on Umbrel, where the published port is fixed; documented), pause scanning (`paused` state),
  friglet log level. `docker run --entrypoint friglet` is documented as the full bypass.
- One state catalog file (`friglet-ui/states.toml`): id, detection evidence, cause, fix per
  platform, docs anchor. UI copy, widget text, StartOS health text and `docs/troubleshooting.md`
  are generated from it and CI fails on drift. `notfound` maps to `node-syncing` only when the
  handshake height is below the requested block, otherwise to "node lacks block H".
  Permanent classes (port-in-use, node-pruned, identity-mismatch, config write errors) stop
  automatic retries until the user acts or the config changes; transient classes back off to
  5 minutes, and the page shows the next retry time. The last failure survives restarts.
- Exact refusal texts with problem, cause, fix and docs anchor for: pruned node (RaspiBlitz and
  myNode install, all platforms at Submit), missing UI password, stray `FRIGLET_*` variable,
  wallet exists without `--replace`. friglet's own "set FRIGLET_X or pass --flag" advice is
  rewritten for friglet-ui users.
- Birthday bounds per network come from the pinned bitcoin crate's taproot activation heights
  (mainnet 709,632); regtest "not sure" means 0; birthday equal to tip shows `up-to-date`
  immediately; wording is "caught up with your node".
- StartOS lifecycle: the critical task's action runs `friglet-ui setup` in a one-off subcontainer
  while the service is stopped; the result says identity is checked once the service starts;
  `main` runs friglet-ui as the daemon, and the "Scan" health check shows `identity-mismatch`
  and the other states. Action timeout 60 s; a failed action leaves the old config untouched.
- Wallet replacement is a durable transaction (supersedes the design block's `.prev` timing):
  friglet-ui writes an intent file first, then stops friglet, snapshots config, key and state
  into `prev/` (0700), writes the new set, starts, verifies identity and waits until the first
  scanned range is persisted. `prev/` is deleted when the user confirms or 24 h after verified
  progress, and the UI says so. On boot an unfinished intent is resumed or rolled back;
  "Restore previous wallet" restores the whole snapshot.
- Upgrades: before starting a different friglet version, friglet-ui stops friglet cleanly and
  copies the state file to a 0600 backup tagged with the old version; a height reset after
  upgrade shows "this update required a rescan". READMEs give rollback steps (re-pin the
  previous digest, restore the backup). Package versions are `<friglet version or short
  commit>` plus a package revision (StartOS ExVer `x.y.z:N`). CI tests upgrade from the previous
  release on compose. Stop deadlines cover the 120 s grace: compose `stop_grace_period: 150s`,
  systemd `TimeoutStopSec=150`, StartOS daemon sigterm timeout 150 s.
- Per-platform acceptance checks before "install-tested": first install, restart, upgrade,
  backup and restore, uninstall and reinstall; on RaspiBlitz, repeated non-interactive `on`
  keeps the wallet and the UI password.
- Docs deliverables at M0: root README routing operators and contributors; compose quickstart
  run verbatim by CI; generated troubleshooting page; CONTRIBUTING (bootstrap, build, test,
  `make dev-regtest`, pin update via one script that rewrites every reference and lockfile,
  reviewer-response flow for each platform); a "try on signet" path; where raw logs live per
  platform with a warning that they contain txids, balances and IPs until BR8; a comparison
  table against the public Frigate server, Frigate on StartOS and Frigate on MiniBolt covering
  setup effort, dependencies, time to useful history, wallet limits and who sees what.
- Connect page copy matches the pinned Sparrow server dialog (per-field copy for host, port and
  SSL, plus the combined string), verified in M0.
<!-- /autoplan-accepted:dx -->

### Phase 3: Eng review (mode: FULL_REVIEW, scope never reduced)

#### Step 0: scope challenge

- Existing code per sub-problem: see CEO 0B; additionally `friglet-ipc` `Start`/`Stop`/`Shutdown`
  requests (`friglet-ipc/src/lib.rs:89-95`), friglet's supervisor `last_error`
  (`friglet/src/supervisor.rs:60-64`), testkit T2's friglet entrypoint and healthcheck.
- Complexity check triggers (well over 8 files, new services friglet-ui and image). The autoplan
  override is "never reduce" (P2); the four-platform scope is the owner's. Instead, friglet-ui
  features are phased: M0 = setup, process and scan supervision, status, Connect, auth,
  Electrum front; M1 adds widget, diagnostics, upgrade backups, Advanced escape hatches.
- Search check (Layer 1): `tini` for PID 1 instead of a hand-rolled reaper; `rustls` for the
  TLS front; `flock` for a single transaction owner. No custom crypto.
- Distribution check: CI builds and publishes binaries and a multi-arch image; registry and
  signing key are human items; release CI refuses `BLINDBIT_RS_SRC` builds.
- TODOS.md: not written (write access limited to this plan); every deferred item is collected
  in "NOT in scope" below.

#### Dual voices

Claude subagent (INPUT eng `3b775a84…b20f`, matched): 6 high (one "verify"), 15 medium/low, 1
scope note. Highest: StartOS interfaces are reachable on the LAN without login (E1); Sparrow's
subscribe likely carries the scan **private** key, so plaintext LAN leaks the secret (E2,
verify in M0); history readable by any caller (E3); dead scanner inside a live process (E4);
`start_height` must equal Sparrow's birthday, already answered by `friglet/src/types.rs:66-69`
(E7). Also: `last_error` fact wrong, Core does not answer `notfound` for pruned blocks, secret
inventory, login hardening behind nginx, two tips, two transaction writers, state truncation,
oracle jitter, log redaction, PID 1, taproot heights not in rust-bitcoin, re-resolve peer, umask.

Codex (completed, `Recommendation: Revise before implementation`): 9 high, 1 medium. Dead
scanner; no durable checkpoint acknowledgement for wallet replacement; first state write
permissions depend on umask; Forget leaves upgrade backups and sidecars; no single transaction
owner; heights can claim false completion; identity check does not gate Electrum exposure;
ingress boundary unspecified on gateway platforms; tests miss spend, reorg, disk-full, kill -9,
upgrade-rollback; one state machine conflates independent conditions.

```
ENG DUAL VOICES — CONSENSUS TABLE:
═══════════════════════════════════════════════════════════════
  Dimension                           Claude  Codex  Consensus
  ──────────────────────────────────── ─────── ─────── ─────────
  1. Architecture sound?               No      No     CONFIRMED (scan health, front, txn owner)
  2. Test coverage sufficient?         No      No     CONFIRMED
  3. Performance risks addressed?      Partly  n/a    DISAGREE (Codex silent) → single voice
  4. Security threats covered?         No      No     CONFIRMED (ingress, umask, inventory)
  5. Error paths handled?              No      No     CONFIRMED (dead scanner, pruned detection)
  6. Deployment risk manageable?       Partly  No     CONFIRMED (upgrade/rollback tests)
═══════════════════════════════════════════════════════════════
```
Single-voice critical-class items kept after checking: E1 (StartOS docs cited by the reviewer),
E2 (needs M0 packet capture; TLS default chosen regardless), E7 (code comment).
Cross-model tension: E3 key check. Claude wants BR1 or a key-checking proxy before the
community store; CEO-phase Codex preferred packaging access controls. Resolved toward BR1 as an
upstream gate (P4: no duplicate key check in friglet-ui) and recorded as part of taste TD2.

#### Section 1: Architecture

```
                 platform ingress (Umbrel gateway / StartOS addSsl.auth / nginx / compose)
 browser ─────────────────────────────▶ friglet-ui HTTP :8097 ──┐ auth, CSRF, Host/Origin
 Sparrow (dedicated) ──TLS :50032──▶ friglet-ui Electrum front ─┤ opens only after identity
 Tor sidecar / StartOS proxy ──:50031─▶ (plaintext, internal)   │ verified; closes on change
                                                                │
   tini (PID 1) ─▶ friglet-ui ──spawn (cleared env, umask 077, --config)──▶ friglet
                     │  supervises: process exit AND scan health (last_error, Start)  │
                     │  NDJSON over /data/friglet.sock (GetStatus/Start/Stop/Shutdown)│
                     │  P2P version handshake ──▶ bitcoind (tip, service bits)        │
                     ▼                                                                ▼
   /data (0700): config.toml, scan.key, state, *.headers.json, prev/, lkg/, intent,   friglet:
                 upgrade backup, ui-state            (secret-bearing inventory)       Electrum
                                                                                      127.0.0.1:50001,
                                                     oracle gRPC ◀── friglet ──▶ P2P  HTTP :8095
```
Findings (all auto-decided, P5/P3): A1 scan health supervision with `Start` and a `scan-stopped`
state; A2 Electrum front owns external listeners (TLS default, identity gate, connection close on
change); A3 one transaction owner (shared library; online CLI submits to the supervisor; offline
StartOS staging under `flock`, result "staged, verification pending"); A4 StartOS UI auth via
`addSsl.auth`; A5 `tini` as PID 1; A6 tip = min(node tip, oracle tip) with freshness; A7
re-resolve the peer on every friglet start on all platforms.

Production failure per integration point: oracle outage after a release (every box retries →
jitter); bridge IP change after bitcoind restart (re-resolve); power loss mid-checkpoint
(last-known-good restore); nginx in front makes every client 127.0.0.1 (per-client limit via
trusted XFF); friglet re-pin changes log text (golden-log fixtures fail CI).

#### Section 2: Code quality

- DRY: one transaction library used by web flow, CLI and StartOS; one state catalog; one pin
  source; one secret-file inventory consumed by Forget, uninstall, backups and the grep test.
- State model: four inputs (desired operation, process lifecycle, scan condition, wallet
  transaction) with explicit precedence deriving the presentation state ids from the design
  phase; events carry a daemon generation id so stale events are dropped.
- friglet-ui never calls `SetConfig` (it rewrites the TOML itself); friglet-ui is the only writer.
- Contradictions removed by this phase: `.prev` wording in the CEO and design blocks is
  superseded by the DX `prev/` transaction; the "StartOS relies on platform login" claim and the
  "`last_error` is always null" fact are corrected above; "birthday equal to tip is up to date"
  (DX block) is superseded: completion requires real scan coverage.

#### Section 3: Tests

No test framework exists yet (empty repo). Planned: `cargo test` for friglet-ui, testkit T2 for
end to end, shellcheck/linters per platform.

```
CODE PATHS                                              USER FLOWS
[+] friglet-ui/descriptor                               [+] Setup
  ├── [PLANNED ★★★] valid/invalid vectors, spspend       ├── [PLANNED →E2E] paste → verify → front opens
  └── [GAP→ADDED]  network mismatch per network          ├── [GAP→ADDED] birthday == Sparrow's (Sparrow-shaped subscribe)
[+] friglet-ui/supervisor                                └── [GAP→ADDED] pruned node blocks Submit (regtest -prune)
  ├── [PLANNED ★★] exits 0/1/134 via fake daemon        [+] Connect
  ├── [GAP→ADDED]  scanner dies, process alive          ├── [GAP→ADDED →E2E] Sparrow over TLS, cert trust
  ├── [GAP→ADDED]  docker stop during blocked P2P read  └── [GAP→ADDED] plaintext LAN refused unless opted in
  └── [GAP→ADDED]  backoff jitter                       [+] Wallet change
[+] friglet-ui/txn (wallet replacement)                  ├── [GAP→ADDED →E2E] kill -9 at each step, resume/rollback
  ├── [GAP→ADDED]  flock vs concurrent CLI               ├── [GAP→ADDED] connections closed during change
  └── [GAP→ADDED]  disk full at write and checkpoint    └── [GAP→ADDED] restore previous wallet
[+] friglet-ui/state + catalog                          [+] Forget / uninstall
  ├── [PLANNED ★★] every state renders                   └── [GAP→ADDED] secret grep over data dir, logs, diag
  └── [GAP→ADDED]  transition tests, stale generation   [+] Auth
[+] friglet-ui/front (Electrum)                          ├── [GAP→ADDED] StartOS addSsl.auth present in package
  ├── [GAP→ADDED]  gated until identity verified         ├── [GAP→ADDED] per-client limit behind nginx
  └── [GAP→ADDED]  notification passthrough              └── [GAP→ADDED] direct backend access, Origin/CSRF
[+] classifier                                          [+] Upgrade
  └── [GAP→ADDED]  golden-log fixtures per pin           ├── [GAP→ADDED →E2E] previous release → current → rollback
[+] daemon behaviour after BR3 re-pin                    └── [GAP→ADDED] state truncation → last-known-good
  ├── [GAP→ADDED →E2E] confirmed spend                  [+] arm64
  └── [GAP→ADDED →E2E] reorg rollback                    └── [GAP→ADDED] binaries execute on arm64 runner

COVERAGE (planned before this phase): 5/29 paths (17%)  |  after: 29/29 specified
QUALITY target: ★★★ for descriptor, txn, front, supervisor; E2E for setup, connect, change, upgrade
```
Regression rule: no existing code, so no regressions; testkit's own friglet stack is untouched.
Test plan artifact: `~/.gstack/projects/blindbit-node-apps/agent-integration-eng-review-test-plan-20261001-134531.md`.

#### Section 4: Performance

- Oracle outage stampede: exponential backoff with full jitter, cap 5 minutes.
- Worst case: a birthday at taproot activation (more than four years of tweaks on a Pi) is
  measured and published (time, oracle bytes, RSS), not only the one-year case.
- friglet-ui: one GetStatus in flight, every 2 s; P2P handshake for the node tip every 10 min;
  Electrum front adds one TLS hop (measure added latency on arm64; budget under 5 ms per request).
- Log redaction runs on forwarded lines only; the classifier reads an in-memory ring of raw lines.

#### Failure modes registry (eng)

```
  CODEPATH              | FAILURE MODE                                 | TEST? | HANDLED? | USER SEES
  ----------------------|----------------------------------------------|-------|----------|------------------
  scan task             | dies, process alive, stale Electrum           | Y     | Y        | scan-stopped, auto Start
  Electrum transport    | scan private key sniffed on LAN (if confirmed)| Y(M0) | Y        | TLS default
  Electrum access       | any caller reads history                      | N     | partial  | disclosed; BR1 gate  ← CRITICAL until BR1
  wallet change         | crash mid-change                              | Y     | Y        | resumed / rolled back
  state checkpoint      | truncated on power loss                       | Y     | Y        | restored or "rescan"
  StartOS UI            | LAN user without login changes settings       | Y     | Y        | basic-auth prompt
  login behind nginx    | global lockout                                | Y     | Y        | per-client limit
  birthday mismatch     | Sparrow rejects notifications silently        | Y     | Y        | birthday = Sparrow's
  pruned/syncing node   | disconnect or 90 s timeout misread            | Y     | Y        | node-pruned/node-syncing
  forget wallet         | secret copies left behind                     | Y     | Y        | inventory sweep
```
One critical gap remains open by design (S1/E3 history readable until BR1); it gates every
release beyond the maintainer.

#### Worktree parallelization

| Step | Modules touched | Depends on |
|---|---|---|
| friglet-ui core (descriptor, txn, supervisor, front, state) | `friglet-ui/` | — |
| image + CI + pins | `Dockerfile`, `.github/`, `versions.env`, `scripts/` | — |
| compose package + dev loop | `compose/`, `Makefile` | image |
| Umbrel package | `umbrel/` | image |
| StartOS package | `startos/` | image, friglet-ui `setup` CLI |
| RaspiBlitz script | `raspiblitz/` | binaries, signing key (human) |
| myNode app | `mynode/` | image |
| docs | `README.md`, `docs/`, `CONTRIBUTING.md` | state catalog |

Lane A: friglet-ui core. Lane B: image + CI + pins. Launch A and B in parallel; then compose and
Umbrel (lane C), StartOS (lane D) and myNode (lane E) in parallel; RaspiBlitz (lane F) waits for
signed binaries. Conflict flag: lanes C-E all edit `versions.env` through the pin script; run the
script on one lane only.

#### NOT in scope (all phases; TODOS.md not writable here)

- Local oracle companion app (owner non-goal); BigBear, MiniBolt, RaspiBolt and other surveyed
  platforms; RaspiBlitz WebUI PRs; official catalog submissions (human).
- Daemon changes: BR1-BR12 plus BR13 (switch off friglet's HTTP API) and BR14 (persisted
  checkpoint acknowledgement and write-failure propagation). Filed as tickets, not patched.
- Mockups (no designer key); localization; telemetry.

#### What already exists

`friglet-ipc` crate; friglet supervisor and `Start`/`Stop`/`Shutdown`; testkit T2 compose,
scenarios and Go client; `friglet-tray` ui, setup and lifecycle (ported); electrs/frigate
platform packages as patterns; `tini`, `rustls`, `flock` as building blocks.

#### Implementation tasks (eng)

- [ ] **E-T1 (P1, human: ~1d / CC: ~1h)** — friglet-ui — Scan-health supervision with `Start`, generation ids, layered state model
  - Surfaced by: E4 / Codex 1, Codex 10
  - Files: `friglet-ui/src/supervisor.rs`, `friglet-ui/src/state.rs`
  - Verify: test where the scanner dies with the process alive
- [ ] **E-T2 (P1, human: ~1d / CC: ~1h)** — friglet-ui — Electrum front (TLS 50032, gated 50031, connection close on change)
  - Surfaced by: E2, E3, Codex 7
  - Files: `friglet-ui/src/front.rs`
  - Verify: Sparrow connects over TLS; front closed until identity verified
- [ ] **E-T3 (P1, human: ~1d / CC: ~1h)** — friglet-ui — Single transaction owner with flock, offline staging, clean-stop checkpoint before commit
  - Surfaced by: E12, Codex 2, Codex 5
  - Files: `friglet-ui/src/txn.rs`, `friglet-ui/src/cli.rs`
  - Verify: kill -9 at each step; concurrent CLI refused
- [ ] **E-T4 (P1, human: ~4h / CC: ~30min)** — packaging — StartOS `addSsl.auth`, ingress binding and Origin/CSRF everywhere
  - Surfaced by: E1, E10, Codex 8
  - Files: `startos/startos/interfaces.ts`, `friglet-ui/src/http.rs`
  - Verify: direct backend access and unauthenticated mutation tests
- [ ] **E-T5 (P1, human: ~4h / CC: ~30min)** — friglet-ui — Secret inventory, umask 077, Forget/uninstall sweep with grep test
  - Surfaced by: E9, E20, Codex 3, Codex 4
  - Files: `friglet-ui/src/secrets.rs`
  - Verify: grep for the key after Forget finds nothing
- [ ] **E-T6 (P1, human: ~4h / CC: ~30min)** — CI — Behavioral tests: spend, reorg, disk full, upgrade+rollback, pruned and lagging node, PID 1 stop timing, arm64
  - Surfaced by: Section 3, Codex 9, E6, E17
  - Files: `.github/workflows/`, `tests/`
  - Verify: all run against the final re-pinned artifact
- [ ] **E-T7 (P2, human: ~2h / CC: ~15min)** — friglet-ui — Birthday equals Sparrow's; per-network taproot table; tips with freshness
  - Surfaced by: E7, E11, E18, Codex 6
  - Files: `friglet-ui/src/birthday.rs`, `friglet-ui/src/p2p.rs`
  - Verify: Sparrow-shaped subscribe in T2
- [ ] **E-T8 (P2, human: ~2h / CC: ~15min)** — friglet-ui — Log redaction for forwarded lines, golden-log fixtures, jittered backoff, last-known-good state
  - Surfaced by: E13, E14, E15
  - Files: `friglet-ui/src/logs.rs`, `tests/fixtures/`
  - Verify: fixtures fail CI when a catalogued line disappears

Completion summary (eng):
- Step 0: scope accepted as-is (override: never reduce); friglet-ui features phased M0/M1.
- Architecture: 7 issues found. Code quality: 4 issues found. Tests: diagram produced, 24 gaps
  identified and added. Performance: 4 issues found.
- NOT in scope: written. What already exists: written. TODOS.md: 0 written (no access), 14 BR
  tickets listed.
- Failure modes: 1 critical gap flagged (history readable until BR1).
- Outside voice: ran (codex completed). Parallelization: 6 lanes, 2 parallel at start then 3.
- Lake Score: 9/9 recommendations chose the complete option. Unresolved decisions: 0 auto;
  gate questions listed in the report.

<!-- autoplan-baseline-edits:eng {"sourceSha256":"03872715fededc6729fbf9f03b54a3dec936d3c3026f9a49cfd813e0cd10d551","replacements":[{"oldText":"  JSON, unversioned), which never waits on the scanner; the HTTP API hangs for as long as\n  scanning is enabled. `GetStatus.last_error` is always null and `scan_progress` covers only\n  the current range, so error states must be derived from friglet's log lines.","newText":"  JSON, unversioned), which never waits on the scanner; the HTTP API hangs for as long as\n  scanning is enabled. `GetStatus.last_error` is filled when the scan task ends with an error\n  (`friglet/src/supervisor.rs:60-64`), and the process then keeps serving stale Electrum data\n  without rescanning; `Start` and `Stop` requests exist. `scan_progress` covers only the\n  current range. Failures before the socket exists, and panics, are visible only in logs."},{"oldText":"   - mandatory password on platforms without gateway auth (RaspiBlitz, myNode, compose);\n     Umbrel and StartOS rely on the platform's own login.","newText":"   - mandatory password on platforms without gateway auth: RaspiBlitz, myNode, compose\n     (friglet-ui login) and StartOS (service interfaces are reachable on the LAN without a\n     StartOS login, so the `ui` binding uses `addSsl.auth` basic auth with a generated\n     password shown and rotated by an action); Umbrel relies on its gateway login (taste TD6);\n   - Electrum front: friglet binds Electrum to loopback only; friglet-ui owns the external\n     Electrum listeners (TLS on 50032 with a self-signed certificate whose fingerprint the\n     Connect page shows, plaintext 50031 only for the onion, the platform proxy or an explicit\n     opt-in), opens them only after the wallet identity is verified and closes client\n     connections during a wallet change;"},{"oldText":"3. **One multi-arch image** `friglet-node` containing both binaries; `friglet-ui` is the\n   entrypoint and PID 1. One image keeps one digest to pin per platform.","newText":"3. **One multi-arch image** `friglet-node` containing both binaries; the entrypoint is\n   `tini -- friglet-ui` so signal forwarding and zombie reaping are not hand-rolled. One image\n   keeps one digest to pin per platform. Native installs ship friglet and friglet-ui as one\n   tarball with one checksum, since the IPC between them is unversioned."},{"oldText":"as the `ui` interface, with \"Change wallet\" pointing back to the action. Electrum is a\n  `p2p`-style interface; StartOS terminates TLS, so wallets see `ssl://` and must trust the\n  server root CA.","newText":"as the `ui` interface, with \"Change wallet\" pointing back to the action. Electrum uses the\n  electrs/fulcrum/frigate pattern (`type: 'api'` with `schemeOverride` ssl/tcp, `addSsl` on the\n  bound port); StartOS terminates TLS, so wallets see `ssl://` and must trust the server root CA."}]} -->

<!-- autoplan-accepted:eng -->
- Scan-health supervision (eng): besides process exits, friglet-ui polls `GetStatus`; when
  `scanning` is false with `last_error` set and the user has not paused, it classifies the error
  and sends `Start` with jittered exponential backoff (cap 5 min); a `scan-stopped` state joins
  the catalog. "Pause scanning" uses `Stop`/`Start`. `Shutdown` is preferred over SIGTERM.
  Error classification uses `last_error` first and log lines only for failures before the
  socket exists and for panics (BR4 narrows to: report pre-socket failures, version the IPC).
- State model: four inputs (desired operation, process lifecycle, scan condition, wallet
  transaction) with explicit precedence produce the presentation state ids; events carry a
  daemon generation id so stale events are dropped; transition tests cover the precedence.
- Electrum front: friglet binds Electrum to `127.0.0.1:50001` inside its namespace;
  friglet-ui owns the external listeners: TLS on 50032 by default on every platform where the
  platform does not terminate TLS itself (self-signed certificate generated at install,
  fingerprint shown on the Connect page), plaintext 50031 only for the onion, the StartOS or
  nginx proxy, or an explicit Advanced opt-in. Listeners open only after identity is verified
  and client connections are closed during a wallet change. This revises the CEO exposure
  policy (taste TD2): the recommended LAN endpoint becomes TLS on every platform. M0 confirms
  by packet capture whether the pinned Sparrow sends the scan private key in
  `blockchain.silentpayments.subscribe`; the TLS default stays either way.
- BR1 (key-checked subscribe) now also gates the community-store announcement, not only
  official catalogs; no duplicate key check is built into friglet-ui.
- Birthday: the setup field takes the birthday height the user's Sparrow wallet uses, because
  friglet returns its configured `start_height` and Sparrow rejects a different one
  (`friglet/src/types.rs:66-69`). This supersedes the tip, taproot and date-estimate helpers in
  the CEO and design blocks for M0; M0 records where Sparrow shows that height and re-enables a
  helper only if Sparrow tolerates a different start. "Change wallet" also tells the user to
  change Sparrow's birthday. Taproot activation heights come from a tested table in friglet-ui
  (mainnet 709,632; signet and regtest active from genesis), not from rust-bitcoin.
- Heights: node tip (P2P handshake, refreshed every 10 minutes), oracle tip, scanned height and
  persisted height are tracked separately with timestamps; progress uses min(node tip, oracle
  tip); `node-syncing` shows when the node is behind the oracle; completion needs real scan
  coverage including the birthday block (supersedes "birthday equal to tip is up to date").
- Pruned and syncing detection does not rely on `notfound`: Bitcoin Core disconnects or stays
  silent, which friglet reports as a closed connection or a 90 s timeout; those signals plus
  handshake height and service bits (rechecked on every start) map to `node-pruned` and
  `node-syncing`. Tested on regtest with `-prune -fastprune` and with a lagging node.
- One transaction owner: a shared friglet-ui library performs setup and wallet replacement;
  the online CLI submits requests to the running supervisor; StartOS stages offline under an
  exclusive `flock` on the data dir and reports "staged, verification pending". Before
  `prev/` is deleted, friglet-ui performs a clean `Shutdown`, checks the state file parses and
  matches the new identity, fsyncs file and directory, and restarts. Kill -9 tests run at every
  step; concurrent writers are refused. BR14 asks friglet for a persisted-checkpoint
  acknowledgement.
- Last-known-good state: at each clean stop friglet-ui keeps one copy of the state file and
  restores it when a height reset after an unclean stop is detected; otherwise the page shows
  that a rescan started.
- Secret-bearing file inventory, owned by one module: `scan.key`, `config.toml`, state file,
  last-known-good copy, `prev/`, the transaction intent file, the single upgrade backup, plus
  platform backups (disclosed, not deleted). Forget and uninstall delete every local item; a
  test greps the data dir, logs, diagnostics and StartOS action results for the key afterwards.
  friglet is spawned with umask 077 and the data dir is 0700.
- Ingress: friglet-ui binds only where the platform's ingress reaches it; every state-changing
  request needs a Host/Origin allowlist match and a CSRF token on every platform, including
  `FRIGLET_UI_AUTH=none`; the widget endpoint is read-only with the fixed schema. StartOS uses
  `addSsl.auth` basic auth on the `ui` binding with a generated password in `store.json` and an
  action to show or rotate it. Umbrel relies on its gateway login (taste TD6; other apps on the
  Umbrel network can reach the backend, which matches Umbrel's existing trust in installed
  apps). Behind nginx the login limit is per client, trusting `X-Forwarded-For` only from local
  nginx; passwords are stored as argon2 hashes, compared in constant time, and a reset ends all
  sessions. Acceptance tests try direct backend access and unauthenticated mutations.
- Process hygiene: the image entrypoint is `tini -- friglet-ui`; friglet-ui never calls
  `SetConfig`; the node peer is re-resolved on every friglet start on all platforms and after
  repeated `peer-unreachable`; a CI test checks `docker stop` finishes inside the 150 s grace
  while friglet is blocked in a P2P read.
- Logs: forwarded friglet lines are redacted (txids, amounts, keys, IPs) on a best-effort basis
  until BR8; the classifier reads an in-memory ring of raw lines; golden-log fixtures per pin
  fail CI when a catalogued line disappears.
- Native installs: the Electrum exposure setting on RaspiBlitz and myNode is changed through
  the SSH menu or bonus script (ufw, nginx stream, Tor need root) and shown read-only in the web
  UI; friglet and friglet-ui ship as one tarball with one checksum.
- Release CI refuses `BLINDBIT_RS_SRC` builds. Behavioral tests run against the final re-pinned
  artifact: confirmed spend, reorg rollback, scanner death with a live process, disk full at
  config write and at checkpoint, kill -9 during wallet change, upgrade then rollback, pruned
  and lagging node, oracle down at boot and mid-scan with jitter, PID 1 stop timing, arm64
  execution. Worst-case scan from taproot activation is measured and published.
- Phasing of friglet-ui inside the milestones (nothing cut): M0 delivers setup, process and
  scan supervision, status, Connect, auth and the Electrum front; M1 adds the Umbrel widget,
  diagnostics, upgrade backup, last-known-good restore and the Advanced escape hatches.
- New blindbit-rs tickets: BR13 switch off the HTTP API (unauthenticated `/subscribe` on
  loopback); BR14 persisted-checkpoint acknowledgement with write-failure propagation.
<!-- /autoplan-accepted:eng -->

<!-- AUTONOMOUS DECISION LOG -->
### Decision Audit Trail

All rows are auto-decided by the review, not confirmed by the maintainer.

| # | Phase | Decision | Classification | Principle | Rationale | Rejected |
|---|-------|----------|----------------|-----------|-----------|----------|
| 1 | 0 | Skip `/office-hours` | Mechanical | P6 | Five research digests already exist | Run office-hours first |
| 2 | 0 | Base branch `master` (no remote) | Mechanical | P3 | Local default branch | `main` fallback |
| 3 | CEO | Mode SELECTIVE EXPANSION | Mechanical | autoplan override | Fixed for the CEO phase | Other modes |
| 4 | CEO | friglet-ui in Rust on `friglet-ipc` (approach C) | Taste TD1 | P4, P1 | Exact types vs unversioned protocol; parser and UI shared with the tray | A (Go), B (no UI) |
| 5 | CEO | Supervisor restarts on any exit, SIGTERM + 120 s | Mechanical | P1 | friglet exits 0 on bind failure, 134 on stream error | Restart on non-zero only |
| 6 | CEO | Image presets no `FRIGLET_*` env for keys friglet-ui writes | Mechanical | P5 | Env beats the config file | Keep upstream presets |
| 7 | CEO | Explicit `oracle_url` per network | Mechanical | P1 | Default is mainnet for every network | Rely on the default |
| 8 | CEO | Per-platform milestones; compose and Umbrel first | Mechanical | P1, P6 | Both voices; nothing cut | Ship all four together |
| 9 | CEO | Release gate: BR3 merged and Sparrow receive/spend/restart | Mechanical | P1 | Linters cannot prove a payment is found | Linter-only gate |
| 10 | CEO | Official catalogs also wait for BR1 | Taste TD2 | P1 | Claude wants a daemon guard, Codex packaging controls | Submit with disclosure only |
| 11 | CEO | Exposure policy: LAN + onion, platform TLS, LAN-scoped ufw, onion behind auth | Taste TD2 | P1, P5 | Usable now, honest about the leak | Electrum off by default |
| 12 | CEO | Document a dedicated Sparrow instance until BR2 | Mechanical | P1 | Single-server mode empties other wallets | Ignore |
| 13 | CEO | SP address cross-check before "setup complete" (D5) | Mechanical | P1 | Catches a wrong descriptor | None |
| 14 | CEO | Wallet change uses a fresh state file | Mechanical | P1 | `SetScanKey` ignored for an existing state | Live `SetScanKey` |
| 15 | CEO | Accept delights D1-D4, D6 | Mechanical | P2 | Inside friglet-ui, under a day each | Skip |
| 16 | CEO | Defer local oracle companion (X1) | Mechanical | Owner non-goal | Premise corrected, decision stays the owner's | Add to scope |
| 17 | CEO | Defer BigBear and guides (X2) | Mechanical | Owner scope | Survey only | Add to scope |
| 18 | CEO | Daemon gaps become BR1-BR12 tickets | Mechanical | Owner scope | No friglet patches here | Patch friglet |
| 19 | CEO | Mandatory UI password on non-gateway platforms | Mechanical | P1 | Setup writes the wallet secret | Optional password |
| 20 | CEO | Origin check and per-session token on state-changing requests | Mechanical | P1 | CSRF behind Umbrel cookies | None |
| 21 | CEO | Reject `spspend` input | Mechanical | P1 | Never store a spend secret | Accept and ignore |
| 22 | CEO | friglet HTTP API on `127.0.0.1:8095` | Mechanical | P5 | 8080 collides natively; endpoints hang | 8080 |
| 23 | CEO | testkit T2 in CI; signet only as manual smoke | Mechanical | P4 | Deterministic, constructed ground truth | Public signet in CI |
| 24 | CEO | No "lighter than Frigate" claim without arm64 numbers | Mechanical | Merge bar | Measure, don't relay | Unmeasured claim |
| 25 | CEO | Enforce unpruned per platform; name the `notfound` error | Mechanical | P1 | Eligibility, not a note | Docs only |
| 26 | CEO | Correct the Core-29.x premise | Mechanical | Evidence | Umbrel 31.1/30.3, StartOS 31.1 | Keep text |
| 27 | Design | State machine with 15 named states shared by page, widget, StartOS health | Mechanical | P5, P1 | Both voices critical: nothing defined while friglet is down | Spinner + raw status |
| 28 | Design | Dedicated-Sparrow step first on Connect and in store copy | Mechanical | P1 | Both voices critical | Warning text only |
| 29 | Design | Birthday question, conservative estimate, "I don't know" = taproot activation | Mechanical | P1 | Too-late birthday silently loses payments | Single height field |
| 30 | Design | Pruned/syncing detection from P2P service bits before oracle contact | Mechanical | P1 | Umbrel/compose users learned only after the wait | Log-only detection |
| 31 | Design | Overall progress computed by friglet-ui; ETA only when stable | Mechanical | P5 | `scan_progress` is per range | Show `scan_progress` |
| 32 | Design | Remove tray Start/Stop, path and bind fields; network and peer read-only | Mechanical | P5 | Desktop controls do not belong on the box | Copy tray as is |
| 33 | Design | Per-platform endpoint matrix, one recommended endpoint each | Mechanical | P1 | Endpoints differ by platform | Generic "Sparrow steps" |
| 34 | Design | Remove QR code | Taste TD4 | P5 | No consumer; desktop Sparrow does not scan | Keep with defined payload |
| 35 | Design | Tray palette + light theme + Plex fonts, no stacked cards | Taste TD3 | P4, P5 | Continuity with the tray; fixes slop items | New visual direction |
| 36 | Design | StartOS action calls `friglet-ui setup` | Mechanical | P4 | One parser and one set of birthday rules | Parser in TypeScript |
| 37 | Design | Password lifecycle per platform, never set on first visit | Mechanical | P1 | LAN claim race | First-visit password |
| 38 | Design | `.prev` deleted after identity verified; Forget wallet | Mechanical | P1 | `.prev` holds the old secret | Keep indefinitely |
| 39 | Design | Oracle URL editable under Advanced everywhere | Mechanical | P5 | One code path; operator-configurable endpoint | Compose only |
| 40 | Design | No balance shown; "Payments found N" | Mechanical | P1 | Balance wrong until BR3 | Show balance |
| 41 | Design | Mockups skipped | Mechanical | n/a | Designer has no OpenAI key (human item) | Auto-pick variants |
| 42 | Design | M0 check of Sparrow's start-height rule | Mechanical | P1 | `types.rs:66-71` comment | Assume either way |
| 43 | DX | Persona: node-box operator first, packaging contributor second | Mechanical | P6 | Inferred from plan and NOTES | Ask the user |
| 44 | DX | TTHW targets: <10 min operator, <10 min contributor | Mechanical | P5 | Competitive tier; Frigate needs hours of indexing | Champion tier |
| 45 | DX | Compose `FRIGLET_UI_NODE` with hostname resolution | Mechanical | P1 | friglet rejects hostnames | Literal IP only |
| 46 | DX | Start friglet with cleared env; refuse stray `FRIGLET_*` | Mechanical | P5 | Env beats the file (`config.rs:132,263-267`) | Inherit env |
| 47 | DX | `friglet-ui setup` contract (stdin secret, --replace, --json, exit codes) | Mechanical | P1 | Secret in argv risk; StartOS needs it | Undefined CLI |
| 48 | DX | Ports table: 50031/50032/8095/8097/8098/2104/8336 | Mechanical | P5 | Gateways and nginx need fixed ports | Leave to implementer |
| 49 | DX | One pin source, generated Cargo rev, `BLINDBIT_RS_SRC` | Mechanical | P4 | Unversioned IPC drift; BR3 testing | Two hand-kept pins |
| 50 | DX | `make dev-regtest` local loop | Mechanical | P1 | Contributors had CI-only e2e | CI only |
| 51 | DX | State catalog generating UI, widget, health, docs | Mechanical | P4 | One source for error copy | Copy per surface |
| 52 | DX | Retry policy: permanent classes stop retrying | Mechanical | P1 | Restart storms on permanent faults | Retry everything |
| 53 | DX | Advanced escape hatches (peer, oracle, exposure, pause, log level) | Mechanical | P1 | "Decide for me, let me override" | Fixed defaults only |
| 54 | DX | `FRIGLET_UI_AUTH=none` on compose with warning | Taste TD5 | P1 vs security | Users with their own auth proxy | Password always |
| 55 | DX | StartOS action runs `friglet-ui setup` in a one-off subcontainer | Mechanical | P5 | Critical task blocks service start | Unconfigured daemon |
| 56 | DX | Wallet replacement as a durable transaction; `prev/` kept until confirmed or 24 h | Mechanical | P1 | Codex: identity check is not proof of scanning | Delete at identity check |
| 57 | DX | State backup before a friglet version change; upgrade test | Mechanical | P1 | Non-atomic state, unknown compatibility | No upgrade contract |
| 58 | DX | Stop deadlines 150 s on every platform | Mechanical | P1 | SIGTERM can wait 90 s on P2P reads | Platform defaults |
| 59 | DX | Per-platform acceptance checks incl. RaspiBlitz repeated `on` | Mechanical | P1 | Linters do not prove install/recovery | Lint only |
| 60 | DX | Docs set at M0 incl. comparison with public Frigate | Mechanical | P1 | Discover stage friction | README later |
| 61 | DX | arm64 execution in CI | Mechanical | P1 | `file` proves architecture only | `file` check |
| 62 | Eng | Scope not reduced; friglet-ui features phased M0/M1 | Mechanical | P2 override | Owner's four-platform scope | Cut platforms |
| 63 | Eng | Scan-health supervision with `Start` and `scan-stopped` | Mechanical | P1 | Scanner can die inside a live process | Exit-only supervision |
| 64 | Eng | Correct the `last_error` fact | Mechanical | Evidence | `supervisor.rs:60-64` fills it | Keep text |
| 65 | Eng | Electrum front in friglet-ui: TLS default, identity gate, close on change | Taste TD2 (revised) | P1, P5 | Sparrow likely sends the scan private key; gate needs one owner of listeners | Plaintext LAN default |
| 66 | Eng | BR1 also gates the community-store announcement | Mechanical | P1 | History readable by any caller | Official catalogs only |
| 67 | Eng | Birthday = Sparrow's birthday; helpers off until M0 shows otherwise | Mechanical | P1 | `types.rs:66-69` | Tip/date helpers |
| 68 | Eng | Heights tracked separately; completion needs scan coverage | Mechanical | P1 | False completion risk | Single tip |
| 69 | Eng | Pruned/syncing detection from disconnects, timeouts, service bits | Mechanical | P1 | Core does not send `notfound` for those blocks | `notfound` mapping |
| 70 | Eng | One transaction owner with flock and clean-stop checkpoint | Mechanical | P5 | Two writers raced | Per-surface logic |
| 71 | Eng | Last-known-good state copy | Mechanical | P1 | Non-atomic writes on Pis | Silent rescan |
| 72 | Eng | Secret inventory, umask 077, Forget sweep test | Mechanical | P1 | Copies left behind | Partial delete |
| 73 | Eng | StartOS `addSsl.auth`; Origin/CSRF everywhere; per-client limit behind nginx | Mechanical | P1 | StartOS UI open on LAN | Rely on platform login |
| 74 | Eng | Umbrel relies on gateway login | Taste TD6 | P5 | Matches Umbrel's trust model | Second login with APP_PASSWORD |
| 75 | Eng | `tini` as PID 1; never `SetConfig`; re-resolve peer everywhere | Mechanical | P5 | Boring by default | Hand-rolled PID 1 |
| 76 | Eng | Redacted log forwarding, golden-log fixtures | Mechanical | P1 | Logs leak wallet data | Raw forwarding |
| 77 | Eng | Behavioral test set on the final artifact | Mechanical | P1 | Release gate must prove spend/reorg/recovery | Smoke only |
| 78 | Eng | BR13 and BR14 tickets | Mechanical | Owner scope | Daemon-side gaps | Patch friglet |

### Phase 4: Final approval gate (non-interactive)

**Plan summary.** Package friglet as one Rust-built image plus glibc 2.36 binaries with a Rust
`friglet-ui` (setup, supervision, status, Electrum front), delivered compose → Umbrel community
store → StartOS → RaspiBlitz → myNode; daemon gaps go to blindbit-rs as BR1-BR14 and gate any
release beyond the maintainer.

**Decisions:** 78 total (72 auto-decided mechanical, 6 taste, 0 user challenges). No change to
the owner's stated direction had agreement from both models, so no user challenge was raised.
Gate result: option A ("approve as-is") **auto-selected by the review**, because the session is
non-interactive; it is not maintainer approval.

**Taste choices (auto-decided, open to override):**
- TD1 (CEO): friglet-ui in Rust on `friglet-ipc`. Go stays viable: faster builds, but
  hand-copied types against an unversioned protocol.
- TD2 (CEO, revised in Eng): TLS by default for LAN Electrum via the friglet-ui front; BR1 gates
  the community store too. Alternative: plaintext LAN by platform convention with disclosure.
- TD3 (Design): tray palette plus light theme and Plex fonts. Alternative: a new visual
  direction from a mockup round once a designer key exists.
- TD4 (Design): no QR code on Connect. Alternative: keep it with a defined `host:port:s` payload.
- TD5 (DX): `FRIGLET_UI_AUTH=none` allowed on compose with warning. Alternative: password always.
- TD6 (Eng): Umbrel relies on its gateway login. Alternative: a second login with Umbrel's
  deterministic `APP_PASSWORD`.

**Premise items queued for the maintainer (from CEO 0A):** P1 Sparrow single-server trap
(launch before BR2 with the dedicated-instance workaround?), P5 local oracle now feasible on
Umbrel/StartOS (keep the non-goal?), P6 pinned friglet lacks spend/reorg fixes (BR3 merges are
the maintainer's).

**Superseded tasks.** D-T3's estimator and helper parts and T6's D2 helper are superseded by
E-T7 for M0 (birthday equals Sparrow's); D-T1's single state machine is refined by E-T1's layered
model; the `.prev` wording in T-level CEO/design text is superseded by the DX/Eng transaction.

#### Cross-phase themes (raised independently in 2+ phases)

- **Electrum port exposes the wallet** (CEO, Design, Eng; both voices each time). High-confidence
  signal: drove TLS default, identity gate, BR1 gate.
- **Proof must be a found payment, not a linter** (CEO Codex, Design Codex, DX Codex, Eng Codex).
- **Wallet change and state durability** (CEO Codex, DX Codex, Eng both).
- **Birthday must match Sparrow** (Design both, DX Codex, Eng both).
- **Error states lack a daemon signal and a state model** (Design both, DX both, Eng both).
- **StartOS lifecycle and auth** (Design Claude, DX Codex, Eng Claude).
- **Single-server Sparrow trap** (CEO Claude, Design both).

#### Implementation tasks (aggregated across phases)

- [ ] **T4 (P1, human: ~1d / CC: ~1h) — ci** — testkit T2 end-to-end with the packaged image as release gate
- [ ] **T5 (P1, human: ~2h / CC: ~15min) — docs** — Sparrow dedicated-instance instructions and connection-page warning
- [ ] **T2 (P1, human: ~1d / CC: ~1h) — friglet-ui** — Supervisor restarts on any exit, SIGTERM + 120 s grace, log classifier
- [ ] **T3 (P1, human: ~4h / CC: ~30min) — friglet-ui** — Descriptor parser rejecting spspend and wrong network; SP address cross-check
- [ ] **T1 (P1, human: ~2h / CC: ~15min) — packaging** — Pin friglet-ui to friglet-ipc at friglet's commit; record both in versions.env
- [ ] **D-T4 (P1, human: ~2h / CC: ~15min) — docs** — Dedicated Sparrow step with tested per-OS commands
- [ ] **D-T3 (P1, human: ~2h / CC: ~15min) — friglet-ui** — Birthday question, conservative estimate, handshake tip, pruned/syncing detection
- [ ] **D-T1 (P1, human: ~1d / CC: ~1h) — friglet-ui** — State machine and state table shared by page, widget and StartOS health
- [ ] **D-T2 (P1, human: ~1d / CC: ~1h) — friglet-ui** — Setup, Status, Connect, Wallet, Diagnostics screens per wireframes
- [ ] **E-T6 (P1, human: ~4h / CC: ~30min) — ci** — Behavioral tests on final artifact: spend, reorg, disk full, upgrade+rollback, pruned/lagging, PID 1, arm64
- [ ] **E-T3 (P1, human: ~1d / CC: ~1h) — friglet-ui** — Single transaction owner with flock, offline staging, clean-stop checkpoint
- [ ] **E-T2 (P1, human: ~1d / CC: ~1h) — friglet-ui** — Electrum front: TLS 50032, gated 50031, close on change
- [ ] **E-T5 (P1, human: ~4h / CC: ~30min) — friglet-ui** — Secret inventory, umask 077, Forget/uninstall sweep with grep test
- [ ] **E-T1 (P1, human: ~1d / CC: ~1h) — friglet-ui** — Scan-health supervision with Start, generation ids, layered state model
- [ ] **E-T4 (P1, human: ~4h / CC: ~30min) — packaging** — StartOS addSsl.auth, ingress binding, Origin/CSRF everywhere
- [ ] **X-T2 (P1, human: ~4h / CC: ~30min) — compose** — FRIGLET_UI_NODE/NETWORK, .env.example, quickstart run in CI
- [ ] **X-T3 (P1, human: ~1d / CC: ~1h) — friglet-ui** — State catalog generating UI copy, widget, health text, troubleshooting
- [ ] **X-T1 (P1, human: ~4h / CC: ~30min) — friglet-ui** — setup CLI contract and env isolation
- [ ] **X-T4 (P1, human: ~4h / CC: ~30min) — repo** — make dev-regtest, single pin source, BLINDBIT_RS_SRC
- [ ] **T6 (P2, human: ~4h / CC: ~30min) — friglet-ui** — Delights D1-D6
- [ ] **T7 (P2, human: ~1d / CC: ~1h) — perf** — Measure time to first balance, RSS, oracle bytes on arm64
- [ ] **T8 (P2, human: ~1h / CC: ~10min) — tickets** — File BR1-BR12 in blindbit-rs
- [ ] **D-T5 (P2, human: ~2h / CC: ~15min) — friglet-ui** — Tokens, Plex fonts, light/dark theme, friglet-ui/DESIGN.md
- [ ] **D-T6 (P2, human: ~4h / CC: ~30min) — packaging** — Password lifecycle per platform; StartOS action calls friglet-ui setup
- [ ] **E-T7 (P2, human: ~2h / CC: ~15min) — friglet-ui** — Birthday equals Sparrow's; taproot table; tips with freshness
- [ ] **E-T8 (P2, human: ~2h / CC: ~15min) — friglet-ui** — Log redaction, golden-log fixtures, jittered backoff, last-known-good state
- [ ] **X-T6 (P2, human: ~4h / CC: ~30min) — docs** — Root README routing, CONTRIBUTING, comparison table, raw-log locations
- [ ] **X-T5 (P2, human: ~4h / CC: ~30min) — friglet-ui** — Upgrade state backup, rollback doc, previous-release upgrade test

#### Coverage by phase

| Phase | Host | Outside provider | Outside status | Native completion | Findings |
|---|---|---|---|---|---|
| CEO | claude | codex | completed | completed (INPUT matched) | Claude 11, Codex 8 |
| Design | claude | codex | completed on retry (first run timed out at 600 s) | completed (INPUT matched) | Claude 20, Codex 12 |
| DX | claude | codex | completed | completed (INPUT matched) | Claude 22, Codex 12 |
| Eng | claude | codex | completed | completed (INPUT matched) | Claude 24, Codex 10 |

Learnings logged: see completion report. Next step: `/ship` only after the maintainer answers
the open questions below and implementation exists; no code was written by this review.

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
|--------|---------|-----|------|--------|----------|
| CEO Review | `/plan-ceo-review` (via /autoplan) | Scope & strategy | 1 | issues_open | 9 proposals, 6 accepted, 3 deferred; 2 daemon-side critical gaps |
| Outside Review | codex via /autoplan (all four phases) | Independent 2nd opinion | 5 (design retried) | completed | CEO 8, Design 12, DX 12, Eng 10 concerns |
| Eng Review | `/plan-eng-review` (via /autoplan) | Architecture & tests (required) | 1 | issues_open | 39 issues, 1 critical gap (history readable until BR1) |
| Design Review | `/plan-design-review` (via /autoplan) | UI/UX gaps | 1 | issues_open | score: 0/10 → 7/10, 12 decisions |
| DX Review | `/plan-devex-review` (via /autoplan) | Developer experience gaps | 1 | issues_open | score: 3/10 → 5/10, TTHW: ~15 min + scan → <10 min |

- **OUTSIDE COVERAGE:** codex completed for CEO, DX and Eng; Design completed on the second attempt
  after a 600 s timeout. No phase relied on a native fallback for outside coverage.
- **CROSS-MODEL:** both voices independently flagged the Electrum wallet exposure, missing
  end-to-end payment proof and the error-state model; Codex alone raised upgrade and transaction
  durability, Claude alone raised the Sparrow single-server trap and StartOS LAN auth (both
  verified against sources before acceptance).
- **VERDICT:** no review is CLEAR. The plan is revised and auto-approved by the review for
  implementation planning; eng review required again after the maintainer answers the questions
  below and BR1/BR3 land, before any release.

**UNRESOLVED DECISIONS:**
- TD1-TD6 taste decisions are auto-decided and await maintainer confirmation or override.
- P1: launch the community store before BR2 forwarding, with the dedicated-Sparrow workaround?
- P5: keep "no bundled oracle" now that Core 30+/31.1 ships on Umbrel and StartOS?
- P6: merge the blindbit-rs fixes behind BR3 (spend detection, reorg, stream panic) so the pin can move?
- Registry, signing key and store-facing app ids (ids are permanent).
- `upstream/` licensing and repo visibility; allow generated publish-only mirror repos for the Umbrel store and the StartOS submission despite the one-repo rule?
- Who owns oracle.setor.dev availability and capacity for public node-box users?
