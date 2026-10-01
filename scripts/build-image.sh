#!/usr/bin/env bash
# Build the friglet-node runtime image for each given architecture (default: amd64 arm64)
# and tag it friglet-node:dev-<arch> (TAG_PREFIX changes the prefix). The image for the host
# architecture is also tagged friglet-node:dev.
#
#   scripts/build-image.sh [amd64] [arm64]
#
# With BuildKit this is one `docker build --platform linux/<arch>` per arch. The legacy
# builder cannot COPY --from a stage built for another platform, so there the binaries are
# built once (scripts/build-binaries.sh) and the Dockerfile's runtime stage is built from
# them; nothing is executed for the target architecture in either path.
# EXTRA_BUILD_ARGS adds `docker build` arguments on the BuildKit path (e.g. CI cache flags).
set -euo pipefail
LOG_TAG=image
# shellcheck source=scripts/lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
load_pins

TAG_PREFIX=${TAG_PREFIX:-friglet-node:dev}
arches=("$@")
[ ${#arches[@]} -gt 0 ] || arches=(amd64 arm64)
for a in "${arches[@]}"; do
  case "$a" in amd64|arm64) ;; *) die "unsupported architecture $a" ;; esac
done
vcs_ref=$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || echo unknown)
common=(--build-arg "VCS_REF=$vcs_ref" --build-arg "CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-}")

tag_host() {
  local a=$1
  if [ "$a" = "$(host_arch)" ]; then docker tag "$TAG_PREFIX-$a" "$TAG_PREFIX"; fi
}

if have_buildkit && [ -z "${BLINDBIT_RS_SRC:-}" ]; then
  read -r -a extra <<<"${EXTRA_BUILD_ARGS:-}"
  for a in "${arches[@]}"; do
    log "BuildKit build linux/$a -> $TAG_PREFIX-$a"
    docker build --platform "linux/$a" --target runtime "${common[@]}" "${extra[@]}" --load -t "$TAG_PREFIX-$a" "$ROOT"
    tag_host "$a"
  done
  exit 0
fi

# Two-step path (legacy builder, or BuildKit with a local blindbit-rs source).
OUT=${OUT:-$ROOT/build/out}
if [ "${SKIP_BINARIES:-0}" != 1 ]; then
  OUT=$OUT "$ROOT/scripts/build-binaries.sh"
fi
[ -f "$OUT/BUILDINFO" ] || die "no binaries in $OUT"
version=$(sed -n 's/^version=//p' "$OUT/BUILDINFO")
friglet_rev=$(sed -n 's/^friglet_rev=//p' "$OUT/BUILDINFO")

# Per architecture: a context holding the runtime overlay (fs/, data/) and a Dockerfile made of
# the global ARGs, a `rootfs` stage that is that context, and the runtime stage verbatim.
for a in "${arches[@]}"; do
  ctx=$ROOT/build/image-ctx/$a
  rm -rf "$ctx" && mkdir -p "$ctx/fs/usr/local/bin" "$ctx/data"
  cp -a "$OUT/image/common/etc" "$ctx/fs/etc"
  install -m 0755 "$OUT/image/linux-$a/tini" "$OUT/bin/linux-$a/friglet" "$OUT/bin/linux-$a/friglet-ui" "$ctx/fs/usr/local/bin/"
  # COPY keeps modes: no group/other write bits from the host umask (/usr/local/bin, /etc).
  chmod -R u+rwX,go-w,go+rX "$ctx"
  {
    awk '/^FROM /{exit} {print}' "$ROOT/Dockerfile"
    printf 'FROM scratch AS rootfs\nCOPY . /\n'
    awk '/^# --- runtime stage/{f=1} f' "$ROOT/Dockerfile"
  } > "$ctx/Dockerfile"
  grep -q '^FROM .* AS runtime$' "$ctx/Dockerfile" || die "Dockerfile has no runtime stage after '# --- runtime stage'"
  printf 'Dockerfile\n.dockerignore\n' > "$ctx/.dockerignore"
  log "runtime stage linux/$a from $OUT -> $TAG_PREFIX-$a"
  docker build --platform "linux/$a" "${common[@]}" \
    --build-arg "FRIGLET_NODE_VERSION=$version" --build-arg "BLINDBIT_RS_REV=$friglet_rev" \
    -t "$TAG_PREFIX-$a" "$ctx"
  tag_host "$a"
done
