import { store } from './fileModels/store.json'
import { i18n } from './i18n'
import { sdk } from './sdk'
import { autoconfig } from 'bitcoin-core-startos/startos/actions/config/autoconfig'

export const setDependencies = sdk.setupDependencies(async ({ effects }) => {
  await sdk.action.createTask(effects, 'bitcoind', autoconfig, 'critical', {
    input: {
      kind: 'partial',
      accept: [{ prune: 0, txindex: true, zmqEnabled: true }],
      set: { prune: 0, txindex: true, zmqEnabled: true },
    },
    reason: i18n(
      'Frigate requires txindex, pruning disabled, and ZMQ enabled in Bitcoin.',
    ),
    when: { condition: 'input-not-matches', once: false },
  })

  const selection = await store
    .read((value) => value.electrumServer)
    .const(effects)

  return {
    bitcoind: {
      kind: 'running',
      versionRange:
        '(>=28.4:17 && <29) || (>=29.4:4 && <30) || (>=30.3:4 && <31) || >=31.1:4',
      healthChecks: ['bitcoind', 'sync-progress'],
    },
    ...(selection === 'electrs'
      ? {
          electrs: {
            kind: 'running',
            versionRange: '>=0.11.1:11',
            healthChecks: ['electrs', 'sync'],
          },
        }
      : {}),
    ...(selection === 'fulcrum'
      ? {
          fulcrum: {
            kind: 'running',
            versionRange: '>=2.1.1:8',
            healthChecks: ['primary', 'sync-progress'],
          },
        }
      : {}),
  }
})
