# nixblitz_official_plugins

Vetted plugins for [NixBlitz](https://github.com/raspiblitz/nixblitz)'s
plugin system. Install any of them by appending the plugin name to
the repo URL:

```bash
nixblitz plugin add \
  github:raspiblitz/nixblitz_official_plugins/<name>
```

(If a GitHub mirror lands, `github:fusion44/nixblitz_official_plugins/<name>`
will work the same way.)

See `docs/decisions/plugins.md` in the NixBlitz repo for the plugin
system design (D1-D17).

## Plugins

| Name | Description |
| --- | --- |
| [bitcoind](bitcoind/) | Bitcoin Core full node via nix-bitcoin; Tor-enabled, pruned or full — the base the Lightning / app plugins build on |
| [blitz-api](blitz-api/) | FastAPI backend for the Blitz web frontend; ships a Python SSE → tile-event streamer that drives the dashboard's bitcoin / lightning tiles |
| [blitz-web](blitz-web/) | RaspiBlitz web UI (single-page app) served over nginx; pairs with blitz-api |
| [cachepop](cachepop/) | Pre-builds the node's own targets and pushes them to an Attic binary cache; thermal-guarded and scheduled |
| [cln](cln/) | Core Lightning (CLN) node via nix-bitcoin, with a Tor hidden service; requires bitcoind |
| [electrs](electrs/) | Electrum-compatible Rust server backed by the local bitcoind |
| [lnbits](lnbits/) | LNBits wallet & accounts, with optional auto-wired LND backend |
| [lnd](lnd/) | Lightning Network Daemon (LND) via nix-bitcoin, with a Tor hidden service; requires bitcoind |
| [netbird](netbird/) | Enable NetBird mesh VPN on a node (Tailscale alternative) |
| [tailscale](tailscale/) | Enable Tailscale on a node |
