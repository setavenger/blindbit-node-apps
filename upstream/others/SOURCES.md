# Sources: upstream/others

These are verbatim copies of upstream packaging docs, templates and reference Electrum-server and
bitcoind packages for the node and app platforms not handled by the Umbrel, StartOS, RaspiBlitz
and myNode folders.

- **Retrieved:** 2026-10-01 for every set. Shallow or blobless `git clone` at the SHA shown, then the
  listed files were copied with their upstream relative paths. The nixpkgs files were fetched from
  `raw.githubusercontent.com` at the pinned SHA.
- **Text only:** binary files (logos, screenshots) were skipped. One upstream symlink is kept as a
  symlink: `btcpay/.../docker-fragments/bitcoin.yml -> bitcoincore.yml`.
- **Verified:** on 2026-10-01 every copied file was compared byte for byte against `git show HEAD:<path>`
  (or the raw URL for nixpkgs) at the SHA shown, with 0 mismatches.
- **Layout:** `<platform>/<repo>/<upstream path>`. Each set includes its upstream LICENSE file where
  one exists.

## Pulled file sets

| Local path | Upstream | Branch | Commit SHA (commit date) | License (SPDX) | Flag |
|---|---|---|---|---|---|
| `minibolt/minibolt/` | https://github.com/minibolt-guide/minibolt | main | `53f972e1471fdcd863d8c8b32c73b64b2892e0b8` (2026-08-10, tag v2.3) | MIT | none |
| `raspibolt/raspibolt/` | https://github.com/raspibolt/raspibolt | master | `8d6fe89c8415b66a82f54d4b87f0ef3a5f2b2e23` (2026-08-19) | MIT | none |
| `btcpay/btcpayserver-docker/` | https://github.com/btcpayserver/btcpayserver-docker | master | `dd1074d1a005789de2629a00fcf5d93e0df32819` (2026-09-30) | MIT | none |
| `parmanode/Parmanode/` | https://github.com/ArmanTheParman/Parmanode | master | `b6ed219759bc9053451b2f5128827b7a0c519795` (2026-09-22, version.conf v3.70.11) | MIT | **Non-standard usage term:** `terms_and_conditions.txt` bars "shitcoin" holders from using Parmanode. It conflicts with MIT; check before republishing |
| `citadel/citadel-legacy-compat-apps/` | https://gitlab.com/nirvati-ug/citadel/legacy-compat/apps | main | `12fa1e88a8f6e20d4ecbcaa216f749f46315ac05` (2026-08-07) | AGPL-3.0-or-later | **Copyleft:** a friglet `app.yml` derived from these files becomes AGPL |
| `citadel/nirvati-apps-foss/` | https://gitlab.com/nirvati-ug/apps/foss | main | `424bcdbb27a745373c78bf9e712c319d2813deb2` (2026-08-07) | GPL-3.0 (LICENSE file) | Copyleft. Nirvati may be changing its license: a 2026-08-31 blog post titled "The end of open source in Bitcoin" exists but was not readable (404). Re-check before relying on it |
| `truenas-scale/truenas-apps/` | https://github.com/truenas/apps | master | `8d93ac087336ab642492209cb707aa1f9556d324` (2026-09-30) | LGPL-3.0 | Copyleft (weak). `templates/library/` was skipped; upstream copies it into each app automatically |
| `runtipi/runtipi-appstore/` | https://github.com/runtipi/runtipi-appstore | master | `94e53591af2ace0c1fcf6428830a5f71e4abaf9b` (2026-10-01) | GPL-3.0 | Copyleft |
| `runtipi/runtipi-docs/` | https://github.com/runtipi/runtipi-docs | main | `c063ce97588495d06152824719b15c5b607664be` (2026-08-27) | MIT | none |
| `runtipi/example-appstore/` | https://github.com/runtipi/example-appstore | main | `c3f890393c63559001ec0ed3d3b558285a5994ab` (2026-03-14) | WTFPL | Permissive but **not OSI-approved**; low risk |
| `bigbear/big-bear-universal-apps/` | https://github.com/bigbeartechworld/big-bear-universal-apps | main | `a37e5f17f139c6dcd361a91b5a11f221879c5879` (2026-10-01) | MIT | none |
| `bigbear/big-bear-casaos/` | https://github.com/bigbeartechworld/big-bear-casaos | master | `d10d73c7ba76f70a69a1a31717224a0bfd280188` (2026-10-01) | MIT | none |
| `casaos-zimaos/CasaOS-AppStore/` | https://github.com/IceWhaleTech/CasaOS-AppStore | main | `0909364b800950030e71ea82355a5969a1c08b39` (2026-09-24) | Apache-2.0 | none (no NOTICE file upstream) |
| `dappnode/DAppNodePackage-bitcoin/` | https://github.com/dappnode/DAppNodePackage-bitcoin | master | `d7bc9dc612580f7b3819d70101e14df575e3dc94` (2026-07-09) | GPL-3.0 | Copyleft |
| `dappnode/DAppNodeDocs/` | https://github.com/dappnode/DAppNodeDocs | master | `223bd0a21ca72c05ad1c01eb28f220411b8a05b3` (2026-06-30) | **none** | **MISSING LICENSE:** all rights reserved by default. Remove or replace with links before this repo goes public |
| `unraid/anym001-unraid-docker-templates/` | https://github.com/anym001/unraid-docker-templates | main | `2a40fcba13655a3598a832a7a3130026ad557938` (2026-07-05) | **none** | **MISSING LICENSE:** remove or replace with links before this repo goes public |
| `unraid/ofawx-unraid-templates/` | https://github.com/ofawx/unraid-templates | master | `e4e8b7b7aff5bce9ff55fa0985f92e85c46d69ce` (2022-01-29) | MIT | Stale (2022). Kept as an MIT-licensed template shape reference |
| `unraid/linuxserver-templates/` | https://github.com/linuxserver/templates | main | `b4058a4d5eb38cf8b97c69d19314567b16ee8788` (2026-09-27) | GPL-3.0 | Copyleft |
| `nixos/nixpkgs/` | https://github.com/NixOS/nixpkgs | master | `694f78e1bab6f6d7c83c5fb4fd1eaaf2181a4ec8` (2026-10-01) | MIT (`COPYING`) | none |
| `nixos/nix-bitcoin-archived/` | https://github.com/fort-nix/nix-bitcoin | master | `33d7a2cb0bfbeee98b7361de3a2beb855aaee54b` (2026-09-21; the last release, v0.0.139, is `37931e5`) | MIT | Upstream is **archived and unmaintained** as of 2026-08-13. Only `LICENSE`, `README.md` and `modules/{electrs,fulcrum}.nix` are kept, as NixOS-module references |

## Files per set (summary)

- **minibolt:** `README.md`, `LICENSE`, `SUMMARY.md`, `.github/**` (PR and issue templates),
  `bitcoin/bitcoin/*.md` (bitcoin-core, fulcrum, btc-rpc-explorer, sparrow), `bonus/bitcoin/electrs.md`,
  `bonus-guides/bitcoin/frigate.md` (BIP-352 Electrum server), `bonus-guides/bitcoin/mempool.md`,
  `bonus-guides/networking/tor-services.md`, `index-1/security.md`, `resources/fulcrum-banner*.txt`.
- **raspibolt:** `README.md`, `CLAUDE.md`, `DECISIONS.md`, `TODO.md`, `LICENSE`, `lib/versions.{json,ts}`,
  `.github/**` templates, `guide/meta.json`, `guide/architecture.mdx`, `guide/bitcoin/*`,
  `guide/bonus/{index.mdx,meta.json}`, `guide/bonus/bitcoin/{index.mdx,meta.json,fulcrum.mdx,mempool.mdx}`,
  `guide/raspberry-pi/security.mdx`.
- **btcpay:** `README.md`, `AGENTS.md`, `CLAUDE.md`, `LICENSE`, `.agents/skills/*/SKILL.md`,
  `docs/*.md` (incl. `development.md`, `fragments.md`, `customization.md`),
  `docker-compose-generator/crypto-definitions.json`, the fragments `opt-add-electrumx.yml`,
  `opt-add-mempool.yml`, `bitcoincore.yml`, `bitcoin.yml` (symlink), `opt-txindex.yml`, `opt-add-zmq.yml`,
  `opt-add-tor.yml`, `opt-add-thunderhub.yml` and `.gitignore`, plus
  `contrib/DockerFileBuildHelper/README.md`.
- **parmanode:** `README.md`, `LICENSE`, `terms_and_conditions.txt`, `version.conf`, `changelog.txt`,
  `run_parmanode.sh`, `source_parmanode.sh`, `src/electrs/**`, `src/fulcrum/**`, `src/MENUS/**`,
  `src/config/app_versions.sh`.
- **citadel-legacy-compat-apps:** `README.md`, `LICENSE.md`, `app-store.yml`, `v5/{electrs,fulcrum,citadel-bitcoin}/**`.
- **nirvati-apps-foss:** `README.md`, `LICENSE`, `app-store.yml`, `v1/{electrs,electrs-v2,bitcoin-core-v3}/**`.
- **truenas-apps:** `README.md`, `CONTRIBUTIONS.md`, `LICENSE`, `.github/PULL_REQUEST_TEMPLATE/app_addition.md`,
  `.github/pull_request_template.md`, `.github/ISSUE_TEMPLATE/app-request.yaml`,
  `ix-dev/community/{electrs,bitcoind,bitcoind-knots,mempool}/**` (without `templates/library/`).
- **runtipi-appstore:** `README.md`, `LICENSE`, `templates/README.md`, `apps/{electrs,bitcoind,mempool}/**` (text only).
- **runtipi-docs:** `LICENSE`, `src/content/docs/guides/{create-your-own-app-store,dynamic-compose-guide,customize-app-config}.mdx`,
  `src/content/docs/learn/apps-and-app-store.mdx`, `src/content/docs/reference/{config-json,dynamic-compose,folder-structure,environment-variables}.mdx`.
- **example-appstore:** `README.md`, `LICENSE`, `config.js`, `apps/whoami/{config.json,docker-compose.yml,metadata/description.md}`, `__tests__/apps.test.ts`.
- **big-bear-universal-apps:** `README.md`, `SCHEMA.md`, `CONTEXT.md`, `LICENSE`, `templates/README.md`,
  `schemas/app-schema-v1.json`, `scripts/{convert-to-platforms,validate-apps}.sh`, `apps/bitcoin-knots/**`,
  `.github/workflows/validate-pr.yml`.
- **big-bear-casaos:** `README.md`, `LICENSE`, `templates/README.md`, `Apps/bitcoin-knots/**` (generated output of universal-apps).
- **CasaOS-AppStore:** `README.md`, `CONTRIBUTING.md`, `LICENSE`, `store-config.json`, `docs/{README,index}.md`,
  `docs/specs/*.md` (incl. `compose-and-x-casaos.md`), `docs/quick-start/*.md`, `docs/guides/third-party-store-guide.md`,
  `docs/resources/recommended-third-party-stores.md`, `docs/migration/overview.md`, `docs/faq/overview.md`,
  `Apps/AlbyHub/docker-compose.yml` (closest Bitcoin app in the official store), `.github/ISSUE_TEMPLATE/app_request.yml`.
- **DAppNodePackage-bitcoin:** `README.md`, `LICENSE`, `dappnode_package.json`, `docker-compose.yml`, `releases.json`,
  `src/{Dockerfile,docker_entrypoint.sh}`, `.github/workflows/{main,build,auto_check}.yml`.
- **DAppNodeDocs:** `docs/dev/references/{manifest,docker-compose,setup-wizard}.md`, `docs/dev/sdk/{overview,sdk-publish}.md`,
  `docs/dev/package-publishing/{publish-packages-clients,package-ownership}.md`.
- **unraid:** anym001 `README.md`, `anym001/{electrs,bitcoind}.xml`; ofawx `README.md`, `LICENSE`, `{bitcoind,electrs,electrumx}.xml`;
  linuxserver `README.md`, `LICENSE`, `unraid/bitcoin-knots.xml`.
- **nixpkgs:** `COPYING`, `CONTRIBUTING.md`, `nixos/README.md`, `nixos/modules/services/networking/bitcoind.nix`,
  `pkgs/by-name/README.md`, `pkgs/by-name/el/electrs/package.nix`, `pkgs/by-name/fu/fulcrum/package.nix`.
- **nix-bitcoin-archived:** `README.md`, `LICENSE`, `modules/electrs.nix`, `modules/fulcrum.nix`.

## Referenced but not pulled

These have no usable repository, are closed or onion-only, or have no license. They are cited in
`SURVEY.md` by URL only.

- **Unraid CA policy and publishing how-to:** forum topics https://forums.unraid.net/topic/87144/ and
  https://forums.unraid.net/topic/101424/. These are forum posts with no license.
- **Unraid CA feed:** https://github.com/Squidly271/AppFeed (`applicationFeed.json`). Too large, and it
  has no license.
- **RoninDojo:** onion-only Gitea. **Ashigaru Dojo:** Tor-only. **nodl:** closed. **FutureBit
  apolloapi-v2:** CC-BY-NC-ND-4.0, which is restrictive. None of these were pulled.
- **Sparrow Frigate** (the reference BIP-352 Electrum server): https://github.com/sparrowwallet/frigate,
  Apache-2.0, release 1.5.3 (2026-05-30). Not a platform, so not pulled.

## Renamed on import

Files that would act on this repository (agent instructions, `.gitignore`, `.gitattributes`)
were renamed with an `.upstream` suffix on import. Their content is unchanged. Renamed here:

- `raspibolt/raspibolt/CLAUDE.md`
- `btcpay/btcpayserver-docker/CLAUDE.md`
- `btcpay/btcpayserver-docker/AGENTS.md`
- `btcpay/btcpayserver-docker/docker-compose-generator/docker-fragments/.gitignore`
- `btcpay/btcpayserver-docker/.agents`
