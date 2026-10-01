#!/usr/bin/env bash
# Local regtest playground (make dev-regtest): blindbit-testkit's bitcoind + oracle, the local
# friglet-node image set up with a freshly generated wallet, and one silent payment already
# found. Prints the web UI URL, the password file and the Electrum endpoint.
#
#   e2e/dev-regtest.sh up      start (or re-use) the stack; idempotent
#   e2e/dev-regtest.sh pay     send another payment (AMOUNT sats, default 50000) and mine it
#   e2e/dev-regtest.sh mine N  mine N blocks to the node wallet
#   e2e/dev-regtest.sh down    remove everything, volumes included
#
# Environment: IMAGE (friglet-node:dev-<host arch>), UI_PORT (8097), ELECTRUM_PORT (50031),
# ELECTRUM_TLS_PORT (50032), E2E_PROJECT (fnode-dev), E2E_WORKDIR (build/dev-regtest).
set -euo pipefail
LOG_TAG=dev-regtest
# shellcheck source=scripts/lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/../scripts/lib.sh"
load_pins
E2E_WORKDIR=${E2E_WORKDIR:-$ROOT/build/dev-regtest}
UI_PORT=${UI_PORT:-8097} ELECTRUM_PORT=${ELECTRUM_PORT:-50031} ELECTRUM_TLS_PORT=${ELECTRUM_TLS_PORT:-50032}
# shellcheck source=e2e/lib.sh
. "$ROOT/e2e/lib.sh"

IMAGE=${IMAGE:-friglet-node:dev-$(host_arch)}
set_project "${E2E_PROJECT:-fnode-dev}"
ui_password

case "${1:-up}" in
  up)
    docker image inspect "$IMAGE" >/dev/null 2>&1 || die "image $IMAGE not found (make image)"
    prepare_testkit
    build_sptool
    stack_up
    node_compose up -d >&2
    if [ ! -s "$KEYS" ]; then
      new_keys
      birthday=$(tip)
      setup_wallet "$birthday"
      wait_healthy "$NODE_CTR" 600
      pay 100000
      electrum_check "$PAY_TXID" >/dev/null
    else
      ADDRESS=$(sptool get -keys "$KEYS" address)
      wait_healthy "$NODE_CTR" 600
    fi
    cat <<MSG

  friglet-node regtest playground is up (project $E2E_PROJECT, image $IMAGE)

  Web UI      http://127.0.0.1:$UI_PORT/
  Password    $E2E_WORKDIR/ui-password
  Electrum    127.0.0.1:$ELECTRUM_PORT (tcp), 127.0.0.1:$ELECTRUM_TLS_PORT (ssl)
  Wallet      $ADDRESS
  Descriptor  $KEYS (field "descriptor"; regtest throwaway keys, mode 0600)

  More payments: e2e/dev-regtest.sh pay [AMOUNT]    Stop: make dev-regtest-down
MSG
    ;;
  pay)
    prepare_testkit
    ADDRESS=$(sptool get -keys "$KEYS" address)
    pay "${2:-50000}"
    ;;
  mine)
    prepare_testkit
    stack mine "${2:-1}"
    ;;
  down)
    prepare_testkit
    node_compose down -v >&2 || true
    stack_down
    rm -f "$KEYS" "$E2E_WORKDIR/ui-password"
    log "removed project $E2E_PROJECT"
    ;;
  *) sed -n '2,13p' "$0"; exit 2 ;;
esac
