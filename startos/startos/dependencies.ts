import { autoconfig } from 'bitcoin-core-startos/startos/actions/config/autoconfig'
import { i18n } from './i18n'
import { sdk } from './sdk'

/**
 * The first bitcoind release on each line that binds `peer-local`, the
 * whitelisted P2P port for services on the bridge (Start9Labs/bitcoin-core-startos
 * commits b3c6423 / ae10150 / 0441748 / 89e11ce, 2026-07-25). friglet fetches
 * blocks over P2P from that port; older releases do not offer it.
 */
export const bitcoindVersionRange =
  '(>=28.4:17 && <29) || (>=29.4:4 && <30) || (>=30.3:4 && <31) || >=31.1:4'

export const setDependencies = sdk.setupDependencies(async ({ effects }) => {
  // friglet downloads every block from the wallet birthday onward from this
  // one node; a pruned node cannot serve them.
  await sdk.action.createTask(effects, 'bitcoind', autoconfig, 'critical', {
    input: {
      kind: 'partial',
      accept: [{ prune: 0 }],
      set: { prune: 0 },
    },
    when: { condition: 'input-not-matches', once: false },
    reason: i18n(
      'friglet downloads every block from your wallet birthday onward from this node, so pruning must be off.',
    ),
  })

  return {
    bitcoind: {
      kind: 'running',
      versionRange: bitcoindVersionRange,
      healthChecks: ['bitcoind', 'sync-progress'],
    },
  }
})
