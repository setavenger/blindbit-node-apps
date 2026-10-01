# STUB of the friglet-node image (docs/contract.md), for packaging tests only.
# Same entrypoint, binary names, user and volume as the real image; the
# binaries are busybox shell scripts that follow the friglet-ui CLI contract
# (setup exit codes, --json, flock staging, status/health) closely enough to
# exercise the StartOS package. It scans nothing and serves no Electrum.
#
# No RUN steps, so it builds for linux/arm64 on an amd64 host without QEMU:
#   docker build --platform linux/amd64 -t friglet-node:dev-x86_64  .
#   docker build --platform linux/arm64 -t friglet-node:dev-aarch64 .
FROM busybox:1.37.0
COPY tini /usr/bin/tini
COPY friglet /usr/local/bin/friglet
COPY friglet-ui /usr/local/bin/friglet-ui
COPY www /usr/share/friglet-ui-stub/www
LABEL org.opencontainers.image.title="friglet-node (STUB)"
USER 1000:1000
WORKDIR /data
VOLUME /data
ENTRYPOINT ["tini", "--", "friglet-ui"]
