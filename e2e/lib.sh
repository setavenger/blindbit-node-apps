# shellcheck shell=bash
# Shared parts of e2e/run.sh and e2e/dev-regtest.sh. Source it after scripts/lib.sh.
#
# The regtest chain and oracle come from blindbit-testkit's compose stack at
# BLINDBIT_TESTKIT_REV (versions.env), cloned into the work directory and never modified.
# friglet-node joins that stack's network: from there bitcoind is `bitcoind:18444` (P2P) and
# `bitcoind:18443` (RPC), the oracle `oracle:7001` (gRPC), and friglet-node `friglet-node`.

E2E_WORKDIR=${E2E_WORKDIR:-$ROOT/build/e2e}
export E2E_WORKDIR
TESTKIT_DIR=$E2E_WORKDIR/blindbit-testkit
SPTOOL=$E2E_WORKDIR/bin/sptool
KEYS=$E2E_WORKDIR/keys.json
# Clone source for the testkit; a local clone that has the pinned commit works offline.
E2E_TESTKIT_SRC=${E2E_TESTKIT_SRC:-$BLINDBIT_TESTKIT_REPO}
# Host ports friglet-node publishes on 127.0.0.1; 0 = random (e2e), dev-regtest sets fixed ones.
UI_PORT=${UI_PORT:-0} ELECTRUM_PORT=${ELECTRUM_PORT:-0} ELECTRUM_TLS_PORT=${ELECTRUM_TLS_PORT:-0}

# set_project NAME: the testkit compose project; friglet-node runs as project NAME-node.
set_project() {
  E2E_PROJECT=$1
  E2E_NETWORK=${E2E_PROJECT}_default
  NODE_PROJECT=${E2E_PROJECT}-node
  NODE_CTR=${NODE_PROJECT}-friglet-node-1
  export E2E_PROJECT E2E_NETWORK
}

prepare_testkit() {
  mkdir -p "$E2E_WORKDIR"
  if [ ! -d "$TESTKIT_DIR/.git" ]; then
    log "cloning blindbit-testkit from $E2E_TESTKIT_SRC"
    git init -q "$TESTKIT_DIR"
  fi
  if [ "$(git -C "$TESTKIT_DIR" rev-parse -q --verify HEAD 2>/dev/null)" != "$BLINDBIT_TESTKIT_REV" ]; then
    git -C "$TESTKIT_DIR" fetch -q --depth 1 "$E2E_TESTKIT_SRC" "$BLINDBIT_TESTKIT_REV"
    git -C "$TESTKIT_DIR" checkout -q --detach FETCH_HEAD
  fi
  [ "$(git -C "$TESTKIT_DIR" rev-parse HEAD)" = "$BLINDBIT_TESTKIT_REV" ] || die "testkit is not at $BLINDBIT_TESTKIT_REV"
  [ -z "$(git -C "$TESTKIT_DIR" status --porcelain)" ] || die "testkit checkout $TESTKIT_DIR is modified; delete it"
  log "testkit at ${BLINDBIT_TESTKIT_REV:0:7}"
}

# stack ARGS...: the testkit's own scripts/stack.sh for our project. Every published port is
# random (0) so parallel stacks on one host do not collide. BBTK_FRIGLET_HTTP_PORT stays while
# the testkit pin still publishes friglet's old HTTP port (dropped there by SNB-780).
stack() {
  (
    cd "$TESTKIT_DIR" || exit
    export COMPOSE_PROJECT_NAME=$E2E_PROJECT
    export BBTK_RPC_PORT=0 BBTK_P2P_PORT=0 BBTK_ORACLE_HTTP_PORT=0 BBTK_ORACLE_GRPC_PORT=0 \
      BBTK_FRIGLET_HTTP_PORT=0 BBTK_ELECTRUM_PORT=0
    [ -n "${E2E_ORACLE_SRC:-}" ] && export ORACLE_SRC=$E2E_ORACLE_SRC
    scripts/stack.sh "$@"
  )
}

# Build images and start bitcoind, chain-init and the oracle (not the testkit's friglet).
stack_up() {
  log "building and starting bitcoind + oracle (project $E2E_PROJECT; first run builds images)"
  stack compose build bitcoind oracle >&2
  stack compose up -d --wait --wait-timeout "${E2E_UP_TIMEOUT:-900}" oracle >&2
  stack wait-indexed
}

stack_down() {
  stack compose down -v --remove-orphans >/dev/null 2>&1 || true
}

tip() { stack rpc getblockcount | tr -d '\r\n'; }

build_sptool() {
  mkdir -p "$E2E_WORKDIR/bin" "$E2E_WORKDIR/go"
  log "building e2e/sptool in $GO_IMAGE"
  docker run --rm -u "$(id -u):$(id -g)" -e HOME=/tmp -e CGO_ENABLED=0 -e GOTOOLCHAIN=local \
    -e GOPATH="$E2E_WORKDIR/go/path" -e GOCACHE="$E2E_WORKDIR/go/cache" -e GOFLAGS=-mod=readonly \
    -v "$ROOT/e2e/sptool:/src:ro" -v "$E2E_WORKDIR:$E2E_WORKDIR" -w /src \
    "$GO_IMAGE" go build -trimpath -o "$SPTOOL" . >&2
}

# sptool ARGS...: run the static helper on the stack network (stdin passes through).
sptool() {
  docker run --rm -i --network "$E2E_NETWORK" -u "$(id -u):$(id -g)" \
    -v "$E2E_WORKDIR:$E2E_WORKDIR" "$RUNTIME_IMAGE" "$SPTOOL" "$@"
}

json_field() { # json_field NAME < json  (flat string or number fields only)
  sed -n "s/.*\"$1\":\"\{0,1\}\([^\",}]*\)\"\{0,1\}[,}].*/\1/p" | head -n1
}

new_keys() {
  rm -f "$KEYS"
  ADDRESS=$(sptool keygen -network regtest -out "$KEYS")
  chmod 600 "$KEYS"
  log "fresh regtest wallet $ADDRESS (keys in $KEYS, mode 0600)"
}

# pay AMOUNT: send a silent payment to the wallet; sets PAY_TXID and PAY_HEIGHT.
pay() {
  local out
  out=$(sptool pay -rpc http://bitcoind:18443 -rpc-user bbtk -rpc-pass bbtk-regtest-only \
    -wallet bbtk -to "$ADDRESS" -amount "$1")
  PAY_TXID=$(json_field txid <<<"$out")
  PAY_HEIGHT=$(json_field height <<<"$out")
  [[ $PAY_TXID =~ ^[0-9a-f]{64}$ ]] || die "pay: unexpected output: $out"
  log "paid $1 sat in $PAY_TXID, confirmed at height $PAY_HEIGHT"
  stack wait-indexed
}

# node_compose ARGS...: our compose/docker-compose.yml, attached to the stack network.
node_compose() {
  (
    cd "$ROOT/compose" || exit
    export FRIGLET_NODE_IMAGE=$IMAGE FRIGLET_UI_NETWORK=regtest FRIGLET_UI_NODE=bitcoind:18444 \
      FRIGLET_UI_ORACLE_URL=http://oracle:7001 FRIGLET_UI_PASSWORD=$UI_PASSWORD \
      BIND_ADDRESS=127.0.0.1 UI_PORT ELECTRUM_PORT ELECTRUM_TLS_PORT
    compose -p "$NODE_PROJECT" --project-directory "$ROOT/compose" \
      -f "$ROOT/compose/docker-compose.yml" -f "$ROOT/e2e/compose.e2e.yml" "$@"
  )
}

ui_password() {
  local f=$E2E_WORKDIR/ui-password
  if [ ! -s "$f" ]; then
    (umask 077 && head -c 18 /dev/urandom | base64 | tr -d '/+=' > "$f")
  fi
  UI_PASSWORD=$(cat "$f")
}

# setup_wallet BIRTHDAY: hand the descriptor to `friglet-ui setup` on stdin (never argv/env).
setup_wallet() {
  local out
  out=$(sptool get -keys "$KEYS" descriptor | docker exec -i "$NODE_CTR" friglet-ui setup --birthday "$1" --json) \
    || die "friglet-ui setup failed: $out"
  log "setup: $out"
}

wait_healthy() { # wait_healthy CONTAINER TIMEOUT_S: friglet-ui health exits 0
  local i
  for ((i = 0; i < $2; i += 3)); do
    docker exec "$1" friglet-ui health >/dev/null 2>&1 && return 0
    sleep 3
  done
  docker exec "$1" friglet-ui status --json >&2 || true
  die "$1 not healthy after $2 s"
}

electrum_check() { # electrum_check TXID
  sptool electrum-check -server friglet-node:50031 -txid "$1" -timeout "${E2E_FIND_TIMEOUT:-10m}"
}
