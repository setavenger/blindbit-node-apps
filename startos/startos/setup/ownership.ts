/**
 * StartOS mounts volumes root-owned; friglet-ui runs as uid 1000 and keeps its
 * data dir at 0700 (docs/contract.md). This fixes ownership from the package's
 * own (root) runtime, so it does not depend on a `chown` binary in the image.
 * No SDK imports, so `test/*.test.ts` loads this file directly.
 */
import { chmod, lchown, lstat, readdir } from 'node:fs/promises'
import { join } from 'node:path'

/**
 * Recursively hand `root` to uid:gid (lchown: symlinks are not followed) and
 * set `root` itself to 0700. Returns the number of entries whose owner changed.
 */
export async function ensureOwnership(
  root: string,
  uid: number,
  gid: number,
): Promise<number> {
  let changed = 0
  const visit = async (path: string): Promise<void> => {
    const st = await lstat(path)
    if (st.uid !== uid || st.gid !== gid) {
      await lchown(path, uid, gid)
      changed++
    }
    if (st.isDirectory()) {
      for (const name of await readdir(path)) await visit(join(path, name))
    }
  }
  await visit(root)
  const st = await lstat(root)
  if ((st.mode & 0o777) !== 0o700) await chmod(root, 0o700)
  return changed
}
