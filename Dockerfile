# friglet-node: friglet (from blindbit-rs at a pinned commit) + friglet-ui (this workspace)
# + tini, on Debian bookworm, for linux/amd64 and linux/arm64.
#
# Both architectures are cross-compiled once, on the build platform, with cargo-zigbuild
# against glibc 2.36. The runtime stage runs no RUN instruction, so an arm64 image builds
# on an amd64 host without QEMU:
#
#   docker build --platform linux/arm64 -t friglet-node:dev-arm64 .
#   docker build --target binaries -o build/out .        # BuildKit: export both arches
#
# The legacy (non-BuildKit) builder cannot COPY --from a stage of another platform; on such
# hosts `make image` / scripts/build-image.sh builds the binaries once and then this file's
# runtime stage from them. Every ARG default below is generated from versions.env by
# scripts/update-pins.sh; change versions.env, not this file.

# --- pins (generated from versions.env) ---
ARG RUST_IMAGE=docker.io/library/rust:1.97.1-slim-bookworm@sha256:2775a09d208ff0d7c1f50490c45b62db929e87ba1dcbc3f2132ac71a704bcdd3
ARG RUNTIME_IMAGE=docker.io/library/debian:bookworm-slim@sha256:3783cc01769c7b2b1b83a5c5ad96c815348e28ed7da68e2e3687004faa906251
ARG BLINDBIT_RS_REPO=https://github.com/setavenger/blindbit-rs.git
ARG BLINDBIT_RS_REV=93f6a92805b97921087b43e626ce4d3d766a3c36
ARG FRIGLET_NODE_VERSION=93f6a92-1
ARG GLIBC_VERSION=2.36
ARG ZIG_VERSION=0.15.2
ARG ZIG_SHA256_X86_64=02aa270f183da276e5b5920b1dac44a63f1a49e55050ebde3aecc9eb82f93239
ARG ZIG_SHA256_AARCH64=958ed7d1e00d0ea76590d27666efbf7a932281b3d7ba0c6b01b0ff26498f667f
ARG CARGO_ZIGBUILD_VERSION=0.23.4
ARG CARGO_ZIGBUILD_SHA256_X86_64=6b69963040818ca1ef573d8135a95aa902e17cf7f873c32c6426642dadfe7f59
ARG CARGO_ZIGBUILD_SHA256_AARCH64=25cf2cdc6cde848b33a761ff2cf46d1efe6ba7e1adfc4b6f3ee2d570cacb5419
ARG PROTOC_VERSION=36.2
ARG PROTOC_SHA256_X86_64=121f6c7afe1d4d0e3ea6aab9432038599250134cbf4474cb1167d2c7decd4278
ARG PROTOC_SHA256_AARCH64=8b8f18bd2b30346efbc698dd5a73dd7c805f3ef8380f6dfc95c768f3f1852f6a
ARG TINI_VERSION=0.19.0
ARG TINI_SHA256_AMD64=c5b0666b4cb676901f90dfcb37106783c5fe2077b04590973b885950611b30ee
ARG TINI_SHA256_ARM64=eae1d3aa50c48fb23b8cbdf4e369d0910dfc538566bfd09df89a774aa84a48b9
# --- end of pins ---

# Not pins: set by the Makefile / CI.
# Repository commit of this packaging repo (OCI revision label).
ARG VCS_REF=unknown
# Empty: as many jobs as cores. The shared dev host uses 2.
ARG CARGO_BUILD_JOBS=
# BuildKit sets BUILDPLATFORM and TARGETARCH itself; the legacy builder does not, hence the
# fallbacks (a legacy build without --build-arg is a native amd64 build).
ARG BUILDPLATFORM=${BUILDPLATFORM:-linux/amd64}
ARG TARGETARCH=${TARGETARCH:-amd64}
# A local blindbit-rs checkout replaces the git fetch when this stage has content:
#   BuildKit: --build-context blindbit-rs-src=<path>; legacy: an imported image named here.
# Such builds are marked "dev" (see BLINDBIT_RS_SRC in the Makefile).
ARG BLINDBIT_RS_SRC_IMAGE=scratch

FROM ${BLINDBIT_RS_SRC_IMAGE} AS blindbit-rs-src

# The runtime base on the build platform, only to take its /etc/passwd and /etc/group.
FROM --platform=${BUILDPLATFORM} ${RUNTIME_IMAGE} AS runtime-base

FROM --platform=${BUILDPLATFORM} ${RUST_IMAGE} AS builder
SHELL ["/bin/bash", "-euo", "pipefail", "-c"]

# The image's CA bundle comes from the digest-pinned builder base, taken before apt runs.
RUN install -D -m 0644 /etc/ssl/certs/ca-certificates.crt /out/image/common/etc/ssl/certs/ca-certificates.crt

# Build-only tools; not shipped. Every downloaded artifact below is checksum-verified.
# hadolint ignore=DL3008
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl git unzip xz-utils \
    && rm -rf /var/lib/apt/lists/*

ARG ZIG_VERSION
ARG ZIG_SHA256_X86_64
ARG ZIG_SHA256_AARCH64
ARG CARGO_ZIGBUILD_VERSION
ARG CARGO_ZIGBUILD_SHA256_X86_64
ARG CARGO_ZIGBUILD_SHA256_AARCH64
ARG PROTOC_VERSION
ARG PROTOC_SHA256_X86_64
ARG PROTOC_SHA256_AARCH64
RUN arch="$(uname -m)"; \
    case "$arch" in \
      x86_64)  zig_sha="$ZIG_SHA256_X86_64";  cz_sha="$CARGO_ZIGBUILD_SHA256_X86_64";  pc_arch=x86_64;  pc_sha="$PROTOC_SHA256_X86_64" ;; \
      aarch64) zig_sha="$ZIG_SHA256_AARCH64"; cz_sha="$CARGO_ZIGBUILD_SHA256_AARCH64"; pc_arch=aarch_64; pc_sha="$PROTOC_SHA256_AARCH64" ;; \
      *) echo "unsupported build platform: $arch" >&2; exit 1 ;; \
    esac; \
    curl -fsSLo /tmp/zig.tar.xz "https://ziglang.org/download/${ZIG_VERSION}/zig-${arch}-linux-${ZIG_VERSION}.tar.xz"; \
    echo "${zig_sha}  /tmp/zig.tar.xz" | sha256sum -c -; \
    mkdir -p /opt/zig && tar -xJf /tmp/zig.tar.xz -C /opt/zig --strip-components=1; \
    curl -fsSLo /tmp/cz.tar.xz "https://github.com/rust-cross/cargo-zigbuild/releases/download/v${CARGO_ZIGBUILD_VERSION}/cargo-zigbuild-${arch}-unknown-linux-gnu.tar.xz"; \
    echo "${cz_sha}  /tmp/cz.tar.xz" | sha256sum -c -; \
    tar -xJf /tmp/cz.tar.xz --strip-components=1 -C /usr/local/cargo/bin --wildcards '*/cargo-zigbuild'; \
    curl -fsSLo /tmp/protoc.zip "https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/protoc-${PROTOC_VERSION}-linux-${pc_arch}.zip"; \
    echo "${pc_sha}  /tmp/protoc.zip" | sha256sum -c -; \
    mkdir -p /opt/protoc && unzip -q /tmp/protoc.zip -d /opt/protoc; \
    rm -f /tmp/zig.tar.xz /tmp/cz.tar.xz /tmp/protoc.zip; \
    rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
ENV PATH=/opt/zig:/opt/protoc/bin:$PATH \
    PROTOC=/opt/protoc/bin/protoc \
    PROTOC_INCLUDE=/opt/protoc/include \
    CARGO_TERM_COLOR=never \
    CARGO_NET_GIT_FETCH_WITH_CLI=true

# tini (static) for both image architectures.
ARG TINI_VERSION
ARG TINI_SHA256_AMD64
ARG TINI_SHA256_ARM64
RUN for a in amd64 arm64; do \
      case "$a" in amd64) sha="$TINI_SHA256_AMD64" ;; arm64) sha="$TINI_SHA256_ARM64" ;; esac; \
      curl -fsSLo "/tmp/tini-$a" "https://github.com/krallin/tini/releases/download/v${TINI_VERSION}/tini-static-$a"; \
      echo "${sha}  /tmp/tini-$a" | sha256sum -c -; \
      install -D -m 0755 "/tmp/tini-$a" "/out/image/linux-$a/tini"; \
      rm -f "/tmp/tini-$a"; \
    done

# uid:gid 1000:1000 on top of the runtime base's own accounts; /data owned by it.
COPY --from=runtime-base /etc/passwd /etc/group /tmp/runtime-base/
RUN ! grep -qE '^[^:]*:[^:]*:1000:' /tmp/runtime-base/passwd /tmp/runtime-base/group; \
    install -d /out/image/common/etc /out/image/common/data; \
    { cat /tmp/runtime-base/passwd; echo 'friglet:x:1000:1000:friglet:/data:/usr/sbin/nologin'; } > /out/image/common/etc/passwd; \
    { cat /tmp/runtime-base/group; echo 'friglet:x:1000:'; } > /out/image/common/etc/group; \
    chmod 0644 /out/image/common/etc/passwd /out/image/common/etc/group; \
    rm -rf /tmp/runtime-base

ARG CARGO_BUILD_JOBS
ARG GLIBC_VERSION
ARG BLINDBIT_RS_REPO
ARG BLINDBIT_RS_REV

# friglet, from the pinned blindbit-rs commit (or the local checkout), with its own lockfile.
COPY --from=blindbit-rs-src / /src/blindbit-rs-local/
WORKDIR /src/blindbit-rs
RUN if [ -f /src/blindbit-rs-local/Cargo.toml ]; then \
      cp -a /src/blindbit-rs-local/. .; \
      echo "dev" > /src/friglet-build-kind; \
    else \
      git init -q .; \
      git fetch -q --depth 1 "$BLINDBIT_RS_REPO" "$BLINDBIT_RS_REV"; \
      git checkout -q --detach FETCH_HEAD; \
      test "$(git rev-parse HEAD)" = "$BLINDBIT_RS_REV"; \
      echo "release" > /src/friglet-build-kind; \
    fi; \
    rm -rf /src/blindbit-rs-local; \
    [ -n "$CARGO_BUILD_JOBS" ] || unset CARGO_BUILD_JOBS; \
    for arch in amd64 arm64; do \
      case "$arch" in amd64) triple=x86_64-unknown-linux-gnu ;; arm64) triple=aarch64-unknown-linux-gnu ;; esac; \
      cargo zigbuild --locked --release -p friglet --target "${triple}.${GLIBC_VERSION}"; \
      install -D -m 0755 "target/${triple}/release/friglet" "/out/bin/linux-$arch/friglet"; \
      install -D -m 0644 LICENSE "/out/bin/linux-$arch/LICENSE.friglet"; \
    done; \
    rm -rf target

# friglet-ui, from this workspace. It consumes friglet-ipc from blindbit-rs at the same commit
# (a local checkout is patched in for dev builds, which then cannot use --locked).
# friglet-ui's build.rs reads the friglet pin from ../versions.env and marks the build "dev"
# when BLINDBIT_RS_SRC is set.
WORKDIR /src/ws
COPY Cargo.toml Cargo.lock versions.env ./
COPY friglet-ui ./friglet-ui
ARG FRIGLET_NODE_VERSION
RUN [ -n "$CARGO_BUILD_JOBS" ] || unset CARGO_BUILD_JOBS; \
    kind="$(cat /src/friglet-build-kind)"; \
    export FRIGLET_NODE_VERSION; \
    if [ "$kind" = dev ]; then \
      export BLINDBIT_RS_SRC=/src/blindbit-rs; \
      set -- --config "patch.'${BLINDBIT_RS_REPO}'.friglet-ipc.path='/src/blindbit-rs/friglet-ipc'"; \
    else \
      unset BLINDBIT_RS_SRC; \
      set -- --locked; \
    fi; \
    for arch in amd64 arm64; do \
      case "$arch" in amd64) triple=x86_64-unknown-linux-gnu ;; arm64) triple=aarch64-unknown-linux-gnu ;; esac; \
      cargo zigbuild "$@" --release -p friglet-ui --target "${triple}.${GLIBC_VERSION}"; \
      install -D -m 0755 "target/${triple}/release/friglet-ui" "/out/bin/linux-$arch/friglet-ui"; \
    done; \
    rm -rf target; \
    printf 'version=%s\nfriglet_rev=%s\nbuild=%s\n' "$FRIGLET_NODE_VERSION" "$BLINDBIT_RS_REV" "$kind" > /out/BUILDINFO

# Export stage: `docker build --target binaries -o <dir> .` yields
#   bin/linux-{amd64,arm64}/{friglet,friglet-ui,LICENSE.friglet}   (tarball contents)
#   image/linux-{amd64,arm64}/tini, image/common/{etc,data}         (runtime image overlay)
#   BUILDINFO                                                        (version, friglet rev, build kind)
FROM scratch AS binaries
COPY --from=builder /out/ /

# The runtime image's additions for one architecture: fs/ goes to /, data/ becomes /data.
# (Legacy builds replace this stage with the same tree assembled from build/out.)
FROM scratch AS rootfs
ARG TARGETARCH
COPY --from=binaries /image/common/etc/ /fs/etc/
COPY --from=binaries /image/linux-${TARGETARCH}/tini /bin/linux-${TARGETARCH}/friglet /bin/linux-${TARGETARCH}/friglet-ui /fs/usr/local/bin/
COPY --from=binaries /image/common/data/ /data/

# --- runtime stage (scripts/build-image.sh reuses everything from here to the end) ---
FROM ${RUNTIME_IMAGE} AS runtime
# The COPYs come first, before any ARG or ENV: the legacy builder with the containerd image
# store rejects a COPY into a foreign-platform stage once a metadata step was committed.
COPY --from=rootfs /fs/ /
COPY --from=rootfs --chown=1000:1000 /data/ /data/
ARG BLINDBIT_RS_REPO
ARG BLINDBIT_RS_REV
ARG FRIGLET_NODE_VERSION
ARG VCS_REF
ARG RUNTIME_IMAGE
ENV FRIGLET_UI_DATA_DIR=/data \
    FRIGLET_UI_LISTEN=0.0.0.0:8097 \
    FRIGLET_UI_FRIGLET_BIN=/usr/local/bin/friglet
LABEL org.opencontainers.image.title="friglet-node" \
      org.opencontainers.image.description="friglet Silent Payments scanner with the friglet-ui setup and status page" \
      org.opencontainers.image.source="https://github.com/setavenger/blindbit-node-apps" \
      org.opencontainers.image.revision="${VCS_REF}" \
      org.opencontainers.image.version="${FRIGLET_NODE_VERSION}" \
      org.opencontainers.image.base.name="${RUNTIME_IMAGE}" \
      dev.blindbit.friglet.source="${BLINDBIT_RS_REPO}" \
      dev.blindbit.friglet.commit="${BLINDBIT_RS_REV}"
WORKDIR /data
VOLUME /data
USER 1000:1000
# 8097 friglet-ui (web UI + JSON API), 50031 Electrum plaintext, 50032 Electrum TLS.
EXPOSE 8097 50031 50032
HEALTHCHECK --interval=30s --timeout=10s --start-period=5m --retries=3 \
    CMD ["/usr/local/bin/friglet-ui", "health"]
ENTRYPOINT ["/usr/local/bin/tini", "--", "/usr/local/bin/friglet-ui"]
CMD ["run"]
