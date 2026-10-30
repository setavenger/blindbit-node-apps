#!/usr/bin/env bash
# Regtest end to end for friglet-node (make e2e).
#
#   e2e/run.sh                 friglet-node image, set up through `friglet-ui setup`
#   e2e/run.sh --friglet-only  the image's friglet alone, configured with a config file and
#                              key file (what friglet-ui would write), no friglet-ui involved
#   --keep                     leave the stacks running for inspection
#   --down                     remove the stacks a --keep run left behind
#
# Steps: bring up blindbit-testkit's bitcoind + oracle (pinned commit), generate a fresh
# silent-payment wallet, pay it once before friglet-node starts (historic scan) and once after
# (live), assert both payments are found over Electrum on port 50031 (silentpayments history
# and scripthash history), restart the container, and assert both are still found.
#
# Environment: IMAGE (default friglet-node:dev-<host arch>), E2E_WORKDIR (build/e2e),
# E2E_PROJECT (fnode-e2e), E2E_TESTKIT_SRC (git URL or local clone of the testkit),
# E2E_ORACLE_SRC (local blindbit-oracle clone for the oracle build), E2E_FIND_TIMEOUT (10m).
set -euo pipefail
LOG_TAG=e2e
# shellcheck source=scripts/lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/../scripts/lib.sh"
load_pins
# shellcheck source=e2e/lib.sh
. "$ROOT/e2e/lib.sh"

mode=full
keep=0
for a in "$@"; do
  case "$a" in
    --friglet-only) mode=friglet-only ;;
    --keep) keep=1 ;;
    --down) mode=down ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) die "unknown argument $a" ;;
  esac
done
IMAGE=${IMAGE:-friglet-node:dev-$(host_arch)}
set_project "${E2E_PROJECT:-fnode-e2e}"
FRIGLET_CTR=${E2E_PROJECT}-friglet
FRIGLET_VOL=${E2E_PROJECT}-friglet-data
UI_PASSWORD=unused
if [ "$mode" = down ]; then
  prepare_testkit
  node_compose down -v >/dev/null 2>&1 || true
  docker rm -f "$FRIGLET_CTR" >/dev/null 2>&1 || true
  docker volume rm "$FRIGLET_VOL" >/dev/null 2>&1 || true
  stack_down
  rm -f "$KEYS"
  log "removed project $E2E_PROJECT"
  exit 0
fi
docker image inspect "$IMAGE" >/dev/null 2>&1 || die "image $IMAGE not found (make image)"

cleanup() {
  local rc=$?
  if [ $rc -ne 0 ]; then
    log "FAILED (exit $rc); last friglet-node log lines:"
    docker logs --tail 60 "$NODE_CTR" 2>&1 | sed 's/^/  | /' >&2 || true
    docker logs --tail 60 "$FRIGLET_CTR" 2>&1 | sed 's/^/  | /' >&2 || true
  fi
  if [ "$keep" = 1 ]; then
    log "--keep: stacks left running (project $E2E_PROJECT); remove with: E2E_PROJECT=$E2E_PROJECT e2e/run.sh --down"
  else
    node_compose down -v >/dev/null 2>&1 || true
    docker rm -f "$FRIGLET_CTR" >/dev/null 2>&1 || true
    docker volume rm "$FRIGLET_VOL" >/dev/null 2>&1 || true
    stack_down
    rm -f "$KEYS"
  fi
  exit $rc
}
trap cleanup EXIT

start_full() {
  ui_password
  node_compose up -d >&2
  setup_wallet "$BIRTHDAY"
  wait_healthy "$NODE_CTR" 600
}

# friglet alone: the packaged binary, a config file and a 0600 key file in its volume.
start_friglet_only() {
  local node_ip spend_pub
  node_ip=$(docker inspect -f "{{(index .NetworkSettings.Networks \"$E2E_NETWORK\").IPAddress}}" "${E2E_PROJECT}-bitcoind-1")
  [ -n "$node_ip" ] || die "no bitcoind address on $E2E_NETWORK"
  spend_pub=$(sptool get -keys "$KEYS" spend_pubkey)
  docker volume create "$FRIGLET_VOL" >/dev/null
  # The scan secret goes through stdin into the key file; it is never an argument or variable
  # of a container.
  sptool get -keys "$KEYS" scan_secret | docker run --rm -i -u 0 -v "$FRIGLET_VOL:/data" \
    --entrypoint /bin/sh "$IMAGE" -c "umask 077; head -c 64 > /data/scan.key; cat > /data/config.toml <<CFG
network = \"regtest\"
spend_pubkey = \"$spend_pub\"
start_height = $BIRTHDAY
p2p_node_addr = \"$node_ip:18444\"
oracle_url = \"http://oracle:7001\"
electrum_addr = \"0.0.0.0:50031\"
state_file = \"/data/scanner_state.json\"
control_socket = \"/data/friglet.sock\"
key_file = \"/data/scan.key\"
CFG
chown -R 1000:1000 /data"
  docker run -d --name "$FRIGLET_CTR" --network "$E2E_NETWORK" --network-alias friglet-node \
    -v "$FRIGLET_VOL:/data" --entrypoint /usr/local/bin/friglet "$IMAGE" \
    scan --config /data/config.toml >/dev/null
  log "friglet (from $IMAGE) started: start_height $BIRTHDAY, peer $node_ip:18444"
}

restart_node() {
  if [ "$mode" = full ]; then
    node_compose restart >&2
    wait_healthy "$NODE_CTR" 600
  else
    docker restart "$FRIGLET_CTR" >/dev/null
  fi
}

prepare_testkit
build_sptool
stack_down
stack_up
new_keys
BIRTHDAY=$(tip)
log "birthday $BIRTHDAY"

pay 100000
TX1=$PAY_TXID H1=$PAY_HEIGHT

if [ "$mode" = full ]; then start_full; else start_friglet_only; fi
log "payment 1 (before start, historic scan): waiting for it over Electrum"
electrum_check "$TX1"

pay 250000
TX2=$PAY_TXID H2=$PAY_HEIGHT
log "payment 2 (while running, live): waiting for it over Electrum"
electrum_check "$TX2"
if [ "$mode" = full ]; then
  sptool widget-check -url http://friglet-node:8097/api/widget -min-height "$H2" -timeout 5m >&2
fi

log "restarting the container"
restart_node
electrum_check "$TX1"
electrum_check "$TX2"
log "PASS ($mode): $TX1 (height $H1) and $TX2 (height $H2) found over Electrum on 50031, before and after a restart"
