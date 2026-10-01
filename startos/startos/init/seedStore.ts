import { generateUiPassword } from '../actions/uiPassword'
import { storeJson } from '../fileModels/store.json'
import { sdk } from '../sdk'

// Runs before setInterfaces, so the web UI's basic-auth gate exists from the
// first moment the interface is bound. A restore brings its own store.json
// back, so this only generates on a fresh install (or a lost store).
export const seedStore = sdk.setupOnInit(async (effects) => {
  const store = await storeJson.read().once()
  if (!store?.uiPassword) {
    await storeJson.merge(effects, { uiPassword: generateUiPassword() })
  }
})
