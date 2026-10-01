#!/usr/bin/env bash
# Static checks, each in its digest-pinned container (versions.env):
#   - ShellCheck: every tracked *.sh outside upstream/
#   - hadolint: every tracked Dockerfile outside upstream/
#   - actionlint: .github/workflows/*.yml (runs ShellCheck on run: blocks too)
#
#   scripts/lint.sh [shellcheck|hadolint|actionlint]...   (default: all three)
set -euo pipefail
LOG_TAG=lint
# shellcheck source=scripts/lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
load_pins

tracked() {
  # Tracked plus new, not-ignored files, so a lint run before the first commit sees them too.
  git -C "$ROOT" ls-files --cached --others --exclude-standard -- "$@" ':!:upstream/**' | sort -u
}

run_shellcheck() {
  local files=()
  mapfile -t files < <(tracked '*.sh')
  [ ${#files[@]} -gt 0 ] || { log "shellcheck: no files"; return 0; }
  log "shellcheck: ${#files[@]} files"
  docker run --rm -v "$ROOT:/mnt:ro" -w /mnt "$SHELLCHECK_IMAGE" -x -P SCRIPTDIR -S warning "${files[@]}"
}

run_hadolint() {
  local files=() f rc=0
  mapfile -t files < <(tracked 'Dockerfile' '**/Dockerfile' '*.Dockerfile' '**/*.Dockerfile')
  [ ${#files[@]} -gt 0 ] || { log "hadolint: no files"; return 0; }
  for f in "${files[@]}"; do
    log "hadolint: $f"
    docker run --rm -i "$HADOLINT_IMAGE" hadolint --failure-threshold warning - < "$ROOT/$f" || rc=1
  done
  return "$rc"
}

run_actionlint() {
  [ -d "$ROOT/.github/workflows" ] || { log "actionlint: no workflows"; return 0; }
  log "actionlint: .github/workflows"
  docker run --rm -v "$ROOT:/repo:ro" -w /repo "$ACTIONLINT_IMAGE" -color=false
}

checks=("$@")
[ ${#checks[@]} -gt 0 ] || checks=(shellcheck hadolint actionlint)
rc=0
for c in "${checks[@]}"; do
  case "$c" in
    shellcheck) run_shellcheck || rc=1 ;;
    hadolint) run_hadolint || rc=1 ;;
    actionlint) run_actionlint || rc=1 ;;
    *) die "unknown check $c" ;;
  esac
done
if [ "$rc" = 0 ]; then log "all checks passed"; else log "some checks FAILED"; fi
exit "$rc"
