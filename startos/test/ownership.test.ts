import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, mkdir, writeFile, stat, symlink, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { ensureOwnership } from '../startos/setup/ownership.ts'

// Without root this can only hand files to the current user, which is exactly
// what it must leave alone; the mode fix and the walk are what is checked.
test('ensureOwnership walks the tree and sets the root to 0700', async () => {
  const root = await mkdtemp(
    join(process.env.TMPDIR ?? tmpdir(), 'friglet-own-'),
  )
  try {
    await mkdir(join(root, 'prev'), { mode: 0o755 })
    await writeFile(join(root, 'prev', 'config.toml'), 'x')
    await symlink('/nonexistent', join(root, 'dangling'))
    const uid = process.getuid!()
    const gid = process.getgid!()
    assert.equal(await ensureOwnership(root, uid, gid), 0)
    assert.equal((await stat(root)).mode & 0o777, 0o700)
  } finally {
    await rm(root, { recursive: true, force: true })
  }
})
