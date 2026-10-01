/**
 * friglet-ui's FRIGLET_UI_PASSWORD_FILE (docs/contract.md): the package writes
 * the web UI password from store.json into the data volume. friglet-ui re-reads
 * it on every start (one trailing newline trimmed); a different value replaces
 * its stored hash and ends all sessions. No SDK imports, so `test/*.test.ts`
 * loads this file directly.
 */
import {
  chmod,
  lchown,
  readFile,
  rename,
  rm,
  writeFile,
} from 'node:fs/promises'

/**
 * Write `password` to `path` as uid:gid, mode 0600, atomically (temp file in
 * the same directory, then rename). Returns false when the file already held
 * exactly this content with the right owner and mode.
 */
export async function writePasswordFile(
  path: string,
  password: string,
  uid: number,
  gid: number,
): Promise<boolean> {
  if (!password || /[\r\n]/.test(password)) {
    throw new Error('web UI password must be a single non-empty line')
  }
  const content = `${password}\n`
  const current = await readFile(path, 'utf8').catch(() => null)
  if (current === content) {
    // Ownership and mode are re-asserted even when the content matches.
    await lchown(path, uid, gid)
    await chmod(path, 0o600)
    return false
  }
  const tmp = `${path}.tmp`
  await rm(tmp, { force: true })
  await writeFile(tmp, content, { mode: 0o600, flag: 'wx' })
  try {
    await lchown(tmp, uid, gid)
    await chmod(tmp, 0o600)
    await rename(tmp, path)
  } catch (e) {
    await rm(tmp, { force: true })
    throw e
  }
  return true
}
