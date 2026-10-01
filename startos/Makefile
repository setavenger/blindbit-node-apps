ARCHES := x86 arm
# overrides to s9pk.mk must precede the include statement
include node_modules/@start9labs/start-sdk/s9pk.mk

# Local builds against friglet-node images built on this machine
# (`friglet-node:dev-x86_64`, `friglet-node:dev-aarch64`; see
# docs/platforms/startos.md). A plain `make` packs the published image named
# in startos/manifest/image.ts instead.
.PHONY: dev dev-x86 dev-arm stub-image
dev: dev-x86 dev-arm

dev-x86:
	rm -f $(BASE_NAME)_x86_64.s9pk
	FRIGLET_NODE_IMAGE=friglet-node:dev-x86_64 $(MAKE) --no-print-directory x86

dev-arm:
	rm -f $(BASE_NAME)_aarch64.s9pk
	FRIGLET_NODE_IMAGE=friglet-node:dev-aarch64 $(MAKE) --no-print-directory arm

# Stand-in for the real image while it does not exist yet: busybox scripts
# with the contract's entrypoint and binary names (test/stub-image/Dockerfile).
stub-image:
	docker build --platform linux/amd64 -t friglet-node:dev-x86_64 test/stub-image
	docker build --platform linux/arm64 -t friglet-node:dev-aarch64 test/stub-image
