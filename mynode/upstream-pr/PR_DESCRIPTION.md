# Upstream PR draft: Friglet for MyNode

Target: `mynodebtc/mynode` `master`, based on `351432c` (`pin.env`). Patch: `blindbit-friglet.patch`
(built by `make-patch.sh`). Everything above the line is for us; everything below it is the PR body.

## Before opening the PR (internal, not part of the PR body)

- [ ] `ghcr.io/setavenger/friglet-node:<version>` is published as a multi-arch index (amd64 + arm64).
- [ ] `latest_version` in the JSON and the tag in `IMAGE_DIGEST` match that version, and the
      all-zero PLACEHOLDER digest in `install_blindbit-friglet.sh` is replaced with the line
      `scripts/print_app_download_hashes.sh blindbit-friglet` prints (needs `docker buildx`).
- [ ] Release gate from `docs/plan.md` met: BR1 (key-checked `silentpayments.subscribe`) and BR3
      (spend detection, reorg rollback, oracle-stream panic) merged in blindbit-rs and re-pinned.
- [ ] Installed, restarted, upgraded, uninstalled and reinstalled on a real MyNode device (Raspberry
      Pi 4/5 arm64 and an x86_64 VM); devices listed below; screenshots added to `screenshots/`.
- [ ] `mynode/tests/check.sh` passes on the rebased patch; patch rebuilt on current upstream master.
- [ ] Maintainer agrees to contribute the folder under LGPL-3.0 (see `docs/platforms/mynode.md`).
- [ ] Short name settled: `blindbit-friglet` (matches the Umbrel app id) is the first MyNode app
      name with a `-`; MyNode loads it fine (checked), but upstream may prefer `[a-z_]`.

---

**Add Friglet, a Silent Payments Electrum server (BlindBit)**

## Description

Friglet lets a MyNode owner receive Silent Payments (BIP 352) into Sparrow using their own node.
The user pastes the Silent Payments descriptor Sparrow shows and the wallet birthday into
Friglet's web interface; Friglet scans the node's blocks for that one wallet and serves its
history to Sparrow over the Electrum protocol. MyNode has no Silent Payments support today, and
electrs cannot serve Silent Payments wallets.

What the app does on MyNode:

- **One Docker container**, `ghcr.io/setavenger/friglet-node` (amd64 + arm64), pulled in the
  install script with `pull_app_docker_image` by its pinned image index digest and run by
  Compose as `<tag>@<digest>` with `pull_policy: never`. The pin is listed in
  `MARKETPLACE_DOCKER_IMAGES`. No on-device build.
- **Bitcoin over P2P only**: the container reaches bitcoind at `host.docker.internal:8333`
  (docker0, which bitcoind binds and whitelists). No RPC credentials are used.
- **Web interface on HTTPS 8098** through nginx with the MyNode certificate
  (`nginx/https_blindbit-friglet.conf`). The container publishes the interface on
  `127.0.0.1:8097` only. The pre-start script generates the login password with
  `generate_app_password` / `save_app_default_password`, and the app page shows it.
- **Electrum**: TLS on 50032 for the local network (Friglet's own self-signed certificate; its
  fingerprint is shown in Friglet), plaintext 50031 published on loopback only for the onion
  service. Both are `extra_ports`, so MyNode creates the firewall rules and the onion ports,
  following electrs (50001/50002). 50031/50032/8097/8098 are unused elsewhere in this repo.
- **App page** (`/app/blindbit-friglet/info`): web interface URL, password (hidden until
  clicked), scan status read from Friglet's unauthenticated read-only `/api/widget`, the Sparrow
  connection strings, and the onion endpoints on Premium with Tor remote access on (otherwise it
  says "Premium feature" like the Electrum Server page).
- **Refusals**: the install stops with a message on pruned nodes (Friglet downloads every block
  from the wallet birthday from this node) and on 32-bit devices.
- **Uninstall** stops the container, removes the image and deletes
  `/mnt/hdd/mynode/blindbit-friglet`, which holds the wallet's scan key; the script lists what it
  deletes. `data_manageable` is off for the same reason. The scan key can be entered again from
  Sparrow.
- `is_beta: true`, `supports_testnet: false`.

Privacy, as the app and its page say: anyone who can reach port 50032 on the LAN, or who knows the
onion address, can read this wallet's history. Friglet gets tweak data from the BlindBit oracle
(`https://oracle.setor.dev`, run by BlindBit); its operator sees the node's IP address and which
block ranges are scanned, never keys.

Checks run on this branch (`app_sdk/create.py check blindbit-friglet` → "blindbit-friglet looks
good!", `create.py check_doc` clean, shellcheck clean). The new files are contributed under the
LGPL-3.0 that covers `mynode_apps/`.

## Checklist

* [ ] tested successfully on local MyNode, if yes, list the device(s) below
* [x] mentioned related open issue, if any (none found for Silent Payments or BlindBit)

## List of test device(s)

<!-- fill in before opening: e.g. Raspberry Pi 5 (arm64), x86_64 VM -->
