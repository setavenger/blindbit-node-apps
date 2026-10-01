#!/usr/bin/env bash
# Lint every Umbrel app folder (<dir>/umbrel-app.yml at the repo root) with Umbrel's own
# linter, .tools/lint-apps.mjs of umbrel-apps at UMBREL_APPS_REV, inside a fresh clone (so the
# store-wide port checks run too), and check the community store id prefixes every app id.
#
#   scripts/ci/umbrel-lint.sh            needs node >= 20.11 and npm on PATH
#   FORMAT=github scripts/ci/umbrel-lint.sh
set -euo pipefail
LOG_TAG=umbrel-lint
# shellcheck source=scripts/lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"
load_pins

apps=()
for f in "$ROOT"/*/umbrel-app.yml; do
  [ -f "$f" ] && apps+=("$(basename "$(dirname "$f")")")
done
[ ${#apps[@]} -gt 0 ] || { log "no Umbrel app folders; nothing to lint"; exit 0; }

work=${UMBREL_WORK:-$(mktemp -d)}
if [ ! -d "$work/umbrel-apps/.git" ]; then
  git init -q "$work/umbrel-apps"
  git -C "$work/umbrel-apps" fetch -q --depth 1 "$UMBREL_APPS_REPO" "$UMBREL_APPS_REV"
  git -C "$work/umbrel-apps" checkout -q --detach FETCH_HEAD
fi
cd "$work/umbrel-apps"
npm ci --ignore-scripts --no-audit --no-fund >/dev/null
for a in "${apps[@]}"; do
  rm -rf "./$a" && cp -r "$ROOT/$a" "./$a"
done
log "umbrel-apps ${UMBREL_APPS_REV:0:7}: linting ${apps[*]}"
node .tools/lint-apps.mjs "${apps[@]}" --format "${FORMAT:-text}"

if [ -f "$ROOT/umbrel-app-store.yml" ]; then
  store_id=$(sed -n 's/^id:[[:space:]]*//p' "$ROOT/umbrel-app-store.yml" | tr -d "\"'")
  [[ $store_id =~ ^[A-Za-z-]+$ ]] || die "umbrel-app-store.yml id '$store_id' must be letters and dashes"
  for a in "${apps[@]}"; do
    id=$(sed -n 's/^id:[[:space:]]*//p' "$ROOT/$a/umbrel-app.yml" | tr -d "\"'")
    [[ $id == "$store_id"-* ]] || die "app id $id does not start with store id $store_id- (umbrelOS would drop it)"
  done
  log "community store id '$store_id' prefixes every app id"
fi
