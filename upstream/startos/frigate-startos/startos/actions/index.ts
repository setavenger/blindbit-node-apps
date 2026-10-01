import { sdk } from '../sdk'
import { setConfig } from './config'

export const actions = sdk.Actions.of().addAction(setConfig)
