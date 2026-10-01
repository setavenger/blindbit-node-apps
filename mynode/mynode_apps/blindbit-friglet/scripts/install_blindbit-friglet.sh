#!/bin/bash

# Installs Friglet. MyNode runs this as the app's linux_user (bitcoin) from the install
# folder /opt/mynode/blindbit-friglet, which it empties first, with VERSION, INSTALL_FOLDER
# and STORAGE_FOLDER set. The storage folder survives upgrades and reinstalls.

source /usr/share/mynode/mynode_device_info.sh
source /usr/share/mynode/mynode_app_versions.sh
source /usr/share/mynode/mynode_functions.sh

set -x
set -e

echo "==================== INSTALLING APP ===================="

: "${VERSION:?VERSION (latest_version) is set by mynode-manage-apps}"

IMAGE="ghcr.io/setavenger/friglet-node"
# "<tag> <image index digest>" for latest_version, from scripts/print_app_download_hashes.sh.
# PLACEHOLDER DIGEST: the all-zero digest is not a real image. Replace it with the printed value
# once ghcr.io/setavenger/friglet-node:93f6a92-1 is published; until then the pull below fails.
IMAGE_DIGEST="93f6a92-1 sha256:0000000000000000000000000000000000000000000000000000000000000000"

STORAGE_DIR="${STORAGE_FOLDER:-/mnt/hdd/mynode/blindbit-friglet}"
BITCOIN_CONF="/mnt/hdd/mynode/bitcoin/bitcoin.conf"

# Print the mainnet prune setting of a bitcoin.conf (empty when not set)
mainnet_prune_setting() {
    awk '
        /^[[:space:]]*\[/ { section = $0; gsub(/[][[:space:]]/, "", section); next }
        {
            line = $0
            sub(/#.*/, "", line)
            gsub(/[[:space:]]/, "", line)
            if (line ~ /^prune=/ && (section == "" || section == "main")) { value = substr(line, 7) }
            if (line ~ /^main\.prune=/) { value = substr(line, 12) }
        }
        END { print value }
    ' "$1"
}

pull_image_with_retries() {
    local attempt
    for attempt in 1 2 3 4 5; do
        if pull_app_docker_image "$@"; then
            return 0
        fi
        if [ "$attempt" -lt 5 ]; then
            echo "Docker pull failed for $1 (attempt $attempt/5); retrying..."
            sleep $((attempt * 5))
        fi
    done
    echo "ERROR: Docker pull failed for $1 after 5 attempts" >&2
    return 1
}

# The image is built for 64-bit ARM and x86 only
if [ "$IS_32_BIT" = "1" ] || { [ "$DEVICE_ARCH" != "aarch64" ] && [ "$DEVICE_ARCH" != "x86_64" ]; }; then
    echo "ERROR: Friglet cannot be installed on this device." >&2
    echo "  Cause: Friglet runs on 64-bit devices (aarch64 or x86_64); this one reports $DEVICE_ARCH." >&2
    echo "  Fix: install Friglet on a 64-bit MyNode device." >&2
    exit 1
fi

# Friglet downloads every block from the wallet birthday onward from this node
if [ -r "$BITCOIN_CONF" ]; then
    PRUNE=$(mainnet_prune_setting "$BITCOIN_CONF")
    if [ -n "$PRUNE" ] && ! [[ "$PRUNE" =~ ^0+$ ]]; then
        echo "ERROR: Friglet cannot be installed: this node is pruned." >&2
        echo "  Cause: $BITCOIN_CONF sets prune=$PRUNE. Friglet downloads every block from your wallet's" >&2
        echo "  birthday onward from this node, and a pruned node has deleted old blocks." >&2
        echo "  Fix: remove the prune setting on MyNode's Bitcoin Config page (/bitcoin/config), let" >&2
        echo "  Bitcoin download the full chain again, then install Friglet again." >&2
        echo "  Details: blindbit-node-apps docs/platforms/mynode.md#pruned-node" >&2
        exit 1
    fi
else
    echo "WARNING: cannot read $BITCOIN_CONF to check for pruning; Friglet reports a pruned node once it runs."
fi

mkdir -p "$STORAGE_DIR/data"
chmod 700 "$STORAGE_DIR" "$STORAGE_DIR/data"

cp -f app_data/docker-compose.yml docker-compose.yml
/usr/local/bin/docker-compose down --remove-orphans 2>/dev/null || true

remove_docker_images_by_name "$IMAGE"
pull_image_with_retries "$IMAGE" "$VERSION" blindbit-friglet "$IMAGE_DIGEST"

# Compose runs the image by digest when this version is pinned, so it cannot drift from the pin
if [ "$VERSION" = "${IMAGE_DIGEST%% *}" ]; then
    FRIGLET_IMAGE="$IMAGE:$VERSION@${IMAGE_DIGEST##* }"
else
    FRIGLET_IMAGE="$IMAGE:$VERSION"
fi

ENV_TMP=$(mktemp .env.XXXXXX)
printf 'FRIGLET_IMAGE=%s\nFRIGLET_UID=%s\nFRIGLET_GID=%s\n' \
    "$FRIGLET_IMAGE" "$(id -u bitcoin)" "$(id -g bitcoin)" > "$ENV_TMP"
chmod 600 "$ENV_TMP"
mv -f "$ENV_TMP" .env

echo "================== DONE INSTALLING APP ================="
