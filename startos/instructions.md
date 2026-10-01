# friglet

## Documentation

- [Start9 Bitcoin Guides](https://docs.start9.com/bitcoin-guides/) — connecting a wallet to an Electrum server on StartOS, trusting your server's certificate, archival versus pruned nodes.
- [BlindBit (blindbit-rs)](https://github.com/setavenger/blindbit-rs) — the project friglet comes from.
- [BIP352](https://github.com/bitcoin/bips/blob/master/bip-0352.mediawiki) — the Silent Payments specification.

## What you get on StartOS

- **Web UI** — the wallet's identity, scan progress and connection details. Your browser asks for a username and password: the username is `friglet`, and the **Show web UI password** action shows the password.
- **Electrum (SSL)** — the endpoint Sparrow connects to. It serves this one wallet's history to anyone who can reach it, so only add addresses (Tor, a domain) to it that you mean to use.

friglet scans for one wallet. It fetches blocks from your Bitcoin node over the node's whitelisted peer port, and it asks the BlindBit oracle at `oracle.setor.dev`, run by a third party, for the data a scan needs. The oracle sees this server's IP address and which block ranges it scans, never your keys.

## Getting set up

1. **Install Bitcoin** and let it sync. friglet needs every block from your wallet's birthday onward, so Bitcoin must not be pruned: run Bitcoin's **Auto-Configure** task when StartOS asks for it.
2. **Run Set up wallet.** In Sparrow, open the Silent Payments wallet and copy its descriptor (it starts with `sp(`), and note the wallet's birthday height. Paste both into the form. The descriptor contains your scan key; friglet keeps it on this server and never shows it again.
3. **Check the address.** The action shows a Silent Payments address. Compare it with the receive address of the same wallet in Sparrow; friglet compares them again when it starts.
4. **Start friglet.** The **Scan** health check shows how far it has got. The **Electrum** check turns green once friglet has verified the wallet and opened the Electrum port.

## Connecting Sparrow

Run a separate Sparrow instance, with its own data directory, for this wallet. friglet only knows this one wallet, so any other wallet in a Sparrow that points at friglet shows no history. Switch that Sparrow back to your old server to see the others again.

1. On the **Electrum (SSL)** interface, copy an address. Take the port from the address; StartOS assigned it at install.
2. In Sparrow, go to **File → Preferences → Server**, choose **Private Electrum**, enter the host and port, and turn **Use SSL** on.
3. Optionally put your server's root CA (`http://<your-server>/static/local-root-ca.crt`) in the certificate field, so a swapped certificate is caught.
4. Click **Test Connection**.

Sparrow shows payments up to the block friglet has scanned so far; more appear as the scan continues.

## Actions

- **Set up wallet / Change wallet** — tells friglet which wallet to scan for. Once a wallet is set up, the action is called **Change wallet**: it replaces the wallet and friglet scans again from the new birthday. Set the same birthday in Sparrow.
- **Show web UI password** — shows the login for the web UI.
- **Reset web UI password** — replaces the web UI password with a new random one.

## Limitations

- One wallet per installation.
- The Electrum port answers for this wallet only and does not serve ordinary address lookups.
- Backups of this service contain the wallet's scan key. They are encrypted like every StartOS backup.
