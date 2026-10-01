import { T } from '@start9labs/start-sdk'
import { sdk } from '../sdk'
import { setInterfaces } from '../interfaces'
import { versionGraph } from '../versions'
import { actions } from '../actions'
import { restoreInit } from '../backups'
import { setDependencies } from '../dependencies'
import { createDefaultConfig } from '../fileModels/config.toml'
import { setConfig } from '../actions/config'
import { ensureStore } from '../fileModels/store.json'
import { i18n } from '../i18n'

type InitKind = 'install' | 'update' | 'restore' | null

async function setupConfig(effects: T.Effects, kind: InitKind) {
  await createDefaultConfig(effects)
  await ensureStore(effects)

  if (kind === 'install') {
    await sdk.action.createOwnTask(effects, setConfig, 'critical', {
      reason: i18n('Configure Frigate settings'),
    })
  }
}

export const init = sdk.setupInit(
  restoreInit,
  versionGraph,
  setInterfaces,
  setDependencies,
  actions,
  setupConfig,
)

export const uninit = sdk.setupUninit(versionGraph)
