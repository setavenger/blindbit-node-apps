#!/usr/bin/env bash
# Build the StartOS package (startos/) for x86_64 and aarch64 with the pinned start-cli and
# inspect the manifests. Packs the local friglet-node images (no registry, no buildx, no QEMU):
# friglet-node:dev-amd64/-arm64 are tagged with StartOS's arch names first.
#
#   scripts/ci/startos-pack.sh    needs node >= 22, npm, tar2sqfs (squashfs-tools-ng), jq
set -euo pipefail
LOG_TAG=startos
# shellcheck source=scripts/lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"
load_pins

[ -f "$ROOT/startos/Makefile" ] || { log "no startos/ package; nothing to build"; exit 0; }
if ! command -v start-cli >/dev/null 2>&1 || ! start-cli --version | grep -q "$START_CLI_VERSION"; then
  case "$(uname -m)" in x86_64) sha=$START_CLI_SHA256_X86_64 ;; aarch64) sha=$START_CLI_SHA256_AARCH64 ;; *) die "no start-cli for $(uname -m)" ;; esac
  bin=${START_CLI_DIR:-$ROOT/build/tools}/start-cli
  mkdir -p "$(dirname "$bin")"
  curl -fsSLo "$bin" "https://github.com/Start9Labs/start-technologies/releases/download/start-cli%2Fv${START_CLI_VERSION}/start-cli_$(uname -m)-linux"
  echo "$sha  $bin" | sha256sum -c - >/dev/null
  chmod 0755 "$bin"
  PATH="$(dirname "$bin"):$PATH"
  export PATH
fi
log "$(start-cli --version)"

for pair in amd64:x86_64 arm64:aarch64; do
  docker image inspect "friglet-node:dev-${pair%%:*}" >/dev/null 2>&1 \
    || die "image friglet-node:dev-${pair%%:*} not found (make image)"
  docker tag "friglet-node:dev-${pair%%:*}" "friglet-node:dev-${pair##*:}"
done

cd "$ROOT/startos"
if [ ! -f .startos/build.key.pem ]; then
  [ -f ~/.startos/id.key.pem ] || start-cli init-key
  mkdir -p .startos && cp ~/.startos/id.key.pem .startos/build.key.pem
fi
npm ci --no-audit --no-fund
make dev
for f in ./*.s9pk; do
  log "$f: $(sha256sum "$f" | cut -c1-16)…"
  start-cli s9pk inspect "$f" manifest | jq -c '{id, version, osVersion, sdkVersion, images: (.images | keys)}'
done
