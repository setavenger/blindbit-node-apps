# blindbit-node-apps: build, package and test friglet-node. Only Docker is required on the
# host; every tool runs in a digest-pinned container (versions.env). See CONTRIBUTING.md.
#
#   make binaries     friglet + friglet-ui for linux amd64/arm64 -> build/out/bin/
#   make image        friglet-node:dev-amd64 and friglet-node:dev-arm64 (no QEMU)
#   make dist         dist/friglet-node-<version>-linux-<arch>.tar.gz + dist/SHA256SUMS
#   make check-pins   fail when a reference derived from versions.env has drifted
#   make update-pins  rewrite those references from versions.env
#   make lint         shellcheck, hadolint, actionlint
#   make test         cargo fmt --check, clippy, test (in RUST_IMAGE)
#   make e2e          regtest end to end against the blindbit-testkit stack
#   make dev-regtest  regtest stack + friglet-node with a found payment; prints the UI URL
#   make clean        remove build/, dist/ and the local builder images

include versions.env

SHELL := /bin/bash
.SHELLFLAGS := -euo pipefail -c
.DEFAULT_GOAL := help

# Parallel rustc jobs inside the builder; empty means all cores.
CARGO_BUILD_JOBS ?=
export CARGO_BUILD_JOBS
# Local blindbit-rs checkout to build friglet from (dev builds); unset for the pinned commit.
BLINDBIT_RS_SRC ?=
export BLINDBIT_RS_SRC
ARCHES ?= amd64 arm64

VERSION := $(shell printf '%.7s' '$(BLINDBIT_RS_REV)')-$(PACKAGE_REVISION)

.PHONY: help binaries image dist check-pins update-pins lint test e2e e2e-friglet-only dev-regtest dev-regtest-down clean version

help:
	@sed -n 's/^#   make/  make/p' Makefile

version:
	@echo $(VERSION)

check-pins:
	scripts/update-pins.sh --check

update-pins:
	scripts/update-pins.sh

binaries: check-pins
	scripts/build-binaries.sh

image: check-pins
	scripts/build-image.sh $(ARCHES)

# Packages whatever `make binaries` exported last; build first when nothing is there.
dist: check-pins
	@[ -f build/out/BUILDINFO ] || scripts/build-binaries.sh
	scripts/dist.sh

lint:
	scripts/lint.sh

# Runs as root in the container (rustup needs it for rustfmt/clippy) and hands build/ back.
test:
	docker run --rm -e CARGO_HOME=/w/build/cargo-home -e CARGO_TARGET_DIR=/w/build/target \
	  -e CARGO_BUILD_JOBS -e RUSTUP_TOOLCHAIN=$(RUST_VERSION) -e HOST_UID="$$(id -u):$$(id -g)" \
	  -v "$(CURDIR):/w" -w /w $(RUST_IMAGE) bash -euo pipefail -c '\
	    trap "chown -R \$$HOST_UID /w/build" EXIT; \
	    [ -n "$${CARGO_BUILD_JOBS:-}" ] || unset CARGO_BUILD_JOBS; \
	    rustup component add rustfmt clippy; \
	    cargo fmt --all --check; \
	    cargo clippy --locked --all-targets -- -D warnings; \
	    cargo test --locked'

e2e:
	e2e/run.sh

e2e-friglet-only:
	e2e/run.sh --friglet-only

dev-regtest:
	e2e/dev-regtest.sh up

dev-regtest-down:
	e2e/dev-regtest.sh down

clean:
	rm -rf build dist
	-docker rmi friglet-node-build:binaries friglet-node-build:blindbit-rs-src 2>/dev/null
