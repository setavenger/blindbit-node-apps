#!/bin/bash

# Uninstalls Friglet. MyNode runs this as the app's linux_user (bitcoin) from the install
# folder, after stopping the service and before deleting /opt/mynode/blindbit-friglet.

source /usr/share/mynode/mynode_device_info.sh
source /usr/share/mynode/mynode_app_versions.sh
source /usr/share/mynode/mynode_functions.sh

set -x

echo "==================== UNINSTALLING APP ===================="

STORAGE_DIR="/mnt/hdd/mynode/blindbit-friglet"

cp -f app_data/docker-compose.yml docker-compose.yml 2>/dev/null || true
/usr/local/bin/docker-compose down --remove-orphans 2>/dev/null || true

remove_docker_images_by_name "ghcr.io/setavenger/friglet-node"

# The data folder holds secrets, so it goes with the app
echo "Deleting $STORAGE_DIR:"
echo "  - the wallet's scan key and Friglet's wallet settings (config.toml, scan.key)"
echo "  - the scan progress, its backups and any previous wallet kept for restore"
echo "  - the TLS certificate of the Electrum port and the web interface login password"
echo "Your bitcoin is not affected: the scan key can be entered again from Sparrow, and a new"
echo "install scans again from the wallet birthday. MyNode keeps the onion service keys."
rm -rf "$STORAGE_DIR"

echo "================== DONE UNINSTALLING APP ================="
