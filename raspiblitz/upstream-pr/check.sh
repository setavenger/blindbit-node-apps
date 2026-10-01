#!/bin/bash
# Assemble the RaspiBlitz app PR on a pinned clone and check it like upstream CI does.
#
#   raspiblitz/upstream-pr/check.sh [base-commit]
#
# 1. clones raspiblitz/raspiblitz and checks out the pinned dev commit (or the one given)
# 2. adds home.admin/config.scripts/bonus.friglet.sh from this repo
# 3. applies 0*.patch with git am
# 4. runs shellcheck: full on bonus.friglet.sh, --severity=error (upstream CI level) on every
#    file the PR touches
# 5. reports whether the patches also apply to rootzoll/raspiblitz dev (a stale fork)
# Leaves the assembled branch in $WORK/raspiblitz (default: mktemp -d) for `git push`.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
base="${1:-c421693dcb09b00eafacbda6128174d2c658cb06}"
work="${WORK:-$(mktemp -d)}"
SHELLCHECK_IMAGE="koalaman/shellcheck:stable@sha256:bb596a0d169b85ddd81d8b6d3a2ff6d5baf5fca10b97f575ebc647c3dff62b3d"

git clone -q --filter=blob:none https://github.com/raspiblitz/raspiblitz.git "${work}/raspiblitz"
cd "${work}/raspiblitz"
git checkout -q -b friglet "${base}"
install -m 755 "${here}/../home.admin/config.scripts/bonus.friglet.sh" home.admin/config.scripts/bonus.friglet.sh
git add home.admin/config.scripts/bonus.friglet.sh
git -c user.name=check -c user.email=check@localhost commit -q -m "friglet: add bonus.friglet.sh"
git -c user.name=check -c user.email=check@localhost am -q "${here}"/0*.patch
echo "# assembled on ${base}:"
git log --oneline "${base}..HEAD"

touched=$(git diff --name-only "${base}..HEAD" -- '*.sh')
docker run --rm -v "${PWD}:/mnt:ro" -w /mnt "${SHELLCHECK_IMAGE}" home.admin/config.scripts/bonus.friglet.sh
echo "# shellcheck (all levels) bonus.friglet.sh: clean"
# shellcheck disable=SC2086
docker run --rm -v "${PWD}:/mnt:ro" -w /mnt "${SHELLCHECK_IMAGE}" --severity=error ${touched}
echo "# shellcheck --severity=error on $(echo "${touched}" | wc -l) touched files: clean"

git remote add rootzoll https://github.com/rootzoll/raspiblitz.git
git fetch -q --filter=blob:none rootzoll dev
tmp=$(mktemp -d)
git worktree add -q --detach "${tmp}" rootzoll/dev
for part in "code:. :!CHANGES.md" "CHANGES.md:CHANGES.md"; do
  label="${part%%:*}"
  # shellcheck disable=SC2086
  if (cd "${tmp}" && git diff "${base}..friglet" -- ${part#*:} | git apply --check 2>/dev/null); then
    echo "# rootzoll/raspiblitz dev ($(git rev-parse --short rootzoll/dev)): ${label} applies cleanly"
  else
    echo "# rootzoll/raspiblitz dev ($(git rev-parse --short rootzoll/dev)): ${label} does NOT apply"
  fi
done
git worktree remove --force "${tmp}"
echo "# branch ready in ${work}/raspiblitz (friglet)"
