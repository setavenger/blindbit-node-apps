#!/bin/bash
# Test bonus.friglet.sh in a Debian bookworm container with stubbed RaspiBlitz services.
#
#   raspiblitz/test/run.sh            # shellcheck + container scenarios, then clean up
#   KEEP=1 raspiblitz/test/run.sh     # keep the container and image for inspection
#   WORK=<dir> raspiblitz/test/run.sh # build context location (default: mktemp -d)
#
# Needs: docker (no --privileged, no extra capabilities), curl, sha256sum, tar.
# The friglet-node tarballs used here are TEST STUBS (stub-tarball/), not friglet.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
script="${here}/../home.admin/config.scripts/bonus.friglet.sh"
work="${WORK:-$(mktemp -d)}"
tag="friglet-rb-test:local"
name="friglet-rb-test-$$"

# pinned test inputs
SHELLCHECK_IMAGE="koalaman/shellcheck:stable@sha256:bb596a0d169b85ddd81d8b6d3a2ff6d5baf5fca10b97f575ebc647c3dff62b3d"
DSR_COMMIT="15a2509551306789afbba32a47360365355a9bc4" # gdraheim/docker-systemctl-replacement
DSR_SHA256="57f9cdb22493bc74b06e125230c2009967bf6d16a7d52913fa7e9cf2e5c920de"
RB_COMMIT="c421693dcb09b00eafacbda6128174d2c658cb06"  # raspiblitz/raspiblitz dev (PR base)
RB_FILES="
e4b8414413098a2683bfe70f70750172bfee29d5e9e2ed8fdebb2fc0fecaa5a1 home.admin/config.scripts/blitz.conf.sh
43e6ed6ad6a5cadcc5ba4828048fd7dac3fd19d1ca2f395a6a6fee7ed36def25 home.admin/config.scripts/tor.onion-service.sh
ce11b6c0d4b4381b163fc0b0a98f81091dac013f63664e13b39ab4a902843c4b home.admin/assets/bitcoin.conf
e82b5e9b363548931f531ff8eb177853f7eb898acf38354542f19817bd1213b3 home.admin/assets/nginx/snippets/ssl-params.conf
81636f5d051f7b8ae9deed660b60178051e9765a852696ea0e327690427f485a home.admin/assets/nginx/snippets/ssl-certificate-app-data.conf
7d5e6b487f0651822b7ef4796a3c26d7f87aec527267eac3ce2a826ee3b2c121 home.admin/assets/nginx/snippets/ssl-certificate-app-data-tor.conf
1683f5c72fda9f9bafe11cddba389d06f2c57924016d7e96dff4fa0f717dde50 home.admin/assets/nginx/snippets/ssl-proxy-params.conf
"

cleanup() {
  if [ "${KEEP:-0}" != "1" ]; then
    docker rm -f "${name}" >/dev/null 2>&1 || true
    docker rmi "${tag}" >/dev/null 2>&1 || true
    [ -z "${WORK:-}" ] && rm -rf "${work}"
  else
    echo "# kept container ${name}, image ${tag}, context ${work}"
  fi
}
trap cleanup EXIT

fetch() { # url sha256 dest
  curl -fsSL --retry 3 -o "$3" "$1"
  echo "$2  $3" | sha256sum -c --quiet -
}

echo "## shellcheck"
docker run --rm -v "$(dirname "${script}"):/mnt:ro" "${SHELLCHECK_IMAGE}" /mnt/bonus.friglet.sh
echo "shellcheck: clean"

echo "## build context in ${work}"
mkdir -p "${work}/rb" "${work}/release/v1" "${work}/release/v2" "${work}/scripts"
cp -a "${here}/rootfs" "${work}/rootfs"
cp "${here}/scenarios.sh" "${work}/scenarios.sh"
fetch "https://raw.githubusercontent.com/gdraheim/docker-systemctl-replacement/${DSR_COMMIT}/files/docker/systemctl3.py" \
  "${DSR_SHA256}" "${work}/systemctl3.py"
echo "${RB_FILES}" | while read -r sum path; do
  [ -n "${path}" ] || continue
  fetch "https://raw.githubusercontent.com/raspiblitz/raspiblitz/${RB_COMMIT}/${path}" "${sum}" "${work}/rb/$(basename "${path}")"
done

# stub tarballs for two package versions (amd64 only: the container is x86_64)
v1=$(sed -n 's/^VERSION="\(.*\)"$/\1/p' "${script}")
v2="${v1%-*}-$(( ${v1##*-} + 1 ))"
for v in "${v1}" "${v2}"; do
  dir="${work}/release/$([ "${v}" = "${v1}" ] && echo v1 || echo v2)"
  stage=$(mktemp -d)
  install -m 755 "${here}/stub-tarball/friglet-ui" "${here}/stub-tarball/friglet" "${stage}/"
  echo "TEST STUB - not friglet (${v})" > "${stage}/LICENSE"
  tar --owner=0 --group=0 --mtime=@0 -czf "${dir}/friglet-node-${v}-linux-amd64.tar.gz" -C "${stage}" friglet friglet-ui LICENSE
  rm -rf "${stage}"
done
sum1=$(sha256sum "${work}/release/v1/friglet-node-${v1}-linux-amd64.tar.gz" | cut -d' ' -f1)
sum2=$(sha256sum "${work}/release/v2/friglet-node-${v2}-linux-amd64.tar.gz" | cut -d' ' -f1)

# script variants: only the release constants differ from the committed script
variant() { # name version base sha
  sed -e "s|^VERSION=\".*\"|VERSION=\"$2\"|" \
      -e "s|^RELEASE_BASE_URL=.*|RELEASE_BASE_URL=\"$3\"|" \
      -e "s|^SHA256_AMD64=\".*\"|SHA256_AMD64=\"$4\"|" \
      "${script}" > "${work}/scripts/$1"
  chmod 755 "${work}/scripts/$1"
}
cp "${script}" "${work}/scripts/shipped"
variant unpinned "${v1}" "file:///srv/release/v1" ""
variant badsum "${v1}" "file:///srv/release/v1" "$(printf '0%.0s' $(seq 64))"
variant pinned "${v1}" "file:///srv/release/v1" "${sum1}"
variant pinned-v2 "${v2}" "file:///srv/release/v2" "${sum2}"
for f in "${work}"/scripts/*; do
  diff -u "${script}" "${f}" | grep -E '^[-+][A-Z0-9_]+=' | grep -v -E '^[-+](VERSION|RELEASE_BASE_URL|SHA256_AMD64)=' && { echo "variant ${f} changed more than the release constants"; exit 1; }
done
echo "stub tarballs: ${v1} ${sum1}, ${v2} ${sum2}"

echo "## docker build"
docker build -q -t "${tag}" -f "${here}/Dockerfile" "${work}" >/dev/null

echo "## run scenarios"
docker run -d --name "${name}" --tmpfs /run --tmpfs /tmp "${tag}" >/dev/null
sleep 3
docker exec -e TERM=xterm "${name}" /opt/test/scenarios.sh "${v1}" "${v2}"
