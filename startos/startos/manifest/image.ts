// The one image this package runs: `friglet-node` (docs/contract.md), which holds
// /usr/local/bin/friglet, /usr/local/bin/friglet-ui and tini, runs as uid 1000
// and has the entrypoint `tini -- friglet-ui`.
//
// Release builds pull the published multi-arch image. Its registry is not
// decided yet (docs/plan.md, open items); the tag follows the contract's
// `<friglet short commit>-<package revision>` scheme. Pin it by digest
// (`…:<tag>@sha256:<digest>`) once it is published, as frigate-startos does.
//
// Local builds set FRIGLET_NODE_IMAGE to a locally built single-arch tag; the
// Makefile's `dev-x86` / `dev-arm` targets use `friglet-node:dev-<arch>`.
// `start-cli s9pk pack` evaluates the manifest once per invocation, so one
// variable per arch-specific pack is enough.
export const FRIGLET_NODE_TAG = '5dbb16f-1'
export const RELEASE_IMAGE = `ghcr.io/setavenger/friglet-node:${FRIGLET_NODE_TAG}`

export const imageTag = process.env.FRIGLET_NODE_IMAGE || RELEASE_IMAGE
