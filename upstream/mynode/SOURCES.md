# myNode upstream sources

Retrieved 2026-10-01. All pulled files are **verbatim** copies taken with
`git show <sha>:<path>` from one commit and kept at their upstream relative paths under
`mynodebtc-mynode/`. Only text files were pulled (icons and screenshots were skipped).
File modes were not kept, so upstream executables are plain files here. Total size is about 400 KB.

## Source: mynodebtc/mynode

| | |
|---|---|
| Repo | https://github.com/mynodebtc/mynode |
| Branch / commit | `master` @ `351432c1347370892c47dbd65c430eb9cbba0eea` (2026-09-28 21:35 -0500, "Use Docker compose v2 instead of pip docker-compose") |
| Latest release tag | `v0.3.49` = `latest_release` @ `7afa70087b9ce35c0cce3bcd1e6d26a4e0751d0d` (2026-09-25) |
| Local folder | `mynodebtc-mynode/` |
| Browse a file | `https://github.com/mynodebtc/mynode/blob/351432c1347370892c47dbd65c430eb9cbba0eea/<path>` |

### License (FLAG: restrictive, two licenses)

- **Everything outside `rootfs/standard/usr/share/mynode_apps/`** is under **CC BY-NC-ND 4.0 with
  myNode's own "Section 9" changes** (`mynodebtc-mynode/LICENSE.md`; GitHub reports `NOASSERTION`).
  Section 9 allows distribution and forks only if modifications stay public and can be merged back
  upstream without restriction. It also forbids changes to licensing, the product key and premium
  access, and forbids using the myNode brand to impersonate it.
  - NonCommercial + NoDerivatives: verbatim, attributed copies for non-commercial use are allowed.
    Publishing **modified** copies (for example a friglet app built by editing `app_sdk/sampleapp`)
    relies on the Section 9 carve-out, not on plain CC BY-NC-ND.
  - **Before this repo goes public:** a public mirror of these files is a "Share" under the
    license. Keep them verbatim, keep `LICENSE.md` next to them, and do not use them commercially.
    Or remove them from the public tree. Needs a human/legal decision.
- **`rootfs/standard/usr/share/mynode_apps/**`** (dynamic app integrations, including `canary/`) is
  under **LGPL-3.0** (`rootfs/standard/usr/share/mynode_apps/LICENSE.md`, added in commit `b805555c`,
  2022-08-13; confirmed by `README.md` lines 172-175). A friglet app folder submitted upstream
  would land under this license.

### File sets pulled (all at commit `351432c`)

| Set | Upstream paths | License |
|---|---|---|
| Dynamic-app docs + SDK template | `doc/applications.md`, `doc/development.md`, `app_sdk/create.py`, `app_sdk/sampleapp/{sampleapp.json,sampleapp.service,nginx/https_sampleapp.conf,scripts/*.sh,www/python/sampleapp.py,www/templates/{README,sampleapp.html}}` (skipped `sampleapp.png`, `screenshots/*.png`) | CC BY-NC-ND 4.0 + s9 |
| App-definition "schema" (there is no JSON Schema upstream; the real definition is the defaults function plus the doc table, checked against each other by `create.py check_doc`) | `rootfs/standard/var/pynode/application_info.py` (+ `doc/applications.md` table above) | CC BY-NC-ND 4.0 + s9 |
| electrs app definition + UI page | `rootfs/standard/usr/share/mynode/application_info.json` (built-in app list; electrs is the `"short_name": "electrs"` entry), `rootfs/standard/var/www/mynode/electrum_server.py`, `rootfs/standard/var/www/mynode/templates/electrum_server.html`, `rootfs/standard/var/pynode/electrum_info.py`, `rootfs/standard/etc/systemd/system/electrs.service`, `rootfs/standard/etc/nginx/modules-enabled/https_electrs.conf`, `rootfs/standard/etc/torrc.d/mynode_electrs`, `rootfs/standard/usr/share/mynode/electrs.toml`, `rootfs/standard/usr/bin/service_scripts/pre_electrs.sh` | CC BY-NC-ND 4.0 + s9 |
| Docker-based dynamic app reference: **Canary** (Docker Compose, digest-pinned registry images, needs electrs, custom info page; contributed by an outside developer in PR #984 and updated in #1020) | `rootfs/standard/usr/share/mynode_apps/LICENSE.md`, `rootfs/standard/usr/share/mynode_apps/canary/{canary.json,canary.service,app_data/docker-compose.yml,scripts/{install,pre,uninstall}_canary.sh,www/python/canary.py,www/templates/{README,canary.html}}` (skipped `canary.png`, `screenshots/*.png`) | **LGPL-3.0** |
| Platform plumbing cited in NOTES.md | `rootfs/standard/var/www/mynode/templates/app/generic_app.html`, `rootfs/standard/usr/bin/{mynode-manage-apps,mynode_reinstall_app.sh,mynode_startup.sh,mynode_gen_bitcoin_config.sh,mynode_firewall.sh,wait_on_bitcoin.sh,wait_on_docker_image_install.sh}`, `rootfs/standard/usr/share/mynode/{mynode_functions.sh,mynode_app_versions.sh,mynode_device_info.sh,bitcoin.conf,nginx.conf}`, `rootfs/standard/var/pynode/device_info.py`, `scripts/print_app_download_hashes.sh` | CC BY-NC-ND 4.0 + s9 |
| Project meta / maintenance evidence | `LICENSE.md`, `README.md`, `CHANGELOG`, `.github/pull_request_template.md`, `.github/copilot-instructions.md` | CC BY-NC-ND 4.0 + s9 |

### Re-verify

```sh
git clone https://github.com/mynodebtc/mynode /tmp/mn && cd mynodebtc-mynode && \
find . -type f | while read f; do git -C /tmp/mn show 351432c1347370892c47dbd65c430eb9cbba0eea:"${f#./}" | cmp -s - "$f" || echo "DIFF $f"; done
```

On 2026-10-01 all 53 files compared byte-identical (`cmp`) with the checkout at `351432c`.

## Consulted, not pulled

| Source | Ref | License | Why not pulled |
|---|---|---|---|
| https://github.com/mynodebtc/mynode_docs `docs/developer/add-app.md` | `4f435cf12a028222ef7e6e1b608604f0f26bb361` (2026-09-29) | MIT | Describes the old **built-in** app process (`application_info.json` + `mynode_post_upgrade.sh`), not dynamic apps. Out of date; do not follow it. |
| https://github.com/mynodebtc/mynode_sdk, https://github.com/mynodebtc/sdk_app_template | archived 2026-09-20 | n/a | Moved into `app_sdk/` + `doc/applications.md` in commit `d2d1eab4` (2026-09-19). Old SDK docs are out of date. |
| GitHub API: repo metadata, PRs #984 / #1020 / #1014, issue search "silent payments" / "blindbit" | retrieved 2026-10-01 | n/a | Maintenance and submission evidence, summarised in NOTES.md |
| Local `/work/blindbit/blindbit-rs` (`friglet/src/config.rs`, `Dockerfile`) | `611a877` (2026-09-22) | see that repo | friglet's own config surface; cited in NOTES.md, not a myNode source |
| Local `/work/blindbit/blindbit-oracle` (`README.md`, `blindbit.example.toml`) | `c29d3a8` (2026-09-22) | see that repo | Oracle backend requirements; cited in NOTES.md |

## Renamed on import

Files that would act on this repository (agent instructions, `.gitignore`, `.gitattributes`)
were renamed with an `.upstream` suffix on import. Their content is unchanged. Renamed here:

- `mynodebtc-mynode/.github/copilot-instructions.md`
