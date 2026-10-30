# friglet on StartOS

The StartOS package lives in [`startos/`](../../startos/). It runs the `friglet-node`
image (see [`docs/contract.md`](../contract.md)) with `friglet-ui` supervising friglet,
gets blocks from the Bitcoin service on the same server, and serves one Silent Payments
wallet to Sparrow over Electrum with TLS from StartOS.

| | |
|---|---|
| Package id | `blindbit-friglet` (permanent: StartOS keys installs, data and backups by it) |
| Version | ExVer `0.1.0:0` (friglet crate version at the blindbit-rs pin, package revision 0) |
| StartOS | 0.4.0 or later; start-sdk 2.0.9 (manifest `osVersion` 0.4.0-beta.10) |
| Architectures | x86_64, aarch64 (one `.s9pk` each) |
| Needs | Bitcoin (`bitcoind`) on the same server, unpruned, at a version with the `peer-local` port |
| Status | Packs and inspects cleanly; not yet installed on a StartOS device. The release image is not published yet. |

## Before you install

- **Bitcoin, unpruned.** friglet downloads every block from the wallet birthday onward from
  your node. On install, StartOS asks you to run Bitcoin's **Auto-Configure** to set
  `prune = 0`; a pruned node has to re-download the chain after that.
- **Bitcoin version.** The package needs the whitelisted peer port (`peer-local`) that
  Bitcoin's StartOS package added in `28.4:17`, `29.4:4`, `30.3:4` and `31.1:4`. Older
  versions show "Bitcoin does not offer the whitelisted peer port…" and friglet does not start.
- **A separate Sparrow.** friglet knows one wallet. A Sparrow that points at friglet shows no
  history for any other wallet, so run the Silent Payments wallet in its own Sparrow instance
  (its own data directory).
- **The oracle.** friglet asks the BlindBit oracle at `https://oracle.setor.dev`, run by a
  third party, for scan data. The oracle sees this server's IP address and which block ranges
  it scans, never your keys.

## Get the package

There is no published `.s9pk` yet. Build it (any Linux x86_64 host):

1. Tools: Docker, make, git, jq, Node.js 22 or later, python3 and a C++ compiler (`npm ci`
   builds a native addon that the Bitcoin package pulls in), `tar2sqfs` (squashfs-tools-ng) and
   `start-cli` 2.1.0 (verify its signature: Start9 `5456 DBFF 1B9D F905 041F A776 5259 ADFC 2D63 C217`
   and Matt Hill `A969 C8EB A8B1 3613 D256 8A85 EE39 5832 E45A 6664`).
2. A signing key: `start-cli init-key`, then
   `mkdir -p startos/.startos && cp ~/.startos/id.key.pem startos/.startos/build.key.pem`.
3. In `startos/`: `npm ci`, then one of
   - `make` — packs the published image named in `startos/startos/manifest/image.ts`
     (fails until that image exists);
   - `make dev` — packs locally built images tagged `friglet-node:dev-x86_64` and
     `friglet-node:dev-aarch64`. Build them from the repository's image build with those
     tags. `make stub-image` builds a stand-in (busybox scripts that follow the CLI contract,
     no scanner) for packaging tests only; never install a stub build on a real server.
4. Result: `blindbit-friglet_x86_64.s9pk` and `blindbit-friglet_aarch64.s9pk`. Check one
   with `start-cli s9pk inspect <file> manifest`.

Checks: `npm run check` (TypeScript and unit tests),
`FRIGLET_NODE_IMAGE=friglet-node:dev-x86_64 npm run test:docker` (runs the setup command
and the daemon from the package against that image in Docker).

## Sideload

In the StartOS web interface, click **Sideload** and drop the `.s9pk` that matches your
server (x86_64 for most PCs, aarch64 for ARM boards). From a development machine that is
logged in to the server, `make install` in `startos/` does the same.

## Set up

1. Run Bitcoin's **Auto-Configure** task if StartOS shows it.
2. Run **Set up wallet** (a critical task, so it appears right after install). Paste:
   - the Silent Payments wallet's descriptor as Sparrow shows it (it starts with `sp(`).
     It contains the scan key. It goes to the setup command on standard input only and is
     stored on the server's data volume with mode 0600; StartOS never shows it again;
   - the wallet's birthday height, the same one Sparrow uses (Sparrow writes it into the
     descriptor as `?bh=<height>` once the wallet has received a payment). The minimum is
     taproot activation, block 709632.
3. The action returns the wallet's Silent Payments address. Compare it with the receive
   address in Sparrow.
4. Start friglet. Health checks on the service page:
   - **Web UI** — friglet-ui is up;
   - **Scan** — state and heights (`Scanning`, `Caught up with your node at block N`, or what
     needs attention, for example a wallet identity mismatch or an unreachable oracle);
   - **Electrum** — green once friglet has verified the wallet and opened the Electrum port.

**Change wallet** (the same action, renamed once a wallet is set up) replaces the wallet:
friglet keeps the old files in `prev/`, scans again from the new birthday, and Sparrow must
use the same birthday.

## Web UI

Open the **Web UI** interface. StartOS serves it over HTTPS with your server's root CA, and
friglet-ui shows its sign-in page: enter the password from **Show web UI password**. The
login applies on every path to port 8097, including other services on the server and a LAN
you mark secure. **Reset web UI password** generates a new password, writes it to
`/data/ui-password` (0600) and restarts friglet if it is running; friglet-ui then replaces
its stored hash and signs everyone out. Resetting does not touch the wallet or the scan.

## Connect Sparrow

1. Copy an address from the **Electrum (SSL)** interface (`ssl://<host>:<port>`). StartOS
   picks the external port at install (it prefers 50032), so take it from the address.
2. In the separate Sparrow: **File → Preferences → Server → Private Electrum**, enter host
   and port, turn **Use SSL** on.
3. Optional, recommended: download your server's root CA from
   `http://<your-server>/static/local-root-ca.crt` and put it in Sparrow's certificate field.
4. **Test Connection.** Payments appear up to the block friglet has scanned so far.

LAN addresses work by default. For Tor, install the Tor service and add an onion address to
the **Electrum (SSL)** interface; Sparrow routes `.onion` through its own Tor. Anyone who can
reach the Electrum port can read this wallet's history, so only add addresses you use.

## Ports

| Port | Where | What |
|---|---|---|
| 8097/tcp | container | friglet-ui (HTTP, own password login); off the box through the StartOS HTTPS proxy |
| 50031/tcp | container, bridge only | plaintext Electrum from friglet-ui; StartOS terminates TLS in front of it |
| assigned at install (prefers 50032) | LAN / onion / domain | Electrum over SSL, the address wallets use |
| 127.0.0.1:50001 | inside the container | friglet's own Electrum, never exposed |
| bitcoind `peer-local` (container port 58334) | bridge | whitelisted P2P the package resolves at start (`FRIGLET_UI_NODE`) |
| 443/tcp outbound | internet | BlindBit oracle |

## Upgrade

Sideload a newer `.s9pk` of the same package id; StartOS stops friglet (up to 150 s for a
clean stop), swaps the code and image, runs the version migrations and starts it again.
Wallet, scan state and the web UI password are kept. Going back to an
older package version is not supported (the migration down is marked impossible): restore a
backup taken before the upgrade instead.

## Backups and restore

StartOS backups include both volumes: the wallet config, the scan key, friglet's state file
(so a restore does not rescan) and the web UI password. They are encrypted with your
StartOS master password, but they do contain the scan key. After a restore, **Change
wallet** is offered, because the restored data already holds a wallet.

## Uninstall

Uninstalling removes the container and both volumes: config, scan key, state file, `prev/`
and the web UI password. Backups you made stay on the backup target.
Bitcoin keeps `prune = 0`. Point Sparrow back at your previous server to see other wallets.

## Logs

The service log on StartOS carries friglet-ui's and friglet's output. Until friglet redacts
them (BR8), these logs can contain transaction ids, amounts and client IP addresses; share
them with care.
