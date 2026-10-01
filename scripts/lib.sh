# shellcheck shell=bash
# Shared helpers for scripts/ and e2e/. Source it; do not execute it.
#
#   ROOT          repository root
#   load_pins     export every KEY=value of versions.env
#   version       package version: <friglet short commit>-<PACKAGE_REVISION>
#   have_buildkit true when `docker build` is BuildKit (buildx installed)
#   compose ...   docker compose, from the plugin or from the pinned docker CLI image

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
export ROOT

log() { printf '[%s %s] %s\n' "${LOG_TAG:-friglet-node}" "$(date -u +%H:%M:%S)" "$*" >&2; }
die() { printf '[%s] FATAL: %s\n' "${LOG_TAG:-friglet-node}" "$*" >&2; exit 1; }

load_pins() {
  local line key value
  [ -f "$ROOT/versions.env" ] || die "no versions.env at $ROOT"
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in ''|'#'*) continue ;; esac
    key=${line%%=*}
    value=${line#*=}
    [[ $key =~ ^[A-Z][A-Z0-9_]*$ ]] || die "versions.env: bad line: $line"
    # An explicit environment value wins (e.g. a CI matrix), but never silently: say so.
    if [ -n "${!key+x}" ] && [ "${!key}" != "$value" ]; then
      log "versions.env: $key overridden by the environment (${!key})"
      continue
    fi
    export "$key=$value"
  done < "$ROOT/versions.env"
}

version() {
  printf '%s-%s\n' "${BLINDBIT_RS_REV:0:7}" "$PACKAGE_REVISION"
}

host_arch() {
  case "$(uname -m)" in
    x86_64|amd64) echo amd64 ;;
    aarch64|arm64) echo arm64 ;;
    *) die "unsupported host architecture $(uname -m)" ;;
  esac
}

have_buildkit() {
  [ "${DOCKER_BUILDKIT:-}" = 0 ] && return 1
  docker buildx version >/dev/null 2>&1
}

# compose ARGS...: `docker compose` when the plugin exists, else the same command from the
# digest-pinned docker CLI image against the host daemon. Paths under $ROOT and under the
# e2e work directory are mounted at their own paths so relative references resolve.
compose() {
  if docker compose version >/dev/null 2>&1; then
    docker compose "$@"
    return
  fi
  local sock=/var/run/docker.sock tty=() mounts=(-v "$ROOT:$ROOT") envs=()
  [[ ${DOCKER_HOST:-} == unix://* ]] && sock=${DOCKER_HOST#unix://}
  [ -t 0 ] && [ -t 1 ] && tty=(-t)
  if [ -n "${E2E_WORKDIR:-}" ] && [[ $E2E_WORKDIR != "$ROOT"/* ]]; then
    mounts+=(-v "$E2E_WORKDIR:$E2E_WORKDIR")
  fi
  # Interpolation variables of our compose files and the testkit's.
  local name
  while IFS= read -r name; do envs+=(-e "$name"); done < <(compgen -e | grep -E '^(COMPOSE_|FRIGLET_|UI_|ELECTRUM_|BIND_|NETWORK$|NODE$|PASSWORD$|BBTK_|BTC_|ORACLE_|BLINDBIT_RS_COMMIT$|INITIAL_BLOCKS$|E2E_)' || true)
  docker run --rm -i "${tty[@]}" -v "$sock:/var/run/docker.sock" "${mounts[@]}" "${envs[@]}" \
    -w "$PWD" "$DOCKER_CLI_IMAGE" compose "$@"
}
