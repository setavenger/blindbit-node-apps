import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  chmod,
  mkdtemp,
  readFile,
  readdir,
  rm,
  stat,
  writeFile,
} from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { writePasswordFile } from '../startos/setup/passwordFile.ts'

test('password file: 0600, one trailing newline, atomic, idempotent', async () => {
  const dir = await mkdtemp(join(process.env.TMPDIR ?? tmpdir(), 'friglet-pw-'))
  const file = join(dir, 'ui-password')
  const uid = process.getuid!()
  const gid = process.getgid!()
  try {
    assert.equal(await writePasswordFile(file, 'abc123', uid, gid), true)
    assert.equal(await readFile(file, 'utf8'), 'abc123\n')
    assert.equal((await stat(file)).mode & 0o777, 0o600)
    assert.equal(await writePasswordFile(file, 'abc123', uid, gid), false)

    // A loose mode left behind is tightened even when the content matches.
    await writeFile(file, 'abc123\n', { mode: 0o644 })
    await chmod(file, 0o644)
    assert.equal(await writePasswordFile(file, 'abc123', uid, gid), false)
    assert.equal((await stat(file)).mode & 0o777, 0o600)

    assert.equal(await writePasswordFile(file, 'new-password', uid, gid), true)
    assert.equal(await readFile(file, 'utf8'), 'new-password\n')
    assert.deepEqual(await readdir(dir), ['ui-password'], 'no temp file left')

    await assert.rejects(writePasswordFile(file, '', uid, gid))
    await assert.rejects(writePasswordFile(file, 'two\nlines', uid, gid))
  } finally {
    await rm(dir, { recursive: true, force: true })
  }
})
