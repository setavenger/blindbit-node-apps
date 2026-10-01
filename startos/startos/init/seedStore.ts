import { generateUiPassword } from '../actions/uiPassword'
import { storeJson } from '../fileModels/store.json'
import { sdk } from '../sdk'

// Generates the web UI password on install, so "Show web UI password" works
// before the first start; main writes it into friglet-ui's password file. A
// restore brings its own store.json back, so this only generates on a fresh
// install (or a lost store).
export const seedStore = sdk.setupOnInit(async (effects) => {
  const store = await storeJson.read().once()
  if (!store?.uiPassword) {
    await storeJson.merge(effects, { uiPassword: generateUiPassword() })
  }
})
