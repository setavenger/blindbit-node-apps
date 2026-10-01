# Survey: other node-in-a-box and self-hosting platforms for friglet

Date: 2026-10-01. Scope: every platform except Umbrel (including Umbrel community app stores),
StartOS, RaspiBlitz and myNode.

friglet is a headless Rust daemon (BIP-352 / Silent Payments). It serves HTTP on :8080 and the
Electrum protocol over TCP on :50001, and it needs a Bitcoin P2P peer and a BlindBit oracle, not bitcoind RPC. Pulled files are under
`<platform>/<repo>/`; provenance and licenses are in [`SOURCES.md`](SOURCES.md).

- **Status dates** are the last commit on the default branch and the last release or tag, taken from
  the GitHub and GitLab APIs (not `pushed_at`).
- **Recommendations** are proposals; none of them has been confirmed.

## Cross-cutting findings

- **A precedent already exists.** Sparrow's **Frigate** (Apache-2.0, 1.5.3, 2026-05-30) is a BIP-352
  Electrum server, and it is already packaged as a MiniBolt bonus guide
  (`minibolt/minibolt/bonus-guides/bitcoin/frigate.md`). It is also on StartOS (`remcoros/frigate-startos`,
  covered by the StartOS agent). It is both the closest template for friglet and its direct competitor.
- **Name collision.** The "Frigate" apps in the CasaOS and TrueNAS stores are the unrelated NVR
  (`blakeblackshear/frigate`), not Sparrow's server.
- **Port clashes.** On MiniBolt, 8080 is LND REST and 50001 is Fulcrum. Frigate deliberately uses
  `50011`/`50012` and forwards non-SP Electrum calls to Fulcrum or electrs
  (`minibolt/minibolt/bonus-guides/bitcoin/frigate.md`, lines 50 and 307). On the general stores,
  50001 is also typically taken by electrs.
- **A bitcoind app must exist.** Among the general stores, only TrueNAS (bitcoind, Knots), Unraid
  (bitcoind, Knots), Runtipi (bitcoind, outdated), DAppNode (bitcoin), Citadel/Nirvati and BigBear
  (Knots only) ship one. The official CasaOS/ZimaOS store, Cosmos and YunoHost ship none.
- **Fulcrum** is packaged by MiniBolt, RaspiBolt, Parmanode, Citadel and the archived nix-bitcoin, but
  by **no** general-purpose store. electrs is the common reference there.
- **Copyleft.** A friglet package adapted from the TrueNAS (LGPL-3.0), Runtipi (GPL-3.0), Citadel
  (AGPL-3.0), DAppNode (GPL-3.0) or linuxserver (GPL-3.0) reference files inherits their license.
  The MIT and Apache sets (MiniBolt, RaspiBolt, BTCPay, BigBear, CasaOS) do not impose this.

## Comparison table

| Platform | Status (last commit / release) | Mechanism | Docker? | Third-party path | Electrum ref app (bitcoind?) | User signal | License | Rec. | Reason |
|---|---|---|---|---|---|---|---|---|---|
| **MiniBolt** (guide) | Active: 2026-08-10 / v2.3 2026-08-10 | GitBook Markdown guide; systemd, build from source | No | PR as an "independent bonus guide" | Fulcrum (core), electrs (bonus), **Frigate (bonus)**; Bitcoin Core and Knots guides | 93★, EN/ES Telegram | MIT | **Now** | Frigate precedent; a small text-only PR reaches the DIY Ubuntu crowd |
| **RaspiBolt v4** (guide) | Active: 2026-08-19 / `v3-final` 2026-06-15 (v4 rewrite live) | Fumadocs MDX guide; systemd | No | PR to `guide/bonus/bitcoin/` | electrs (core), Fulcrum (bonus); Bitcoin Core | 1,260★ (largest guide) | MIT | **Now** | Largest guide audience, no SP page yet, clear bonus-guide path |
| **TrueNAS SCALE** (community train) | Active: 2026-09-30 / rolling | `app.yaml`, `ix_values.yaml`, `questions.yaml`, Jinja compose plus a Python library | Yes | PR to `ix-dev/community/` (the BTC apps came from an outside contributor) | electrs; bitcoind 31.1, Knots | 428 community apps, ~200 contributors, big NAS base | LGPL-3.0 | **Now** | Mature, accepts Bitcoin apps, and electrs gives a 1:1 template |
| **Unraid CA** | Active: feed updated 2026-10-01 | XML template in your own repo plus a forum support thread | Yes | Self-hosted template repo, submitted to CA (2FA needed; nodes allowed, mining banned) | electrs (anym001, ofawx), ElectrumX (ofawx, stale); bitcoind, Knots (linuxserver) | 4,543 templates; Knots ~26k pulls | Varies per repo (anym001: none) | **Now** | Cheap (one XML file), but needs a maintained image plus a support thread |
| **BigBear universal apps** (feeds CasaOS, ZimaOS, Runtipi, Cosmos, Portainer, Dockge, Umbrel) | Active: 2026-10-01 / none | Universal `app.json` plus compose, auto-converted per platform | Yes | PR to `big-bear-universal-apps` | none; **Knots only** | 620★ (casaos store), 462 apps; ZimaOS-recommended store | MIT | **Now** | One PR reaches about 6 platforms; Knots gives a bitcoind dependency |
| **BTCPay Server docker** | Active: 2026-09-30 / rolling (no tags) | compose-generator fragments (`opt-add-*.yml`) | Yes | Upstream PR, or a local `*.custom.yml` fragment without upstream | ElectrumX (`opt-add-electrumx.yml`: needs `opt-txindex`, incompatible with pruning); bundled bitcoind | 753★, large merchant base | MIT | **Later** | Default installs are pruned; maintainers slow on indexers (Fulcrum PR abandoned 2023) |
| **ZimaOS / CasaOS** (official store) | Store active 2026-09-24 (v2.0.19); CasaOS core stagnant (last tag v0.4.17-alpha1, 2025-04) | compose plus `x-casaos` metadata, built to `store.json` | Yes | PR to the official store (crypto accepted: AlbyHub), or any third-party store URL | none; **no bitcoind** | CasaOS 37k★ | Apache-2.0 | **Later** | No bitcoind in the official store; reach it through BigBear first |
| **Runtipi** | Active: 2026-09-12 / v4.10.2 2026-09-08 | `config.json` plus `docker-compose.json` (dynamic compose) | Yes | **Official store closed to new apps**; custom store repo by git URL | electrs (getumbrel image); bitcoind (lncm v28, outdated) | 9.7k★ | GPL-3.0 (store) | **Later** | Only via own or BigBear store; electrs reference exists |
| **DAppNode** | Active: 2026-08-31 / v0.3.24; bitcoin pkg 2026-07-09 | `dappnode_package.json` plus compose, published to IPFS/APM | Yes | Public (`.public`) registry or install by IPFS hash; main registry is whitelisted | none; bitcoin (v31.1) | 625★, Ethereum-centric | GPL-3.0 (pkg); docs unlicensed | **Later** | bitcoind exists, but the user base is Ethereum stakers |
| **Citadel / Nirvati** | Low: legacy store 2026-08-07, core v0.3.12 2025-09-20; Citadel 0.4 on Nirvati not shipped | compose plus `app.yml` (Jinja) in app stores (`app-store.yml`) | Yes | PR to the store, or your own store repo | electrs, Fulcrum (v5); Nirvati electrs; Bitcoin Core and Knots | 49★ (old GitHub), 0–2★ on GitLab | AGPL-3.0 / GPL-3.0; possible license change | **Later (low)** | Tiny base, platform in flux, license unclear |
| **Parmanode** | Active: 2026-09-22 / v3.70.11 (`version.conf`; git tags stale) | Bash installer (some apps in docker) | Partly | PR (98 of the last 100 commits by the maintainer) | electrs, Fulcrum, ElectrumX modules; bitcoind and Knots | 114★ | MIT plus a non-standard T&C | **Later (low)** | Single maintainer; integration means bash menus plus systemd and docker variants |
| **NixOS / nixpkgs** (successor for nix-bitcoin users) | Active: 2026-10-01 | Nix package plus NixOS module | No | PR to nixpkgs | electrs and Fulcrum **packages only, no modules**; `services.bitcoind` module | nixpkgs-wide | MIT | **Later** | Absorbs nix-bitcoin users; needs a Rust package and a module |
| nix-bitcoin | **Dead**: archived 2026-08-13 (final v0.0.139) | NixOS modules | No | none | electrs, Fulcrum modules (kept as references) | 615★ | MIT | Skip | Unmaintained |
| RoninDojo | Semi-active: v2.4.x, v3 beta (~2026-04); onion-only Gitea | Bash plus Samourai Dojo compose | Yes | none | Fulcrum/electrs built in | unknown | unverified | Skip | Fixed stack, no app channel, no clearnet source |
| Ashigaru Dojo | Tor-only repo | Dojo compose | Yes | none | Fulcrum bundled | unknown | unverified | Skip | Fixed stack |
| nodl (One Mk2 / Onyx) | Commercial; Onyx "ships Summer 2026" (unconfirmed) | Closed appliance | ? | none | electrs bundled | ">1,300 users" (vendor claim) | unverifiable | Skip | Closed, no third-party path |
| FutureBit Apollo II/III | Active: apolloapi-v2 v2.1.3 2026-08-19 | Dashboard on Ubuntu, no app store | No | none | none (community electrs script only) | 42★ | **CC-BY-NC-ND-4.0** | Skip | No app mechanism; restrictive license |
| Cyphernode | Low: dev 2026-02-24 / v0.9.0-rc.4 2023 | Swarm plus configurator | Yes | PR | none (an unmerged `features/silent` branch has BIP-352 bits) | 396★, B2B | MIT | Skip | Merchant API, not wallet-connect; watch the SP branch |
| Cosmos Cloud | Active: v0.23.4 2026-08-30 | `servapps` `description.json` plus `cosmos-compose.json` | Yes | PR or custom market URL | none (Knots via BigBear) | 6.2k★ | Market AGPL-3.0; server Apache-2.0 plus Commons Clause | Skip | Covered by BigBear conversion |
| YunoHost | Active | Bash `_ynh` packages | No | PR, but **rejects crypto apps** (`rejectedlist.toml`) | none | 262★ (apps repo) | GPL-3.0 | Skip | Policy refuses crypto |
| AVADO | Stale (bitcoin pkg 2021) | DAppNode fork | Yes | — | stale bitcoin | 1★ | GPL-3.0 | Skip | Inactive |
| Portainer templates | 2026-06-12 | `templates-2.0.json` | Yes | Curated; custom URL | none | 413★ | none | Skip | No Bitcoin apps; BigBear converts anyway |
| Coolify / CapRover / HA add-ons / SynoCommunity / Proxmox community-scripts | Active | Various | Mixed | Various | none (Coolify has a stale bitcoin-core) | — | — | Skip | No Bitcoin node ecosystem |
| Bitcoin-Standup-Scripts | 2026-08-06 / v0.8.0 (2021) | Bash | No | PR (CLA) | none | 57★ | BSD-2-Clause-Patent | Skip | Bitcoin Core plus Tor only |
| Ministry of Nodes Nodebox | 2024 video series | Video guide, no repo | No | none | electrs | YouTube | none | Skip | No contribution channel |
| BitBox Base / Casa Node / Lightning-in-a-box / eznode / "Bitcoin Suite" | Dead or not found | — | — | — | — | — | — | Skip | Discontinued |

## Per-platform notes: support now

### MiniBolt
- **Path:** a PR adding `bonus-guides/bitcoin/friglet.md`, a link in `SUMMARY.md`, and a filled
  `.github/pull_request_template.md`. Copy the structure of `minibolt/minibolt/bonus-guides/bitcoin/frigate.md`:
  - requirements;
  - a dedicated `frigate` user with systemd;
  - a `ufw allow` rule;
  - the ZMQ `zmqpubsequence` check against Bitcoin Core;
  - ports offset from Fulcrum's.
- **Ports:** the README port table (`minibolt/minibolt/README.md`, around line 209) is the registry to
  extend. friglet needs non-default ports there.
- **Backend:** Fulcrum is the core Electrum server (`minibolt/minibolt/bitcoin/bitcoin/fulcrum.md`), so
  friglet sits next to it, as Frigate does, or fronts it.

### RaspiBolt v4
- **Path:** a PR adding `guide/bonus/bitcoin/friglet.mdx` and an entry in
  `guide/bonus/bitcoin/meta.json`.
- **Conventions:** versions are tokens defined in `lib/versions.json`; Callouts follow
  `raspibolt/raspibolt/CLAUDE.md.upstream`; one shell command per code block (`README.md`, §Contributing).
- **Templates:** the electrs page `guide/bitcoin/electrum-server.mdx` and the Fulcrum bonus page
  `guide/bonus/bitcoin/fulcrum.mdx`. The PR template asks whether the author will maintain the bonus guide.

### TrueNAS SCALE
- **Path:** a PR to `truenas/apps` adding `ix-dev/community/friglet/` modelled on
  `truenas-scale/truenas-apps/ix-dev/community/electrs/`. That app contains:
  - `app.yaml` (metadata);
  - `ix_values.yaml` (image tags);
  - `questions.yaml`, whose UI form has `external_bitcoind_ip`, `bitcoind_rpc_port`, `bitcoind_rpc_user`
    and `bitcoind_rpc_password`;
  - `templates/docker-compose.yaml`;
  - `templates/test_values/basic-values.yaml`.
- **Process:** `CONTRIBUTIONS.md` and `.github/PULL_REQUEST_TEMPLATE/app_addition.md`. The
  `templates/library/` directory is copied in by upstream tooling.

### Unraid Community Applications
- **Path:** host a template XML in a BlindBit repo, open a support thread on forums.unraid.net,
  then submit the repo to CA.
- **Template shape:** `unraid/anym001-unraid-docker-templates/anym001/electrs.xml`, which has
  `<Repository>`, `<Support>` and `Config` entries for the data dir, a read-only bitcoind dir and port
  50001. The MIT-licensed shape is `unraid/ofawx-unraid-templates/electrs.xml`.
- **Licensing:** the anym001 files are unlicensed, so do not copy them; use ofawx or write from scratch.

### BigBear universal apps
- **Path:** a PR adding `apps/friglet/{app.json,docker-compose.yml,README.md}` per
  `bigbear/big-bear-universal-apps/SCHEMA.md` and `schemas/app-schema-v1.json`, modelled on
  `apps/bitcoin-knots/`.
- **Conversion:** `scripts/convert-to-platforms.sh` then generates the CasaOS, Cosmos, Runtipi, Portainer
  and Umbrel variants. `bigbear/big-bear-casaos/Apps/bitcoin-knots/` shows that generated output.
- **Dependency:** the only bitcoind in this store is Knots (linuxserver image), so friglet depends on it
  or takes external RPC settings.

## Per-platform notes: support later

### BTCPay Server docker
- **Path:** add `docker-compose-generator/docker-fragments/opt-add-friglet.yml`, modelled on
  `btcpay/btcpayserver-docker/docker-compose-generator/docker-fragments/opt-add-electrumx.yml`. That
  fragment:
  - sets `BITCOIN_EXTRA_ARGS` for an rpcauth user;
  - declares `required: [opt-txindex]`;
  - declares `incompatible: [pruning]`.
- **Process:** `docs/development.md` (§Add a fragment), `docs/fragments.md`,
  `.agents.upstream/skills/docker-fragments/SKILL.md`. Every new image must be registered in
  `contrib/DockerFileBuildHelper` (see `AGENTS.md.upstream`).
- **No upstream PR needed:** users can drop in `*.custom.yml` today (`docs/customization.md`).

### ZimaOS / CasaOS official store
- **Path:** a PR adding `Apps/Friglet/docker-compose.yml` with `x-casaos` metadata
  (`casaos-zimaos/CasaOS-AppStore/docs/specs/compose-and-x-casaos.md`, `CONTRIBUTING.md`).
- **Closest Bitcoin example:** `Apps/AlbyHub/docker-compose.yml`.
- **Blocker:** there is no bitcoind in this store, so ship through BigBear first.

### Runtipi
- **Official store:** closed to new apps (`runtipi/runtipi-appstore/README.md`, line 7).
- **Path:** a custom store from the `runtipi/example-appstore/` template, using
  `runtipi/runtipi-docs/.../reference/config-json.mdx` and `dynamic-compose.mdx`.
- **Template:** `runtipi/runtipi-appstore/apps/electrs/`, with `BITCOIND_HOST` and `BITCOIND_DIR` form
  fields and the bitcoind cookie mounted from `../bitcoind/data`.
- **Alternative:** the BigBear conversion covers Runtipi.

### DAppNode
- **Package:** `dappnode_package.json` plus compose, modelled on
  `dappnode/DAppNodePackage-bitcoin/`, which exposes RPC from `bitcoin.dnp.dappnode.eth`.
- **Publishing:** see `dappnode/DAppNodeDocs/docs/dev/package-publishing/*.md` and `.../sdk/sdk-publish.md`.
- **Licensing:** those docs are unlicensed, so read them but do not republish them.

### Citadel / Nirvati
- **Path:** an `app.yml` in the store format of
  `citadel/citadel-legacy-compat-apps/v5/electrs/app.yml`, which requires `bitcoind` and uses the
  `BITCOIN_IP` and `BITCOIN_RPC_*` variables plus `required_ports`. A Fulcrum example sits alongside it.
- **Nirvati format:** `citadel/nirvati-apps-foss/v1/electrs-v2/app.yml.jinja`.
- **Before investing:** confirm Nirvati's license direction and whether Citadel 0.4 ships.

### Parmanode
- **Path:** a PR to `src/` mirroring `parmanode/Parmanode/src/fulcrum/`, which contains:
  - `install_fulcrum.sh`;
  - `menu_fulcrum.sh`;
  - systemd and docker variants;
  - menu hooks in `src/MENUS/menu_add_node.sh`;
  - pinned versions in `src/config/app_versions.sh`.
- **Risk:** single maintainer, and a non-standard T&C.

### NixOS / nixpkgs
- **Package:** add a `pkgs/by-name/fr/friglet/package.nix` (Rust; template
  `nixos/nixpkgs/pkgs/by-name/el/electrs/package.nix`).
- **Module:** add a NixOS module under `nixos/modules/services/networking/`, consuming
  `services.bitcoind` (`nixos/nixpkgs/nixos/modules/services/networking/bitcoind.nix`).
- **Module shape:** the archived `nixos/nix-bitcoin-archived/modules/electrs.nix` and `fulcrum.nix` are
  the best references (MIT).
