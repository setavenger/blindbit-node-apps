import { T, utils } from '@start9labs/start-sdk'
import { storeJson } from '../fileModels/store.json'
import { i18n } from '../i18n'
import { sdk } from '../sdk'
import { uiUsername } from '../utils'

/** 32 alphanumerics, from the SDK's generator (never a hand-rolled RNG). */
export const generateUiPassword = () =>
  utils.getDefaultString({ charset: 'a-z,A-Z,0-9', len: 32 })

const credentials = (password: string): T.ActionResultValue => ({
  type: 'group',
  value: [
    {
      type: 'single',
      name: i18n('Username'),
      description: null,
      value: uiUsername,
      masked: false,
      copyable: true,
      qr: false,
    },
    {
      type: 'single',
      name: i18n('Password'),
      description: null,
      value: password,
      masked: true,
      copyable: true,
      qr: false,
    },
  ],
})

export const showUiPassword = sdk.Action.withoutInput(
  'show-ui-password',

  async () => ({
    name: i18n('Show web UI password'),
    description: i18n(
      'Show the username and password your browser asks for when you open the Web UI.',
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
      message: i18n('Use these credentials when your browser asks to sign in.'),
      result: credentials(password),
    }
  },
)

export const resetUiPassword = sdk.Action.withoutInput(
  'reset-ui-password',

  async () => ({
    name: i18n('Reset web UI password'),
    description: i18n(
      'Generate a new random password for the Web UI. The old password stops working.',
    ),
    warning: i18n('Replaces the current web UI password.'),
    allowedStatuses: 'any',
    group: i18n('Web UI'),
    visibility: 'enabled',
  }),

  async ({ effects }) => {
    const password = generateUiPassword()
    // setupInterfaces watches this value and hands the new credential to the
    // StartOS proxy; friglet-ui itself is not restarted.
    await storeJson.merge(effects, { uiPassword: password })
    return {
      version: '1',
      title: i18n('Web UI login'),
      message: i18n(
        'The new password is active. Use these credentials when your browser asks to sign in.',
      ),
      result: credentials(password),
    }
  },
)
