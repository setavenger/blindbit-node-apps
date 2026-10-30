#!/usr/bin/env bash
# Check the exported binaries (scripts/build-binaries.sh):
#   - ELF machine matches the directory (x86-64 / AArch64)
#   - the highest GLIBC_x.y symbol version they need is <= GLIBC_VERSION (versions.env)
#   - they execute on Debian bookworm (RUNTIME_IMAGE): friglet --help, friglet-ui --version,
#     friglet-ui self-test. arm64 runs only where the host can execute arm64 containers
#     (arm64 host or QEMU binfmt); REQUIRE_ARM64_EXEC=1 turns a skip into a failure (CI).
#
#   OUT=build/out scripts/check-binaries.sh
set -euo pipefail
LOG_TAG=check-binaries
# shellcheck source=scripts/lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
load_pins

OUT=${OUT:-$ROOT/build/out}
[ -d "$OUT/bin" ] || die "no binaries in $OUT"

readelf_() {
  if command -v readelf >/dev/null 2>&1; then
    readelf "$@"
  else
    docker run --rm -v "$OUT:$OUT:ro" "$RUST_IMAGE" readelf "$@"
  fi
}

# version_le A B: A <= B for dotted versions.
version_le() { [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -n1)" = "$1" ]; }

# can_run ARCH: this host runs linux/ARCH containers; when not, $why holds docker's answer.
# The ARCH variant is pulled first: with Docker's classic image store (CI), the amd64 copy
# already pulled under the same digest makes `docker run --platform linux/arm64` fail instead
# of pulling.
can_run() {
  why=$(docker pull -q --platform "linux/$1" "$RUNTIME_IMAGE" 2>&1) &&
    why=$(docker run --rm --platform "linux/$1" "$RUNTIME_IMAGE" true 2>&1)
}

rc=0
for a in amd64 arm64; do
  case "$a" in amd64) want_machine='X86-64' ;; arm64) want_machine='AArch64' ;; esac
  for f in friglet friglet-ui; do
    bin=$OUT/bin/linux-$a/$f
    [ -x "$bin" ] || die "missing $bin"
    machine=$(readelf_ -h "$bin" | sed -n 's/^ *Machine: *//p')
    glibc=$(readelf_ -V --wide "$bin" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/GLIBC_//' | sort -uV | tail -n1)
    status=ok
    [[ $machine == *"$want_machine"* ]] || status="WRONG MACHINE"
    version_le "$glibc" "$GLIBC_VERSION" || status="NEEDS GLIBC $glibc > $GLIBC_VERSION"
    [ "$status" = ok ] || rc=1
    printf '%-6s %-11s machine=%-38s max GLIBC_%-5s %s\n' "$a" "$f" "$machine" "$glibc" "$status"
  done
  if can_run "$a"; then
    for cmd in "friglet --help" "friglet-ui --version" "friglet-ui self-test"; do
      read -r f arg <<<"$cmd"
      if out=$(docker run --rm --platform "linux/$a" -v "$OUT/bin/linux-$a:/b:ro" "$RUNTIME_IMAGE" "/b/$f" "$arg" 2>&1); then
        printf '%-6s exec ok: %s -> %s\n' "$a" "$cmd" "$(head -n1 <<<"$out")"
      else
        printf '%-6s exec FAILED: %s\n%s\n' "$a" "$cmd" "$out"
        rc=1
      fi
    done
  elif [ "${REQUIRE_ARM64_EXEC:-0}" = 1 ]; then
    log "cannot run linux/$a containers here (no QEMU binfmt?): $why"
    rc=1
  else
    log "skipping execution of linux/$a (this host cannot run it; CI runs it under QEMU)"
  fi
done
[ $rc = 0 ] || die "binary checks failed"
log "binaries ok (glibc <= $GLIBC_VERSION)"
