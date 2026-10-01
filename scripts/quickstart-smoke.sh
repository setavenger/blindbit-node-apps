#!/usr/bin/env bash
# Run the compose quickstart exactly as compose/README.md writes it (the block between the
# quickstart markers), against a local image, then check the container and stop it within
# the 150 s grace period.
#
#   IMAGE=friglet-node:dev-amd64 scripts/quickstart-smoke.sh
#
# The quickstart runs in a temporary copy of compose/ with EDITOR=true, i.e. with the values
# of .env.example. Its default image name (IMAGE_REPO:<version>) is tagged from IMAGE for the
# run when no such image exists locally, and untagged afterwards.
set -euo pipefail
LOG_TAG=quickstart
# shellcheck source=scripts/lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
load_pins

IMAGE=${IMAGE:-friglet-node:dev-$(host_arch)}
docker image inspect "$IMAGE" >/dev/null 2>&1 || die "image $IMAGE not found (make image)"
default_image="$IMAGE_REPO:$(version)"
tagged=0

work=$(mktemp -d)
script=$work/quickstart.sh
awk '/<!-- quickstart:begin/{f=1; next} /<!-- quickstart:end/{f=0} f && /^```sh$/{c=1; next} f && /^```$/{c=0} c' \
  "$ROOT/compose/README.md" > "$script"
[ -s "$script" ] || die "no quickstart block in compose/README.md"
cp -r "$ROOT/compose" "$work/compose"
rm -f "$work/compose/.env"

# Hosts without the compose plugin: route `docker compose` to the pinned CLI image.
shim=()
if ! docker compose version >/dev/null 2>&1; then
  mkdir -p "$work/bin"
  cat > "$work/bin/docker" <<SHIM
#!/usr/bin/env bash
if [ "\${1:-}" = compose ]; then
  shift
  exec $(command -v docker) run --rm -i -v /var/run/docker.sock:/var/run/docker.sock -v "$work:$work" -w "\$PWD" "$DOCKER_CLI_IMAGE" compose "\$@"
fi
exec $(command -v docker) "\$@"
SHIM
  chmod +x "$work/bin/docker"
  shim=(env "PATH=$work/bin:$PATH")
fi

cleanup() {
  local rc=$?
  [ $rc -ne 0 ] && (cd "$work/compose" && "${shim[@]}" docker compose logs --tail 50 >&2 || true)
  (cd "$work/compose" && "${shim[@]}" docker compose down -v >/dev/null 2>&1 || true)
  rm -rf "$work"
  if [ "$tagged" = 1 ]; then docker rmi "$default_image" >/dev/null 2>&1 || true; fi
  exit $rc
}
trap cleanup EXIT

# Tag the local image with the release name the compose file defaults to; the tag is removed
# again afterwards unless it already existed (a local stand-in must not linger as "the release").
if ! docker image inspect "$default_image" >/dev/null 2>&1; then
  docker tag "$IMAGE" "$default_image"
  tagged=1
fi

log "running the quickstart block:"
sed 's/^/  $ /' "$script" >&2
(cd "$work" && "${shim[@]}" env EDITOR=true bash -euo pipefail "$script")

cd "$work/compose"
ctr=$("${shim[@]}" docker compose ps -q friglet-node)
[ -n "$ctr" ] || die "friglet-node is not running"
[ "$(docker inspect -f '{{.State.Running}}' "$ctr")" = true ] || die "friglet-node exited"
[ "$(docker inspect -f '{{.Config.User}}' "$ctr")" = 1000:1000 ] || die "container does not run as 1000:1000"
docker exec "$ctr" friglet-ui --version
start=$(date +%s)
"${shim[@]}" docker compose stop >&2
took=$(( $(date +%s) - start ))
[ "$took" -lt 150 ] || die "stop took ${took}s (grace period 150s)"
log "PASS: quickstart up, healthz answered, stopped in ${took}s"
