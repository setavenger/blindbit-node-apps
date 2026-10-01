import {
  peerLocalHostId,
  peerPortLocal,
} from 'bitcoin-core-startos/startos/utils'
import { storeJson } from '../fileModels/store.json'
import { i18n } from '../i18n'
import { sdk } from '../sdk'
import {
  buildSetupInvocation,
  descriptorLooksValid,
  interpretSetupResult,
  SETUP_TIMEOUT_MS,
  SetupFailure,
  TAPROOT_ACTIVATION_HEIGHT,
} from '../setup/command'
import { ensureOwnership } from '../setup/ownership'
import { baseEnv, dataDir, frigletGid, frigletUid } from '../utils'

const { InputSpec, Value } = sdk

const oracleDisclosure = () =>
  i18n(
    "friglet asks a BlindBit oracle run by a third party (oracle.setor.dev) for scan data. The oracle sees this server's IP address and which block ranges it scans, never your keys.",
  )

const inputSpec = InputSpec.of({
  descriptor: Value.text({
    name: i18n('Silent Payments descriptor'),
    description: i18n(
      "The Silent Payments descriptor Sparrow shows for this wallet, starting with sp(. It holds the wallet's scan key: friglet keeps it on this server and never shows it again.",
    ),
    required: true,
    default: null,
    masked: true,
    placeholder: 'sp([…]spscan1…)',
    minLength: null,
    maxLength: 4096,
    patterns: [
      {
        regex: '^\\s*(sp\\(|hex:)\\S+\\s*$',
        description: i18n('Starts with sp( and contains no spaces.'),
      },
    ],
    inputmode: 'text',
  }),
  birthday: Value.number({
    name: i18n('Wallet birthday height'),
    description: i18n(
      'The birthday block height your Sparrow wallet uses; once the wallet has received a payment, Sparrow adds it to the descriptor as ?bh=<height>. friglet scans from this block and does not find payments before it. If you are unsure, pick an earlier block.',
    ),
    footnote: oracleDisclosure(),
    required: true,
    default: null,
    min: TAPROOT_ACTIVATION_HEIGHT,
    max: null,
    step: 1,
    integer: true,
    units: null,
    placeholder: '840000',
  }),
})

const failureMessage = (failure: SetupFailure): string => {
  switch (failure) {
    case 'descriptor':
      return i18n(
        'This is not a valid Silent Payments descriptor. Copy the descriptor Sparrow shows for the wallet; it starts with sp(.',
      )
    case 'birthday':
      return i18n(
        'This birthday height is not valid. Use the birthday height your Sparrow wallet uses: at or after block 709632 and not above the tip of your Bitcoin node.',
      )
    case 'network':
      return i18n(
        'This descriptor belongs to a different network. friglet on StartOS scans Bitcoin mainnet; use a mainnet wallet (spscan1…).',
      )
    case 'node':
      return i18n(
        'friglet could not use your Bitcoin node. Make sure Bitcoin is running and not pruned, then run this action again.',
      )
    case 'wallet-exists':
      return i18n(
        'A different wallet is already set up on this server. Run Change wallet to replace it.',
      )
    case 'io':
      return i18n(
        'friglet could not write its files. Check that the server has free disk space, then run this action again.',
      )
    case 'timeout':
      return i18n(
        'Setting up the wallet took longer than 60 seconds and was stopped. Nothing was changed. Try again.',
      )
    case 'usage':
    case 'unknown':
      return i18n(
        'Setting up the wallet failed unexpectedly. Nothing was changed. The service log may say more.',
      )
  }
}

export const setupWallet = sdk.Action.withInput(
  'setup-wallet',

  async ({ effects }) => {
    const configured =
      (await storeJson.read((s) => s.walletConfigured).const(effects)) ?? false
    return configured
      ? {
          name: i18n('Change wallet'),
          description: i18n(
            'Replace the wallet friglet scans for with another one. friglet rescans from the new birthday; set the same birthday in Sparrow.',
          ),
          warning: i18n(
            "This replaces the current wallet. Sparrow pointed at this server will show the new wallet's history only, starting from its birthday.",
          ),
          allowedStatuses: 'any',
          group: i18n('Wallet'),
          visibility: 'enabled',
        }
      : {
          name: i18n('Set up wallet'),
          description: i18n(
            'Tell friglet which Silent Payments wallet to scan for: paste its descriptor and birthday height from Sparrow.',
          ),
          warning: null,
          allowedStatuses: 'any',
          group: i18n('Wallet'),
          visibility: 'enabled',
        }
  },

  inputSpec,

  // Never prefill: the descriptor is a secret and is not stored where the
  // package could read it back.
  async () => null,

  async ({ effects, input }) => {
    if (!descriptorLooksValid(input.descriptor)) {
      throw new Error(failureMessage('descriptor'))
    }
    const configured =
      (await storeJson.read((s) => s.walletConfigured).once()) ?? false

    // bitcoind's whitelisted P2P port over the bridge; null while Bitcoin is
    // not installed. friglet-ui then skips the node check and the daemon picks
    // the address up once Bitcoin appears.
    const node = await sdk.host
      .getBridgeAddress(effects, {
        packageId: 'bitcoind',
        hostId: peerLocalHostId,
        internalPort: peerPortLocal,
      })
      .once()

    await ensureOwnership(sdk.volumes.main.path, frigletUid, frigletGid)

    const invocation = buildSetupInvocation(
      {
        descriptor: input.descriptor,
        birthday: input.birthday,
        node,
        replace: configured,
      },
      baseEnv,
    )

    // A one-off subcontainer on the same volume. With friglet-ui running, the
    // CLI hands the request to it; otherwise it stages the files under an
    // exclusive lock on the data dir (docs/contract.md).
    const result = await sdk.SubContainer.withTemp(
      effects,
      { imageId: 'main' },
      sdk.Mounts.of().mountVolume({
        volumeId: 'main',
        subpath: null,
        mountpoint: dataDir,
        readonly: false,
      }),
      'setup-wallet',
      (sub) =>
        sub.exec(
          invocation.command,
          { env: invocation.env, input: invocation.input },
          SETUP_TIMEOUT_MS,
        ),
    )

    const outcome = interpretSetupResult(result, input.descriptor)
    if (!outcome.ok) {
      if (outcome.failure === 'wallet-exists' && !configured) {
        // The data dir already holds a wallet the store did not know about
        // (for example after a restore); from now on this is "Change wallet".
        await storeJson.merge(effects, { walletConfigured: true })
      }
      console.error(
        `friglet-ui setup failed: exit ${outcome.exitCode}, ${outcome.failure}${outcome.message ? `: ${outcome.message}` : ''}`,
      )
      throw new Error(
        outcome.message
          ? `${failureMessage(outcome.failure)} (${outcome.message})`
          : failureMessage(outcome.failure),
      )
    }

    await storeJson.merge(effects, {
      walletConfigured: true,
      spAddress: outcome.spAddress,
    })

    return {
      version: '1',
      title: configured ? i18n('Wallet changed') : i18n('Wallet set up'),
      message: outcome.staged
        ? i18n(
            'Saved. friglet checks the wallet when the service starts and opens the Electrum port once the address it derives matches. Compare the address below with the Silent Payments receive address in Sparrow.',
          )
        : i18n(
            'friglet is switching to this wallet now and opens the Electrum port once the address it derives matches. Compare the address below with the Silent Payments receive address in Sparrow.',
          ),
      result: outcome.spAddress
        ? {
            type: 'single',
            name: i18n('Silent Payments address'),
            description: null,
            value: outcome.spAddress,
            masked: false,
            copyable: true,
            qr: false,
          }
        : null,
    }
  },
)
