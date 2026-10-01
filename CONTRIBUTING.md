# Contributing to blindbit-node-apps

This repository packages BlindBit's `friglet` daemon, plus the `friglet-ui` setup and status
page from this workspace, for node-in-a-box platforms. `docs/plan.md` is the why,
`docs/contract.md` the interface every package builds against, `AGENTS.md` the house rules.

## Bootstrap

Only Docker is required. Every other tool (Rust, zig, protoc, Go, shellcheck, hadolint,
actionlint, Compose) runs in a digest-pinned container from `versions.env`.

```sh
git clone https://github.com/setavenger/blindbit-node-apps.git
cd blindbit-node-apps
make help
```

With the buildx plugin (Docker Desktop, docker-ce) builds use BuildKit. Without it (e.g.
Ubuntu's `docker.io` package) the scripts fall back to the legacy builder automatically; see
"Images without BuildKit" below. `docker compose` is used when the plugin exists, otherwise
the pinned `docker:29-cli` image runs Compose. On a shared or small machine set
`CARGO_BUILD_JOBS=2`.

## Build

| Command | Result |
|---|---|
| `make binaries` | `build/out/bin/linux-{amd64,arm64}/{friglet,friglet-ui}` |
| `make image` | `friglet-node:dev-amd64`, `friglet-node:dev-arm64`, and `friglet-node:dev` for the host |
| `make dist` | `dist/friglet-node-<version>-linux-{amd64,arm64}.tar.gz` and `dist/SHA256SUMS` |
| `make version` | the package version, `<friglet short commit>-<PACKAGE_REVISION>` |

How the build works:

- The `Dockerfile` builder stage runs on the build platform. It fetches blindbit-rs at
  `BLINDBIT_RS_REV` and builds friglet with friglet's own `Cargo.lock` (`--locked`).
- It builds `friglet-ui` from this workspace, also `--locked`.
- Both are cross-compiled for x86_64 and aarch64 with `cargo zigbuild` against glibc 2.36,
  so they run on Debian bookworm and newer.
- The runtime stage runs no `RUN`, so an arm64 image builds on an amd64 host without QEMU.
- `scripts/check-binaries.sh` checks the ELF machine and the glibc floor. It also runs the
  binaries in a bookworm container; arm64 runs only where the host can execute it, which CI
  arranges with QEMU.

Build friglet from a local or unmerged blindbit-rs checkout:

```sh
make image BLINDBIT_RS_SRC=../blindbit-rs
```

This takes the checkout's tracked and untracked, non-ignored files and patches its
`friglet-ipc` into friglet-ui. The build is versioned `dev-<commit>[-dirty]` and marked dev in
the UI. Release builds refuse it.

### Images without BuildKit

The legacy builder cannot `COPY --from` a stage built for another platform. So
`scripts/build-image.sh` takes two steps there:

1. It builds the `binaries` target once for the host.
2. It builds the `Dockerfile` runtime stage (everything from `# --- runtime stage` to the
   end) from those binaries, once per architecture.

The resulting images are the same as BuildKit's single-pass `docker build --platform`
builds. Keep the runtime stage free of `RUN` and keep that marker line in place.

## Test

| Command | What it checks |
|---|---|
| `make check-pins` | every reference derived from `versions.env` matches it |
| `make lint` | shellcheck (all `*.sh` outside `upstream/`), hadolint, actionlint |
| `make test` | `cargo fmt --check`, `clippy -D warnings`, `cargo test --locked` in the pinned Rust image |
| `make e2e` | regtest end to end against the blindbit-testkit stack, see below |
| `scripts/quickstart-smoke.sh` | runs the `compose/README.md` quickstart verbatim against the local image |
| `scripts/ci/umbrel-lint.sh`, `mynode-check.sh`, `startos-pack.sh` | each platform's own tooling, as CI runs it |

### `make e2e`

`make e2e` (`e2e/run.sh`) does the following:

1. Clones blindbit-testkit at `BLINDBIT_TESTKIT_REV` into `build/e2e/`, never modified.
2. Starts the testkit's bitcoind (regtest) and blindbit-oracle.
3. Generates a fresh silent-payment wallet with `e2e/sptool`. The keys go in a 0600 file,
   never into logs or argv.
4. Sends one BIP-352 payment, built with go-bip352 the way the testkit's `cmd/bootstrap`
   builds them.
5. Starts `compose/docker-compose.yml` on the stack's network.
6. Sets the wallet up through `friglet-ui setup`, with the descriptor on stdin.
7. Sends a second payment.
8. Asserts that both payments are found over Electrum on port 50031: they appear in the
   silentpayments history and in the scripthash history.
9. Restarts the container and asserts the same again.

Options:

- `e2e/run.sh --friglet-only` runs the image's friglet alone, configured with a config file
  and key file, without friglet-ui.
- `--keep` leaves the stacks running and `--down` removes them.
- First runs build the testkit's bitcoind and oracle images, which takes a few minutes.
- `E2E_TESTKIT_SRC` and `E2E_ORACLE_SRC` take local clones instead of GitHub.

### `make dev-regtest`

Brings up the same regtest stack with the local image. It sets up a generated wallet,
confirms one payment, and prints the UI URL (`http://127.0.0.1:8097/`), the password file,
the Electrum endpoints and where the throwaway descriptor is.

- `e2e/dev-regtest.sh pay [sats]` sends another payment.
- `mine N` mines blocks.
- `make dev-regtest-down` removes it all.

## Pins

`versions.env` is the only place a version, commit, digest or checksum is written by hand.
To change one:

```sh
$EDITOR versions.env          # e.g. BLINDBIT_RS_REV, RUST_VERSION, an image digest
scripts/update-pins.sh        # rewrites every derived reference, incl. Cargo.lock
make check-pins               # what CI runs; prints a diff on drift
```

`scripts/update-pins.sh` maintains these derived references:

- the `Dockerfile` ARG defaults;
- the `friglet-ipc` git `rev` in every `Cargo.toml`, plus `Cargo.lock` (via `cargo update`);
- `rust-toolchain.toml`;
- any `ghcr.io/setavenger/friglet-node:<version>[@sha256:<digest>]` reference;
- any `FRIGLET_NODE_TAG` / `FRIGLET_NODE_VERSION = '<version>'` assignment;
- `version:` in `*/umbrel-app.yml`.

Platform packages should write their pins in one of those shapes so they stay in step. A new
shape needs a rule in the script.

When re-pinning friglet (`BLINDBIT_RS_REV`):

- reset `PACKAGE_REVISION` to 1;
- reset `IMAGE_DIGEST` to all zeros;
- check that `BLINDBIT_TESTKIT_REV` still pins a compatible stack.

Any other shipped packaging change bumps `PACKAGE_REVISION`.

## Release

1. On `integration`: bump `PACKAGE_REVISION` (or re-pin), run `scripts/update-pins.sh`,
   commit, and let CI pass.
2. The maintainer reviews and merges into `master`.
3. The maintainer tags `v$(make version)` on `master` and pushes the tag.
4. `.github/workflows/release.yml` then:
   - checks the tag against `versions.env` and refuses dev builds;
   - builds and checks the binaries (arm64 runs under QEMU);
   - pushes the multi-arch image `IMAGE_REPO:<version>` (GHCR placeholder);
   - attaches the tarballs and `SHA256SUMS` to a **draft** release.
5. Copy the image digest from the run summary into `IMAGE_DIGEST` in `versions.env` and run
   `scripts/update-pins.sh`. This rewrites the digest-pinned platform manifests, which you
   then commit.
6. The maintainer publishes the draft release.

Signing is not implemented yet: the key is undecided, and the workflow marks the artifacts
**unsigned** (`TODO(signing)` in `release.yml`).

## Platform packages and upstream review

Each platform folder has its own README and `docs/platforms/<platform>.md` with its install
test and submission steps. Reviewer feedback on an upstream submission is answered in the
same way everywhere:

1. Make the fix here on a branch off `integration`, with that platform's check.
2. Regenerate the submission artifact: the Umbrel app folder, `mynode/upstream-pr` patch, the
   StartOS `.s9pk`, or the RaspiBlitz bonus script.
3. Reply on the upstream PR or e-mail with the commit.

Never edit `upstream/`; re-pull it and update its `SOURCES.md` in the same commit.
