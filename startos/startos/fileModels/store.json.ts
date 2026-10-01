import { FileHelper, z } from '@start9labs/start-sdk'
import { sdk } from '../sdk'

// StartOS-side state on the `startos` volume, which no subcontainer mounts.
// friglet-ui's own files (config, scan key, state) live on `main` and are
// never modelled here: friglet-ui is the only writer of those.
const shape = z.object({
  // Basic-auth password the StartOS proxy enforces in front of the web UI.
  uiPassword: z.string().optional().catch(undefined),
  // Set once "Set up wallet" succeeds; switches the action to "Change wallet".
  walletConfigured: z.boolean().catch(false),
  // Public Silent Payments address of the configured wallet (not a secret).
  spAddress: z.string().nullable().catch(null),
})

export const storeJson = FileHelper.json(
  { base: sdk.volumes.startos, subpath: 'store.json' },
  shape,
)
