# Sources: Umbrel packaging references

Retrieved 2026-10-01. Every file in this folder is a byte-identical copy of the upstream file at the
commit listed below (checked with `cmp` after copying). Images and binaries were skipped, and so
were two large npm lockfiles. Paths inside each per-source folder match the upstream repository.

> **LICENSE WARNING: read this before the repo goes public**
>
> | Folder | License | Risk |
> |---|---|---|
> | `umbrel-apps/`, `umbrel-apps-legacy-readme/` | **None.** No LICENSE file, and GitHub reports `license: null`. | Default copyright applies, so redistribution rights are unclear. |
> | `umbrel-community-app-store/` | **None.** No LICENSE file, and GitHub reports `license: null`. | Same as above. The repo is meant as a "Use this template" starter, but no grant is written down. |
> | `umbrel/`, `umbrel-electrs/`, `umbrel-bitcoin/` | **PolyForm Noncommercial 1.0.0.** The license text is copied alongside as `LICENSE.md`. | Noncommercial use only. Redistributed copies must carry the license or notice. Not OSI-open-source. |
> | `umbrel.wiki/` | **None stated.** This is the GitHub wiki of the PolyForm-licensed `getumbrel/umbrel` repo. | Unclear. |
>
> If the repo will be public or used commercially, decide whether to keep these copies or replace them
> with links. The URLs and SHAs below are enough to re-fetch everything.

## 1. `umbrel-apps/`: official App Store repo (app framework docs, linter, reference apps)

- URL: https://github.com/getumbrel/umbrel-apps (branch `master`)
- Commit: `01be6c36007e9bfbca2d44bb4959eb9a9d0b4858` (2026-10-01T06:46:12Z, "Update poznote to v6.103.0")
- License: **NONE** (see warning above)
- Files pulled:
  - Docs and policy:
    - `README.md`, `AGENTS.md`, `CLAUDE.md`
    - `.claude/skills/umbrel-package-app/SKILL.md`
    - `.claude/skills/umbrel-develop-app/SKILL.md`
    - `.claude/skills/umbrel-test-app/SKILL.md`
    - `.claude/skills/umbrel-update-app/SKILL.md`
  - Since commit `e0a982b` (2026-06-18), the four `SKILL.md` files hold the authoritative packaging
    guidance. They replaced the long README (see section 2).
  - Tooling: `package.json`, `package-lock.json`, `.tools/lint-apps.mjs`, `.tools/lint-apps.test.mjs`,
    `.github/workflows/lint-apps.yml`.
  - Submission: `.github/PULL_REQUEST_TEMPLATE.md`, `.github/ISSUE_TEMPLATE/{config,app-request,package-issue}.yml`.
  - Reference apps (all files):
    - `electrs/`: Electrum server with a connection-info UI, raw port 50001 and a Tor hidden service
    - `fulcrum/`: Electrum server; `implements: electrs`; raw port 50002
    - `electrumx/`: Electrum server; `implements: electrs`; raw port 50003
    - `bitcoin/`: includes `exports.sh`, the `APP_BITCOIN_*` dependency contract
  - Simple app that ships its own tiny web UI: `mosquitto/`. It is an MQTT broker on raw TCP 1883,
    plus an nginx sidecar serving a static `data/www/index.html` that shows connection instructions.
    Text files only; `favicon.png` and `logo.png` were skipped.
- Note: `CLAUDE.md`, `AGENTS.md` and `.claude/skills/` are agent-instruction files. Coding agents
  working inside this folder may load them automatically.

## 2. `umbrel-apps-legacy-readme/`: superseded long-form "Umbrel App Framework" README

- URL: https://github.com/getumbrel/umbrel-apps/blob/847325456394704f308d7990f8cea1378f7b9ae7/README.md
- Commit: `847325456394704f308d7990f8cea1378f7b9ae7` (2026-02-16, the last commit before the README
  was replaced by `.claude/skills/`)
- License: **NONE** (same repo as section 1)
- Files: `README.md` (upstream path `README.md`)
- Kept only for historical context: the umbrel-dev workflow, the submission template and the
  app_proxy whitelist examples. Prefer section 1 wherever the two disagree.

## 3. `umbrel-community-app-store/`: Community App Store template

- URL: https://github.com/getumbrel/umbrel-community-app-store (branch `master`)
- Commit: `1b147ae7209f42ef37917a94af2d99cf5b77cc7d` (2024-04-09, "Revise instructions in README.md")
- License: **NONE** (see warning above)
- Files: `README.md`, `umbrel-app-store.yml`, `.gitignore`, `sparkles-hello-world/umbrel-app.yml`,
  `sparkles-hello-world/docker-compose.yml` (this is every file in the repo)

## 4. `umbrel/`: umbrelOS source (manifest schema, app lifecycle, app gateway, store loader)

- URL: https://github.com/getumbrel/umbrel (branch `master`)
- Commit: `11e7f5bdfcb5bb8b46f39edd6defb4c37641763a` (2026-09-22, "Merge tag '2.0.0'"; umbrelOS 2.0.0)
- License: **PolyForm Noncommercial 1.0.0** (`umbrel/LICENSE.md`, copied verbatim)
- Files (all under `packages/umbreld/source/modules/`):
  - `apps/schema.ts`: the `umbrel-app.yml` zod schema, i.e. the full field list
  - `apps/manifest-compatibility.ts`: `manifestVersion` vs OS version gate
  - `apps/app-repository.ts`: community store loading, the `umbrel-app-store.yml` ID-prefix filter, depth-1 clone
  - `apps/app-store.ts`: store update polling
  - `apps/legacy-compat/app-script`: env sourcing, `exports.sh`, `derive_entropy`, templates, update whitelist, compose assembly
  - `apps/legacy-compat/docker-compose.{app_gateway,common,tor}.yml`, `apps/legacy-compat/tor-entrypoint.sh`
  - `app-gateway/app-gateway.ts`: umbrelOS 2.0 in-process replacement for the `app_proxy` container

## 5. `umbrel-electrs/`: source of the electrs app's connection-info web UI

- URL: https://github.com/getumbrel/umbrel-electrs (branch `master`). Published as the image
  `getumbrel/umbrel-electrs`; `umbrel-apps/electrs/docker-compose.yml` pins `v1.0.4`.
- Commit: `3dd2fc74ed28cdc0c7f957632410fb84fc96d7cc` (2024-11-21)
- License: **PolyForm Noncommercial 1.0.0** (`umbrel-electrs/LICENSE.md`).
  `LICENSE.legacy` (MIT, Casa Inc.) covers only the code at old commit `4607e73b…`.
- Files: all text files. Excluded: `package-lock.json` (1.6 MB), `yarn.lock` (0.5 MB), and image/font binaries.
- Note: the repo-root `.gitignore` rule `.env` matches `umbrel-electrs/apps/frontend/.env`. That file is a
  harmless one-line `VUE_APP_API_BASE_URL=9`, but it will only be committed if added with
  `git add -f`.

## 6. `umbrel-bitcoin/`: Bitcoin Node app source (how bitcoind is configured for dependent apps)

- URL: https://github.com/getumbrel/umbrel-bitcoin (branch `master`)
- Commit: `2fe07948f99e101dbee95ce34e5947a69c441ee4` (2026-08-21)
- License: **PolyForm Noncommercial 1.0.0** (`umbrel-bitcoin/LICENSE.md`)
- Files:
  - `apps/backend/src/modules/config/config.ts`: `rpcallowip`, `whitebind` and ZMQ lines in the generated `bitcoin.conf`
  - `libs/settings/settings.meta.ts`: user setting defaults (REST off, prune, txindex)

## 7. `umbrel.wiki/`: getumbrel/umbrel GitHub wiki

- URL: https://github.com/getumbrel/umbrel/wiki (git: `https://github.com/getumbrel/umbrel.wiki.git`)
- Commit: `508aa6ed1e0768317de3d936859d3adaec25996a` (2026-09-02)
- License: **none stated** (the wiki of a PolyForm repo)
- Files: `License-FAQ.md` (what PolyForm NC allows), `Install-umbrelOS-on-a-Linux-VM.md` (test VM setup)

## Referenced but not pulled

| Resource | Why not pulled | License |
|---|---|---|
| https://umbrel.com/support/apps/how-to-add-a-community-app-store (read 2026-10-01) | Marketing site with no VCS. Content: App Store → "⋯" → Community App Stores → paste URL → Add; plus a warning that community apps are unvetted. | n/a |
| https://github.com/nmfretz/umbrel-fulcrum @ `349e24666fe9cd819015e845612c3ff7e5340c0c` | A fork of `umbrel-electrs`; the fulcrum UI image `nmfretz/umbrel-fulcrum`. Adds `ELECTRUM_PUBLIC_CONNECTION_PORT`. | PolyForm NC 1.0.0 |
| https://github.com/getumbrel/umbrel-dev | Archived 2023-05. Superseded by `npm run dev` in `getumbrel/umbrel` and by the `ghcr.io/getumbrel/umbrelos:<ver>` container. | none |
| `ghcr.io/getumbrel/umbrelos` tags (registry API, 2026-10-01) | Not a file. Latest tag is `2.0.0`, multi-arch amd64+arm64; used for local testing. | n/a |
| GitHub releases of getumbrel/umbrel | `2.0.0` published 2026-09-22 (non-prerelease). | n/a |

## Renamed on import

Files that would act on this repository (agent instructions, `.gitignore`, `.gitattributes`)
were renamed with an `.upstream` suffix on import. Their content is unchanged. Renamed here:

- `umbrel-electrs/.gitignore`
- `umbrel-community-app-store/.gitignore`
- `umbrel-apps/CLAUDE.md`
- `umbrel-apps/AGENTS.md`
- `umbrel-apps/.claude`
