ARCHES := x86 arm
# Keep each variant/architecture pair as a leaf target so the shared build and
# release workflows can fan them out across separate runners.
TARGETS := generic-x86 generic-arm nvidia-x86 amd-x86

# overrides to s9pk.mk must precede the include statement
include node_modules/@start9labs/start-sdk/s9pk.mk

.PHONY += generic generic-x86 generic-arm nvidia nvidia-x86 amd amd-x86

# Aggregate targets remain available for local variant builds.
generic: generic-x86 generic-arm
nvidia: nvidia-x86
amd: amd-x86

# Leaf targets build one package each. NVIDIA and AMD remain x86-only.
generic-%:; VARIANT=generic $(MAKE) $*
nvidia-%:; VARIANT=nvidia $(MAKE) $*
amd-%:; VARIANT=amd $(MAKE) $*
