import { setupManifest, T } from '@start9labs/start-sdk'
import { FRIGATE_VERSION } from '../versions'
import {
  bitcoindDescription,
  electrumBackendDescription,
  long,
  short,
} from './i18n'

// `remcoros/frigate-docker` is contributor-built, so its tags are not immutable:
// the digest is what selects the image, and the tag rides along only to keep
// this readable. Both must be re-resolved together on a bump — see UPDATING.md.
const defaultSource = {
  dockerTag: `ghcr.io/remcoros/frigate-docker:${FRIGATE_VERSION}@sha256:5c42000d397fedc045ad8d4dfa49f1ca53977c9a450f70180d20580bfba3d67b`,
}

const rocmSource = {
  dockerTag: `ghcr.io/remcoros/frigate-docker:${FRIGATE_VERSION}-rocm@sha256:e12bbc0e0079367cb34aa49ca33159c8ec381a1e5e44643dc74b505e2880d240`,
}

const images = {
  generic: {
    source: defaultSource,
    arch: ['x86_64', 'aarch64'],
  },
  nvidia: {
    source: defaultSource,
    arch: ['x86_64'],
    nvidiaContainer: true,
  },
  amd: {
    source: rocmSource,
    arch: ['x86_64'],
  },
} satisfies Record<string, T.SDKManifest['images'][string]>

type Variant = keyof typeof images

const deviceRequirements: Record<Variant, T.DeviceFilter[]> = {
  generic: [],
  nvidia: [
    {
      class: 'display',
      product: null,
      vendor: null,
      driver: 'nvidia',
      description: 'An NVIDIA GPU',
    },
  ],
  amd: [
    {
      class: 'display',
      product: null,
      vendor: null,
      driver: 'amdgpu',
      description: 'An AMD GPU supported by ROCm',
    },
  ],
}

const isVariant = (value: string): value is Variant => value in images

const variant = process.env.VARIANT ?? 'generic'
if (!isVariant(variant))
  throw new Error(
    `unknown VARIANT '${variant}': expected one of ${Object.keys(images).join(', ')}`,
  )

export const manifest = setupManifest({
  id: 'frigate',
  title: 'Frigate Electrum Server',
  license: 'Apache-2.0',
  packageRepo: 'https://github.com/Start9-Community/frigate-startos',
  upstreamRepo: 'https://github.com/sparrowwallet/frigate',
  marketingUrl: 'https://github.com/sparrowwallet/frigate',
  donationUrl: 'https://sparrowwallet.com/donate/',
  description: { short, long },
  volumes: ['main'],
  images: { main: images[variant] },
  hardwareAcceleration: true,
  hardwareRequirements: {
    device: deviceRequirements[variant],
  },
  dependencies: {
    bitcoind: {
      description: bitcoindDescription,
      optional: false,
      metadata: {
        title: 'Bitcoin',
        icon: 'https://raw.githubusercontent.com/Start9Labs/bitcoin-core-startos/feec0b1dae42961a257948fe39b40caf8672fce1/dep-icon.svg',
      },
    },
    electrs: {
      description: electrumBackendDescription,
      optional: true,
      metadata: {
        title: 'Electrs',
        icon: 'https://raw.githubusercontent.com/Start9-Community/electrs-startos/refs/heads/master/icon.svg',
      },
    },
    fulcrum: {
      description: electrumBackendDescription,
      optional: true,
      metadata: {
        title: 'Fulcrum',
        icon: 'https://raw.githubusercontent.com/Start9Labs/fulcrum-startos/refs/heads/master/icon.png',
      },
    },
  },
})
