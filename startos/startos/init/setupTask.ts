import { setupWallet } from '../actions/setupWallet'
import { storeJson } from '../fileModels/store.json'
import { i18n } from '../i18n'
import { sdk } from '../sdk'

// friglet cannot scan without a wallet, so "Set up wallet" is a critical task
// until it has succeeded once. Idempotent on its replay key; re-evaluated when
// the store changes.
export const setupTask = sdk.setupOnInit(async (effects) => {
  const configured = await storeJson
    .read((s) => s.walletConfigured)
    .const(effects)
  if (!configured) {
    await sdk.action.createOwnTask(effects, setupWallet, 'critical', {
      reason: i18n(
        'Paste your Silent Payments descriptor and wallet birthday from Sparrow so friglet knows what to scan for.',
      ),
    })
  }
})
