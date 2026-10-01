#!/bin/bash
#
# Runs the app's install, pre-start and uninstall scripts the way mynode-manage-apps and
# systemd run them, inside a throwaway Debian 12 + Python 3.8 container (python:3.8-slim, as on
# MyNode), with MyNode's real helper scripts and stand-ins for docker, docker-compose and the
# network. Started by check.sh; expects:
#   /mynode  a mynodebtc/mynode checkout (read-only)
#   /app     the blindbit-friglet app folder (read-only)
#   /out     where the files the scripts wrote are left for `docker compose config`

set -u

APP=blindbit-friglet
INSTALL_DIR=/opt/mynode/$APP
STORAGE_DIR=/mnt/hdd/mynode/$APP
SETTINGS_DIR=/mnt/hdd/mynode/settings
BITCOIN_CONF=/mnt/hdd/mynode/bitcoin/bitcoin.conf
VERSION=$(python3 -c 'import json; print(json.load(open("/app/'$APP'.json"))["latest_version"])')
DIGEST=$(sed -n 's/^IMAGE_DIGEST="[^ ]* \(sha256:[0-9a-f]*\)"$/\1/p' /app/scripts/install_$APP.sh)
LOG=/tmp/shim.log
FAILED=0

pass() { echo "PASS: $*"; }
fail() { echo "FAIL: $*"; FAILED=1; }
check() { local what=$1; shift; if "$@"; then pass "$what"; else fail "$what"; fi; }
mode_owner() { stat -c '%a %U' "$1"; }

# --- A MyNode-like system ------------------------------------------------------------------
useradd -m -u 1001 -U bitcoin
mkdir -p /usr/share/mynode /usr/bin/service_scripts /mnt/hdd/mynode/bitcoin "$SETTINGS_DIR" /home/bitcoin/.mynode
for f in mynode_functions.sh mynode_app_versions.sh mynode_device_info.sh; do
    cp "/mynode/rootfs/standard/usr/share/mynode/$f" /usr/share/mynode/
done
# The generated default bitcoin.conf: base, IPv4 disabled, Tor (mynode_gen_bitcoin_config.sh)
cat /mynode/rootfs/standard/usr/share/mynode/{bitcoin.conf,bitcoin_no_ipv4.conf,bitcoin_tor.conf} > "$BITCOIN_CONF"
cp "$BITCOIN_CONF" /tmp/bitcoin.conf.default
chown -R bitcoin:bitcoin /mnt/hdd/mynode /home/bitcoin

cat > /usr/local/bin/docker <<'EOF'
#!/bin/bash
echo "docker $*" >> /tmp/shim.log
case "$1" in
    images) [ -f /tmp/shim.images ] && cat /tmp/shim.images ;;
    pull) [ -z "${SHIM_PULL_FAIL:-}" ] || exit 1 ;;
esac
exit 0
EOF
cat > /usr/local/bin/docker-compose <<'EOF'
#!/bin/bash
echo "docker-compose $*" >> /tmp/shim.log
exit 0
EOF
printf '#!/bin/sh\necho 192.168.1.50\n' > /usr/bin/get_local_ip.py
chmod 755 /usr/local/bin/docker /usr/local/bin/docker-compose /usr/bin/get_local_ip.py
touch "$LOG" && chmod 666 "$LOG"

# What mynode-manage-apps init and install do around the app's scripts (application_info.py)
cp /app/scripts/*.sh /usr/bin/service_scripts/
prepare_install_folder() {
    rm -rf "$INSTALL_DIR" && mkdir -p "$INSTALL_DIR" "$STORAGE_DIR"
    cp -r /app/app_data "$INSTALL_DIR/app_data"
    chown -R bitcoin:bitcoin "$INSTALL_DIR" "$STORAGE_DIR"
}
# sudo -u bitcoin --preserve-env HOME=/home/bitcoin /bin/bash install_<app>.sh, from the install folder
run_as_app_user() {
    local script=$1; shift
    (cd "$INSTALL_DIR" && env "$@" VERSION="${RUN_VERSION:-$VERSION}" INSTALL_FOLDER="$INSTALL_DIR" \
        STORAGE_FOLDER="$STORAGE_DIR" HOME=/home/bitcoin \
        setpriv --reuid=bitcoin --regid=bitcoin --init-groups /bin/bash "/usr/bin/service_scripts/${script}_$APP.sh")
}
# ExecStartPre=+... from WorkingDirectory=/opt/mynode/<app>
run_pre() {
    (cd "$INSTALL_DIR" && /bin/bash "/usr/bin/service_scripts/pre_$APP.sh")
}

# --- install ------------------------------------------------------------------------------
echo "== install (pinned version $VERSION)"
prepare_install_folder
echo "ghcr.io/setavenger/friglet-node:old-1" > /tmp/shim.images
run_as_app_user install > /tmp/install.out 2>&1; rc=$?
check "install exits 0" [ "$rc" -eq 0 ]
check "install pulls the image by index digest" grep -qx "docker pull ghcr.io/setavenger/friglet-node@$DIGEST" "$LOG"
check "install tags the pinned image" grep -qx "docker tag ghcr.io/setavenger/friglet-node@$DIGEST ghcr.io/setavenger/friglet-node:$VERSION" "$LOG"
check "install removes old friglet-node images" grep -qx "docker rmi ghcr.io/setavenger/friglet-node:old-1" "$LOG"
check "compose .env pins the image by digest" grep -qx "FRIGLET_IMAGE=ghcr.io/setavenger/friglet-node:$VERSION@$DIGEST" "$INSTALL_DIR/.env"
check "compose .env has the bitcoin uid:gid" grep -qx "FRIGLET_UID=1001" "$INSTALL_DIR/.env"
check ".env is 0600 bitcoin" [ "$(mode_owner "$INSTALL_DIR/.env")" = "600 bitcoin" ]
check "data folder is 0700 bitcoin" [ "$(mode_owner "$STORAGE_DIR/data")" = "700 bitcoin" ]
check "storage folder is 0700 bitcoin" [ "$(mode_owner "$STORAGE_DIR")" = "700 bitcoin" ]

# --- pre-start ----------------------------------------------------------------------------
echo "== pre (Community Edition)"
touch "$SETTINGS_DIR/.product_key_skipped"
run_pre > /tmp/pre.out 2>&1; rc=$?
check "pre exits 0" [ "$rc" -eq 0 ]
PW=$(cat "$STORAGE_DIR/.app_password" 2>/dev/null)
check "pre generates a 24-character password" grep -Eqx '[A-Za-z0-9]{24}' "$STORAGE_DIR/.app_password"
check "password file is 0600 bitcoin" [ "$(mode_owner "$STORAGE_DIR/.app_password")" = "600 bitcoin" ]
check "compose file is refreshed from app_data" cmp -s /app/app_data/docker-compose.yml "$INSTALL_DIR/docker-compose.yml"
check "compose file is readable by bitcoin" [ "$(mode_owner "$INSTALL_DIR/docker-compose.yml")" = "644 bitcoin" ]
check "runtime.env is 0600 bitcoin" [ "$(mode_owner "$INSTALL_DIR/runtime.env")" = "600 bitcoin" ]
check "runtime.env has the LAN host" grep -qx "FRIGLET_UI_PUBLIC_HOST=192.168.1.50" "$INSTALL_DIR/runtime.env"
check "runtime.env allows the nginx host" grep -q "^FRIGLET_UI_ALLOWED_HOSTS=192.168.1.50:8098,.*mynode.local:8098" "$INSTALL_DIR/runtime.env"
check "no onion on Community Edition" bash -c "! grep -q ONION '$INSTALL_DIR/runtime.env'"
check "no secrets in runtime.env" bash -c "! grep -q '$PW' '$INSTALL_DIR/runtime.env'"

echo "== pre (Premium, Tor remote access on)"
rm -f "$SETTINGS_DIR/.product_key_skipped"
ONION="abcdefghijklmnopqrstuvwxyz234567abcdefghijklmnopqrstuvwx.onion"
mkdir -p "/var/lib/tor/mynode_$APP" && echo "$ONION" > "/var/lib/tor/mynode_$APP/hostname"
run_pre > /tmp/pre2.out 2>&1; rc=$?
check "pre exits 0 again" [ "$rc" -eq 0 ]
check "password is kept across starts" [ "$(cat "$STORAGE_DIR/.app_password")" = "$PW" ]
check "runtime.env has the onion host" grep -qx "FRIGLET_UI_ONION_HOST=$ONION" "$INSTALL_DIR/runtime.env"
check "onion is an allowed host" grep -q "^FRIGLET_UI_ALLOWED_HOSTS=.*,$ONION\$" "$INSTALL_DIR/runtime.env"

echo "== pre (Premium, Tor remote access off)"
touch "$SETTINGS_DIR/tor_remote_access_disabled"
run_pre > /tmp/pre3.out 2>&1
check "no onion with Tor remote access off" bash -c "! grep -q ONION '$INSTALL_DIR/runtime.env'"
rm -f "$SETTINGS_DIR/tor_remote_access_disabled"
run_pre > /tmp/pre4.out 2>&1
mkdir -p /out && cp "$INSTALL_DIR/docker-compose.yml" "$INSTALL_DIR/.env" "$INSTALL_DIR/runtime.env" /out/
# Readable and removable by whoever ran check.sh
chown "$(stat -c %u:%g /out)" /out/docker-compose.yml /out/.env /out/runtime.env
chmod 644 /out/docker-compose.yml /out/.env /out/runtime.env

echo "== pre refuses links planted in the data folder"
mv "$STORAGE_DIR/data" "$STORAGE_DIR/data.real" && ln -s /etc "$STORAGE_DIR/data"
run_pre > /tmp/pre5.out 2>&1; rc=$?
check "pre refuses a symlinked data folder" [ "$rc" -ne 0 ]
check "pre names the refused path" grep -q "Refusing to use $STORAGE_DIR/data" /tmp/pre5.out
check "/etc ownership untouched" [ "$(stat -c %U /etc)" = "root" ]
rm "$STORAGE_DIR/data" && mv "$STORAGE_DIR/data.real" "$STORAGE_DIR/data"
mv "$STORAGE_DIR/.app_password" /tmp/pw && ln -s /etc/shadow "$STORAGE_DIR/.app_password"
run_pre > /tmp/pre6.out 2>&1; rc=$?
check "pre refuses a symlinked password file" [ "$rc" -ne 0 ]
check "/etc/shadow untouched" [ "$(mode_owner /etc/shadow)" = "640 root" ]
rm "$STORAGE_DIR/.app_password" && mv /tmp/pw "$STORAGE_DIR/.app_password"

# --- install refusals ---------------------------------------------------------------------
refused() {  # refused <bitcoin.conf lines to append...>
    cp /tmp/bitcoin.conf.default "$BITCOIN_CONF" && printf '%s\n' "$@" >> "$BITCOIN_CONF"
    prepare_install_folder
    run_as_app_user install > /tmp/install.refused 2>&1
}
echo "== install refuses a pruned node"
refused "prune=550"; rc=$?
check "install refuses prune=550" [ "$rc" -ne 0 ]
check "refusal names problem, cause and fix" bash -c "grep -q 'this node is pruned' /tmp/install.refused && grep -q 'sets prune=550' /tmp/install.refused && grep -q 'Fix:' /tmp/install.refused"
check "refusal pulls nothing" bash -c "! grep -q 'docker pull' /tmp/install.refused"
refused "main.prune=1000"; rc=$?
check "install refuses main.prune=1000" [ "$rc" -ne 0 ]
refused "[main]" "prune = 2000"; rc=$?
check "install refuses prune in [main]" [ "$rc" -ne 0 ]
refused "prune=0"; rc=$?
check "install accepts prune=0" [ "$rc" -eq 0 ]
refused "[test]" "prune=550"; rc=$?
check "install ignores prune for testnet only" [ "$rc" -eq 0 ]
refused "#prune=550"; rc=$?
check "install ignores a commented prune" [ "$rc" -eq 0 ]
cp /tmp/bitcoin.conf.default "$BITCOIN_CONF"

echo "== install refuses a 32-bit device"
# shellcheck disable=SC2016 # the stand-in script expands $1 itself
mkdir -p /tmp/arm32 && printf '#!/bin/sh\nif [ "$1" = "-m" ]; then echo armv7l; else echo "Linux mynode armv7l GNU/Linux"; fi\n' > /tmp/arm32/uname
chmod 755 /tmp/arm32/uname
prepare_install_folder
run_as_app_user install PATH="/tmp/arm32:$PATH" > /tmp/install.arm32 2>&1; rc=$?
check "install refuses armv7l" [ "$rc" -ne 0 ]
check "arch refusal says why" grep -q "64-bit devices" /tmp/install.arm32

echo "== install with a custom version (no pin: pulled by tag)"
echo "93f6a92-9" > "$SETTINGS_DIR/${APP}_version_latest_custom"
prepare_install_folder
: > "$LOG"
RUN_VERSION=93f6a92-9 run_as_app_user install > /tmp/install.custom 2>&1; rc=$?
check "custom-version install exits 0" [ "$rc" -eq 0 ]
check "custom version is pulled by tag" grep -qx "docker pull ghcr.io/setavenger/friglet-node:93f6a92-9" "$LOG"
check "compose .env names the custom tag" grep -qx "FRIGLET_IMAGE=ghcr.io/setavenger/friglet-node:93f6a92-9" "$INSTALL_DIR/.env"
rm -f "$SETTINGS_DIR/${APP}_version_latest_custom"

echo "== install fails when the pull fails"
prepare_install_folder
run_as_app_user install SHIM_PULL_FAIL=1 > /tmp/install.pullfail 2>&1; rc=$?
check "install fails without the image" [ "$rc" -ne 0 ]

# --- uninstall ----------------------------------------------------------------------------
echo "== uninstall"
prepare_install_folder
run_as_app_user install > /dev/null 2>&1
run_pre > /dev/null 2>&1
echo "secret-scan-key" > "$STORAGE_DIR/data/scan.key" && chown bitcoin:bitcoin "$STORAGE_DIR/data/scan.key"
: > "$LOG"
echo "ghcr.io/setavenger/friglet-node:$VERSION" > /tmp/shim.images
run_as_app_user uninstall > /tmp/uninstall.out 2>&1; rc=$?
check "uninstall exits 0" [ "$rc" -eq 0 ]
check "uninstall stops the containers" grep -qx "docker-compose down --remove-orphans" "$LOG"
check "uninstall removes the image" grep -qx "docker rmi ghcr.io/setavenger/friglet-node:$VERSION" "$LOG"
check "uninstall deletes the data folder with the scan key" [ ! -e "$STORAGE_DIR" ]
check "uninstall says it deletes the scan key" grep -q "scan key" /tmp/uninstall.out

if [ "$FAILED" -ne 0 ]; then
    echo "--- install.out"; cat /tmp/install.out
    echo "--- pre.out"; cat /tmp/pre.out
    echo "LIFECYCLE: FAILED"
    exit 1
fi
echo "LIFECYCLE: all checks passed"
