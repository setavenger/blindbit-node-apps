# electrs

Run [electrs](https://github.com/romanz/electrs) — the Rust
Electrum server — on this NixBlitz node, backed by the host's
Bitcoin Core. Wraps the
[nix-bitcoin electrs module](https://github.com/fort-nix/nix-bitcoin/blob/master/modules/electrs.nix),
so the systemd unit, RPC auth, P2P whitelisting, hardening, and
network-aware `electrs.toml` are handled for you.

## Install

```bash
nixblitz plugin add \
  github:raspiblitz/nixblitz_official_plugins/electrs
```

Then Apply in the TUI. First rebuild downloads the electrs binary
from the nix-bitcoin pkgs set; subsequent rebuilds hit the store
cache.

## Requirements

- `features.apps.bitcoind.enable = true` — electrs has nothing to
  index without a bitcoind to talk to.
- `features.apps.bitcoind.pruned = false` — electrs **cannot**
  index a pruned chain. The upstream nix-bitcoin module asserts
  this; the rebuild fails loudly at evaluation time if pruning is
  on. Reserve ~700 GB of disk for an unpruned mainnet chain
  before flipping pruning off.
- ~60 GB free on the partition holding `db_dir` (the index ends
  up around 45 GB on mainnet, with headroom for the RocksDB
  compaction pass that runs once initial sync finishes).

## Configuration

Edit fields in **Configure → plugins → electrs**:

| Field | Default | Notes |
| --- | --- | --- |
| `address` | `127.0.0.1` | Bind address for both Electrum RPC and the Prometheus monitoring port (4224). Leave on loopback unless you've fronted electrs with nginx. |
| `port` | `50001` | Electrum RPC port. |
| `db_dir` | `/var/lib/electrs` | Where the RocksDB index lives. |
| `open_firewall` | `false` | Opens `port` in nftables for the LAN. No-op when `address = 127.0.0.1`. |

Advanced knobs (network selection, monitoring port, Tor enforce
mode, extra electrs CLI args) live in the upstream module's
defaults; edit `~/nixblitz/plugins/<dirName>/config.json` if you
need them — or wait for a manifest bump.

## Dashboard tile

Polled every 30 s. Three states:

| Badge | Trigger |
| --- | --- |
| **active** (green) | `electrs.service` is `active` AND the Electrum RPC answers `blockchain.headers.subscribe` with a height. |
| **syncing** (amber) | Service is `active` but the RPC times out — electrs binds the port immediately on startup but doesn't answer until the index is far enough along. Expect this to last most of the initial sync. |
| **down** (red) | Service is `inactive` / `failed` / `dead`. Footer names the systemd state. |

Two rows:

- `bind` — the configured `address:port`.
- `height` — the indexed tip electrs is currently serving (or
  `(syncing)` while still building).

The tile reports electrs's own state only; the existing **Bitcoin**
tile reports the chain tip, so the lag (chain tip − indexed
height) is an eye-comparison between the two.

## Actions

Available from **Configure → plugins → electrs → Actions**:

| Action | What it does |
| --- | --- |
| Restart electrs | `systemctl restart electrs`. Use after editing config or when the index is misbehaving. **Does NOT wipe the index** — see the manual reindex procedure below. |

The action dispatches `electrs-plugin-restart.service` via
`sudo systemctl start --wait`. The TUI prompts for the admin
password the first time you trigger a privileged action in a
session, then reuses the cached sudo timestamp for ~10 min (sudo
posture D18).

## Access

```
electrum --oneserver --server <node-ip>:50001:t  # plaintext, LAN
```

The `:t` suffix tells Electrum to skip TLS (electrs binds
plaintext only). For TLS or remote access, terminate at nginx —
out of plugin scope.

## Initial sync

First mainnet run takes 6–18 h depending on hardware. During
this time:

- The systemd unit is `active` (it's working, not crashed).
- The Electrum RPC port is bound but doesn't answer queries —
  the tile shows **syncing**.
- Disk usage briefly peaks ~20 % above the final 45 GB during
  the trailing RocksDB compaction.

There's no progress endpoint upstream; `journalctl -u electrs -f`
is the only feedback channel.

## Manual reindex

If the index gets corrupted (rare; usually a power-loss artefact)
or you upgrade across an electrs major version that requires a
reindex, do this from a root shell:

```bash
systemctl stop electrs
rm -rf /var/lib/electrs/<network>   # e.g. /var/lib/electrs/mainnet
systemctl start electrs
```

The plugin deliberately doesn't ship a one-keystroke "reset DB"
action — multi-hour reindex is too consequential to risk a
stray Enter.
