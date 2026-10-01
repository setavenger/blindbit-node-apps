#!/bin/bash

# Runs as root before every start (ExecStartPre=+ in blindbit-friglet.service), from the
# install folder. Prepares what the container needs and nothing else:
#   - the data folder friglet-ui keeps its config, scan key and scan state in
#   - the login password (saved where MyNode's app page reads it, see save_app_default_password)
#   - runtime.env: the addresses Friglet shows and accepts, which can change between starts

set -eu

source /usr/share/mynode/mynode_functions.sh

APP="blindbit-friglet"
APP_USER="bitcoin"
STORAGE_DIR="/mnt/hdd/mynode/$APP"
DATA_DIR="$STORAGE_DIR/data"
PASSWORD_FILE="$STORAGE_DIR/.app_password"
INSTALL_DIR="/opt/mynode/$APP"
RUNTIME_ENV="$INSTALL_DIR/runtime.env"
ONION_HOSTNAME_FILE="/var/lib/tor/mynode_$APP/hostname"
HTTPS_PORT=8098

# Refuse anything but a real directory or regular file at our paths: the data folder is
# written from inside the container and these scripts run as root.
require_dir() {
    if [ -L "$1" ] || { [ -e "$1" ] && [ ! -d "$1" ]; }; then
        echo "Refusing to use $1: not a directory" >&2
        exit 1
    fi
    mkdir -p "$1"
    chown "$APP_USER:$APP_USER" "$1"
    chmod 700 "$1"
}

require_regular_file_or_missing() {
    if [ -L "$1" ] || { [ -e "$1" ] && [ ! -f "$1" ]; }; then
        echo "Refusing to use $1: not a regular file" >&2
        exit 1
    fi
}

# Same checks as MyNode's own pages (device_info.py: is_community_edition, is_tor_remote_access_enabled)
is_premium() {
    [ ! -f /home/bitcoin/.mynode/.product_key_skipped ] && [ ! -f /mnt/hdd/mynode/settings/.product_key_skipped ]
}

is_tor_remote_access_enabled() {
    local f
    for f in /home/bitcoin/.mynode/tor_remote_access_disabled /mnt/hdd/mynode/settings/tor_remote_access_disabled \
             /home/bitcoin/.mynode/.tor_remote_access_disabled /mnt/hdd/mynode/settings/.tor_remote_access_disabled; do
        [ -f "$f" ] && return 1
    done
    return 0
}

require_dir "$STORAGE_DIR"
require_dir "$DATA_DIR"

# Login password for the web interface, generated once and shown on MyNode's app page
require_regular_file_or_missing "$PASSWORD_FILE"
if ! has_app_password "$APP"; then
    save_app_default_password "$APP" "$(generate_app_password)"
fi
chown "$APP_USER:$APP_USER" "$PASSWORD_FILE"
chmod 600 "$PASSWORD_FILE"

# Compose file: refreshed from app_data, which MyNode updates on every boot
install -m 0644 -o "$APP_USER" -g "$APP_USER" "$INSTALL_DIR/app_data/docker-compose.yml" "$INSTALL_DIR/docker-compose.yml"

# Addresses for Friglet's Connect page and its Host/Origin check
LOCAL_IP=$(/usr/bin/get_local_ip.py 2>/dev/null || true)
if ! [[ "$LOCAL_IP" =~ ^[0-9.]+$ ]]; then
    LOCAL_IP=$(hostname -I 2>/dev/null | awk '{print $1}')
fi
HOST_NAME=$(hostname)

ONION_HOST=""
if is_premium && is_tor_remote_access_enabled && [ -f "$ONION_HOSTNAME_FILE" ]; then
    ONION_HOST=$(tr -d '[:space:]' < "$ONION_HOSTNAME_FILE")
    if ! [[ "$ONION_HOST" =~ ^[a-z2-7]{56}\.onion$ ]]; then
        ONION_HOST=""
    fi
fi

# Host values browsers send through nginx (host:8098) or Tor (the onion on port 443)
ALLOWED_HOSTS=$(printf '%s\n' ${LOCAL_IP:+"$LOCAL_IP:$HTTPS_PORT"} "$HOST_NAME.local:$HTTPS_PORT" \
    "$HOST_NAME:$HTTPS_PORT" "mynode.local:$HTTPS_PORT" ${ONION_HOST:+"$ONION_HOST"} | awk '!seen[$0]++' | paste -sd, -)

require_regular_file_or_missing "$RUNTIME_ENV"
umask 077
ENV_TMP=$(mktemp "$INSTALL_DIR/.runtime.env.XXXXXX")
{
    echo "FRIGLET_UI_PUBLIC_HOST=${LOCAL_IP:-$HOST_NAME.local}"
    if [ -n "$ONION_HOST" ]; then
        echo "FRIGLET_UI_ONION_HOST=$ONION_HOST"
    fi
    echo "FRIGLET_UI_ALLOWED_HOSTS=$ALLOWED_HOSTS"
} > "$ENV_TMP"
chown "$APP_USER:$APP_USER" "$ENV_TMP"
mv -f "$ENV_TMP" "$RUNTIME_ENV"

# A node pruned after install cannot serve old blocks; Friglet shows this as node-pruned
if grep -Eq '^[[:space:]]*(main\.)?prune[[:space:]]*=[[:space:]]*0*[1-9]' /mnt/hdd/mynode/bitcoin/bitcoin.conf 2>/dev/null; then
    echo "WARNING: bitcoin.conf sets prune=; Friglet needs an unpruned node and will report node-pruned"
fi
