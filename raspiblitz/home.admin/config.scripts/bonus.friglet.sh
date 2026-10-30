#!/bin/bash

# friglet: BlindBit Silent Payments scanner with an Electrum server for one wallet.
# friglet-ui (web UI + supervisor) runs friglet; both come from one verified release
# tarball. No Docker, no RPC credentials: friglet talks to bitcoind over local P2P only.
# Packaging home: https://github.com/setavenger/blindbit-node-apps (raspiblitz/).

APPID="friglet"

# friglet-node package version: <friglet short commit>-<package revision>
VERSION="5dbb16f-1"

# Release download location.
# TODO(release-location): the publishing location of the friglet-node tarballs is not decided
# yet (blindbit-node-apps docs/plan.md "Open items"). Placeholder until the first release.
RELEASE_BASE_URL="https://github.com/setavenger/blindbit-node-apps/releases/download/friglet-node-${VERSION}"

# Pinned SHA256 of friglet-node-<VERSION>-linux-<arch>.tar.gz per architecture.
# Empty means no release is pinned for that architecture, and install refuses.
SHA256_ARM64=""
SHA256_AMD64=""

# TODO(signing-key): the release signing key is NOT decided yet. While PGP_FINGERPRINT is
# empty the pinned SHA256 above is the only integrity check. Once a key is decided, set both
# values; install then also requires a good signature over SHA256SUMS by that key.
PGP_PUBKEY_URL=""
PGP_FINGERPRINT=""

# ports (blindbit-node-apps docs/contract.md)
PORT_UI="8097"     # friglet-ui, loopback only
PORT_HTTPS="8098"  # nginx HTTPS in front of friglet-ui
PORT_TCP="50031"   # Electrum plaintext (friglet-ui)
PORT_SSL="50032"   # Electrum TLS (friglet-ui, self-signed, fingerprint on its Connect page)
# bitcoind P2P whitebind for friglet. The contract says 8336, but on RaspiBlitz BTCPay Server
# already binds <chain>.whitebind=127.0.0.1:8336 and a second bind stops bitcoind from starting.
PORT_P2P="8337"

APP_USER="${APPID}"
APP_HOME="/home/${APPID}"
APP_DATA="/mnt/hdd/app-data/${APPID}"
PASSWORD_FILE="${APP_DATA}/ui-password"
HOST_ENV_FILE="${APP_HOME}/friglet-ui.env"
ONION_FILE="${APP_HOME}/onion-hostname"
UNIT_FILE="/etc/systemd/system/${APPID}.service"
NGINX_SITE="/etc/nginx/sites-available/${APPID}_ssl.conf"
NGINX_LINK="/etc/nginx/sites-enabled/${APPID}_ssl.conf"
BITCOIN_CONF="/mnt/hdd/app-data/bitcoin/bitcoin.conf"
LAN_RANGES="10.0.0.0/8 172.16.0.0/12 192.168.0.0/16"
EXPOSURE_MODES="lan lan-tcp onion local"

# exact refusal text for a pruned node (keep in sync with docs/platforms/raspiblitz.md)
prunedRefusal() {
  echo "# REFUSED: friglet needs an unpruned Bitcoin node."
  echo "# Problem: this node is pruned ($1 in ${BITCOIN_CONF})."
  echo "# Cause: friglet downloads every block from your wallet birthday onward from this node, and a pruned node has deleted old blocks."
  echo "# Fix: remove the prune setting (or set prune=0), restart bitcoind and let it download the full chain again, then switch friglet on again."
  echo "# Help: blindbit-node-apps docs/troubleshooting.md#node-pruned"
  echo "error='node is pruned ($1), friglet needs an unpruned node'"
}

usage() {
  echo "# config script for friglet (BlindBit Silent Payments scanner and Electrum server)"
  echo "# bonus.${APPID}.sh status [showAddress]   -> status information (key=value)"
  echo "# bonus.${APPID}.sh status-sync            -> scan state (cheap, loop safe)"
  echo "# bonus.${APPID}.sh menu                   -> SSH menu dialog"
  echo "# bonus.${APPID}.sh install                -> install the verified binaries only"
  echo "# bonus.${APPID}.sh on                     -> install and switch on"
  echo "# bonus.${APPID}.sh off [--delete-data]    -> switch off, optionally delete wallet data"
  echo "# bonus.${APPID}.sh update                 -> install the version pinned in this script"
  echo "# bonus.${APPID}.sh password-reset         -> new web UI password, ends all sessions"
  echo "# bonus.${APPID}.sh electrum-exposure [${EXPOSURE_MODES// /|}]"
  echo "# bonus.${APPID}.sh prestart               -> called by systemd before start"
  echo "# installs friglet-node ${VERSION}"
}

if [ $# -eq 0 ] || [ "$1" = "-h" ] || [ "$1" = "-help" ] || [ "$1" = "--help" ]; then
  usage
  exit 1
fi

# shellcheck disable=SC1091
source /mnt/hdd/app-data/raspiblitz.conf 2>/dev/null
# raspiblitz.conf values used here (defaults when unset)
chain="${chain:-main}"
runBehindTor="${runBehindTor:-off}"
friglet="${friglet:-off}"
frigletElectrum="${frigletElectrum:-}"
onionChanged=0

#########################
# HELPERS
#########################

getLocalIP() {
  hostname -I 2>/dev/null | awk '{print $1}'
}

isServiceRunning() {
  if systemctl is-active --quiet "${APPID}" 2>/dev/null; then echo 1; else echo 0; fi
}

installedVersion() {
  local target
  target=$(sudo readlink "${APP_HOME}/current" 2>/dev/null)
  echo "${target#friglet-node-}"
}

# map raspiblitz.conf chain to the bitcoin.conf section prefix and to FRIGLET_UI_NETWORK
chainPrefix() {
  case "${chain}" in
    main|"") echo "main" ;;
    test) echo "test" ;;
    sig) echo "signet" ;;
    *) echo "" ;;
  esac
}

frigletNetwork() {
  case "${chain}" in
    main|"") echo "bitcoin" ;;
    sig) echo "signet" ;;
    *) echo "" ;;  # RaspiBlitz chain=test is testnet3, which friglet does not support
  esac
}

currentExposure() {
  local mode="${frigletElectrum}"
  case " ${EXPOSURE_MODES} " in
    *" ${mode} "*) echo "${mode}" ;;
    *) echo "lan" ;;
  esac
}

# write stdin to a root-managed file; prints "changed" when the content differs
writeFile() { # path mode owner:group
  local path="$1" mode="$2" owner="$3" tmp
  tmp=$(mktemp) || return 1
  cat > "${tmp}"
  if sudo test -f "${path}" && sudo cmp -s "${tmp}" "${path}"; then
    sudo chmod "${mode}" "${path}"
    sudo chown "${owner}" "${path}"
    rm -f "${tmp}"
    return 0
  fi
  sudo install -m "${mode}" -o "${owner%%:*}" -g "${owner##*:}" "${tmp}" "${path}" || { rm -f "${tmp}"; return 1; }
  rm -f "${tmp}"
  echo "changed"
}

# prune check: bitcoin.conf (plain, chain-prefixed and [section] forms), then RPC if available
prunedSetting() {
  local prefix="$1" setting
  setting=$(sudo awk -v want="${prefix}" '
    /^[[:space:]]*\[/ { section=$0; gsub(/[][[:space:]]/, "", section); next }
    /^[[:space:]]*#/ { next }
    {
      line=$0; sub(/[[:space:]]*#.*/, "", line); gsub(/[[:space:]]/, "", line)
      split(line, kv, "="); key=kv[1]; val=kv[2]
      scope=section
      n=split(key, parts, ".")
      if (n == 2) { scope=parts[1]; key=parts[2] }
      if (key != "prune") next
      if (scope != "" && scope != want) next
      # any positive value for this chain counts (prune=1 is manual pruning)
      if (val ~ /^[0-9]+$/ && val + 0 > 0) found=kv[1] "=" val
    }
    END { print found }' "${BITCOIN_CONF}" 2>/dev/null)
  if [ -n "${setting}" ]; then
    echo "${setting}"
    return
  fi
  if command -v bitcoin-cli >/dev/null 2>&1; then
    local cliChain="${prefix}"
    local pruned
    pruned=$(timeout 15 sudo -u bitcoin bitcoin-cli -chain="${cliChain}" getblockchaininfo 2>/dev/null | jq -r '.pruned' 2>/dev/null)
    if [ "${pruned}" = "true" ]; then
      echo "pruned per bitcoind getblockchaininfo"
    fi
  fi
}

archName() {
  case "$(uname -m)" in
    aarch64|arm64) echo "arm64" ;;
    x86_64|amd64) echo "amd64" ;;
    *) echo "" ;;
  esac
}

pinnedSha256() {
  case "$1" in
    arm64) echo "${SHA256_ARM64}" ;;
    amd64) echo "${SHA256_AMD64}" ;;
  esac
}

# signature hook: active once PGP_FINGERPRINT is pinned (see TODO(signing-key) at the top)
verifySignature() { # downloadDir tarballName expectedSha256
  local dir="$1" tarball="$2" sha="$3"
  if [ -z "${PGP_FINGERPRINT}" ]; then
    echo "# NOTE: no release signing key is pinned yet: only the pinned SHA256 is checked"
    return 0
  fi
  echo "# verifying the signature of SHA256SUMS"
  curl -fsSL -o "${dir}/SHA256SUMS" "${RELEASE_BASE_URL}/SHA256SUMS" || return 1
  curl -fsSL -o "${dir}/SHA256SUMS.asc" "${RELEASE_BASE_URL}/SHA256SUMS.asc" || return 1
  curl -fsSL -o "${dir}/signer.asc" "${PGP_PUBKEY_URL}" || return 1
  local gnupg="${dir}/gnupg" status fpr
  fpr=$(echo "${PGP_FINGERPRINT}" | tr -d ' ' | tr 'a-f' 'A-F')
  mkdir -m 700 "${gnupg}" || return 1
  gpg --homedir "${gnupg}" --batch --quiet --import "${dir}/signer.asc" 2>/dev/null || return 1
  status=$(LANG=C gpg --homedir "${gnupg}" --batch --status-fd 1 --verify "${dir}/SHA256SUMS.asc" "${dir}/SHA256SUMS" 2>/dev/null)
  # VALIDSIG <signing key fpr> ... <primary key fpr>: accept the pinned key or its subkey
  if ! echo "${status}" | awk -v f="${fpr}" '$1 == "[GNUPG:]" && $2 == "VALIDSIG" && ($3 == f || $NF == f) { ok = 1 } END { exit !ok }'; then
    echo "# FAIL: SHA256SUMS is not signed by ${fpr}"
    return 1
  fi
  if ! grep -Eq "^${sha}[[:space:]]+\*?${tarball}\$" "${dir}/SHA256SUMS"; then
    echo "# FAIL: the signed SHA256SUMS does not list ${tarball} with the pinned checksum"
    return 1
  fi
  echo "# OK good signature by ${fpr}"
}

ensureUser() {
  if ! id "${APP_USER}" >/dev/null 2>&1; then
    echo "# create user ${APP_USER}"
    sudo adduser --system --group --home "${APP_HOME}" "${APP_USER}" >/dev/null || return 1
  fi
  sudo install -d -m 0750 -o "${APP_USER}" -g "${APP_USER}" "${APP_HOME}"
}

# download, verify and unpack the pinned release (code only, no HDD access)
installCode() {
  local arch sha tarball url tmp actual dir
  arch=$(archName)
  if [ -z "${arch}" ]; then
    echo "# FAIL: unsupported CPU architecture $(uname -m) (friglet-node ships arm64 and amd64)"
    echo "error='unsupported architecture'"
    return 1
  fi
  sha=$(pinnedSha256 "${arch}")
  tarball="friglet-node-${VERSION}-linux-${arch}.tar.gz"
  if [ -z "${sha}" ]; then
    echo "# REFUSED: no pinned SHA256 for ${tarball} in this script."
    echo "# friglet-node ${VERSION} has no published, pinned release for linux-${arch} yet."
    echo "error='no pinned checksum for ${tarball}'"
    return 1
  fi

  dir="${APP_HOME}/friglet-node-${VERSION}"
  if sudo test -x "${dir}/friglet-ui" && sudo test -x "${dir}/friglet"; then
    echo "# friglet-node ${VERSION} is already installed"
  else
    url="${RELEASE_BASE_URL}/${tarball}"
    tmp=$(mktemp -d) || return 1
    echo "# download ${url}"
    if ! curl -fsSL --retry 3 -o "${tmp}/${tarball}" "${url}"; then
      echo "# FAIL: download failed"
      echo "error='download failed'"
      rm -rf "${tmp}"
      return 1
    fi
    actual=$(sha256sum "${tmp}/${tarball}" | cut -d' ' -f1)
    if [ "${actual}" != "${sha}" ]; then
      echo "# REFUSED: checksum mismatch for ${tarball}"
      echo "# pinned:     ${sha}"
      echo "# downloaded: ${actual}"
      echo "error='checksum mismatch'"
      rm -rf "${tmp}"
      return 1
    fi
    echo "# OK pinned SHA256 matches"
    if ! verifySignature "${tmp}" "${tarball}" "${sha}"; then
      echo "error='signature check failed'"
      rm -rf "${tmp}"
      return 1
    fi
    mkdir "${tmp}/x"
    if ! tar -xzf "${tmp}/${tarball}" -C "${tmp}/x" --no-same-owner --no-same-permissions; then
      echo "# FAIL: could not unpack ${tarball}"
      echo "error='unpack failed'"
      rm -rf "${tmp}"
      return 1
    fi
    local binUi binDaemon
    binUi=$(find "${tmp}/x" -maxdepth 2 -type f -name friglet-ui | head -n1)
    binDaemon=$(find "${tmp}/x" -maxdepth 2 -type f -name friglet | head -n1)
    if [ -z "${binUi}" ] || [ -z "${binDaemon}" ]; then
      echo "# FAIL: ${tarball} does not contain friglet and friglet-ui"
      echo "error='bad tarball layout'"
      rm -rf "${tmp}"
      return 1
    fi
    # the user is created only once the download is verified
    ensureUser || { echo "error='could not create user ${APP_USER}'"; rm -rf "${tmp}"; return 1; }
    # binaries are root-owned so the friglet user cannot change them
    sudo rm -rf "${dir}"
    sudo install -d -m 0755 -o root -g root "${dir}"
    sudo install -m 0755 -o root -g root "${binUi}" "${dir}/friglet-ui"
    sudo install -m 0755 -o root -g root "${binDaemon}" "${dir}/friglet"
    local lic
    while IFS= read -r lic; do
      sudo install -m 0644 -o root -g root "${lic}" "${dir}/$(basename "${lic}")"
    done < <(find "${tmp}/x" -maxdepth 2 -type f -name 'LICENSE*')
    rm -rf "${tmp}"
  fi

  # smoke test as the app user (never run the app binaries as root)
  if ! sudo -u "${APP_USER}" "${dir}/friglet-ui" --version >/dev/null 2>&1; then
    echo "# FAIL: ${dir}/friglet-ui does not run on this system (glibc or architecture?)"
    echo "error='friglet-ui does not execute'"
    return 1
  fi
  sudo ln -sfn "friglet-node-${VERSION}" "${APP_HOME}/current"

  # keep the current and the previous version, remove older ones
  local keep previous old
  keep="friglet-node-${VERSION}"
  previous=$(sudo find "${APP_HOME}" -maxdepth 1 -type d -name 'friglet-node-*' ! -name "${keep}" -printf '%T@ %f\n' 2>/dev/null | sort -rn | head -n1 | cut -d' ' -f2)
  while IFS= read -r old; do
    [ -n "${old}" ] || continue
    [ "${old}" = "${keep}" ] && continue
    [ "${old}" = "${previous}" ] && continue
    sudo rm -rf "${APP_HOME:?}/${old}"
  done < <(sudo find "${APP_HOME}" -maxdepth 1 -type d -name 'friglet-node-*' -printf '%f\n' 2>/dev/null)

  echo "# OK friglet-node ${VERSION} installed in ${dir}"
  return 0
}

# generate the web UI password once; never printed (shown only by the SSH menu)
ensurePassword() {
  if sudo test -s "${PASSWORD_FILE}"; then
    echo "# keep existing web UI password"
    sudo chown "${APP_USER}:${APP_USER}" "${PASSWORD_FILE}"
    sudo chmod 600 "${PASSWORD_FILE}"
    return 0
  fi
  newPassword && echo "# generated a new web UI password (see: bonus.${APPID}.sh menu)"
}

newPassword() {
  # generated and written by the app user inside its 0700 data dir: never in argv, env or stdout
  # shellcheck disable=SC2016
  sudo -u "${APP_USER}" sh -c 'umask 077; tmp="$1.new"; LC_ALL=C tr -dc "A-HJ-NP-Za-km-z2-9" < /dev/urandom | head -c 24 > "$tmp" && [ "$(wc -c < "$tmp")" -eq 24 ] && mv -f "$tmp" "$1"' _ "${PASSWORD_FILE}"
}

# ensure <prefix>.whitebind=download@127.0.0.1:PORT_P2P; echo "restart" when bitcoind must restart
ensureWhitebind() {
  local prefix="$1" line other
  line="${prefix}.whitebind=download@127.0.0.1:${PORT_P2P}"
  # migrate a non-prefixed line written by hand
  sudo sed -i "s/^whitebind=download@127.0.0.1:${PORT_P2P}\$/${line}/" "${BITCOIN_CONF}"
  other=$(sudo grep -E "^([a-z]+\.)?whitebind=.*127\.0\.0\.1:${PORT_P2P}\$" "${BITCOIN_CONF}" | grep -v -x -F "${line}" | head -n1)
  if [ -n "${other}" ]; then
    echo "# FAIL: port ${PORT_P2P} is already used by another whitebind in ${BITCOIN_CONF}: ${other}" >&2
    return 1
  fi
  if ! sudo grep -q -x -F "${line}" "${BITCOIN_CONF}"; then
    echo "# add ${line} to bitcoin.conf" >&2
    sudo sh -c 'printf "%s\n" "$1" >> "$2"' _ "${line}" "${BITCOIN_CONF}"
    # restart only if bitcoind is not already listening there (e.g. off and on again
    # without a bitcoind restart in between)
    if ! ss -Hltn "sport = :${PORT_P2P}" 2>/dev/null | grep -q .; then
      echo "restart"
    fi
  fi
  return 0
}

removeWhitebind() {
  sudo sed -i "/^\([a-z]*\.\)\{0,1\}whitebind=download@127\.0\.0\.1:${PORT_P2P}\$/d" "${BITCOIN_CONF}" 2>/dev/null
}

exposureListeners() { # mode -> "tcpAddr tlsAddr"
  case "$1" in
    lan) echo "127.0.0.1:${PORT_TCP} 0.0.0.0:${PORT_SSL}" ;;
    lan-tcp) echo "0.0.0.0:${PORT_TCP} 0.0.0.0:${PORT_SSL}" ;;
    onion) echo "127.0.0.1:${PORT_TCP} off" ;;
    local) echo "127.0.0.1:${PORT_TCP} 127.0.0.1:${PORT_SSL}" ;;
  esac
}

exposurePorts() { # mode -> LAN ports to open in ufw
  case "$1" in
    lan) echo "${PORT_HTTPS} ${PORT_SSL}" ;;
    lan-tcp) echo "${PORT_HTTPS} ${PORT_TCP} ${PORT_SSL}" ;;
    *) echo "${PORT_HTTPS}" ;;
  esac
}

writeUnit() { # mode -> echo "changed" if the unit changed
  local mode="$1" network listeners tcp tls
  network=$(frigletNetwork)
  listeners=$(exposureListeners "${mode}")
  tcp="${listeners% *}"
  tls="${listeners#* }"
  writeFile "${UNIT_FILE}" 0644 root:root <<EOF
# written by bonus.${APPID}.sh: changes are overwritten on the next 'on'
[Unit]
Description=friglet-ui (BlindBit Silent Payments scanner)
Wants=bitcoind.service network-online.target
After=bitcoind.service network-online.target

[Service]
Type=simple
User=${APP_USER}
Group=${APP_USER}
UMask=0077
WorkingDirectory=${APP_HOME}
ExecStartPre=-/home/admin/config.scripts/bonus.${APPID}.sh prestart
ExecStart=${APP_HOME}/current/friglet-ui run
Environment=FRIGLET_UI_PLATFORM=raspiblitz
Environment=FRIGLET_UI_DATA_DIR=${APP_DATA}
Environment=FRIGLET_UI_AUTH=password
Environment=FRIGLET_UI_PASSWORD_FILE=${PASSWORD_FILE}
Environment=FRIGLET_UI_NETWORK=${network}
Environment=FRIGLET_UI_NODE=127.0.0.1:${PORT_P2P}
Environment=FRIGLET_UI_LISTEN=127.0.0.1:${PORT_UI}
Environment=FRIGLET_UI_ELECTRUM_TCP=${tcp}
Environment=FRIGLET_UI_ELECTRUM_TLS=${tls}
# FRIGLET_UI_PUBLIC_HOST and FRIGLET_UI_ONION_HOST, refreshed by prestart on every start
EnvironmentFile=-${HOST_ENV_FILE}
KillMode=mixed
KillSignal=SIGTERM
TimeoutStartSec=120
TimeoutStopSec=150
Restart=always
RestartSec=30
StandardOutput=journal
StandardError=journal

# Hardening measures
PrivateTmp=true
ProtectSystem=strict
ReadWritePaths=${APP_DATA} ${APP_HOME}
NoNewPrivileges=true
PrivateDevices=true

[Install]
WantedBy=multi-user.target
EOF
}

writeNginx() { # echo "changed" if the site changed
  writeFile "${NGINX_SITE}" 0644 root:root <<EOF
# written by bonus.${APPID}.sh
server {
    listen ${PORT_HTTPS} ssl;
    listen [::]:${PORT_HTTPS} ssl;
    server_name _;
    include /etc/nginx/snippets/ssl-params.conf;
    include /etc/nginx/snippets/ssl-certificate-app-data.conf;
    access_log /var/log/nginx/access_${APPID}.log;
    error_log /var/log/nginx/error_${APPID}.log;
    location / {
        proxy_pass http://127.0.0.1:${PORT_UI};
        include /etc/nginx/snippets/ssl-proxy-params.conf;
    }
}
EOF
}

ufwRules() { # allow|delete ports...
  local action="$1" port range
  shift
  for port in "$@"; do
    for range in ${LAN_RANGES}; do
      if [ "${action}" = "allow" ]; then
        sudo ufw allow from "${range}" to any port "${port}" proto tcp comment "${APPID} LAN" >/dev/null
      else
        sudo ufw delete allow from "${range}" to any port "${port}" proto tcp >/dev/null 2>&1
      fi
    done
  done
}

applyFirewall() { # mode
  local wanted port
  wanted=" $(exposurePorts "$1") "
  for port in ${PORT_HTTPS} ${PORT_TCP} ${PORT_SSL}; do
    case "${wanted}" in
      *" ${port} "*) ufwRules allow "${port}" ;;
      *) ufwRules delete "${port}" ;;
    esac
  done
  echo "# firewall: LAN access to TCP${wanted% }"
}

# the HiddenServicePort lines tor.onion-service.sh would write for a mode
torPorts() { # mode
  if [ "$1" != "local" ]; then
    echo "HiddenServicePort ${PORT_TCP} 127.0.0.1:${PORT_TCP}"
  fi
  echo "HiddenServicePort 80 127.0.0.1:${PORT_UI}"
}

# sets onionChanged=1 when the onion hostname handed to friglet-ui changed
applyTor() { # mode
  local before after rc current
  before=$(sudo cat "${ONION_FILE}" 2>/dev/null)
  onionChanged=0
  if [ "${runBehindTor}" != "on" ]; then
    sudo rm -f "${ONION_FILE}"
    [ -n "${before}" ] && onionChanged=1
    return 0
  fi
  current=$(sudo sed -n "/^# Hidden Service for ${APPID}\$/,/^[[:space:]]*\$/p" /mnt/hdd/app-data/tor/torrc.d/services 2>/dev/null | grep '^HiddenServicePort' | sort)
  if [ -n "${current}" ] && [ "${current}" = "$(torPorts "$1" | sort)" ] && sudo test -s "/mnt/hdd/app-data/tor/${APPID}/hostname"; then
    rc=0
  elif [ "$1" = "local" ]; then
    /home/admin/config.scripts/tor.onion-service.sh "${APPID}" 80 "${PORT_UI}" >/dev/null 2>&1
    rc=$?
  else
    /home/admin/config.scripts/tor.onion-service.sh "${APPID}" "${PORT_TCP}" "${PORT_TCP}" 80 "${PORT_UI}" >/dev/null 2>&1
    rc=$?
  fi
  if [ ${rc} -ne 0 ]; then
    echo "# WARNING: tor.onion-service.sh failed (${rc}); the onion address is not available"
    return 0
  fi
  # hand the onion hostname to prestart (the Tor dir is not readable by the app user)
  if sudo test -s "/mnt/hdd/app-data/tor/${APPID}/hostname"; then
    sudo install -m 0640 -o root -g "${APP_USER}" "/mnt/hdd/app-data/tor/${APPID}/hostname" "${ONION_FILE}"
    echo "# Tor onion service ready (address shown in the web UI and SSH menu)"
  fi
  after=$(sudo cat "${ONION_FILE}" 2>/dev/null)
  [ "${before}" != "${after}" ] && onionChanged=1
  return 0
}

# write FRIGLET_UI_PUBLIC_HOST/ONION_HOST now (prestart refreshes them on every start)
refreshHostEnv() {
  sudo -u "${APP_USER}" "/home/admin/config.scripts/bonus.${APPID}.sh" prestart >/dev/null 2>&1
}

#########################
# PRESTART (as app user)
#########################

if [ "$1" = "prestart" ]; then
  if [ "$(id -un)" != "${APP_USER}" ]; then
    echo "# FAIL: run as user ${APP_USER}"
    exit 1
  fi
  umask 077
  {
    echo "FRIGLET_UI_PUBLIC_HOST=$(getLocalIP)"
    if [ -s "${ONION_FILE}" ]; then
      echo "FRIGLET_UI_ONION_HOST=$(head -n1 "${ONION_FILE}")"
    fi
  } > "${HOST_ENV_FILE}.new" && mv -f "${HOST_ENV_FILE}.new" "${HOST_ENV_FILE}"
  echo "# prestart: host environment refreshed"
  exit 0
fi

#########################
# STATUS
#########################

if [ "$1" = "status" ] || [ "$1" = "status-sync" ]; then
  serviceRunning=$(isServiceRunning)
  state=""
  scannedHeight=""
  tipHeight=""
  if [ "${serviceRunning}" = "1" ]; then
    widget=$(curl -s --max-time 2 "http://127.0.0.1:${PORT_UI}/api/widget" 2>/dev/null)
    state=$(echo "${widget}" | jq -r '.state // empty' 2>/dev/null | tr -cd 'a-z-')
    scannedHeight=$(echo "${widget}" | jq -r '.scanned_height // empty' 2>/dev/null | tr -cd '0-9')
    tipHeight=$(echo "${widget}" | jq -r '.tip_height // empty' 2>/dev/null | tr -cd '0-9')
  fi
  case "${state}" in
    "") if [ "${serviceRunning}" = "1" ]; then infoSync="starting"; else infoSync="not running - check: sudo journalctl -u ${APPID}"; fi ;;
    needs-setup) infoSync="needs setup in the web UI (port ${PORT_HTTPS})" ;;
    scanning) infoSync="scanning block ${scannedHeight:-?} of ${tipHeight:-?}" ;;
    up-to-date) infoSync="" ;;
    *) infoSync="${state}" ;;
  esac

  if [ "$1" = "status-sync" ]; then
    echo "serviceRunning=${serviceRunning}"
    echo "state='${state}'"
    echo "scannedHeight='${scannedHeight}'"
    echo "tipHeight='${tipHeight}'"
    echo "infoSync='${infoSync}'"
    exit 0
  fi

  serviceInstalled=0
  [ -f "${UNIT_FILE}" ] && serviceInstalled=1
  configured=0
  [ "${friglet}" = "on" ] && configured=1
  echo "appID='${APPID}'"
  echo "version='${VERSION}'"
  echo "installedVersion='$(installedVersion)'"
  echo "configured=${configured}"
  echo "installed=${serviceInstalled}"
  echo "serviceInstalled=${serviceInstalled}"
  echo "serviceRunning=${serviceRunning}"
  echo "network='$(frigletNetwork)'"
  echo "electrumExposure='$(currentExposure)'"
  echo "state='${state}'"
  echo "scannedHeight='${scannedHeight}'"
  echo "tipHeight='${tipHeight}'"
  echo "infoSync='${infoSync}'"
  if [ "${serviceInstalled}" = "1" ]; then
    echo "localIP='$(getLocalIP)'"
    echo "httpsPort='${PORT_HTTPS}'"
    echo "httpsForced='1'"
    echo "httpsSelfsigned='1'"
    echo "authMethod='userdefined'"
    echo "portTCP='${PORT_TCP}'"
    echo "portSSL='${PORT_SSL}'"
    if [ "$2" = "showAddress" ] && [ "${runBehindTor}" = "on" ]; then
      echo "toraddress='$(sudo cat "/mnt/hdd/app-data/tor/${APPID}/hostname" 2>/dev/null | tr -cd 'a-z0-9.')'"
    fi
  fi
  exit 0
fi

# echoing comments is useful for logs - output starts with # when not a key=value
echo "# Running: 'bonus.${APPID}.sh $*'"

#########################
# MENU
#########################

if [ "$1" = "menu" ]; then
  if ! [ -t 0 ] || ! [ -t 1 ]; then
    echo "# FAIL: the menu needs an interactive terminal"
    exit 1
  fi
  if [ "${friglet}" != "on" ] || [ ! -f "${UNIT_FILE}" ]; then
    whiptail --title " friglet " --msgbox "friglet is not switched on.\nUse SERVICES in the main menu to install it." 9 60
    exit 0
  fi
  localIP=""; toraddress=""; state=""; infoSync=""; serviceRunning=0
  electrumExposure=""; installedVersion=""
  # shellcheck disable=SC1090
  source <(sudo /home/admin/config.scripts/bonus.${APPID}.sh status showAddress)
  fingerprint=$(sudo openssl x509 -in /mnt/hdd/app-data/nginx/tls.cert -fingerprint -sha256 -noout 2>/dev/null | cut -d"=" -f2)

  showInfo() {
    local tmp
    tmp=$(umask 077 && mktemp) || return 1
    {
      echo "Open the friglet web UI in your browser (LAN):"
      echo "  https://${localIP}:${PORT_HTTPS}"
      echo "The certificate is self-signed. SHA-256 fingerprint:"
      echo "  ${fingerprint}"
      if [ -n "${toraddress}" ]; then
        echo "Tor Browser:"
        echo "  http://${toraddress}"
      fi
      echo
      printf "Web UI password: "
      sudo cat "${PASSWORD_FILE}"
      echo
      echo
      echo "Status: ${state:-unknown} (service running: ${serviceRunning})"
      if [ -n "${infoSync}" ]; then echo "  ${infoSync}"; fi
      echo "Electrum exposure: ${electrumExposure}"
      echo
      echo "Wallet setup and Electrum connection details are in the web UI."
    } > "${tmp}"
    whiptail --title " friglet ${installedVersion} " --scrolltext --textbox "${tmp}" 22 74
    rm -f "${tmp}"
  }

  OPTIONS=(
    INFO "Web UI address, password and status"
    PASSWORD "Reset the web UI password"
    EXPOSURE "Electrum exposure (now: ${electrumExposure})"
  )
  CHOICE=$(whiptail --title " friglet " --menu "" 12 64 3 "${OPTIONS[@]}" 2>&1 >/dev/tty)
  clear
  case "${CHOICE}" in
    INFO)
      showInfo
      ;;
    PASSWORD)
      if whiptail --title " Reset password " --yesno "Generate a new web UI password?\nAll open web UI sessions will be logged out." 9 60; then
        /home/admin/config.scripts/bonus.${APPID}.sh password-reset
        # shellcheck disable=SC1090
        source <(sudo /home/admin/config.scripts/bonus.${APPID}.sh status showAddress)
        showInfo
      fi
      ;;
    EXPOSURE)
      newMode=$(whiptail --title " Electrum exposure " --radiolist \
"Who can reach friglet's Electrum server?\nAnyone who reaches it can read this wallet's history." 16 74 4 \
        lan "LAN with TLS on ${PORT_SSL} (recommended) + Tor" "$([ "${electrumExposure}" = lan ] && echo ON || echo OFF)" \
        lan-tcp "also LAN plaintext on ${PORT_TCP}" "$([ "${electrumExposure}" = lan-tcp ] && echo ON || echo OFF)" \
        onion "Tor onion only" "$([ "${electrumExposure}" = onion ] && echo ON || echo OFF)" \
        local "this RaspiBlitz only (SSH tunnel)" "$([ "${electrumExposure}" = local ] && echo ON || echo OFF)" \
        2>&1 >/dev/tty)
      clear
      if [ -n "${newMode}" ] && [ "${newMode}" != "${electrumExposure}" ]; then
        /home/admin/config.scripts/bonus.${APPID}.sh electrum-exposure "${newMode}"
        echo "Press ENTER to get back to the main menu."
        read -r
      fi
      ;;
  esac
  exit 0
fi

#########################
# INSTALL (code only)
#########################

if [ "$1" = "install" ]; then
  installCode || exit 1
  exit 0
fi

#########################
# ON
#########################

if [ "$1" = "1" ] || [ "$1" = "on" ]; then
  echo "# switching ${APPID} on"

  if [ ! -d /mnt/hdd/app-data ] || [ ! -f "${BITCOIN_CONF}" ]; then
    echo "# FAIL: /mnt/hdd/app-data or ${BITCOIN_CONF} is missing"
    echo "error='data drive not ready'"
    exit 1
  fi

  network=$(frigletNetwork)
  prefix=$(chainPrefix)
  if [ -z "${network}" ] || [ -z "${prefix}" ]; then
    echo "# REFUSED: friglet supports chain=main and chain=sig on RaspiBlitz; this node runs chain=${chain} (testnet3 is not supported)."
    echo "error='unsupported chain ${chain}'"
    exit 1
  fi

  pruned=$(prunedSetting "${prefix}")
  if [ -n "${pruned}" ]; then
    prunedRefusal "${pruned}"
    exit 1
  fi

  installCode || exit 1

  echo "# app-data directory ${APP_DATA}"
  sudo install -d -m 0700 -o "${APP_USER}" -g "${APP_USER}" "${APP_DATA}"
  sudo chown -R "${APP_USER}:${APP_USER}" "${APP_DATA}"
  sudo chmod 0700 "${APP_DATA}"
  ensurePassword || { echo "# FAIL: could not create the web UI password"; echo "error='password'"; exit 1; }

  bitcoindRestart=$(ensureWhitebind "${prefix}") || { echo "error='whitebind port ${PORT_P2P} in use'"; exit 1; }

  if ! grep -q "^frigletElectrum=" /mnt/hdd/app-data/raspiblitz.conf 2>/dev/null; then
    /home/admin/config.scripts/blitz.conf.sh set frigletElectrum "lan"
    frigletElectrum="lan"
  fi
  mode=$(currentExposure)
  if [ "${mode}" = "onion" ] && [ "${runBehindTor}" != "on" ]; then
    echo "# Tor is off: Electrum exposure falls back from onion to local"
    mode="local"
  fi

  unitChanged=$(writeUnit "${mode}") || { echo "error='systemd unit'"; exit 1; }
  if [ "${unitChanged}" = "changed" ]; then
    echo "# systemd unit written"
    sudo systemctl daemon-reload
  fi

  nginxChanged=$(writeNginx) || { echo "error='nginx site'"; exit 1; }
  sudo ln -sf "${NGINX_SITE}" "${NGINX_LINK}"
  if ! sudo nginx -t >/dev/null 2>&1; then
    echo "# FAIL: nginx config test failed - removing the ${APPID} site again"
    sudo rm -f "${NGINX_LINK}" "${NGINX_SITE}"
    echo "error='nginx config test failed'"
    exit 1
  fi
  if [ "${nginxChanged}" = "changed" ]; then
    sudo systemctl reload nginx 2>/dev/null
    echo "# nginx HTTPS on port ${PORT_HTTPS}"
  fi

  applyFirewall "${mode}"
  applyTor "${mode}"
  refreshHostEnv

  /home/admin/config.scripts/blitz.conf.sh set ${APPID} "on"
  sudo systemctl enable "${APPID}" >/dev/null 2>&1
  echo "# OK - ${APPID}.service is enabled"

  # start only when the blitz is ready (provisioning re-runs 'on' before that)
  # shellcheck disable=SC1090
  source <(/home/admin/_cache.sh get state)
  if [ "${state}" = "ready" ]; then
    if [ "${bitcoindRestart}" = "restart" ]; then
      echo "# restarting bitcoind to activate the whitebind on port ${PORT_P2P}"
      sudo systemctl restart bitcoind
    fi
    if systemctl is-active --quiet "${APPID}"; then
      if [ "${unitChanged}" = "changed" ] || [ "${onionChanged}" = "1" ]; then
        sudo systemctl restart "${APPID}"
        echo "# OK - ${APPID}.service restarted"
      else
        echo "# OK - ${APPID}.service keeps running"
      fi
    else
      sudo systemctl start "${APPID}"
      echo "# OK - ${APPID}.service started"
    fi
  fi

  echo "# Open https://$(getLocalIP):${PORT_HTTPS} - the password is in: bonus.${APPID}.sh menu"
  echo "# Monitor with: sudo journalctl -f -u ${APPID}"
  echo "result='OK'"
  exit 0
fi

#########################
# ELECTRUM EXPOSURE
#########################

if [ "$1" = "electrum-exposure" ]; then
  newMode="$2"
  case " ${EXPOSURE_MODES} " in
    *" ${newMode} "*) ;;
    *) echo "# FAIL: mode must be one of: ${EXPOSURE_MODES}"; exit 1 ;;
  esac
  if [ "${newMode}" = "onion" ] && [ "${runBehindTor}" != "on" ]; then
    echo "# FAIL: Tor is off on this RaspiBlitz, so 'onion' is not available"
    exit 1
  fi
  /home/admin/config.scripts/blitz.conf.sh set frigletElectrum "${newMode}"
  if [ ! -f "${UNIT_FILE}" ]; then
    echo "# saved; applies when ${APPID} is switched on"
    exit 0
  fi
  unitChanged=$(writeUnit "${newMode}") || exit 1
  [ "${unitChanged}" = "changed" ] && sudo systemctl daemon-reload
  applyFirewall "${newMode}"
  applyTor "${newMode}"
  refreshHostEnv
  if systemctl is-active --quiet "${APPID}"; then
    sudo systemctl restart "${APPID}"
  fi
  echo "# OK - Electrum exposure is now: ${newMode}"
  echo "result='OK'"
  exit 0
fi

#########################
# PASSWORD RESET
#########################

if [ "$1" = "password-reset" ]; then
  if ! id "${APP_USER}" >/dev/null 2>&1 || ! sudo test -d "${APP_DATA}"; then
    echo "# FAIL: ${APPID} is not installed"
    exit 1
  fi
  newPassword || { echo "# FAIL: could not write a new password"; exit 1; }
  # hand the new password to friglet-ui on stdin (ends all sessions); restart as fallback
  # shellcheck disable=SC2016
  if sudo -u "${APP_USER}" sh -c 'exec env -i PATH=/usr/bin:/bin HOME="$2" FRIGLET_UI_PLATFORM=raspiblitz FRIGLET_UI_DATA_DIR="$3" "$2/current/friglet-ui" password set < "$1"' _ "${PASSWORD_FILE}" "${APP_HOME}" "${APP_DATA}" >/dev/null 2>&1; then
    echo "# OK - friglet-ui accepted the new password and ended all sessions"
  elif systemctl is-active --quiet "${APPID}"; then
    sudo systemctl restart "${APPID}"
    echo "# OK - ${APPID}.service restarted with the new password"
  fi
  echo "# new web UI password stored - show it with: bonus.${APPID}.sh menu"
  exit 0
fi

#########################
# UPDATE
#########################

if [ "$1" = "update" ]; then
  if [ "${friglet}" != "on" ]; then
    echo "# FAIL: ${APPID} is not switched on"
    exit 1
  fi
  if [ "$(installedVersion)" = "${VERSION}" ]; then
    echo "# friglet-node ${VERSION} is already installed (the version pinned in this script)"
    exit 0
  fi
  echo "# updating friglet-node $(installedVersion) -> ${VERSION}"
  installCode || exit 1
  if systemctl is-active --quiet "${APPID}"; then
    # friglet-ui stops friglet cleanly and backs up its state before a version change
    sudo systemctl restart "${APPID}"
    echo "# OK - ${APPID}.service restarted on ${VERSION}"
  fi
  echo "result='OK'"
  exit 0
fi

#########################
# OFF
#########################

if [ "$1" = "0" ] || [ "$1" = "off" ]; then
  echo "# switching ${APPID} off"
  sudo systemctl stop "${APPID}" 2>/dev/null
  sudo systemctl disable "${APPID}" >/dev/null 2>&1
  sudo rm -f "${UNIT_FILE}"
  sudo systemctl daemon-reload

  echo "# remove nginx site"
  sudo rm -f "${NGINX_LINK}" "${NGINX_SITE}"
  if sudo nginx -t >/dev/null 2>&1; then
    sudo systemctl reload nginx 2>/dev/null
  fi

  echo "# close firewall ports"
  ufwRules delete "${PORT_HTTPS}" "${PORT_TCP}" "${PORT_SSL}"

  echo "# remove Tor onion service (if any)"
  if sudo grep -q "# Hidden Service for ${APPID}\$" /mnt/hdd/app-data/tor/torrc.d/services 2>/dev/null; then
    /home/admin/config.scripts/tor.onion-service.sh off "${APPID}" >/dev/null 2>&1
  fi

  # the whitebind line goes; bitcoind keeps the port until its next restart (no restart here)
  removeWhitebind

  /home/admin/config.scripts/blitz.conf.sh set ${APPID} "off"

  echo "# remove user ${APP_USER} and the installed code"
  sudo userdel -rf "${APP_USER}" 2>/dev/null

  if [ "$(echo "$@" | grep -c delete-data)" -gt 0 ]; then
    echo "# delete-data: deleting ${APP_DATA} (wallet config, scan key, state, web UI password)"
    sudo rm -rf "${APP_DATA:?}"
  else
    echo "# kept ${APP_DATA} (wallet and web UI password are reused on the next 'on')"
  fi
  echo "# OK - ${APPID} is off"
  echo "result='OK'"
  exit 0
fi

echo "# FAIL - Unknown Parameter $1"
exit 1
