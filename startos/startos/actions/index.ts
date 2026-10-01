import { sdk } from '../sdk'
import { setupWallet } from './setupWallet'
import { resetUiPassword, showUiPassword } from './uiPassword'

export const actions = sdk.Actions.of()
  .addAction(setupWallet)
  .addAction(showUiPassword)
  .addAction(resetUiPassword)
