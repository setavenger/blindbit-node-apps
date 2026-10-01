import { sdk } from '../sdk'
import { i18n } from '../i18n'
import {
  createDefaultConfig,
  config,
  ElectrumServerType,
  indexStartHeightDefault,
  isCacheSize,
} from '../fileModels/config.toml'
import { ensureStore, store } from '../fileModels/store.json'

const { InputSpec, Value, Variants } = sdk

const inputSpec = InputSpec.of({
  electrumServer: Value.dynamicUnion(async ({ effects }) => {
    const installedPackages = await effects.getInstalledPackages()
    let serverType: ElectrumServerType = 'none'
    const disabled: ElectrumServerType[] = []

    if (installedPackages.includes('electrs')) {
      serverType = 'electrs'
    } else {
      disabled.push('electrs')
    }

    if (installedPackages.includes('fulcrum')) {
      serverType = 'fulcrum'
    } else {
      disabled.push('fulcrum')
    }

    return {
      name: i18n('Electrum Server'),
      description: i18n('Electrum Backend Server'),
      default: serverType,
      disabled: disabled,
      variants: Variants.of({
        fulcrum: {
          name:
            i18n('Fulcrum (recommended)') +
            (disabled.includes('fulcrum') ? ' ' + i18n('(not installed)') : ''),
          spec: InputSpec.of({}),
        },
        electrs: {
          name:
            i18n('Electrs') +
            (disabled.includes('electrs') ? ' ' + i18n('(not installed)') : ''),
          spec: InputSpec.of({}),
        },
        none: {
          name: i18n('None (not recommended)'),
          spec: InputSpec.of({}),
        },
      }),
    }
  }),
  advanced: Value.object(
    {
      name: i18n('Advanced settings'),
      description: i18n('Advanced settings'),
    },
    InputSpec.of({
      indexStartHeight: Value.number({
        name: i18n('Index Start Height'),
        description: i18n(
          'The block height from which Frigate should start indexing.',
        ),
        required: true,
        integer: true,
        min: 0,
        max: null,
        default: indexStartHeightDefault,
      }),
      scriptPubKeyCacheSize: Value.select({
        name: i18n('Script PubKey Cache Size'),
        description: i18n(
          'Size of the Script PubKey cache (default 10M ≈ 4GB RAM).',
        ),
        values: {
          '1M': '1M',
          '5M': '5M',
          '10M': i18n('10M (default)'),
          '20M': '20M',
          '50M': '50M',
        },
        default: '10M',
      }),
      computeBackend: Value.select({
        name: i18n('Compute Backend'),
        description: i18n(
          'GPU acceleration backend for Silent Payments scanning. AUTO detects and prefers GPU over CPU.',
        ),
        values: {
          AUTO: i18n('Auto (prefer GPU)'),
          GPU: i18n('GPU only'),
          CPU: i18n('CPU only'),
        },
        default: 'AUTO',
      }),
      batchSize: Value.number({
        name: i18n('Batch Size'),
        description: i18n(
          'Rows per GPU dispatch (default 300,000). Reduce if scanning hangs on older GPUs.',
        ),
        required: true,
        integer: true,
        min: 1,
        max: null,
        default: 300000,
      }),
    }),
  ),
})

export const setConfig = sdk.Action.withInput(
  'config',

  async ({ effects }) => ({
    name: i18n('Configure Frigate'),
    description: i18n('Set or update Frigate configuration settings.'),
    warning: null,
    allowedStatuses: 'any',
    group: i18n('Configuration'),
    visibility: 'enabled',
  }),

  inputSpec,

  async ({ effects }) => {
    await createDefaultConfig(effects)
    await ensureStore(effects)
    const currentConfig = (await config.read().once())!
    const currentStore = (await store.read().once())!

    return {
      electrumServer: {
        selection: currentStore.electrumServer,
        value: {},
      },
      advanced: {
        indexStartHeight: currentConfig.index.startHeight,
        scriptPubKeyCacheSize: isCacheSize(currentConfig.index.cacheSize)
          ? currentConfig.index.cacheSize
          : '10M',
        computeBackend: currentConfig.scan.computeBackend,
        batchSize: currentConfig.scan.batchSize,
      },
    }
  },

  async ({ effects, input }) => {
    await store.merge(effects, {
      electrumServer: input.electrumServer.selection,
    })
    await config.merge(effects, {
      index: {
        startHeight: input.advanced.indexStartHeight,
        cacheSize: input.advanced.scriptPubKeyCacheSize,
      },
      scan: {
        computeBackend: input.advanced.computeBackend,
        batchSize: input.advanced.batchSize,
      },
    })
  },
)
