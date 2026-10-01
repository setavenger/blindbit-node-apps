# Friglet on Umbrel

Friglet runs on umbrelOS from the BlindBit community app store in this repository. The package is
`blindbit-friglet/` at the repository root, next to `umbrel-app-store.yml` (store id `blindbit`,
shown in Umbrel as "BlindBit App Store").

> **Release gate.** The store entry is for maintainer testing only. It is not announced, and no
> catalog submission is made, until all of these hold (`docs/plan.md`):
>
> - the blindbit-rs fixes for spend detection, reorg rollback and the oracle-stream panic are
>   merged and re-pinned here (BR3);
> - friglet checks the client's keys in `blockchain.silentpayments.subscribe` (BR1);
> - a pinned Sparrow version sets up the wallet, receives a signet payment, spends it, and shows
>   the same history after a restart, both on a real Umbrel and on compose.
>
> Until the first `friglet-node` image is published, the image digest in
> `blindbit-friglet/docker-compose.yml` is an all-zero placeholder, so the app cannot be installed
> yet.

## Before you install

- **umbrelOS 2.0 or later.** The manifest declares `manifestVersion: 2.0`, so older umbrelOS
  versions refuse the install.
- **The Bitcoin Node app, fully synced and not pruned.** Friglet downloads every block from your
  wallet's birthday onward from this node. A pruned node cannot serve those blocks. Pruning is off
  by default in the Bitcoin Node app; leave it off. Bitcoin Knots and Libre Relay can stand in for
  the Bitcoin Node app, but they are untested with Friglet.
- **Network.** Friglet follows the Bitcoin Node app's network setting: mainnet, signet, testnet4
  or regtest. testnet3 is not supported.
- **Outbound internet access** to the BlindBit oracle (see "What the oracle sees").
- **A dedicated Sparrow instance** for the Silent Payments wallet (see "Connect Sparrow").

## Install

1. In umbrelOS, open the **App Store**, click **⋯** (top right), then **Community App Stores**.
2. Paste `https://github.com/setavenger/blindbit-node-apps` and click **Add**. Umbrel warns that
   community apps are not reviewed by Umbrel.
3. Open **BlindBit App Store**, choose **Friglet**, and click **Install**. If the Bitcoin Node app
   is missing, Umbrel installs it first.
4. Open Friglet from the home screen (`http://umbrel.local:2104`). Umbrel asks for your Umbrel
   login (and 2FA, if enabled) first. Friglet has no password of its own on Umbrel.
5. Paste the Silent Payments descriptor that Sparrow shows (`sp(spscan1…)`) and the wallet's
   birthday height, as shown in Sparrow. Friglet shows the wallet's Silent Payments address; make
   sure it matches Sparrow's receive address.

Umbrel serves the store from this repository's **default branch**. It checks that branch every
5 minutes and offers an update whenever the app's `version` changes.

## Ports

| Port | Where | What |
|---|---|---|
| 2104 | Umbrel host | Friglet web UI behind Umbrel's login (Umbrel app gateway) |
| 50031 | Umbrel host, and the Electrum onion | Electrum, **unencrypted** |
| 50032 | Umbrel host | Electrum over TLS with a self-signed certificate (fingerprint on the Connect page) |
| 8097 | inside the container only | friglet-ui; the gateway forwards port 2104 here |
| 8095 | container loopback only | friglet's HTTP API; never published |
| 8335 | Bitcoin Node app (outbound from Friglet) | the node's whitelisted P2P listener, where Friglet downloads blocks |

The published Electrum ports are fixed on Umbrel. The "Electrum exposure" setting under Advanced
does not apply here.

## Connect Sparrow

The app's **Connect** page is the reference; it shows this Umbrel's exact host names, the TLS
fingerprint and the onion address.

1. **Use a dedicated Sparrow instance.** Friglet serves only this one wallet. Any other wallet in a
   Sparrow that points at Friglet shows up empty. Start a second Sparrow with its own data
   directory (the Connect page shows the launch command for your operating system) and keep the
   Silent Payments wallet there. Your other wallets are not changed: switch your usual Sparrow
   back to its usual server and they show up again.
2. In that Sparrow, open **Settings → Server → Private Electrum** and enter one endpoint:
   - **Recommended, local network:** host `umbrel.local` (or the Umbrel's IP address), port
     `50032`, **Use SSL** on. Sparrow asks you to trust the self-signed certificate; compare its
     SHA-256 fingerprint with the one on the Connect page first.
   - **Tor:** the onion address from the Connect page, port `50031`, **Use SSL** off. Tor itself
     encrypts the connection. Sparrow must be set up to use Tor.
   - **Local network, unencrypted:** host `umbrel.local`, port `50031`, **Use SSL** off. Anyone on
     your network can read this traffic. Use it only if TLS fails.
3. Click **Test Connection**, then open the Silent Payments wallet with the same birthday height
   that you entered in Friglet.

While Friglet is still scanning, Sparrow shows payments up to the block that has been scanned so
far. More appear as the scan continues.

## Who can read the Electrum ports

Ports 50031 and 50032 have no password. Anyone who can reach them can read this wallet's
addresses, transactions and balance history. That includes every device on your local network
and anyone who learns the onion address. Do not forward these ports on your router. The onion
address is shown only inside the app, behind your Umbrel login.

## What the oracle sees

Friglet downloads per-block Silent Payments data from the BlindBit oracle
(`https://oracle.setor.dev`; `https://signet.oracle.setor.dev` on signet). The BlindBit developer
runs the oracle; neither Umbrel nor you do. The operator can see your Umbrel's public IP address,
which block ranges it scans, and when. The oracle never receives your keys, addresses or balance.

## Home-screen widget

Friglet offers a "Silent Payments scan" widget with scan progress. It reads only the scan state,
the scanned height and the tip height from friglet-ui's unauthenticated `/api/widget` endpoint.
Umbrel fetches that endpoint from inside the device, not through the browser.

## Data, backups and logs

- Everything Friglet keeps is in `app-data/blindbit-friglet/data/friglet/` on the Umbrel (or on
  the drive you moved the app's storage to): `config.toml`, `scan.key`, `scanner_state.json`
  (which also contains the scan secret), the TLS certificate, the `prev/` snapshot during a
  wallet change, and the upgrade backup.
- **Umbrel backups include your scan key.** Only the rebuildable block-header cache
  (`scanner_state.headers.json`) is excluded.
- Friglet's logs are in the app's logs view in umbrelOS. Until BR8 ships they can contain txids,
  amounts and client IP addresses; check them before you share them. The **Diagnostics** page
  produces a report without the secret.

## Upgrade and rollback

- **Upgrade.** When a new version is pushed to the store branch, Umbrel shows an update for
  Friglet. The update replaces the package files (`docker-compose.yml`, `exports.sh`,
  `torrc.template`, `hooks/`) and keeps the data folder. Before a different friglet version runs,
  friglet-ui stops friglet cleanly and backs up the state file. If the update needs a rescan, the
  app says so.
- **Rollback.** Umbrel has no downgrade button. Umbrel offers an update whenever the store's
  `version` differs from the installed one, so the maintainer rolls back by restoring the previous
  image tag, digest and `version` on the store branch. Users then install that "update". The
  backup friglet-ui made before the upgrade stays in the data folder. Restoring it needs the
  umbrelOS terminal for now; without it, Friglet rescans from the birthday.

## Uninstall

Uninstalling Friglet:

- removes its containers;
- deletes `app-data/blindbit-friglet/`, including the scan key, config, state file, `prev/`, the
  upgrade backup and the TLS certificate. A reinstall gets a new certificate, so Sparrow has to
  trust it again;
- keeps the Tor key in `tor/data/app-blindbit-friglet-electrum/`. umbrelOS does not delete it, so a
  reinstall keeps the same onion address;
- keeps your Umbrel backups, which still contain the scan key until they are deleted;
- leaves the Bitcoin Node app and Sparrow unchanged.

The Bitcoin Node app cannot be uninstalled while Friglet is installed.

## For maintainers

- **Store layout.** umbreld reads `umbrel-app-store.yml` at the repository root and only the
  manifests that match `*/umbrel-app.yml` one level below it, then drops apps whose id does not
  start with `blindbit` (`upstream/umbrel/umbrel/packages/umbreld/source/modules/apps/app-repository.ts:157-184`). Other
  top-level folders (`docs/`, `upstream/`, `startos/`, …) are ignored, and so are the nested
  manifests under `upstream/`.
- **Clone cost.** Every Umbrel that adds the store makes a depth-1 clone of the whole default
  branch. It makes a fresh full clone again after every new commit on that branch, whatever the
  commit touches. At `feat/umbrel` @ `a98308d` that clone is a 1.9 MB pack and 13 MB on disk
  (1,013 files). `upstream/` accounts for 11 MB of it.
- **Pinning.** `blindbit-friglet/docker-compose.yml` pins
  `ghcr.io/setavenger/friglet-node:<tag>@sha256:<index digest>`, and
  `blindbit-friglet/umbrel-app.yml` carries the same `<tag>` as `version`. Change both together.
- **Static IP.** `friglet-node` uses `10.21.21.104` (`exports.sh`) because the Tor
  `HiddenServicePort` target must be a fixed address. That address was unused in
  `getumbrel/umbrel-apps@01be6c3`. Another community store could still claim it.
- **Lint.** From a checkout of `getumbrel/umbrel-apps` at `01be6c3` (the commit pulled into
  `upstream/umbrel/`), copy `blindbit-friglet/` in and run
  `npm ci && npm run lint:apps -- blindbit-friglet --check-images`. The image check fails until the
  placeholder digest is replaced.
