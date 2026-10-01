# SOURCES — upstream/raspiblitz

All files under this folder (except `SOURCES.md` and `NOTES.md`) are **verbatim** copies.
Each one was written with `git show <commit>:<path>`, then checked with `git hash-object`
against the upstream blob: 75/75 files match. Every file is text (us-ascii/utf-8). Total size is about 0.7 MB.

Retrieved: **2026-10-01**. Each folder keeps the upstream relative paths.

## License summary (this repo may become public)

| Folder | Upstream | License | Flag |
|---|---|---|---|
| `raspiblitz/` | raspiblitz/raspiblitz | MIT (`raspiblitz/LICENSE`) | OK |
| `raspiblitz-removed-c73c5f0/` | raspiblitz/raspiblitz | MIT (same repo) | OK |
| `blitz_api/` | fusion44/blitz_api | MIT (`blitz_api/LICENSE`) | OK |
| `raspiblitz-web/` | raspiblitz/raspiblitz-web | MIT (`raspiblitz-web/LICENSE`) | OK |
| `raspiblitz-docs/` | raspiblitz/raspiblitz-docs | **none**: no LICENSE file, GitHub API reports no license | **FLAG**: all rights reserved by default. Remove or replace with links before going public, unless the maintainers grant permission. |
| `nixblitz/` | raspiblitz/nixblitz | README says "MIT" (`nixblitz/README.md:87-89`), but there is **no LICENSE file** and no copyright line | **FLAG (weak)**: probably fine, but there is no formal license text |
| `nixblitz_official_plugins/` | raspiblitz/nixblitz_official_plugins | **none**: no LICENSE file, no README statement, GitHub API reports none | **FLAG**: all rights reserved by default. Remove before going public, or ask for a license. |

---

## 1. `raspiblitz/` — RaspiBlitz main repo, release branch `v1.12`

- URL: https://github.com/raspiblitz/raspiblitz/tree/6c4c6c1e7f2ebf29cb75219ac71597440e137a73
- Branch: `v1.12`. This is the release branch of the latest release, **v1.12.1** (tag
  `35a9dafe09d0f7a8e8fc6f074f84de122baa1a8b`, 2026-03-01). The branch head is newer:
  - Commit: `6c4c6c1e7f2ebf29cb75219ac71597440e137a73` (2026-08-23, "update joninbox to v0.9.0 …")
  - Between the v1.12.1 tag and this head, 5 of the pulled files changed: `SECURITY.md`,
    `00mainMenu.sh`, `bitcoin.update.sh`, `blitz.fatpack.sh` and `bonus.electrs.sh` (chain-prefixed whitebind).
  - Compared with `dev` (`c421693dcb09b00eafacbda6128174d2c658cb06`, 2026-08-23), only `build_sdcard.sh` differs
    among the pulled files. On `dev` it adds Python 3.13 / Debian 13 Trixie handling.
- Note: `rootzoll/raspiblitz` is now a **fork** (parent `Labelbase/raspiblitz`). The canonical
  repo is `raspiblitz/raspiblitz`.
- License: MIT. `LICENSE` says "Copyright (c) 2018-2022 The RaspiBlitz developers".
- Files:
  - Docs/process: `LICENSE`, `README.md`, `CONTRIBUTING.md`, `pull_request_template.md`, `SECURITY.md`,
    `alternative.platforms/README.md`, `alternative.platforms/amd64/README.md`
  - Base system: `build_sdcard.sh`, `home.admin/assets/bitcoin.conf`, `home.admin/assets/custom-installs.sh`
  - App template and reference apps: `home.admin/config.scripts/bonus.template.sh`,
    `bonus.electrs.sh`, `bonus.fulcrum.sh`, `bonus.albyhub.sh` (built from the template, per-arch binary, WebUI app),
    `bonus.lndk.sh` (Rust/cargo build), `bonus.lit.sh` (binary download with PGP-signed manifest)
  - Helpers called by those scripts: `tor.onion-service.sh`, `tor.network.sh`, `blitz.conf.sh`,
    `blitz.git-verify.sh`, `blitz.display.sh`, `blitz.web.sh`, `internet.sh`, `network.txindex.sh`,
    `bitcoin.install.sh`, `bitcoin.update.sh`, `blitz.data.sh` (DATA/STORAGE layout), `home.admin/_cache.sh`
  - Integration points (files an app PR touches): `home.admin/_provision_.sh`, `00mainMenu.sh`,
    `00settingsMenuServices.sh`, `99connectMenu.sh`, `99updateMenu.sh`, `00infoBlitz.sh`,
    `config.scripts/blitz.debug.sh`, `blitz.shutdown.sh`, `blitz.passwords.sh`, `blitz.fatpack.sh`
  - WebUI/API deployers: `config.scripts/blitz.web.api.sh`, `blitz.web.ui.sh`
  - nginx: `home.admin/assets/nginx/snippets/*.conf` (8 files), `home.admin/assets/nginx/sites-available/public.conf`

## 2. `raspiblitz-removed-c73c5f0/` — the removed Docker helper (historical)

- URL: https://github.com/raspiblitz/raspiblitz/blob/336a3418e9c2ec42cfebf0b37ae8b6b234d26c96/home.admin/config.scripts/blitz.docker.sh
- Commit: `336a3418e9c2ec42cfebf0b37ae8b6b234d26c96`. This is the parent of
  `c73c5f022a4cbc667f3324ccaf9bdd9326a190ea` ("#2074 remove docker install", rootzoll, 2024-08-06),
  and it is also the `v1.11.1` tag commit.
- License: MIT (same repo).
- Files: `home.admin/config.scripts/blitz.docker.sh`. Kept only as evidence that Docker support existed and was removed.

## 3. `blitz_api/` — RaspiBlitz backend API (the WebUI's backend)

- URL: https://github.com/fusion44/blitz_api/tree/e0f4d7e2fc8add8053b698df309989af70708d61
- Branch: `blitz-v1.12`. `blitz.web.api.sh` deploys `blitz-<release-branch>` on release images.
- Commit: `e0f4d7e2fc8add8053b698df309989af70708d61` (2026-02-21)
- `dev` (`96569fa79a2176d51b18292e075f9eafbb260714`, 2026-08-16) has the same `AppId` list. It only differs
  in `app/apps/tasks_impl/*`, which was not pulled.
- License: MIT. `LICENSE` says "Copyright (c) 2021 fusion44".
- Files: `LICENSE`, `README.md`, `app/apps/models.py`, `app/apps/impl/raspiblitz.py`, `app/apps/router.py`

## 4. `raspiblitz-web/` — RaspiBlitz WebUI

- URL: https://github.com/raspiblitz/raspiblitz-web/tree/15e422cc8165d40afe802b9617c92eff7c6a1896
- Branch: `release/v1.12`. `blitz.web.ui.sh` deploys `release/<release-branch>`.
- Commit: `15e422cc8165d40afe802b9617c92eff7c6a1896` (2026-09-05). For reference, `master` was at
  `a67374c7f9e8f07ad26e80591a35b357316bc9e3` (2026-09-07).
- License: MIT. `LICENSE` says "Copyright (c) 2021 Christoph Stenglein".
- Files: `LICENSE`, `src/utils/availableApps.ts`, `src/models/app-status.ts`, `src/models/app.model.ts`,
  `src/models/advanced-app-status.ts`, `src/pages/Apps/customApps/Electrs.tsx`, `src/i18n/langs/en.json`

## 5. `raspiblitz-docs/` — official docs site (docs.raspiblitz.org)

- URL: https://github.com/raspiblitz/raspiblitz-docs/tree/e39b0a38dfa6850b84e070b62ddd1fbd33461f6b
- Branch: `master`. Commit: `e39b0a38dfa6850b84e070b62ddd1fbd33461f6b` (2026-03-03)
- License: **none found (FLAG)**
- Files: `README.md`, `docs/faq/dev.md` (contains "How to add an app to the RaspiBlitz?"),
  `docs/faq/contribute.md`, `docs/usage/1_ssh_menu.mdx`, `docs/usage/2_webui.mdx`

## 6. `nixblitz/` — experimental NixOS-based successor ("RaspiBlitz reimagined")

- URL: https://github.com/raspiblitz/nixblitz/tree/8410b9c5cb9f7277b4c9d0066d1ad19fcfce511f
- Branch: `main`. Commit: `8410b9c5cb9f7277b4c9d0066d1ad19fcfce511f` (2026-08-22)
- License: README states MIT, but there is no LICENSE file **(weak FLAG)**
- Files: `README.md`, `docs/plugin-authoring.md`, `docs/architecture.md`, `docs/decisions/plugins.md`

## 7. `nixblitz_official_plugins/` — vetted NixBlitz plugins

- URL: https://github.com/raspiblitz/nixblitz_official_plugins/tree/816ceb934480f6bb987b8c9c60cdde38e62e7a4c
- Branch: `main`. Commit: `816ceb934480f6bb987b8c9c60cdde38e62e7a4c` (2026-07-31)
- License: **none found (FLAG)**
- Files: `README.md`, `electrs/README.md`, `electrs/plugin.json`, `electrs/plugin.nix`

---

## Not pulled, but cited in NOTES.md (URLs only)

- Docker stance: issue raspiblitz/raspiblitz#2074 ("Add experimentell docker support", closed 2024-08-06)
  https://github.com/raspiblitz/raspiblitz/issues/2074
- Open Docker/OCI discussions: #4283, #4761, #5198 (2026-09-28 activity)
- friglet facts come from the local `blindbit-rs` checkout at `611a877` (2026-09-22):
  `friglet/src/config.rs`, `friglet-ipc/src/lib.rs`, `Dockerfile`, `docs/docker.md`.
  blindbit-oracle facts come from the local checkout at `c29d3a8` (2026-09-22): `README.md`, `blindbit.example.toml`.
