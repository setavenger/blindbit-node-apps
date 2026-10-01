#!/usr/bin/env bash
#
# Builds blindbit-friglet.patch: one commit on top of mynodebtc/mynode at MYNODE_REV (pin.env)
# that adds the app folder and its line in MARKETPLACE_DOCKER_IMAGES, which is what an upstream
# pull request contains (doc/applications.md, "Publishing an Application").
#
# Usage: mynode/upstream-pr/make-patch.sh [<mynode checkout that has MYNODE_REV>]
#   Without an argument it clones MYNODE_REPO. The patch is reproducible: author and date come
#   from this repository's last commit that touched the app folder.

set -euo pipefail

APP=blindbit-friglet
HERE=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(git -C "$HERE" rev-parse --show-toplevel)
# shellcheck source-path=SCRIPTDIR source=pin.env
source "$HERE/pin.env"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

if [ $# -ge 1 ]; then
    git clone -q --shared "$1" "$TMP/mynode"
else
    git clone -q "$MYNODE_REPO" "$TMP/mynode"
fi
git -C "$TMP/mynode" checkout -q -b "add-$APP" "$MYNODE_REV"

cp -r "$REPO_ROOT/mynode/mynode_apps/$APP" "$TMP/mynode/rootfs/standard/usr/share/mynode_apps/$APP"

# Register the image with the pin script, keeping MARKETPLACE_DOCKER_IMAGES sorted
HASHES="$TMP/mynode/scripts/print_app_download_hashes.sh"
LINE="$APP IMAGE_DIGEST ghcr.io/setavenger/friglet-node {VERSION}"
grep -qx 'MARKETPLACE_DOCKER_IMAGES="' "$HASHES"
awk -v line="$LINE" '
    $0 == "MARKETPLACE_DOCKER_IMAGES=\"" { print; inside = 1; next }
    inside && !done && ($0 == "\"" || $1 > line) { print line; done = 1 }
    inside && $0 == "\"" { inside = 0 }
    { print }
' "$HASHES" > "$TMP/hashes" && cat "$TMP/hashes" > "$HASHES"
grep -qx "$LINE" "$HASHES"

AUTHOR_NAME=$(git -C "$REPO_ROOT" config user.name)
AUTHOR_EMAIL=$(git -C "$REPO_ROOT" config user.email)
DATE=$(git -C "$REPO_ROOT" log -1 --format=%cI -- "mynode/mynode_apps/$APP")
DATE=${DATE:-$(git -C "$REPO_ROOT" log -1 --format=%cI)}

git -C "$TMP/mynode" add -A
GIT_AUTHOR_NAME="$AUTHOR_NAME" GIT_AUTHOR_EMAIL="$AUTHOR_EMAIL" GIT_AUTHOR_DATE="$DATE" \
GIT_COMMITTER_NAME="$AUTHOR_NAME" GIT_COMMITTER_EMAIL="$AUTHOR_EMAIL" GIT_COMMITTER_DATE="$DATE" \
    git -C "$TMP/mynode" commit -q -F - <<EOF
Add Friglet, a Silent Payments Electrum server (BlindBit)

Friglet scans this node's blocks for one Silent Payments (BIP 352) wallet and
serves its history to Sparrow over Electrum. It runs as one Docker container
(ghcr.io/setavenger/friglet-node, amd64 + arm64) pulled by its pinned index
digest, talks to the local bitcoind over P2P only, and has a web interface on
HTTPS 8098 behind nginx with a password MyNode generates and shows on the
app page.

The install refuses pruned nodes and 32-bit devices. Uninstalling deletes
/mnt/hdd/mynode/blindbit-friglet, which holds the wallet's scan key.
EOF

git -C "$TMP/mynode" format-patch -1 --stdout --no-signature HEAD > "$HERE/$APP.patch"
echo "wrote $HERE/$APP.patch ($(git -C "$TMP/mynode" diff --shortstat HEAD~1 HEAD))"
