#!/usr/bin/env bash
# Rewrite every reference derived from versions.env, or check that none has drifted.
#
#   scripts/update-pins.sh           rewrite the derived references in place
#   scripts/update-pins.sh --check   change nothing; print a diff and exit 1 on drift
#
# Derived references (add a rule here when a new file copies a pin):
#   Dockerfile                 ARG defaults between "--- pins" and "--- end of pins"
#   Cargo.toml, */Cargo.toml   friglet-ipc = { git = BLINDBIT_RS_REPO, rev = BLINDBIT_RS_REV }
#   Cargo.lock                 friglet-ipc source; rewritten with `cargo update` in RUST_IMAGE
#   (rust-toolchain.toml is deliberately not pinned: local dev tracks stable; reproducible
#    builds use RUST_IMAGE by digest)
#   any other tracked file     IMAGE_REPO:<version>[@sha256:<IMAGE_DIGEST>] image references,
#   (outside upstream/ and test fixtures) and FRIGLET_NODE_TAG / FRIGLET_NODE_VERSION = '<version>' (or "")
#                              assignments, e.g. platform manifests and compose files
#   */umbrel-app.yml           version: "<version>"
set -euo pipefail
LOG_TAG=pins
# shellcheck source=scripts/lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

mode="write"
case "${1:-}" in
  --check) mode=check ;;
  '') ;;
  -h|--help) sed -n '2,15p' "$0"; exit 0 ;;
  *) die "unknown argument: $1 (use --check)" ;;
esac

# Pins come from the file only: the environment must not mask drift.
unset_pins() {
  local key
  while IFS= read -r key; do unset "$key"; done < <(grep -oE '^[A-Z][A-Z0-9_]*' "$ROOT/versions.env")
}
unset_pins
load_pins

FRIGLET_NODE_VERSION=$(version)
export FRIGLET_NODE_VERSION
[[ $BLINDBIT_RS_REV =~ ^[0-9a-f]{40}$ ]] || die "BLINDBIT_RS_REV must be a full 40-hex commit"
[[ $PACKAGE_REVISION =~ ^[0-9]+$ ]] || die "PACKAGE_REVISION must be a number"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
drift=0

# apply FILE RENDERED: compare a rendered copy with FILE; check mode reports, write mode replaces.
apply() {
  local file=$1 rendered=$2 rel=${1#"$ROOT"/}
  if cmp -s "$file" "$rendered"; then
    return 0
  fi
  if [ "$mode" = check ]; then
    log "drift in $rel (run scripts/update-pins.sh):"
    diff -u --label "$rel" --label "$rel (from versions.env)" "$file" "$rendered" >&2 || true
    drift=1
  else
    cat "$rendered" > "$file"
    log "updated $rel"
  fi
}

# --- Dockerfile ARG defaults -----------------------------------------------------------------
render_dockerfile() {
  awk '
    /^# --- pins/        { inpins = 1; print; next }
    /^# --- end of pins/ { inpins = 0; print; next }
    inpins && /^ARG [A-Z][A-Z0-9_]*=/ {
      line = $0; sub(/^ARG /, "", line); key = line; sub(/=.*/, "", key)
      if (!(key in ENVIRON)) { printf "Dockerfile pins block: %s is not in versions.env\n", key > "/dev/stderr"; bad = 1; print; next }
      print "ARG " key "=" ENVIRON[key]; next
    }
    { print }
    END { exit bad }
  ' "$1"
}
if [ -f "$ROOT/Dockerfile" ]; then
  grep -q '^# --- pins' "$ROOT/Dockerfile" || die "Dockerfile has no '# --- pins' block"
  render_dockerfile "$ROOT/Dockerfile" > "$tmp/Dockerfile" || die "Dockerfile pins block names a key missing from versions.env"
  apply "$ROOT/Dockerfile" "$tmp/Dockerfile"
fi

# --- Cargo.toml: the friglet-ipc git dependency ----------------------------------------------
render_cargo_toml() {
  # Only single-line inline tables are rewritten; the rule fails loudly on anything else.
  awk -v repo="$BLINDBIT_RS_REPO" -v rev="$BLINDBIT_RS_REV" '
    /^[[:space:]]*friglet-ipc[[:space:]]*=/ {
      if ($0 !~ /git[[:space:]]*=/ || $0 !~ /rev[[:space:]]*=/) { print "friglet-ipc dependency must be { git = ..., rev = ... } on one line" > "/dev/stderr"; bad = 1 }
      sub(/git[[:space:]]*=[[:space:]]*"[^"]*"/, "git = \"" repo "\"")
      sub(/rev[[:space:]]*=[[:space:]]*"[^"]*"/, "rev = \"" rev "\"")
    }
    { print }
    END { exit bad }
  ' "$1"
}
cargo_tomls=()
while IFS= read -r f; do cargo_tomls+=("$f"); done < <(
  find "$ROOT" -name Cargo.toml -not -path "$ROOT/upstream/*" -not -path '*/target/*' -not -path "$ROOT/build/*" | sort)
lock_needs_update=0
for f in "${cargo_tomls[@]}"; do
  render_cargo_toml "$f" > "$tmp/Cargo.toml" || die "cannot rewrite ${f#"$ROOT"/}"
  if ! cmp -s "$f" "$tmp/Cargo.toml"; then lock_needs_update=1; fi
  apply "$f" "$tmp/Cargo.toml"
done

# --- Cargo.lock: friglet-ipc must resolve to the pinned commit -------------------------------
if [ -f "$ROOT/Cargo.lock" ] && grep -q '^name = "friglet-ipc"' "$ROOT/Cargo.lock"; then
  want="source = \"git+${BLINDBIT_RS_REPO}?rev=${BLINDBIT_RS_REV}#${BLINDBIT_RS_REV}\""
  got=$(awk '/^name = "friglet-ipc"$/ { f = 1; next } f && /^source = / { print; exit } /^\[\[package\]\]/ { f = 0 }' "$ROOT/Cargo.lock")
  if [ "$got" != "$want" ] || [ "$lock_needs_update" = 1 ]; then
    if [ "$mode" = check ]; then
      log "drift in Cargo.lock: friglet-ipc has '$got', want '$want'"
      drift=1
    else
      log "updating Cargo.lock (cargo update -p friglet-ipc in $RUST_IMAGE)"
      docker run --rm -u "$(id -u):$(id -g)" -e CARGO_HOME=/tmp/cargo -v "$ROOT:/w" -w /w \
        "$RUST_IMAGE" cargo update -p friglet-ipc
    fi
  fi
fi

# --- image references and version assignments in every other tracked text file -------------
[[ $IMAGE_DIGEST =~ ^sha256:[0-9a-f]{64}$ ]] || die "IMAGE_DIGEST must be sha256:<64 hex>"
repo_re=$(printf '%s' "$IMAGE_REPO" | sed 's/[.[\*^$/]/\\&/g')
ver_re='[0-9a-f]{7}-[0-9]+'
render_refs() {
  local extra=()
  [ "$(basename "$1")" = umbrel-app.yml ] && extra=(-e "s#^(version:[[:space:]]*[\"']?)${ver_re}([\"']?[[:space:]]*)\$#\1${FRIGLET_NODE_VERSION}\2#")
  sed -E "${extra[@]}" \
    -e "s#(${repo_re}):${ver_re}@sha256:[0-9a-f]{64}#\1:${FRIGLET_NODE_VERSION}@${IMAGE_DIGEST}#g" \
    -e "s#(${repo_re}):${ver_re}([^@0-9A-Za-z.-]|\$)#\1:${FRIGLET_NODE_VERSION}\2#g" \
    -e "s#(FRIGLET_NODE_(TAG|VERSION)[[:space:]]*[:=][[:space:]]*['\"])${ver_re}(['\"])#\1${FRIGLET_NODE_VERSION}\3#g" \
    "$1"
}
while IFS= read -r rel; do
  f=$ROOT/$rel
  case "$rel" in Dockerfile|versions.env|Cargo.lock|*/Cargo.lock) continue ;; esac
  if [ ! -f "$f" ] || ! grep -Iq . "$f"; then continue; fi
  grep -Eq "${repo_re}:${ver_re}|FRIGLET_NODE_(TAG|VERSION)[[:space:]]*[:=][[:space:]]*['\"]${ver_re}|^version:[[:space:]]*[\"']?${ver_re}" "$f" || continue
  render_refs "$f" > "$tmp/refs"
  apply "$f" "$tmp/refs"
done < <(git -C "$ROOT" ls-files --cached --others --exclude-standard -- ':!:upstream/**' ':!:build/**' ':!:dist/**' ':!:*/tests/**' ':!:*/test/**' | grep -v '/node_modules/' | sort -u)

if [ "$drift" = 1 ]; then
  die "derived references differ from versions.env"
fi
if [ "$mode" = check ]; then
  log "all derived references match versions.env (version $FRIGLET_NODE_VERSION)"
fi
