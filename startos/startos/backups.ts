import { sdk } from './sdk'

// The service is stopped while StartOS backs up, so the state file is
// quiescent. Kept on purpose: restoring it avoids a full rescan. The backup
// therefore holds the scan key (scan.key and the state file), encrypted with
// the StartOS master password like every StartOS backup.
// Sockets are excluded; rsync would skip them anyway.
export const { createBackup, restoreInit } = sdk.setupBackups(
  async ({ effects }) =>
    sdk.Backups.ofVolumes('main', 'startos').setOptions({
      exclude: ['*.sock'],
    }),
)
