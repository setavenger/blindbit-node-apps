#!/usr/bin/env bash
# Package the exported binaries (scripts/build-binaries.sh) as native tarballs:
#
#   dist/friglet-node-<version>-linux-<amd64|arm64>.tar.gz
#   dist/SHA256SUMS
#
# Each tarball holds one directory friglet-node-<version>-linux-<arch>/ with friglet,
# friglet-ui, LICENSE.friglet (blindbit-rs), LICENSE (this repository, when present) and
# BUILDINFO. The archives are reproducible: sorted, numeric root ownership, mtime from the
# last commit (SOURCE_DATE_EPOCH), gzip without a timestamp.
set -euo pipefail
LOG_TAG=dist
# shellcheck source=scripts/lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

OUT=${OUT:-$ROOT/build/out}
DIST=${DIST:-$ROOT/dist}
[ -f "$OUT/BUILDINFO" ] || die "no binaries in $OUT (run scripts/build-binaries.sh)"
version=$(sed -n 's/^version=//p' "$OUT/BUILDINFO")
[ -n "$version" ] || die "BUILDINFO has no version"
SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git -C "$ROOT" log -1 --format=%ct 2>/dev/null || echo 0)}

rm -rf "$DIST" && mkdir -p "$DIST"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
for a in amd64 arm64; do
  name="friglet-node-$version-linux-$a"
  d="$stage/$name"
  mkdir -m 0755 "$d"
  install -m 0755 "$OUT/bin/linux-$a/friglet" "$OUT/bin/linux-$a/friglet-ui" "$d/"
  install -m 0644 "$OUT/bin/linux-$a/LICENSE.friglet" "$OUT/BUILDINFO" "$d/"
  if [ -f "$ROOT/LICENSE" ]; then install -m 0644 "$ROOT/LICENSE" "$d/LICENSE"; fi
  tar -C "$stage" --sort=name --owner=0 --group=0 --numeric-owner --format=gnu \
    --mtime="@$SOURCE_DATE_EPOCH" -cf - "$name" | gzip -9n > "$DIST/$name.tar.gz"
  log "wrote dist/$name.tar.gz"
done
(cd "$DIST" && sha256sum -- *.tar.gz > SHA256SUMS)
cat "$DIST/SHA256SUMS"
