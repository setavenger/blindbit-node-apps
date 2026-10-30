# PR draft: New app friglet (Silent Payments scanner with an Electrum server)

Target: `raspiblitz/raspiblitz` branch `dev`. Prepared against `dev` at
`c421693dcb09b00eafacbda6128174d2c658cb06` (2026-08-23).

**Do not open yet.** These must be true first:

- friglet-node release tarballs are published, and `VERSION`, `RELEASE_BASE_URL` and
  `SHA256_ARM64`/`SHA256_AMD64` in `bonus.friglet.sh` are pinned to them.
- The release signing key is decided and `PGP_PUBKEY_URL`/`PGP_FINGERPRINT` are set.
- The blindbit-node-apps release gate is met: blindbit-rs BR1 (key-checked
  `blockchain.silentpayments.subscribe`) and BR3 are merged and re-pinned.
- The script ran on a real RaspiBlitz (arm64 Pi and an amd64 VM) with `github` + `patch`,
  using the checklist below.

Assemble the branch with `raspiblitz/upstream-pr/check.sh`. It adds `bonus.friglet.sh` from
`raspiblitz/home.admin/config.scripts/`, applies the `0*.patch` files and runs shellcheck.

---

## Title

New app: friglet, a Silent Payments scanner with an Electrum server (SSH menu)

## Description

Thank you for your contribution to RaspiBlitz. Before submitting this PR, please make sure:
- [x] That its based against the `dev` branch - not the default release branch.

**What it is.** [friglet](https://github.com/setavenger/blindbit-rs) scans the chain for one
BIP 352 Silent Payments wallet and serves its history over the Electrum protocol, so Sparrow
can use the node for an `sp1…` wallet. It uses a BlindBit oracle (tweak data, third party,
clearnet) and the node's own bitcoind over local P2P for blocks and broadcasts. It needs no
RPC credentials. A small companion, `friglet-ui`, runs friglet and serves the setup, status
and connection pages.

**How it is installed.** `bonus.friglet.sh` follows `bonus.template.sh` and the fulcrum
pattern (prebuilt binary, verified before use). There is no Docker and no build on the Pi.

- `install`: downloads `friglet-node-<version>-linux-<arm64|amd64>.tar.gz` for the version
  pinned in the script. It refuses unless the SHA256 matches the pinned value, and checks a
  PGP signature over `SHA256SUMS` by the pinned key. Binaries are root-owned in
  `/home/friglet/friglet-node-<version>/`, and `current` points at the active one.
- `on`: refuses on a pruned node and on chain=test (testnet3 is not supported). It creates
  `/mnt/hdd/app-data/friglet` (0700), generates a web UI password, adds
  `<chain>.whitebind=download@127.0.0.1:8337` to bitcoin.conf and writes `friglet.service`.
  It then adds the nginx HTTPS site (8098) with the standard snippets, LAN-only ufw rules,
  the onion service and `friglet=on`. bitcoind is restarted once, only when the whitebind
  was added and the node is ready. Re-running `on` (provisioning) keeps the wallet and the
  password, and restarts nothing that did not change.
- `off [--delete-data]`: removes the unit, nginx site, ufw rules, onion service, whitebind
  line and the user. Wallet data is kept unless `--delete-data` is given.
- `menu`: web UI address, certificate fingerprint, password, status, password reset and
  Electrum exposure.
- `update`: installs the version pinned in the script, and keeps the previous one for
  rollback.
- `status` / `status-sync`: key=value output, fast, nothing on stderr (WebUI-compatible
  keys).

**Why port 8337 for the whitebind:** 8335 is electrs. BTCPay Server already binds
`<chain>.whitebind=127.0.0.1:8336`, and a second bind on the same address stops bitcoind
from starting. `on` refuses if another whitebind already uses 8337.

**Security notes**

- The wallet's scan key and config live only in `/mnt/hdd/app-data/friglet` (0700, user
  friglet). They are written by friglet-ui, never by the script. The script never prints the
  UI password, never uses `tee`, and passes the password to friglet-ui only through files
  and stdin.
- The web UI is password protected. The password is generated at install and shown only in
  the SSH menu. `menu` refuses to run without a terminal, so the password cannot reach the
  provisioning log.
- Electrum serves one wallet's history to anyone who can reach it. By default: TLS on 50032
  for the LAN (friglet-ui's self-signed certificate, fingerprint shown in its UI), plaintext
  50031 on loopback for the onion only. ufw rules allow only 10/8, 172.16/12 and 192.168/16,
  as for HTTPS 443 in `_provision_.sh`. The SSH menu offers `lan-tcp` (also plaintext on the
  LAN), `onion` and `local`.
- `blitz.debug.sh` shows unit status and `bonus.friglet.sh status` only. It leaves out
  friglet's journal, which currently logs txids and amounts.
- The oracle is a third-party service reached over clearnet (friglet has no SOCKS support
  yet). The web UI says so before setup.

**Requirements:** an unpruned node (friglet fetches every block from the wallet birthday),
`chain=main` or `chain=sig`, arm64 or amd64.

**Ports**

| Port | Bind | What |
|---|---|---|
| 8097 | 127.0.0.1 | friglet-ui web UI (behind nginx and Tor) |
| 8098 | nginx, ufw LAN | web UI over HTTPS |
| 50031 | 127.0.0.1 (LAN with `lan-tcp`) | Electrum plaintext, onion port 50031 |
| 50032 | 0.0.0.0, ufw LAN | Electrum TLS |
| 8337 | 127.0.0.1 | bitcoind whitebind (download) for friglet |

**Files touched besides the new script:** `_provision_.sh`, `00settingsMenuServices.sh`
(`fg`), `00mainMenu.sh`, `99connectMenu.sh`, `99updateMenu.sh`, `00infoBlitz.sh`,
`blitz.debug.sh`, `blitz.shutdown.sh` (clean stop before power off), `tor.network.sh`
(recreate the onion when Tor is switched on) and `CHANGES.md`. The WebUI (blitz_api `AppId`,
raspiblitz-web) is not part of this PR.

## How to test

On a synced, unpruned RaspiBlitz: `github` → REPO → this fork, BRANCH → this branch, then `patch`.

1. A pruned node refuses: set `prune=550` in bitcoin.conf, run
   `config.scripts/bonus.friglet.sh on`, and check that it prints the REFUSED text and
   changes nothing.
2. SERVICES → `fg` on. The FRIGLET menu shows `https://<ip>:8098` and a password. Log in,
   paste the descriptor from Sparrow, set the birthday and check the address matches.
3. `bonus.friglet.sh on` again: nothing restarts, and the password and wallet are unchanged.
4. Connect Sparrow to `<ip>:50032:s`, then to the onion `:50031:t` over Tor.
5. FRIGLET menu → Electrum exposure → `lan-tcp` / `local`: check `sudo ufw status` and
   `ss -ltn`.
6. FRIGLET menu → reset the password: the old browser session is logged out.
7. `sudo shutdown -r now` (or an SD card update): friglet comes back with the same wallet.
8. SERVICES → `fg` off → keep data, then on again: same wallet. Off → delete data: the
   data dir is gone.
9. `bonus.friglet.sh status` as root: key=value only, nothing on stderr.

Tested so far (blindbit-node-apps `raspiblitz/test/run.sh`): Debian bookworm container with
stub friglet binaries, a fake bitcoind/Tor/ufw and real nginx with the RaspiBlitz snippets.
It was not yet tested on RaspiBlitz hardware.
