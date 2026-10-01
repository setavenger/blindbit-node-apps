// Constants shared across the package. Container-side values come from
// docs/contract.md (the friglet-node image and the friglet-ui CLI).

/** friglet-ui web UI + JSON API inside the container (contract: FRIGLET_UI_LISTEN). */
export const uiPort = 8097
/** Plaintext Electrum listener owned by friglet-ui (contract: FRIGLET_UI_ELECTRUM_TCP). */
export const electrumPort = 50031
/** External TLS port StartOS prefers for Electrum; assigned at install, never hard-code it in docs. */
export const electrumSslPreferredPort = 50032

/** Host ids (the `sdk.MultiHost.of` groups). Public contract for dependents. */
export const uiHostId = 'ui'
export const electrumHostId = 'electrum'

/** The data directory friglet-ui owns inside every subcontainer (contract: FRIGLET_UI_DATA_DIR). */
export const dataDir = '/data'

/**
 * friglet-ui's web UI password file inside the data dir (contract:
 * FRIGLET_UI_PASSWORD_FILE; `ui-password` is UI credentials, not wallet data).
 */
export const uiPasswordFile = `${dataDir}/ui-password`

/** uid:gid the friglet-node image runs as (contract: "runs as uid:gid 1000:1000"). */
export const frigletUid = 1000
export const frigletGid = 1000

/** friglet-ui waits up to 120 s for friglet on SIGTERM; give it 150 s (docs/plan.md). */
export const sigtermTimeoutMs = 150_000

/**
 * Environment for every friglet-ui process the package starts (daemon, setup,
 * health probes). Only FRIGLET_UI_* variables: friglet-ui refuses to start when
 * any other FRIGLET_* variable is set. The bitcoind peer is passed separately.
 */
export const baseEnv = {
  FRIGLET_UI_PLATFORM: 'startos',
  FRIGLET_UI_DATA_DIR: dataDir,
  FRIGLET_UI_NETWORK: 'bitcoin',
  FRIGLET_UI_LISTEN: `0.0.0.0:${uiPort}`,
  // friglet-ui's own login: port 8097 is reachable from other services and
  // from a LAN marked secure without passing the StartOS proxy.
  FRIGLET_UI_AUTH: 'password',
  FRIGLET_UI_PASSWORD_FILE: uiPasswordFile,
  FRIGLET_UI_ELECTRUM_TCP: `0.0.0.0:${electrumPort}`,
  // StartOS terminates TLS for Electrum (addSsl on the electrum binding).
  FRIGLET_UI_ELECTRUM_TLS: 'off',
} as const
