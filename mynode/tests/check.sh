#!/usr/bin/env bash
#
# Checks the MyNode app with MyNode's own tooling, using only Docker on the host:
#   1. app_sdk/create.py check blindbit-friglet   (must print "looks good")
#   2. app_sdk/create.py check_doc
#   3. shellcheck on the app's scripts, following MyNode's real helper scripts
#   4. lifecycle.sh: install, pre-start and uninstall run as MyNode runs them
#   5. docker compose config on the files those scripts wrote
#   6. web_sim.py: app defaults, Tor and firewall config, blueprint loading, app page
#
# Usage:
#   mynode/tests/check.sh <mynode checkout>             copy this repo's app into a scratch clone of it
#   mynode/tests/check.sh --in-place <mynode checkout>  check the app already in it (patch applied)
#
# Images used: python:3.8-slim (MyNode runs Python 3.8), koalaman/shellcheck:stable, docker:29-cli.

set -euo pipefail

APP=blindbit-friglet
HERE=$(cd "$(dirname "$0")" && pwd)
REPO_APP="$HERE/../mynode_apps/$APP"
PYTHON_IMAGE=${PYTHON_IMAGE:-python:3.8-slim}
SHELLCHECK_IMAGE=${SHELLCHECK_IMAGE:-koalaman/shellcheck:stable}
COMPOSE_IMAGE=${COMPOSE_IMAGE:-docker:29-cli}

IN_PLACE=0
if [ "${1:-}" = "--in-place" ]; then
    IN_PLACE=1
    shift
fi
if [ $# -ne 1 ] || [ ! -f "$1/app_sdk/create.py" ]; then
    echo "usage: $0 [--in-place] <mynodebtc/mynode checkout>" >&2
    exit 2
fi
CHECKOUT=$(cd "$1" && pwd)

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

if [ "$IN_PLACE" -eq 1 ]; then
    MYNODE="$CHECKOUT"
    [ -d "$MYNODE/rootfs/standard/usr/share/mynode_apps/$APP" ] || { echo "no $APP in $MYNODE" >&2; exit 2; }
else
    MYNODE="$TMP/mynode"
    git clone -q --shared "$CHECKOUT" "$MYNODE"
    git -C "$MYNODE" checkout -q --detach "$(git -C "$CHECKOUT" rev-parse HEAD)"
    cp -r "$REPO_APP" "$MYNODE/rootfs/standard/usr/share/mynode_apps/$APP"
fi
APPDIR="$MYNODE/rootfs/standard/usr/share/mynode_apps/$APP"
set +e
echo "myNode checkout: $(git -C "$MYNODE" log --oneline -1)"
echo "app checked:     $APPDIR"

FAILED=()
step() { echo; echo "=== $*"; }
record() { if [ "$1" -eq 0 ]; then echo "--> OK"; else echo "--> FAILED"; FAILED+=("$2"); fi; }

step "1. create.py check $APP"
# Pull first: on a host without the image, docker run's pull progress would land in $out.
docker pull -q "$PYTHON_IMAGE" >/dev/null
out=$(docker run --rm -v "$MYNODE":/mynode:ro -w /mynode "$PYTHON_IMAGE" python3 app_sdk/create.py check "$APP" 2>&1)
rc=$?
echo "$out"
[ "$rc" -eq 0 ] && [ "$out" = "$APP looks good!" ]
record $? "create.py check"

step "2. create.py check_doc"
docker run --rm -v "$MYNODE":/mynode:ro -w /mynode "$PYTHON_IMAGE" python3 app_sdk/create.py check_doc
record $? "create.py check_doc"

step "3. shellcheck (sourcing MyNode's /usr/share/mynode helpers)"
docker run --rm \
    -v "$APPDIR/scripts":/app/scripts:ro \
    -v "$MYNODE/rootfs/standard/usr/share/mynode":/usr/share/mynode:ro \
    -v "$HERE":/tests:ro \
    -v "$HERE/../upstream-pr":/upstream-pr:ro \
    "$SHELLCHECK_IMAGE" -x \
    "/app/scripts/install_$APP.sh" "/app/scripts/uninstall_$APP.sh" \
    "/app/scripts/pre_$APP.sh" "/app/scripts/post_$APP.sh" \
    /tests/lifecycle.sh /tests/check.sh /upstream-pr/make-patch.sh
record $? "shellcheck"

step "4. lifecycle: install, pre-start, uninstall"
mkdir -p "$TMP/out"
docker run --rm \
    -v "$MYNODE":/mynode:ro -v "$APPDIR":/app:ro -v "$TMP/out":/out \
    -v "$HERE/lifecycle.sh":/lifecycle.sh:ro \
    "$PYTHON_IMAGE" bash /lifecycle.sh
record $? "lifecycle"

step "5. docker compose config with the .env and runtime.env the scripts wrote"
if [ -f "$TMP/out/.env" ]; then
    echo "--- .env";  cat "$TMP/out/.env"
    echo "--- runtime.env"; cat "$TMP/out/runtime.env"
    docker run --rm -v "$TMP/out":/opt/mynode/$APP:ro -w /opt/mynode/$APP "$COMPOSE_IMAGE" \
        docker compose config > "$TMP/compose.out" 2>&1
    rc=$?
    echo "--- docker compose config"; cat "$TMP/compose.out"
    if [ "$rc" -eq 0 ]; then
        problems=()
        grep -Eq "image: ghcr.io/setavenger/friglet-node:[^ ]+@sha256:[0-9a-f]{64}$" "$TMP/compose.out" || problems+=("image not pinned by digest")
        grep -q "FRIGLET_UI_PLATFORM: mynode" "$TMP/compose.out" || problems+=("platform is not mynode")
        grep -q "FRIGLET_UI_AUTH: password" "$TMP/compose.out" || problems+=("auth is not password")
        grep -q "FRIGLET_UI_PASSWORD_FILE: /run/friglet-ui/password" "$TMP/compose.out" || problems+=("no password file")
        grep -q "FRIGLET_UI_PUBLIC_HOST: 192.168.1.50" "$TMP/compose.out" || problems+=("runtime.env not applied")
        grep -E "^ +FRIGLET_" "$TMP/compose.out" | grep -Ev "^ +FRIGLET_UI_" && problems+=("a FRIGLET_ variable that is not FRIGLET_UI_ reaches the container")
        grep -Eiq "scan|descriptor|spscan" "$TMP/compose.out" && problems+=("scan key material named in compose")
        grep -q 'host_ip: 127.0.0.1' "$TMP/compose.out" || problems+=("no loopback-only ports")
        for p in "${problems[@]}"; do echo "problem: $p"; done
        [ ${#problems[@]} -eq 0 ]
        rc=$?
    fi
else
    echo "lifecycle wrote no files"
    rc=1
fi
record "$rc" "docker compose config"

step "6. web: app defaults, Tor and firewall, blueprint loading, app page (Python 3.8, Flask 2.2-2.3)"
docker run --rm \
    -v "$MYNODE":/mynode:ro -v "$APPDIR":/app:ro -v "$HERE/web_sim.py":/web_sim.py:ro \
    "$PYTHON_IMAGE" sh -c 'pip install -q --disable-pip-version-check --root-user-action=ignore "flask>=2.2.2,<2.4.0" && python3 -c "import flask; print(\"Flask\", flask.__version__)" && python3 /web_sim.py'
record $? "web"

echo
if grep -q 'sha256:0\{64\}' "$APPDIR/scripts/install_$APP.sh"; then
    echo "NOTE: install_$APP.sh still has the PLACEHOLDER image digest (all zeros); not installable yet."
fi
if [ ${#FAILED[@]} -ne 0 ]; then
    echo "FAILED: ${FAILED[*]}"
    exit 1
fi
echo "ALL CHECKS PASSED"
