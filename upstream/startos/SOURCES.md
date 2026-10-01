# SOURCES — upstream/startos

Verbatim reference material for packaging `friglet` on StartOS. Nothing here is edited.
Retrieval date for every set: **2026-10-01**. Paths below are relative to this folder.

Verification: every file was checked byte-for-byte against its upstream git blob
(`git hash-object` vs. the blob SHA at the pinned commit) or, for the npm set, `cmp` against
the extracted tarball whose sha1/sha512 matched `npm view ... dist.integrity`. 0 mismatches.

Selection rule for the package repos: all git-tracked text files at the pinned commit; skipped
images (`*.png|*.svg|...`), git submodules, and `bitcoin-core-startos/assets/release-keys/`.

## File sets

| Folder | Source URL | Ref / commit | License (as found) |
| --- | --- | --- | --- |
| `start-technologies-live-docs/` | https://github.com/Start9Labs/start-technologies | branch `live-docs` @ `12c2ea8dc8908ccc68fc07fa95f5d5fb8e0b02a1` (2026-09-29) | MIT — `LICENSE` (Start9 Labs, Inc. 2023); `projects/start-sdk/LICENSE` MIT. `NOTICE.md` lists non-MIT paths; none of them were pulled. |
| `start-technologies-master/` | https://github.com/Start9Labs/start-technologies | branch `master` @ `69010348792eaf9a18dac80b7db154a4a2503f02` (2026-10-01) | MIT — `LICENSE` |
| `npm-start-sdk-2.0.9/package/` | https://registry.npmjs.org/@start9labs/start-sdk/-/start-sdk-2.0.9.tgz | npm `2.0.9` (published 2026-07-25), shasum `6007405cdb79e70aad07405b8bd246185c6faebd` | MIT — `package/LICENSE`, `package.json` `"license": "MIT"` |
| `hello-world-startos/` | https://github.com/Start9Labs/hello-world-startos | `master` @ `8484f70752d76cf71df46de47a5d8e1a70154f9a` (2026-08-17) | MIT — `LICENSE` (Start9 Labs 2024) |
| `electrs-startos/` | https://github.com/Start9Labs/electrs-startos | `master` @ `da893f6ac17ef031a343bfcf46f69e2c0cc28d1e` (2026-09-26) | MIT — `LICENSE` (Start9 Labs 2025). Submodule `electrs` (romanz/electrs) **not** pulled. |
| `fulcrum-startos/` | https://github.com/Start9Labs/fulcrum-startos | `master` @ `638328390e207083553abbed525b0fcc8e07984e` (2026-09-30) | **FLAG** — `LICENSE` is Fulcrum's GPL-3.0-or-later text; GitHub reports `NOASSERTION`; manifest says `license: 'MIT'`. Treat as GPL-3.0: reference only, do not copy code into our packaging. |
| `frigate-startos/` | https://github.com/Start9-Community/frigate-startos | `main` @ `bbaeb60bc48cb1073afa02c4209b4412428d1419` (2026-08-30) | Apache-2.0 — `LICENSE` (no NOTICE file upstream) |
| `bitcoin-core-startos/` | https://github.com/Start9Labs/bitcoin-core-startos | `31.x` (default) @ `4ceccce427ae5b17a53a092219fa69d38b781c25` (2026-09-26) | MIT per GitHub. **FLAG (minor)** — the `LICENSE` file is the Bootstrap MIT text ("Copyright (c) 2011-2025 The Bootstrap Authors"), not a Start9 notice. Terms are MIT either way. |
| `documentation-0.3.5.x/` | https://github.com/Start9Labs/documentation | branch `0.3.5.x` @ `2361a6c5e662ab0f92edf8ddf27d7538c8026905` (2026-08-25) | **FLAG — no license.** Repo has no LICENSE and GitHub reports none (all rights reserved by default). Two small files kept only as evidence of the legacy model. **Delete this folder before the repo goes public**, or replace it with links. |

## Files per set

### `start-technologies-live-docs/` (what docs.start9.com serves; documents SDK 2.0.10)

- `LICENSE`, `NOTICE.md`
- `projects/start-sdk/{README.md,CHANGELOG.md,LICENSE,s9pk.mk,tsconfig.base.json,package.json}`
- `projects/start-sdk/docs/src/*.md` — the full packaging guide (https://docs.start9.com/packaging/0.4.0.x/)
- `projects/start-sdk/docs/package-template/**` — template that `start-cli s9pk init-package` scaffolds
- `projects/start-sdk/docs/skills/package-service/**` — Start9's agent skill for packaging
- `projects/start-os/docs/src/*.md` — StartOS 0.4.0.x user manual (no images, no firmware binary)
- `projects/start-docs/bitcoin-guides/src/*.md` — incl. `connecting-wallets.md`, `electrum-servers.md`
- `projects/start-cli/README.md`
- `shared-libs/crates/start-core/s9pk-structure.md`, `shared-libs/crates/start-core/src/s9pk/v2/specv2.md` — s9pk format
- `shared-libs/crates/start-core/src/s9pk/mod.rs` — v1 → v2 auto-conversion on load (evidence for 0.3.5 compat)
- `shared-libs/crates/start-core/src/s9pk/v2/pack.rs` — how `s9pk pack` builds images (buildx, platform, tar2sqfs)

### `start-technologies-master/` (unreleased work; packages' CI calls these `@master`)

- `.github/workflows/{build,release,tagAndRelease,syncNext}.yml` — reusable CI every package calls
- `.github/actions/setup-build-env/action.yml` — CI toolchain (Node 22, QEMU, buildx, squashfs-tools-ng, start-cli)
- `projects/start-sdk/release-notes/{2.0.9,3.0.0}.md`, `projects/start-sdk/CHANGELOG.md`, `projects/start-sdk/s9pk.mk` — SDK 3.0.0 (unreleased)
- `projects/start-os/release-notes/{0.4.0,0.4.0.1,0.4.0.2}.md` — 0.4.0.2 is unreleased
- `projects/start-os/docs/src/update-040.md` — master copy; adds the "Legacy Services" section not yet on `live-docs`
- `projects/start-sdk/docs/src/environment-setup.md` — master copy; explains `live-docs` vs `master` and workspace config
- `LICENSE`

### `npm-start-sdk-2.0.9/package/` (the SDK that `npm install` resolves today)

- `s9pk.mk`, `tsconfig.base.json`, `package.json`, `LICENSE`, `README.md`, `CHANGELOG.md`

### Package repos

- `hello-world-startos/` — the official template, 34 files
- `electrs-startos/` — Electrum server, Dockerfile build, cookie auth, 48 files
- `fulcrum-startos/` — Electrum server, prebuilt image, 41 files
- `frigate-startos/` — **closest analogue**: BIP-352 Silent Payments Electrum server, 37 files
- `bitcoin-core-startos/` — the dependency's exported constants and actions, 60 files

### `documentation-0.3.5.x/`

- `site/source/developer-docs/submission.rst`, `site/source/developer-docs/packaging.rst`

## Not pulled (linked only)

- Live site: https://docs.start9.com/packaging/0.4.0.x/ (built from `live-docs` above)
- start-cli 2.1.0 release (binary, signatures): https://github.com/Start9Labs/start-technologies/releases/tag/start-cli%2Fv2.1.0
- Legacy 0.3.5 bitcoind wrapper (`manifest.yaml` model): branch `0.3.5-30.x` of bitcoin-core-startos

## Renamed on import

Files that would act on this repository (agent instructions, `.gitignore`, `.gitattributes`)
were renamed with an `.upstream` suffix on import. Their content is unchanged. Renamed here:

- `bitcoin-core-startos/CLAUDE.md`
- `bitcoin-core-startos/.gitignore`
- `bitcoin-core-startos/AGENTS.md`
- `fulcrum-startos/CLAUDE.md`
- `fulcrum-startos/.gitignore`
- `fulcrum-startos/AGENTS.md`
- `frigate-startos/CLAUDE.md`
- `frigate-startos/.gitignore`
- `frigate-startos/AGENTS.md`
- `hello-world-startos/CLAUDE.md`
- `hello-world-startos/.gitignore`
- `hello-world-startos/AGENTS.md`
- `electrs-startos/CLAUDE.md`
- `electrs-startos/.gitignore`
- `electrs-startos/AGENTS.md`
- `start-technologies-live-docs/projects/start-sdk/docs/package-template/CLAUDE.md`
- `start-technologies-live-docs/projects/start-sdk/docs/package-template/.gitignore`
- `start-technologies-live-docs/projects/start-sdk/docs/package-template/AGENTS.md`
