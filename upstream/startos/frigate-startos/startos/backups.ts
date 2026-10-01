import { sdk } from './sdk'

export const { createBackup, restoreInit } = sdk.setupBackups(
  async ({ effects }) =>
    // /db is the DuckDB tweak index — large, and rebuilt by re-indexing.
    // frigate.log is an unrotated FileAppender; StartOS captures the same
    // lines from stdout.
    sdk.Backups.ofVolumes('main').setOptions({
      exclude: ['/db', '/frigate.log'],
    }),
)
