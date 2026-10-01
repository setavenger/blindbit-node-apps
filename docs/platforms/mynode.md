# Friglet on MyNode

MyNode runs third-party apps as **dynamic apps**: a folder under
`rootfs/standard/usr/share/mynode_apps/<short_name>/` in the `mynodebtc/mynode` repository. There
is no app store or sideloading (removed from the UI in v0.3.48-2), so Friglet reaches MyNode users
only through a pull request upstream. Background and sources: `upstream/mynode/NOTES.md`.

| | |
|---|---|
| App folder | `mynode/mynode_apps/blindbit-friglet/` (lands upstream as `rootfs/standard/usr/share/mynode_apps/blindbit-friglet/`) |
| Upstream PR | `mynode/upstream-pr/blindbit-friglet.patch`, description in `mynode/upstream-pr/PR_DESCRIPTION.md` |
| MyNode pin | `mynode/upstream-pr/pin.env` (`351432c`, master on 2026-10-01) |
| Checks | `mynode/tests/check.sh <mynode checkout>` |
| Short name | `blindbit-friglet`; display name "Friglet" |
| Status | built and checked with MyNode's tooling; **not installable** (placeholder image digest), not install-tested on a device, not submitted |

## Requirements

- A 64-bit MyNode device (`aarch64` or `x86_64`). The install refuses 32-bit devices.
- Bitcoin fully synced and **not pruned**. MyNode's default config is unpruned (`txindex=1`).
- Docker, which MyNode installs. Installing from the Marketplace reboots the device (MyNode's
  `reinstall_app` behaviour, not Friglet's).
- Sparrow, in a separate instance for this wallet (see [Connect Sparrow](#connect-sparrow)).

## How it runs

`blindbit-friglet.service` (systemd, user `bitcoin`) waits for Bitcoin and MyNode's Docker image
install, runs the pre-start script as root, then `docker-compose up` in
`/opt/mynode/blindbit-friglet` (Compose v2, `/usr/local/bin/docker-compose` since `351432c`).

| Script | Runs as | Does |
|---|---|---|
| `install_blindbit-friglet.sh` | `bitcoin`, on install, reinstall and upgrade | refuses 32-bit and pruned nodes; creates the data folder (0700); pulls `ghcr.io/setavenger/friglet-node` by the pinned index digest; writes `.env` with `FRIGLET_IMAGE=<image>:<tag>@<digest>` and the `bitcoin` uid/gid |
| `pre_blindbit-friglet.sh` | root, before every start | refuses symlinked or odd data paths; generates the login password once; refreshes `docker-compose.yml` from `app_data/`; writes `runtime.env` (LAN host, allowed hosts, onion host on Premium) |
| `post_blindbit-friglet.sh` | `bitcoin` | nothing |
| `uninstall_blindbit-friglet.sh` | `bitcoin`, on uninstall | stops the container, removes the image, deletes the data folder (see [Uninstall](#uninstall)) |

The container gets only `FRIGLET_UI_*` variables (contract: `docs/contract.md`):

| Variable | Value on MyNode |
|---|---|
| `FRIGLET_UI_PLATFORM` | `mynode` |
| `FRIGLET_UI_NETWORK` | `bitcoin` (the app is disabled in MyNode's testnet mode) |
| `FRIGLET_UI_NODE` | `host.docker.internal:8333`: bitcoind P2P on docker0 (`172.17.0.1`), which MyNode's bitcoind binds and whitelists in every network mode |
| `FRIGLET_UI_ORACLE_URL` | `https://oracle.setor.dev` |
| `FRIGLET_UI_AUTH` / `FRIGLET_UI_PASSWORD_FILE` | `password` / `/run/friglet-ui/password` (read-only bind of `.app_password`) |
| `FRIGLET_UI_LISTEN` | `0.0.0.0:8097` in the container, published on `127.0.0.1:8097` |
| `FRIGLET_UI_ELECTRUM_TCP` / `_TLS` | `0.0.0.0:50031` / `0.0.0.0:50032` in the container |
| `FRIGLET_UI_PUBLIC_HOST`, `FRIGLET_UI_ALLOWED_HOSTS`, `FRIGLET_UI_ONION_HOST` | from `runtime.env`, refreshed at every start |

The container runs as the `bitcoin` user with all capabilities dropped and `no-new-privileges`,
on the default bridge network. Compose never pulls (`pull_policy: never`): it runs exactly the
image the install pulled by digest, or refuses to start.

## Ports

| Port | Bound on the host | What | Firewall / Tor (from the app JSON) |
|---|---|---|---|
| 8098 | all interfaces (nginx) | web interface over HTTPS, MyNode certificate | ufw allow; onion port 443 |
| 8097 | `127.0.0.1` (container) | web interface, plain HTTP, for nginx and Tor only | none |
| 50032 | all interfaces (container) | Electrum over TLS, Friglet's self-signed certificate | ufw allow; onion port 50032 |
| 50031 | `127.0.0.1` (container) | Electrum without TLS, for the onion only | ufw allow (nothing listens on the LAN); onion port 50031 |

MyNode writes `/etc/torrc.d/blindbit-friglet` from `https_port` and `extra_ports`; an app cannot
ship its own Tor or nginx `stream {}` config. The generated file is:

```
HiddenServiceDir /var/lib/tor/mynode_blindbit-friglet/
HiddenServicePort 443 127.0.0.1:8098
HiddenServicePort 50031 127.0.0.1:50031
HiddenServicePort 50032 127.0.0.1:50032
```

None of these ports is used by anything else in the MyNode tree at the pin (electrs owns
50001/50002, JoinMarket 8080).

## Paths

| Path | Contents | Kept on upgrade |
|---|---|---|
| `/opt/mynode/blindbit-friglet/` | `docker-compose.yml`, `.env`, `runtime.env` | no (MyNode empties it) |
| `/mnt/hdd/mynode/blindbit-friglet/data/` | friglet-ui data dir (`/data`): `config.toml`, `scan.key`, scan state, TLS certificate, `prev/`, UI secrets | yes |
| `/mnt/hdd/mynode/blindbit-friglet/.app_password` | web interface password (0600, `bitcoin`) | yes |
| `/var/lib/tor/mynode_blindbit-friglet/` | onion service keys (MyNode's) | yes, also after uninstall |

## Login

The pre-start script generates a 24-character password on first start with MyNode's
`generate_app_password` and saves it with `save_app_default_password`. Friglet's app page in MyNode
(behind the MyNode login) shows it. Friglet is not in MyNode's list of apps whose passwords
"Reset app passwords" handles.

To get a new password: `sudo rm /mnt/hdd/mynode/blindbit-friglet/.app_password && sudo systemctl
restart blindbit-friglet`. This relies on friglet-ui taking `FRIGLET_UI_PASSWORD_FILE` as the
current password at every start (open question for friglet-ui).

## Connect Sparrow

Use a **separate Sparrow instance with its own data directory** for the Silent Payments wallet.
The Electrum server set in Sparrow applies to every wallet in that instance, and Friglet only knows
this one: the others would show no history. Switch that instance back to your usual server to see
them again.

| Endpoint | When | Sparrow |
|---|---|---|
| `<node IP>:50032:s` (recommended) | always, local network | host = node IP, port 50032, SSL on; check the certificate fingerprint against Friglet's Connect page |
| `<onion>:50031:t` | Premium with Tor remote access on | port 50031, SSL off, Tor on in Sparrow |

On Community Edition, or with Tor remote access off, MyNode returns no onion hostname and the app
page says so, like the Electrum Server page.

## Privacy

- Anyone who can reach port 50032 on the local network, or who knows the onion address, can read
  this wallet's addresses, transactions and balance. Do not forward Friglet's ports on the router.
- Friglet gets tweak data from the BlindBit oracle (`https://oracle.setor.dev`), a third-party
  service run by BlindBit. The operator sees this node's IP address, which block ranges are scanned
  and when. It never sees keys.
- The scan key is entered only in Friglet's web interface and stored in the data folder (0700). It
  is never in the compose file, the environment, the images or the logs.
- Until BR8, Friglet's logs contain txids, balances and client IP addresses. They are in the
  journal (`journalctl -u blindbit-friglet`, MyNode's Status page) and in Docker's log files.

## Pruned node

The install stops with:

```
ERROR: Friglet cannot be installed: this node is pruned.
  Cause: /mnt/hdd/mynode/bitcoin/bitcoin.conf sets prune=<N>. Friglet downloads every block from your wallet's
  birthday onward from this node, and a pruned node has deleted old blocks.
  Fix: remove the prune setting on MyNode's Bitcoin Config page (/bitcoin/config), let
  Bitcoin download the full chain again, then install Friglet again.
```

It checks `prune=` at the top level or in `[main]`, and `main.prune=`; `prune=0`, comments and
other networks' sections are ignored. A node pruned after install is reported by Friglet itself
(state `node-pruned`), and the pre-start script logs a warning.

## Uninstall

Uninstalling stops the container, removes the `friglet-node` images and deletes
`/mnt/hdd/mynode/blindbit-friglet`: the scan key, Friglet's wallet settings, the scan progress
and its backups, any previous wallet kept for restore, the Electrum TLS certificate and the login
password. The script prints this list. Bitcoin is not affected: the scan key can be entered again
from Sparrow, and a new install scans again from the wallet birthday. MyNode keeps the onion keys,
so a reinstall gets the same onion address.

## Upgrades and rollback

- A new Friglet version is a new upstream PR: `latest_version` in the JSON and the
  `IMAGE_DIGEST` line printed by `scripts/print_app_download_hashes.sh blindbit-friglet`.
- MyNode then runs the install script again; the data folder is kept and friglet-ui handles state
  backups across versions.
- Rollback: set Friglet's version to the previous tag on MyNode's "Customize Application
  Versions" page (`/apps/customize-app-versions`) and reinstall. MyNode pulls custom versions
  **by tag, without a pin**.

## Checks

`mynode/tests/check.sh <mynode checkout>` copies the app into a scratch clone of the checkout and
runs, in containers: `app_sdk/create.py check blindbit-friglet` (must print "looks good"),
`create.py check_doc`, shellcheck following MyNode's real `/usr/share/mynode` helpers, a lifecycle
run of install, pre-start and uninstall as MyNode runs them (`tests/lifecycle.sh`, Python 3.8 on
Debian 12 with stand-ins for Docker), `docker compose config` on the files those scripts wrote,
and `tests/web_sim.py`, which loads the app through functions taken from MyNode's
`application_info.py` (defaults, Tor, firewall, blueprint loader) and renders the app page with
MyNode's templates. `--in-place` checks a checkout with the patch already applied.

Not covered: a real MyNode device, a real image pull, nginx and Tor themselves.

To try it on a development device (not possible for users since sideloading was removed):

```sh
scp -r mynode/mynode_apps/blindbit-friglet admin@<device>:/tmp/
# on the device
sudo cp -r /tmp/blindbit-friglet /usr/share/mynode_apps/
sudo mynode-manage-apps init && sudo mynode-manage-apps install blindbit-friglet
```

## Submitting upstream

1. Publish the image and replace the placeholder digest (see `PR_DESCRIPTION.md`, internal list).
2. `mynode/upstream-pr/make-patch.sh` rebuilds the patch; rebase `pin.env` to current upstream
   master first and run `mynode/tests/check.sh` again.
3. Test on devices, add screenshots, then open the PR from a fork with the description draft.

The Canary app took about 3.5 months and 12 reviews to merge (PR #984), and MyNode has a single
maintainer, so expect review rounds.

## License

`mynode_apps/` in MyNode is LGPL-3.0; the rest of MyNode, including the `app_sdk/sampleapp`
template, is CC BY-NC-ND 4.0 with MyNode's own changes (`upstream/mynode/SOURCES.md`).

- The app files are new work. Their structure follows the sample app and the LGPL apps Canary and
  albyhub. Every line they share with `app_sdk/sampleapp` (56 lines on 2026-10-01: helper `source`
  lines, `ExecStartPre` wrappers, nginx `include` lines, page markup) also appears in LGPL app
  folders at the pin.
- The app page template reuses the layout markup of Canary's page (LGPL-3.0), so it is a
  derivative of LGPL code.
- Once merged, the folder is distributed under LGPL-3.0. This repository should offer
  `mynode/mynode_apps/` under LGPL-3.0 too (a maintainer decision; no license file is added yet).
- The icon is `friglet-tray/icons/icon.png` from blindbit-rs `93f6a92` (MIT, same author).
