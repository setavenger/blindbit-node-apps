export const DEFAULT_LANG = 'en_US'

const dict = {
  // main.ts
  'Starting Frigate...': 1,
  'Frigate is running': 2,
  'Frigate is syncing...': 3,
  'Sync Progress': 4,
  'Fully synced': 6,

  // interfaces.ts
  'Electrum (SSL)': 100,
  'The Electrum protocol endpoint, served over SSL': 101,

  // actions/config.ts
  'Configure Frigate': 200,
  'Set or update Frigate configuration settings.': 201,
  'Electrum Server': 202,
  'Electrum Backend Server': 203,
  'Fulcrum (recommended)': 204,
  '(not installed)': 205,
  Electrs: 206,
  'None (not recommended)': 207,
  'Advanced settings': 208,
  'Index Start Height': 211,
  'The block height from which Frigate should start indexing.': 212,
  'Script PubKey Cache Size': 213,
  'Size of the Script PubKey cache (default 10M ≈ 4GB RAM).': 214,
  Configuration: 215,
  'Compute Backend': 216,
  'GPU acceleration backend for Silent Payments scanning. AUTO detects and prefers GPU over CPU.': 217,
  'Auto (prefer GPU)': 218,
  'GPU only': 219,
  'CPU only': 220,
  'Batch Size': 221,
  'Rows per GPU dispatch (default 300,000). Reduce if scanning hangs on older GPUs.': 222,
  '10M (default)': 223,

  // dependencies.ts and init/index.ts
  'Frigate requires txindex, pruning disabled, and ZMQ enabled in Bitcoin.': 300,
  'Configure Frigate settings': 301,
} as const

/**
 * Plumbing. DO NOT EDIT.
 */
export type I18nKey = keyof typeof dict
export type LangDict = Record<(typeof dict)[I18nKey], string>
export default dict
