import { setupManifest } from '@start9labs/start-sdk'
import { imageTag } from './image'
import { bitcoindDescription, long, short } from './i18n'

export const manifest = setupManifest({
  // Package ids are permanent on StartOS: an installed service, its backups and
  // any registry listing are keyed by it. Do not rename.
  id: 'blindbit-friglet',
  title: 'friglet',
  license: 'MIT',
  packageRepo:
    'https://github.com/setavenger/blindbit-node-apps/tree/master/startos',
  upstreamRepo: 'https://github.com/setavenger/blindbit-rs',
  marketingUrl: 'https://github.com/setavenger/blindbit-rs',
  donationUrl: null,
  description: { short, long },
  // main: friglet-ui's data dir (/data). startos: store.json, never mounted.
  volumes: ['main', 'startos'],
  images: {
    main: {
      source: { dockerTag: imageTag },
      arch: ['x86_64', 'aarch64'],
    },
  },
  dependencies: {
    bitcoind: {
      description: bitcoindDescription,
      optional: false,
      metadata: {
        title: 'Bitcoin',
        icon: 'https://raw.githubusercontent.com/Start9Labs/bitcoin-core-startos/4ceccce427ae5b17a53a092219fa69d38b781c25/dep-icon.svg',
      },
    },
  },
})
