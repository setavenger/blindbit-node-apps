#!/usr/bin/env bash
# Manual smoke test of friglet-ui against a real friglet on signet (network required).
#
#   SMOKE_FRIGLET=/path/to/friglet SMOKE_UI=/path/to/friglet-ui \
#   SMOKE_GEN=/path/to/examples/gen_test_descriptor WORK=/short/dir \
#   friglet-ui/scripts/signet-smoke.sh
#
# (Not FRIGLET_*: friglet-ui refuses to start with any FRIGLET_* variable but FRIGLET_UI_*.)
#
# WORK must be short (Unix socket paths are limited to ~104 bytes). The descriptor is a
# throwaway test wallet generated here; it is never printed. Every check prints PASS/FAIL.
set -u

: "${SMOKE_FRIGLET:?path to the friglet binary}"
: "${SMOKE_UI:?path to the friglet-ui binary}"
: "${SMOKE_GEN:?path to the gen_test_descriptor example binary}"
WORK=${WORK:-/tmp/fu-smoke}
NODE=${NODE:-152.53.151.148:38333}
PORT=${PORT:-18097}
TCP=${TCP:-50031}
TLS=${TLS:-50032}
D=$WORK/data
LOG=$WORK/friglet-ui.log
JAR=$WORK/cookies
BASE=http://127.0.0.1:$PORT
PW=smoke-$RANDOM-$RANDOM
FAILS=0

pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILS=$((FAILS + 1)); }
check() { local what=$1; shift; if "$@"; then pass "$what"; else fail "$what"; fi; }

rm -rf "$WORK"; mkdir -p "$WORK"; chmod 700 "$WORK"
export FRIGLET_UI_DATA_DIR=$D FRIGLET_UI_NETWORK=signet FRIGLET_UI_NODE=$NODE \
  FRIGLET_UI_LISTEN=127.0.0.1:$PORT FRIGLET_UI_ELECTRUM_TCP=127.0.0.1:$TCP \
  FRIGLET_UI_ELECTRUM_TLS=127.0.0.1:$TLS FRIGLET_UI_FRIGLET_BIN=$SMOKE_FRIGLET \
  FRIGLET_UI_PLATFORM=compose

csrf() { curl -s -c "$JAR" -b "$JAR" "$BASE/api/session" | python3 -c 'import sys,json; print(json.load(sys.stdin)["csrf_token"])'; }
post() { local t; t=$(csrf); curl -s -c "$JAR" -b "$JAR" -H "X-CSRF-Token: $t" -H "Origin: $BASE" -H 'Content-Type: application/json' -X POST "$BASE$1" ${2:+-d "$2"} -o /dev/null -w '%{http_code}'; }
login() { post /api/login "{\"password\":\"$PW\"}" >/dev/null; }
field() { curl -s -b "$JAR" "$BASE/api/state" | python3 -c "import sys,json; d=json.load(sys.stdin); print(eval(sys.argv[1]))" "$1" 2>/dev/null; }
wait_for() { # expr expected timeout_s
  local i; for ((i = 0; i < $3 * 2; i++)); do [ "$(field "$1")" = "$2" ] && return 0; sleep 0.5; done; return 1; }
friglet_pid() { pgrep -P "$(cat "$WORK/pid")" -f "$(basename "$SMOKE_FRIGLET")"; }
start_daemon() {
  FRIGLET_UI_PASSWORD=$PW "$SMOKE_UI" >>"$LOG" 2>&1 &
  echo $! >"$WORK/pid"
  local i; for ((i = 0; i < 120; i++)); do curl -sf "$BASE/healthz" >/dev/null && break; sleep 0.5; done
  login
}
wait_friglet() { local i; for ((i = 0; i < 120; i++)); do friglet_pid >/dev/null && return 0; sleep 0.5; done; return 1; }

"$SMOKE_GEN" "$WORK/desc" signet >"$WORK/expected"
EXPECTED=$(sed -n 's/^sp_address=//p' "$WORK/expected")
TIP=$(curl -s https://mempool.space/signet/api/blocks/tip/height)
BIRTHDAY=$((TIP - 400))
echo "signet tip $TIP, birthday $BIRTHDAY"

out=$("$SMOKE_UI" setup --birthday "$BIRTHDAY" --descriptor-file "$WORK/desc" --node "$NODE" --json)
echo "setup (offline): $out"
check "offline setup is staged" grep -q '"state":"staged"' <<<"$out"

start_daemon
check "friglet started and supervised" wait_friglet
check "state reaches scanning or up-to-date" wait_for "d['state']['id'] in ('scanning','up-to-date')" True 120
echo "  $(field "d['state']['id'], d['heights']")"
check "identity verified (friglet's sp_address = descriptor's)" wait_for "d['wallet']['identity_verified'] and d['wallet']['reported_sp_address'] == '$EXPECTED'" True 30
check "state reaches up-to-date" wait_for "d['state']['id']" up-to-date 900
echo "  $(field "d['state']['id'], d['heights']")"

ev='{"jsonrpc":"2.0","id":0,"method":"server.version","params":["smoke","1.4"]}'
check "Electrum server.version over TCP $TCP" bash -c "printf '%s\n' '$ev' | timeout 5 nc -q 2 127.0.0.1 $TCP | grep -q result"
check "Electrum server.version over TLS $TLS" bash -c "printf '%s\n' '$ev' | timeout 5 openssl s_client -connect 127.0.0.1:$TLS -quiet 2>/dev/null | grep -q result"

old=$(friglet_pid); kill -9 "$old"
check "kill -9 of friglet triggers a restart" wait_for "d['restarts'] >= 1 and d['state']['id'] in ('up-to-date','rescanning-after-reset','scanning')" True 60
new=$(friglet_pid); check "new friglet process ($old -> $new)" test -n "$new" -a "$new" != "$old"

fp=$(friglet_pid); start=$(date +%s.%N); kill -TERM "$(cat "$WORK/pid")"
while kill -0 "$fp" 2>/dev/null; do sleep 0.1; done; secs=$(python3 -c "import time; print(round(time.time() - $start, 1))")
check "SIGTERM to friglet-ui stops friglet within 120 s (${secs} s)" python3 -c "import sys; sys.exit(0 if $secs < 120 else 1)"
wait "$(cat "$WORK/pid")" 2>/dev/null

start_daemon
wait_for "d['state']['id']" up-to-date 120
SECRET=$(cat "$D/scan.key")
code=$(post /api/wallet/forget)
check "forget answers 202" test "$code" = 202
check "no scan secret left in the data dir" bash -c "! grep -rq '$SECRET' '$D'"
check "no scan secret in the logs" bash -c "! grep -q '$SECRET' '$LOG'"
kill -TERM "$(cat "$WORK/pid")"; wait "$(cat "$WORK/pid")" 2>/dev/null
check "self-test passes with an empty environment" bash -c "env -i '$SMOKE_UI' self-test >/dev/null"

echo "$FAILS failure(s); log: $LOG"
exit $((FAILS > 0))
