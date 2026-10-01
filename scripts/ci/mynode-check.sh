#!/usr/bin/env bash
# Check the myNode app with myNode's own tooling at MYNODE_REV.
#   mynode/tests/check.sh exists: run it against a fresh mynode checkout (it copies the app).
#   otherwise: app_sdk/create.py check for every mynode/mynode_apps/<app>.
set -euo pipefail
LOG_TAG=mynode-check
# shellcheck source=scripts/lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"
load_pins

[ -d "$ROOT/mynode" ] || { log "no mynode/ folder; nothing to check"; exit 0; }
work=${MYNODE_WORK:-$(mktemp -d)}
if [ ! -d "$work/mynode/.git" ]; then
  git init -q "$work/mynode"
  git -C "$work/mynode" fetch -q --depth 1 "$MYNODE_REPO" "$MYNODE_REV"
  git -C "$work/mynode" checkout -q --detach FETCH_HEAD
fi
log "mynode ${MYNODE_REV:0:7}"
if [ -x "$ROOT/mynode/tests/check.sh" ]; then
  "$ROOT/mynode/tests/check.sh" "$work/mynode"
  exit 0
fi
for d in "$ROOT"/mynode/mynode_apps/*/; do
  app=$(basename "$d")
  rm -rf "$work/mynode/rootfs/standard/usr/share/mynode_apps/$app"
  cp -r "$d" "$work/mynode/rootfs/standard/usr/share/mynode_apps/$app"
  python3 "$work/mynode/app_sdk/create.py" check "$app"
done
