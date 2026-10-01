import { i18n } from './i18n'
import { sdk } from './sdk'
import {
  electrumHostId,
  electrumPort,
  electrumSslPreferredPort,
  uiHostId,
  uiPort,
} from './utils'

export const setInterfaces = sdk.setupInterfaces(async ({ effects }) => {
  const receipts = []

  // Web UI. `protocol: 'http'` publishes plaintext only where StartOS trusts
  // the path (this server, other services on the bridge, a LAN marked
  // secure); everything else goes through the StartOS proxy's TLS. friglet-ui
  // runs its own password login on every path (FRIGLET_UI_AUTH=password).
  const uiOrigin = await sdk.MultiHost.of(effects, uiHostId).bindPort(uiPort, {
    protocol: 'http',
  })
  const ui = sdk.createInterface(effects, {
    name: i18n('Web UI'),
    id: 'ui',
    description: i18n(
      'Wallet status, scan progress and connection details. Sign in with the password from the Show web UI password action.',
    ),
    type: 'ui',
    masked: false,
    schemeOverride: null,
    username: null,
    path: '',
    query: {},
  })
  receipts.push(await uiOrigin.export([ui]))

  // Electrum, the electrs/fulcrum/frigate way: friglet-ui listens in plaintext
  // and StartOS terminates TLS with the server's root CA. `secure: null`
  // keeps the plaintext port on the bridge only, so off the box the TLS port
  // is all there is.
  const electrumOrigin = await sdk.MultiHost.of(
    effects,
    electrumHostId,
  ).bindPort(electrumPort, {
    protocol: null,
    addSsl: {
      preferredExternalPort: electrumSslPreferredPort,
      alpn: null,
      addXForwardedHeaders: false,
      auth: null,
    },
    preferredExternalPort: electrumPort,
    secure: null,
  })
  const electrum = sdk.createInterface(effects, {
    name: i18n('Electrum (SSL)'),
    id: 'electrum',
    description: i18n(
      "Electrum endpoint for Sparrow, served over SSL. Anyone who can reach it can read this wallet's history.",
    ),
    type: 'api',
    masked: false,
    // protocol: null leaves the origin scheme-less; mark the addresses so they
    // paste straight into a wallet.
    schemeOverride: { ssl: 'ssl', noSsl: 'tcp' },
    username: null,
    path: '',
    query: {},
  })
  receipts.push(await electrumOrigin.export([electrum]))

  return receipts
})
