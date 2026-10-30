# friglet-node with Docker Compose

Runs **friglet** (a Silent Payments scanner with an Electrum server) and **friglet-ui** (its
setup and status page) on any Docker host, next to a Bitcoin node you already run.

Before you start:

- A synced, **unpruned** Bitcoin node reachable over P2P from this host. friglet downloads
  candidate blocks from it, from your wallet's birthday onward; a pruned node cannot serve them.
- Docker with the Compose plugin (`docker compose version`).
- Sparrow: run the Silent Payments wallet in a **dedicated Sparrow instance** with its own
  data directory. friglet serves only this one wallet, so pointing your main Sparrow at it
  empties every other wallet's history there.

## Quickstart

<!-- quickstart:begin (CI runs this block verbatim with EDITOR=true; keep it runnable) -->
```sh
cd compose
cp .env.example .env
${EDITOR:-nano} .env   # set FRIGLET_UI_NETWORK, FRIGLET_UI_NODE and FRIGLET_UI_PASSWORD
docker compose up -d
docker compose ps
curl -fsS --retry 30 --retry-delay 2 --retry-all-errors http://127.0.0.1:8097/healthz
```
<!-- quickstart:end -->

Outside a checkout of this repository, fetch the two files first:

```sh
mkdir friglet-node && cd friglet-node
curl -fsSLO https://raw.githubusercontent.com/setavenger/blindbit-node-apps/master/compose/docker-compose.yml
curl -fsSL -o .env.example https://raw.githubusercontent.com/setavenger/blindbit-node-apps/master/compose/.env.example
```

Then open `http://<this host>:8097`, log in with `FRIGLET_UI_PASSWORD`, paste the descriptor
Sparrow shows for the wallet (`sp(spscan1…)`) and the wallet's birthday height, and follow
the Connect page.

Headless setup instead of the web page (the descriptor is read from stdin, never argv):

```sh
docker compose exec -T friglet-node friglet-ui setup --birthday 840000 < descriptor.txt
docker compose exec friglet-node friglet-ui status --json
```

Until a release image is published, build it from this repository (`make image`) and set
`FRIGLET_NODE_IMAGE=friglet-node:dev` in `.env`.

## Configuration

Everything is in `.env` (see `.env.example`). Only `FRIGLET_UI_*` variables reach the
container; any other `FRIGLET_*` variable there is a startup error by design.

| Variable | Required | Meaning |
|---|---|---|
| `FRIGLET_UI_NETWORK` | yes | `bitcoin`, `signet`, `testnet4`, `regtest` |
| `FRIGLET_UI_NODE` | yes | node P2P `host:port`; resolved to an IP on every start. `host.docker.internal` is the Docker host |
| `FRIGLET_UI_PASSWORD` | yes | web page password, hashed into the data volume on first start |
| `FRIGLET_UI_ORACLE_URL` | no | BlindBit oracle; defaults per network (a third-party service) |
| `FRIGLET_UI_PUBLIC_HOST` | no | host name shown on the Connect page |
| `FRIGLET_UI_ELECTRUM_TCP` / `_TLS` | no | listener inside the container, or `off` |
| `FRIGLET_UI_AUTH` | no | `password` (default) or `none` behind your own authenticating proxy (warning banner) |
| `FRIGLET_NODE_IMAGE` | no | image to run (Compose only) |
| `BIND_ADDRESS`, `UI_PORT`, `ELECTRUM_PORT`, `ELECTRUM_TLS_PORT` | no | published host address and ports (Compose only) |

## Ports and data

| Host port | What |
|---|---|
| 8097 | friglet-ui web page and JSON API (password) |
| 50031 | Electrum, plaintext. Anyone who can reach it can read this wallet's history |
| 50032 | Electrum, TLS with a self-signed certificate; the Connect page shows its fingerprint |

friglet's own Electrum server (50001) stays on loopback inside the container.
All state lives in the `friglet-data` volume at `/data`: config, the scan key (mode 0600),
scanner state, TLS certificate and the UI password hash. `docker compose down` keeps it;
`docker compose down -v` deletes it, the scan key included. friglet logs contain txids,
balances and IP addresses (blindbit-rs BR8); treat `docker compose logs` output accordingly.

Stopping waits up to 150 s (`stop_grace_period`) because friglet may be blocked in a P2P read
for up to 120 s.
