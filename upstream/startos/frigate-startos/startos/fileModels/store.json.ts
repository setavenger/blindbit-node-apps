import { FileHelper, T, z } from '@start9labs/start-sdk'
import { sdk } from '../sdk'
import {
  config,
  ElectrumServerType,
  legacyElectrumServerByUrl,
} from './config.toml'

const shape = z.object({
  electrumServer: z
    .union([z.literal('fulcrum'), z.literal('electrs'), z.literal('none')])
    .catch('none'),
})

export const store = FileHelper.json(
  { base: sdk.volumes.main, subpath: '/store.json' },
  shape,
)

export async function ensureStore(effects: T.Effects) {
  if (await store.read().once()) return
  const existingConfig = await config.read().once()
  const electrumServer: ElectrumServerType = existingConfig
    ? (legacyElectrumServerByUrl[existingConfig.server.backendElectrumServer] ??
      'none')
    : 'none'
  await store.write(effects, { electrumServer })
}
