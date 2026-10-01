# blindbit-node-apps

Run [BlindBit](https://github.com/setavenger/blindbit-rs)'s **friglet** on a Bitcoin node box.
friglet scans for one Silent Payments (BIP-352) wallet using your own node and serves it to
Sparrow as an Electrum server. This repository packages it, together with a small web page
(`friglet-ui`) for setup and connection details, for every supported platform.

> **Status: pre-release.** Packages are built and checked with each platform's own tooling.
> They are not yet offered for general use: see the release gate in `docs/plan.md`.

## For node operators

| Platform | How | Guide |
|---|---|---|
| Umbrel | Community app store: add this repository's URL in App Store → Community App Stores | `docs/platforms/umbrel.md` |
| StartOS | Sideload the `.s9pk` | `docs/platforms/startos.md` |
| RaspiBlitz | Bonus app (`bonus.friglet.sh`), native install | `docs/platforms/raspiblitz.md` |
| myNode | App (pending upstream PR) | `docs/platforms/mynode.md` |
| Any Docker host | `compose/` | `compose/README.md` |

Before you install:

- **Use a dedicated Sparrow instance.** friglet serves one wallet. Pointing your everyday
  Sparrow at it makes every other wallet look empty.
- **Your node must be unpruned and synced.** friglet fetches historical blocks from your
  node, starting at the wallet birthday.
- **Tweak data comes from a BlindBit oracle**, by default the public `oracle.setor.dev`. It
  sees your box's IP and which block ranges you scan, never your keys.
- **The Electrum port reveals this wallet's history to anyone who can reach it.** Keep it on
  your LAN or Tor.

## For contributors

- `docs/plan.md`: the plan, reviewed with gstack `/autoplan`, and its decision record.
- `docs/contract.md`: the interface between `friglet-ui`, the `friglet-node` image, the native
  tarball and every platform package. `docs/api.md`: the friglet-ui JSON API.
- `CONTRIBUTING.md`: build, test, `make dev-regtest`, pin updates, releases.
- `versions.env`: every pinned version, including the blindbit-rs commit friglet is built from.
- `upstream/`: reference material pulled verbatim from each platform provider, with
  `SOURCES.md` (URL, commit, license) and our notes per platform. Read-only.

## Layout

```
friglet-ui/            setup + status + connection web UI and supervisor (Rust)
Dockerfile, Makefile   friglet-node image, binaries, tarballs
compose/  e2e/         generic Docker Compose package; regtest end-to-end harness
umbrel-app-store.yml   Umbrel community store (must live at the repo root)
blindbit-friglet/      Umbrel app (must live at the repo root)
startos/               StartOS package
raspiblitz/            RaspiBlitz bonus script and upstream PR patches
mynode/                myNode app and upstream PR patches
upstream/              pulled platform references (read-only)
docs/                  plan, contract, API, platform guides
```

## License

MIT for this repository's own files (pending, see `docs/plan.md` open items). Material under
`upstream/` keeps its original license, recorded per file set in each `SOURCES.md`.
