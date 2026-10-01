import {
  peerLocalHostId,
  peerPortLocal,
} from 'bitcoin-core-startos/startos/utils'
import { applyUiPassword, generateUiPassword } from './actions/uiPassword'
import { storeJson } from './fileModels/store.json'
import { scanHealth, ScanHealth } from './health/scan'
import { i18n } from './i18n'
import { sdk } from './sdk'
import { FRIGLET_UI_BIN } from './setup/command'
import { ensureOwnership } from './setup/ownership'
import {
  baseEnv,
  dataDir,
  electrumPort,
  frigletGid,
  frigletUid,
  sigtermTimeoutMs,
  uiPort,
} from './utils'

const PROBE_TIMEOUT_MS = 20_000

const heightsSuffix = (h: ScanHealth): string =>
  h.status.scanned !== null && h.status.tip !== null
    ? ` ${i18n('Block ${scanned} of ${tip}.', {
        scanned: h.status.scanned,
        tip: h.status.tip,
      })}`
    : ''

const scanMessage = (h: ScanHealth): string => {
  switch (h.kind) {
    case 'up-to-date':
      return h.status.scanned !== null
        ? i18n('Caught up with your node at block ${height}.', {
            height: h.status.scanned,
          })
        : i18n('Caught up with your node.')
    case 'scanning':
      return (h.status.headline ?? i18n('Scanning.')) + heightsSuffix(h)
    case 'needs-setup':
      return i18n('No wallet is set up. Run Set up wallet.')
    case 'starting':
      return h.status.headline ?? i18n('friglet is starting.')
    case 'catching-up':
      return (h.status.headline ?? i18n('Catching up.')) + heightsSuffix(h)
    case 'paused':
      return h.status.headline ?? i18n('Scanning is paused in the web UI.')
    case 'unavailable':
      return i18n('friglet is not answering yet.')
    case 'attention':
      return (
        h.status.headline ??
        i18n(
          'friglet needs attention (${state}). Open the web UI for details.',
          {
            state: h.status.stateId ?? '?',
          },
        )
      )
  }
}

export const main = sdk.setupMain(async ({ effects }) => {
  console.info(i18n('Starting friglet'))

  // bitcoind's whitelisted P2P port over the bridge (`10.0.3.1:<port>`).
  // .const() restarts main when it appears, moves or goes away.
  const node = await sdk.host
    .getBridgeAddress(effects, {
      packageId: 'bitcoind',
      hostId: peerLocalHostId,
      internalPort: peerPortLocal,
    })
    .const()

  if (!node) {
    // No peer to scan from: never invent an address. Report why instead.
    return sdk.Daemons.of(effects).addHealthCheck('bitcoin-peer', {
      ready: {
        display: i18n('Bitcoin node'),
        gracePeriod: 0,
        trigger: sdk.trigger.cooldownTrigger(60_000),
        fn: async () =>
          (
            await sdk.checkDependencies(effects, ['bitcoind'])
          ).installedSatisfied('bitcoind')
            ? {
                result: 'failure',
                message: i18n(
                  'Bitcoin does not offer the whitelisted peer port friglet fetches blocks from. Update Bitcoin.',
                ),
              }
            : {
                result: 'loading',
                message: i18n(
                  'Bitcoin is not installed. Install Bitcoin to use friglet.',
                ),
              },
      },
      requires: [],
    })
  }

  // StartOS mounts volumes root-owned; friglet-ui runs as uid 1000.
  await ensureOwnership(sdk.volumes.main.path, frigletUid, frigletGid)

  // friglet-ui's login password, from store.json into FRIGLET_UI_PASSWORD_FILE
  // (re-read by friglet-ui on every start). Read once: "Reset web UI password"
  // rewrites the file and restarts the service itself.
  let uiPassword = await storeJson.read((s) => s.uiPassword).once()
  if (!uiPassword) {
    uiPassword = generateUiPassword()
    await storeJson.merge(effects, { uiPassword })
  }
  await applyUiPassword(uiPassword)

  const env = { ...baseEnv, FRIGLET_UI_NODE: node }

  const subcontainer = sdk.SubContainer.of(
    effects,
    { imageId: 'main' },
    sdk.Mounts.of().mountVolume({
      volumeId: 'main',
      subpath: null,
      mountpoint: dataDir,
      readonly: false,
    }),
    'friglet',
  )

  return sdk.Daemons.of(effects)
    .addDaemon('primary', {
      subcontainer,
      // The image entrypoint, `tini -- friglet-ui`: friglet-ui supervises
      // friglet and forwards SIGTERM, waiting up to 120 s for it.
      exec: {
        command: sdk.useEntrypoint(),
        env,
        sigtermTimeout: sigtermTimeoutMs,
      },
      ready: {
        display: i18n('Web UI'),
        fn: () =>
          sdk.healthCheck.checkPortListening(effects, uiPort, {
            successMessage: i18n('The web UI is ready'),
            errorMessage: i18n('The web UI is not ready'),
          }),
      },
      requires: [],
    })
    .addHealthCheck('scan', {
      ready: {
        display: i18n('Scan'),
        trigger: sdk.trigger.cooldownTrigger(15_000),
        fn: async () => {
          const health = await subcontainer.exec(
            [FRIGLET_UI_BIN, 'health'],
            { env },
            PROBE_TIMEOUT_MS,
          )
          const status = await subcontainer.exec(
            [FRIGLET_UI_BIN, 'status', '--json'],
            { env },
            PROBE_TIMEOUT_MS,
          )
          const h = scanHealth(
            health.exitCode,
            status.exitCode,
            status.stdout.toString(),
          )
          return { result: h.result, message: scanMessage(h) }
        },
      },
      requires: ['primary'],
    })
    .addHealthCheck('electrum', {
      ready: {
        display: i18n('Electrum'),
        fn: async () => {
          const r = await sdk.healthCheck.checkPortListening(
            effects,
            electrumPort,
            {
              successMessage: i18n('Electrum is accepting connections'),
              errorMessage: '',
            },
          )
          return r.result === 'success'
            ? r
            : {
                result: 'starting',
                message: i18n(
                  'Electrum opens once a wallet is set up and friglet has verified its address.',
                ),
              }
        },
      },
      requires: ['primary'],
    })
})
