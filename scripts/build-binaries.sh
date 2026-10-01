#!/usr/bin/env bash
# Build friglet and friglet-ui for linux amd64 and arm64 (Dockerfile target `binaries`) and
# export them to OUT (default build/out):
#
#   OUT/bin/linux-{amd64,arm64}/{friglet,friglet-ui,LICENSE.friglet}
#   OUT/image/...   runtime overlay (tini, passwd/group, CA bundle) used by build-image.sh
#   OUT/BUILDINFO   version, friglet commit, build kind (release|dev)
#
# Environment:
#   BLINDBIT_RS_SRC=<path>  build friglet from this local blindbit-rs checkout (tracked and
#                           untracked, non-ignored files) instead of BLINDBIT_RS_REV; the build
#                           is marked dev. Release CI refuses such builds.
#   CARGO_BUILD_JOBS=<n>    parallel rustc jobs inside the builder (empty: all cores)
#   OUT=<dir>               export directory
#   EXTRA_BUILD_ARGS="..."  extra `docker build` arguments (BuildKit only), e.g. CI cache flags
set -euo pipefail
LOG_TAG=binaries
# shellcheck source=scripts/lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
load_pins

OUT=${OUT:-$ROOT/build/out}
BUILDER_TAG=${BUILDER_TAG:-friglet-node-build:binaries}
args=(--target binaries --build-arg "CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-}")

src_ctx=""
if [ -n "${BLINDBIT_RS_SRC:-}" ]; then
  [ -f "$BLINDBIT_RS_SRC/Cargo.toml" ] || die "BLINDBIT_RS_SRC=$BLINDBIT_RS_SRC is not a blindbit-rs checkout"
  head=$(git -C "$BLINDBIT_RS_SRC" rev-parse HEAD)
  dirty=$(git -C "$BLINDBIT_RS_SRC" status --porcelain --untracked-files=no | head -c1)
  dev_version="dev-${head:0:7}${dirty:+-dirty}"
  src_ctx=$ROOT/build/blindbit-rs-src
  log "staging local blindbit-rs $BLINDBIT_RS_SRC ($dev_version) into $src_ctx"
  rm -rf "$src_ctx" && mkdir -p "$src_ctx"
  git -C "$BLINDBIT_RS_SRC" ls-files -z --cached --others --exclude-standard \
    | (cd "$BLINDBIT_RS_SRC" && tar --null -T - -cf -) | tar -xf - -C "$src_ctx"
  args+=(--build-arg "FRIGLET_NODE_VERSION=$dev_version" --build-arg "BLINDBIT_RS_REV=$head")
fi

rm -rf "$OUT" && mkdir -p "$OUT"
if have_buildkit; then
  log "BuildKit build of target binaries -> $OUT"
  [ -n "$src_ctx" ] && args+=(--build-context "blindbit-rs-src=$src_ctx")
  read -r -a extra <<<"${EXTRA_BUILD_ARGS:-}"
  docker build "${args[@]}" "${extra[@]}" --output "type=local,dest=$OUT" "$ROOT"
else
  # Legacy builder: build on the host platform, keep the tag as the layer cache, copy out.
  log "legacy build of target binaries ($BUILDER_TAG) -> $OUT"
  args+=(--build-arg "BUILDPLATFORM=linux/$(host_arch)")
  if [ -n "$src_ctx" ]; then
    tar -C "$src_ctx" -cf - . | docker import --platform "linux/$(host_arch)" - friglet-node-build:blindbit-rs-src >/dev/null
    args+=(--build-arg BLINDBIT_RS_SRC_IMAGE=friglet-node-build:blindbit-rs-src)
  fi
  docker build "${args[@]}" -t "$BUILDER_TAG" "$ROOT"
  cid=$(docker create "$BUILDER_TAG" /none)
  trap 'docker rm -f "$cid" >/dev/null 2>&1 || true' EXIT
  # Named paths only: a container's root also holds /etc/hosts, /dev, /.dockerenv, ...
  for p in bin image BUILDINFO; do docker cp "$cid:/$p" "$OUT/$p"; done
fi

for a in amd64 arm64; do
  for f in friglet friglet-ui; do
    [ -x "$OUT/bin/linux-$a/$f" ] || die "missing $OUT/bin/linux-$a/$f"
  done
done
log "built: $(tr '\n' ' ' < "$OUT/BUILDINFO")"
