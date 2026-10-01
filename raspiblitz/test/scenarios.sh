#!/bin/bash
# bonus.friglet.sh scenarios. Runs as root inside the container that run.sh builds; calls the
# bonus script as the admin user, like RaspiBlitz does. Prints PASS/FAIL per check.
# shellcheck disable=SC2024 # root writes the logs on purpose
set -u
v1="$1"
v2="$2"

S=/home/admin/config.scripts/bonus.friglet.sh
DATA=/mnt/hdd/app-data/friglet
PW="${DATA}/ui-password"
CONF=/mnt/hdd/app-data/bitcoin/bitcoin.conf
RBCONF=/mnt/hdd/app-data/raspiblitz.conf
UNIT=/etc/systemd/system/friglet.service
SITE=/etc/nginx/sites-available/friglet_ssl.conf
TORSVC=/mnt/hdd/app-data/tor/torrc.d/services
RULES=/var/lib/ufw-stub/rules
STARTS=/var/log/fake-bitcoind.starts
out=/opt/test/out
secrets=/opt/test/secrets
mkdir -p "${out}"
: > "${secrets}"
chmod 600 "${secrets}"
pass=0
fail=0
n=0
last=""

ok() { pass=$((pass + 1)); echo "PASS $*"; }
ko() { fail=$((fail + 1)); echo "FAIL $*"; }
check() { local d="$1"; shift; if "$@" >/dev/null 2>&1; then ok "${d}"; else ko "${d}"; fi; }
waitfor() { local i; for i in $(seq 1 40); do "$@" >/dev/null 2>&1 && return 0; sleep 0.5; done; return 1; }

use() { install -m 755 -o admin -g admin "/opt/test/scripts/$1" "${S}"; }
# run the bonus script as admin without a terminal; stdout+stderr go to a numbered log
rb() {
  n=$((n + 1))
  last=$(printf '%s/%02d-%s.log' "${out}" "${n}" "$(echo "$*" | tr ' /' '__')")
  sudo -u admin -H "${S}" "$@" < /dev/null > "${last}" 2>&1
}
logHas() { grep -qF -- "$1" "${last}"; }
remember() { cat "${PW}" >> "${secrets}" 2>/dev/null; echo >> "${secrets}"; }
uiPid() { pgrep -u friglet -f 'friglet-ui run' | head -n1; }
starts() { wc -l < "${STARTS}"; }
rules() { sort "${RULES}" 2>/dev/null | tr '\n' ';'; }
ruleCount() { grep -c . "${RULES}" 2>/dev/null || true; }
listening() { ss -Hltn "sport = :$2" | awk '{print $4}' | grep -qx "$1:$2"; }
notListening() { ! ss -Hltn "sport = :$1" | grep -q .; }
walletHash() { (cd "${DATA}" && sha256sum config.toml scan.key scanner_state.json ui-password 2>/dev/null) | sha256sum | cut -c1-16; }
envIs() { [ "$(jq -r ".$1" "${DATA}/stub-env.json")" = "$2" ]; }
torBlock() { sed -n '/^# Hidden Service for friglet$/,/^[[:space:]]*$/p' "${TORSVC}" | grep '^HiddenServicePort' | sort | tr '\n' ';'; }
fresh() {
  install -m 640 -o bitcoin -g bitcoin /opt/rb/bitcoin.conf "${CONF}"
  printf 'network=bitcoin\nchain=main\nrunBehindTor=on\nlightning=lnd\n' > "${RBCONF}"
  chown admin:admin "${RBCONF}"
  echo ready > /var/cache/stub/state
}

expectedRefusal() { # setting
  cat <<EOF
# REFUSED: friglet needs an unpruned Bitcoin node.
# Problem: this node is pruned ($1 in /mnt/hdd/app-data/bitcoin/bitcoin.conf).
# Cause: friglet downloads every block from your wallet birthday onward from this node, and a pruned node has deleted old blocks.
# Fix: remove the prune setting (or set prune=0), restart bitcoind and let it download the full chain again, then switch friglet on again.
# Help: blindbit-node-apps docs/troubleshooting.md#node-pruned
error='node is pruned ($1), friglet needs an unpruned node'
EOF
}

echo "## setup"
fresh
systemctl restart bitcoind >/dev/null 2>&1
waitfor systemctl is-active nginx || systemctl start nginx
check "nginx running with the RaspiBlitz snippets" systemctl is-active nginx
check "fake bitcoind running" systemctl is-active bitcoind

echo "## release pinning"
use shipped
rb install; rc=$?
check "shipped script (no pinned SHA256) refuses install" test "${rc}" -ne 0
check "refusal names the missing pin" logHas "REFUSED: no pinned SHA256 for friglet-node-${v1}-linux-amd64.tar.gz"
check "nothing installed without a pin" bash -c '! id friglet'
rb on; rc=$?
check "shipped script refuses on" test "${rc}" -ne 0
check "no unit written without a pin" test ! -e "${UNIT}"
use badsum
rb install; rc=$?
check "checksum mismatch refuses install" test "${rc}" -ne 0
check "mismatch is reported" logHas "REFUSED: checksum mismatch"
check "no code unpacked on mismatch" test ! -e "/home/friglet/friglet-node-${v1}"
check "no user created on mismatch" bash -c '! id friglet' 

echo "## prune refusal"
use pinned
for setting in "prune=550" "main.prune=550" "[main]|prune=1"; do
  fresh
  printf '%s\n' "${setting//|/$'\n'}" >> "${CONF}"
  shown="${setting#*|}"
  rb on; rc=$?
  check "pruned node (${setting}) refuses on" test "${rc}" -ne 0
  if diff <(expectedRefusal "${shown}") <(grep -E "^# (REFUSED|Problem|Cause|Fix|Help)|^error=" "${last}") >/dev/null; then
    ok "exact refusal text for ${setting}"
  else
    ko "exact refusal text for ${setting}"; cat "${last}"
  fi
  check "nothing installed on a pruned node (${setting})" bash -c "! id friglet && test ! -e ${UNIT}"
done
fresh
sed -i 's/^chain=main/chain=test/' "${RBCONF}"
rb on; rc=$?
check "chain=test (testnet3) refuses on" test "${rc}" -ne 0
check "testnet3 refusal text" logHas "testnet3 is not supported"
fresh

echo "## on (first install)"
echo "signet.prune=550" >> "${CONF}"   # another chain's prune setting must not block mainnet
s0=$(starts)
rb on; rc=$?
check "on succeeds" test "${rc}" -eq 0
check "on reports result='OK'" logHas "result='OK'"
remember
check "user friglet exists" id friglet
check "app-data dir is 0700 friglet" test "$(stat -c '%a %U' "${DATA}")" = "700 friglet"
check "password file is 0600 friglet" test "$(stat -c '%a %U' "${PW}")" = "600 friglet"
check "password is 24 chars of the unambiguous alphabet" grep -qxE '[A-HJ-NP-Za-km-z2-9]{24}' "${PW}"
check "current -> friglet-node-${v1}" test "$(readlink /home/friglet/current)" = "friglet-node-${v1}"
check "binaries are root-owned 0755" test "$(stat -c '%a %U' "/home/friglet/friglet-node-${v1}/friglet-ui" "/home/friglet/friglet-node-${v1}/friglet" | sort -u)" = "755 root"
for line in "User=friglet" "ExecStartPre=-/home/admin/config.scripts/bonus.friglet.sh prestart" \
  "ExecStart=/home/friglet/current/friglet-ui run" "Environment=FRIGLET_UI_PLATFORM=raspiblitz" \
  "Environment=FRIGLET_UI_AUTH=password" "Environment=FRIGLET_UI_PASSWORD_FILE=${PW}" \
  "Environment=FRIGLET_UI_NETWORK=bitcoin" "Environment=FRIGLET_UI_NODE=127.0.0.1:8337" \
  "Environment=FRIGLET_UI_LISTEN=127.0.0.1:8097" "Environment=FRIGLET_UI_DATA_DIR=${DATA}" \
  "TimeoutStopSec=150" "KillMode=mixed" "UMask=0077"; do
  check "unit has ${line}" grep -qxF "${line}" "${UNIT}"
done
check "friglet service active" waitfor systemctl is-active friglet
check "stub friglet-ui recorded its environment" waitfor test -s "${DATA}/stub-env.json"
check "env PLATFORM=raspiblitz" envIs FRIGLET_UI_PLATFORM raspiblitz
check "env AUTH=password" envIs FRIGLET_UI_AUTH password
check "env NETWORK=bitcoin" envIs FRIGLET_UI_NETWORK bitcoin
check "env NODE=127.0.0.1:8337" envIs FRIGLET_UI_NODE 127.0.0.1:8337
check "env LISTEN=127.0.0.1:8097" envIs FRIGLET_UI_LISTEN 127.0.0.1:8097
check "env PUBLIC_HOST=local IP" envIs FRIGLET_UI_PUBLIC_HOST "$(hostname -I | awk '{print $1}')"
check "env ONION_HOST=onion hostname" envIs FRIGLET_UI_ONION_HOST "$(cat /mnt/hdd/app-data/tor/friglet/hostname)"
check "env has no password value" bash -c "! grep -qF \"\$(cat ${PW})\" ${DATA}/stub-env.json"
check "friglet-ui runs as friglet" envIs _uid "$(id -u friglet)"
check "friglet-ui read the password file" test "$(cat "${DATA}/stub-ui-password.sha256")" = "$(tr -d '\n' < "${PW}" | sha256sum | cut -d' ' -f1)"
m0=$(stat -c %Y /home/friglet/friglet-ui.env); sleep 1.1
systemctl restart friglet >/dev/null 2>&1
check "prestart (as friglet) rewrites the host env on restart" waitfor bash -c "systemctl is-active friglet && test \$(stat -c %Y /home/friglet/friglet-ui.env) -gt ${m0}"
check "host env file is 0600 friglet" test "$(stat -c '%a %U' /home/friglet/friglet-ui.env)" = "600 friglet"
check "one main.whitebind=download@127.0.0.1:8337" test "$(grep -c '^main.whitebind=download@127.0.0.1:8337$' "${CONF}")" -eq 1
check "bitcoind restarted exactly once" test "$(starts)" -eq $((s0 + 1))
check "bitcoind listens on 127.0.0.1:8337" waitfor listening 127.0.0.1 8337
check "nginx -t passes" nginx -t
check "HTTPS 8098 proxies to friglet-ui" waitfor bash -c "curl -sk https://127.0.0.1:8098/api/widget | jq -e '.state == \"scanning\"'"
check "ufw: 8098 and 50032 from the 3 LAN ranges only" test "$(rules)" = "10.0.0.0/8 50032 tcp;10.0.0.0/8 8098 tcp;172.16.0.0/12 50032 tcp;172.16.0.0/12 8098 tcp;192.168.0.0/16 50032 tcp;192.168.0.0/16 8098 tcp;"
check "Tor onion: 50031 and UI on 80" test "$(torBlock)" = "HiddenServicePort 50031 127.0.0.1:50031;HiddenServicePort 80 127.0.0.1:8097;"
check "raspiblitz.conf friglet=on" grep -qx "friglet=on" "${RBCONF}"
check "raspiblitz.conf frigletElectrum='lan'" grep -qx "frigletElectrum='lan'" "${RBCONF}"
check "Electrum TCP on 127.0.0.1:50031" waitfor listening 127.0.0.1 50031
check "Electrum TLS on 0.0.0.0:50032" listening 0.0.0.0 50032
check "UI on 127.0.0.1:8097 only" listening 127.0.0.1 8097

echo "## status"
sudo -u admin -H "${S}" status > "${out}/status.out" 2> "${out}/status.err"; rc=$?
check "status exits 0" test "${rc}" -eq 0
check "status writes nothing to stderr" test ! -s "${out}/status.err"
check "status prints only key=value or # lines" bash -c "! grep -vE '^(#|[A-Za-z]+=)' ${out}/status.out"
check "status installed=1" grep -qx "installed=1" "${out}/status.out"
check "status serviceRunning=1" grep -qx "serviceRunning=1" "${out}/status.out"
check "status state='scanning'" grep -qx "state='scanning'" "${out}/status.out"
check "status installedVersion='${v1}'" grep -qx "installedVersion='${v1}'" "${out}/status.out"
check "status httpsPort='8098'" grep -qx "httpsPort='8098'" "${out}/status.out"
check "status hides the onion without showAddress" bash -c "! grep -q onion ${out}/status.out"
sudo "${S}" status showAddress > "${out}/status-show.out" 2>&1
check "status showAddress gives the onion" grep -qx "toraddress='$(cat /mnt/hdd/app-data/tor/friglet/hostname)'" "${out}/status-show.out"
sudo -u admin -H "${S}" status-sync > "${out}/status-sync.out" 2> "${out}/status-sync.err"
check "status-sync infoSync" grep -qx "infoSync='scanning block 850000 of 900000'" "${out}/status-sync.out"
check "status-sync writes nothing to stderr" test ! -s "${out}/status-sync.err"

echo "## repeated non-interactive on keeps wallet and password"
sudo -u friglet sh -c "umask 077; openssl rand -hex 32 > ${DATA}/scan.key; printf 'network = \"bitcoin\"\nstart_height = 840000\n' > ${DATA}/config.toml; echo '{}' > ${DATA}/scanner_state.json"
cat "${DATA}/scan.key" >> "${secrets}"
w0=$(walletHash); p0=$(uiPid); s0=$(starts)
u0=$(sha256sum "${UNIT}" "${SITE}" "${TORSVC}" | sha256sum); r0=$(rules)
for i in 1 2; do
  rb on; rc=$?
  check "repeat ${i}: on succeeds" test "${rc}" -eq 0
  check "repeat ${i}: keeps the password" logHas "keep existing web UI password"
  check "repeat ${i}: wallet files and password unchanged" test "$(walletHash)" = "${w0}"
  check "repeat ${i}: friglet-ui not restarted" test "$(uiPid)" = "${p0}"
  check "repeat ${i}: bitcoind not restarted" test "$(starts)" -eq "${s0}"
  check "repeat ${i}: unit, nginx site and torrc unchanged" test "$(sha256sum "${UNIT}" "${SITE}" "${TORSVC}" | sha256sum)" = "${u0}"
  check "repeat ${i}: ufw rules unchanged" test "$(rules)" = "${r0}"
  check "repeat ${i}: still one whitebind line" test "$(grep -c 'whitebind=download@127.0.0.1:8337' "${CONF}")" -eq 1
done

echo "## SD card update: provisioning re-runs on before the node is ready"
systemctl stop friglet >/dev/null 2>&1
userdel -rf friglet >/dev/null 2>&1
rm -f "${UNIT}" "${SITE}" /etc/nginx/sites-enabled/friglet_ssl.conf "${RULES}"
echo provision > /var/cache/stub/state
rb on; rc=$?
check "provisioning on succeeds" test "${rc}" -eq 0
check "provisioning on does not start the service" bash -c "! systemctl is-active friglet"
check "provisioning keeps wallet and password" test "$(walletHash)" = "${w0}"
check "data re-owned by the new friglet user" test "$(stat -c '%U' "${DATA}/scan.key")" = "friglet"
echo ready > /var/cache/stub/state
systemctl start friglet >/dev/null 2>&1
check "service starts after provisioning" waitfor systemctl is-active friglet
check "friglet-ui uses the kept password" waitfor bash -c "test \"\$(cat ${DATA}/stub-ui-password.sha256)\" = \"\$(tr -d '\n' < ${PW} | sha256sum | cut -d' ' -f1)\""

echo "## Electrum exposure"
rb electrum-exposure lan-tcp; rc=$?
check "lan-tcp accepted" test "${rc}" -eq 0
check "lan-tcp: ufw opens 8098, 50031, 50032 to the LAN" test "$(ruleCount)" -eq 9
check "lan-tcp: Electrum TCP on 0.0.0.0:50031" waitfor listening 0.0.0.0 50031
check "lan-tcp saved" grep -qx "frigletElectrum='lan-tcp'" "${RBCONF}"
rb electrum-exposure onion; rc=$?
check "onion accepted" test "${rc}" -eq 0
check "onion: ufw keeps only 8098" test "$(ruleCount)" -eq 3
check "onion: TLS listener off" waitfor notListening 50032
check "onion: TCP on 127.0.0.1:50031" waitfor listening 127.0.0.1 50031
check "onion: Tor maps 50031" test "$(torBlock)" = "HiddenServicePort 50031 127.0.0.1:50031;HiddenServicePort 80 127.0.0.1:8097;"
rb electrum-exposure local; rc=$?
check "local accepted" test "${rc}" -eq 0
check "local: TLS on 127.0.0.1:50032" waitfor listening 127.0.0.1 50032
check "local: Tor maps only the UI" test "$(torBlock)" = "HiddenServicePort 80 127.0.0.1:8097;"
rb electrum-exposure everyone; rc=$?
check "unknown mode refused" test "${rc}" -ne 0
sed -i 's/^runBehindTor=on/runBehindTor=off/' "${RBCONF}"
rb electrum-exposure onion; rc=$?
check "onion refused while Tor is off" test "${rc}" -ne 0
sed -i 's/^runBehindTor=off/runBehindTor=on/' "${RBCONF}"
rb electrum-exposure lan; rc=$?
check "back to lan" test "${rc}" -eq 0
check "lan: 6 ufw rules again" test "$(ruleCount)" -eq 6
check "lan: TLS on 0.0.0.0:50032" waitfor listening 0.0.0.0 50032
check "lan: TCP back on loopback" waitfor listening 127.0.0.1 50031

echo "## password reset"
old=$(cat "${PW}")
rb password-reset; rc=$?
remember
check "password-reset succeeds" test "${rc}" -eq 0
check "password changed" test "$(cat "${PW}")" != "${old}"
check "new password is 0600 friglet" test "$(stat -c '%a %U' "${PW}")" = "600 friglet"
check "friglet-ui got the new password on stdin (password set)" test "$(cat "${DATA}/stub-ui-password.sha256")" = "$(tr -d '\n' < "${PW}" | sha256sum | cut -d' ' -f1)"
check "sessions ended" test -s "${DATA}/stub-sessions-ended"
w0=$(walletHash)

echo "## SSH menu"
rb menu; rc=$?
check "menu without a terminal refuses" test "${rc}" -ne 0
check "menu refusal text" logHas "the menu needs an interactive terminal"
# Enter picks INFO; the text box then closes with Tab+Enter (or ESC); timeout guards a stuck dialog
(sleep 4; printf '\r'; sleep 4; printf '\t\r'; sleep 2; printf '\033'; sleep 3) \
  | timeout 40 script -qec "sudo -u admin -H TERM=xterm ${S} menu" /dev/null > /opt/test/menu-tty.log 2>&1
check "menu dialog closes" test "$?" -eq 0
check "menu INFO shows the HTTPS URL" grep -qF "https://$(hostname -I | awk '{print $1}'):8098" /opt/test/menu-tty.log
check "menu INFO shows the password" grep -qF "$(cat "${PW}")" /opt/test/menu-tty.log
check "menu INFO shows the status" grep -qF "Status: scanning" /opt/test/menu-tty.log

echo "## update"
p0=$(uiPid)
rb update; rc=$?
check "update on the pinned version is a no-op" bash -c "test ${rc} -eq 0 && grep -qF 'friglet-node ${v1} is already installed (the version pinned' ${last}"
check "no-op update does not restart friglet-ui" test "$(uiPid)" = "${p0}"
use pinned-v2
p0=$(uiPid)
rb update; rc=$?
check "update to ${v2} succeeds" test "${rc}" -eq 0
check "current -> friglet-node-${v2}" test "$(readlink /home/friglet/current)" = "friglet-node-${v2}"
check "previous version kept for rollback" test -x "/home/friglet/friglet-node-${v1}/friglet-ui"
check "service restarted on the new version" waitfor bash -c "systemctl is-active friglet && test \"\$(pgrep -u friglet -f 'friglet-ui run' | head -n1)\" != '${p0}'"
check "update keeps wallet and password" test "$(walletHash)" = "${w0}"

echo "## off keeps data"
s0=$(starts)
rb off; rc=$?
check "off succeeds" test "${rc}" -eq 0
check "service stopped" bash -c "! systemctl is-active friglet"
check "unit removed" test ! -e "${UNIT}"
check "nginx site removed" test ! -e "${SITE}" -a ! -e /etc/nginx/sites-enabled/friglet_ssl.conf
check "nginx -t still passes" nginx -t
check "ufw rules removed" test "$(ruleCount)" -eq 0
check "Tor onion removed" test -z "$(torBlock)"
check "whitebind line removed" bash -c "! grep -q 'whitebind=download@127.0.0.1:8337' ${CONF}"
check "bitcoind not restarted by off" test "$(starts)" -eq "${s0}"
check "raspiblitz.conf friglet=off" grep -qx "friglet=off" "${RBCONF}"
check "user and code removed" bash -c "! id friglet && test ! -e /home/friglet"
check "wallet and password kept" test "$(walletHash)" = "${w0}"
sudo -u admin -H "${S}" status > "${out}/status-off.out" 2> "${out}/status-off.err"
check "status after off: installed=0, no stderr" bash -c "grep -qx installed=0 ${out}/status-off.out && test ! -s ${out}/status-off.err"

echo "## on again reuses the wallet, no bitcoind restart while the port is still bound"
rb on; rc=$?
check "on after off succeeds" test "${rc}" -eq 0
check "wallet and password reused" test "$(walletHash)" = "${w0}"
check "no bitcoind restart (port still bound)" test "$(starts)" -eq "${s0}"
check "service active again" waitfor systemctl is-active friglet

echo "## off --delete-data"
rb off --delete-data; rc=$?
check "off --delete-data succeeds" test "${rc}" -eq 0
check "data dir deleted" test ! -e "${DATA}"

echo "## secrets never reach script output or service logs"
leaks=0
while IFS= read -r secret; do
  [ -n "${secret}" ] || continue
  if grep -rqF -- "${secret}" "${out}" /var/log/journal 2>/dev/null; then leaks=$((leaks + 1)); fi
done < "${secrets}"
check "no password or scan key in any script output or unit log ($(grep -c . "${secrets}") secrets checked)" test "${leaks}" -eq 0
check "bitcoin.conf rpcpassword never printed" bash -c "! grep -rqF 'rpcpassword' ${out}"
check "script never uses tee" bash -c "! grep -n 'tee' ${S}"

echo
echo "RESULT: ${pass} passed, ${fail} failed"
[ "${fail}" -eq 0 ]
