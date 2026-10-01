import { T, utils } from '@start9labs/start-sdk'
import { storeJson } from '../fileModels/store.json'
import { i18n } from '../i18n'
import { sdk } from '../sdk'
import { writePasswordFile } from '../setup/passwordFile'
import { frigletGid, frigletUid } from '../utils'

/** 32 alphanumerics, from the SDK's generator (never a hand-rolled RNG). */
export const generateUiPassword = () =>
  utils.getDefaultString({ charset: 'a-z,A-Z,0-9', len: 32 })

/** The file friglet-ui reads as FRIGLET_UI_PASSWORD_FILE, seen from the package. */
export const uiPasswordPath = sdk.volumes.main.subpath('ui-password')

/**
 * Write the stored web UI password into friglet-ui's password file (0600,
 * uid 1000). friglet-ui picks it up on its next start.
 */
export async function applyUiPassword(password: string): Promise<boolean> {
  return writePasswordFile(uiPasswordPath, password, frigletUid, frigletGid)
}

const passwordResult = (password: string): T.ActionResultValue => ({
  type: 'single',
  value: password,
  masked: true,
  copyable: true,
  qr: false,
})

export const showUiPassword = sdk.Action.withoutInput(
  'show-ui-password',

  async () => ({
    name: i18n('Show web UI password'),
    description: i18n(
      'Show the password the Web UI asks for on its sign-in page.',
    ),
    warning: null,
    allowedStatuses: 'any',
    group: i18n('Web UI'),
    visibility: 'enabled',
  }),

  async ({ effects }) => {
    const password = await storeJson.read((s) => s.uiPassword).once()
    if (!password) {
      throw new Error(
        i18n('No web UI password is set yet. Run Reset web UI password.'),
      )
    }
    return {
      version: '1',
      title: i18n('Web UI login'),
      message: i18n('Enter this password on the Web UI sign-in page.'),
      result: passwordResult(password),
    }
  },
)

export const resetUiPassword = sdk.Action.withoutInput(
  'reset-ui-password',

  async () => ({
    name: i18n('Reset web UI password'),
    description: i18n(
      'Generate a new random password for the Web UI. The old password stops working and everyone signed in is signed out.',
    ),
    warning: i18n(
      'Replaces the current web UI password and restarts friglet if it is running.',
    ),
    allowedStatuses: 'any',
    group: i18n('Web UI'),
    visibility: 'enabled',
  }),

  async ({ effects }) => {
    const password = generateUiPassword()
    await storeJson.merge(effects, { uiPassword: password })
    await applyUiPassword(password)

    // friglet-ui reads the file only at start, so restart a running service.
    const status = await sdk.getStatus(effects).once()
    const running =
      status?.desired.main === 'running' ||
      status?.desired.main === 'restarting'
    if (running) await sdk.restart(effects)

    return {
      version: '1',
      title: i18n('Web UI login'),
      message: running
        ? i18n(
            'friglet is restarting with the new password. Enter it on the Web UI sign-in page.',
          )
        : i18n(
            'The new password takes effect when friglet starts. Enter it on the Web UI sign-in page.',
          ),
      result: passwordResult(password),
    }
  },
)
