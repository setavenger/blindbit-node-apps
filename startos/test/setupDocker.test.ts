// Runs the StartOS package's `friglet-ui setup` invocation and health probes
// against a friglet-node image in plain Docker, the way the "Set up wallet"
// action and main.ts run them in a StartOS subcontainer: same command, env,
// stdin, user and /data mount. Skipped unless FRIGLET_NODE_IMAGE names a
// locally available image, e.g.
//   FRIGLET_NODE_IMAGE=friglet-node:dev-x86_64 npm run test:docker
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { mkdtemp, readdir, rm, stat } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { scanHealth } from '../startos/health/scan.ts'
import {
  buildSetupInvocation,
  FRIGLET_UI_BIN,
  interpretSetupResult,
  type SetupRequest,
} from '../startos/setup/command.ts'
import { ensureOwnership } from '../startos/setup/ownership.ts'
import { writePasswordFile } from '../startos/setup/passwordFile.ts'
import {
  baseEnv,
  electrumPort,
  frigletGid,
  frigletUid,
  uiPort,
} from '../startos/utils.ts'
import { randomDescriptor } from './helpers/descriptor.ts'
import { randomBytes } from 'node:crypto'

const image = process.env.FRIGLET_NODE_IMAGE
const skip = image ? false : 'FRIGLET_NODE_IMAGE is not set'

function docker(args: string[], input?: string) {
  const r = spawnSync('docker', args, {
    input,
    encoding: 'utf8',
    timeout: 120_000,
  })
  return {
    exitCode: r.status,
    exitSignal: r.signal,
    stdout: r.stdout ?? '',
    stderr: r.stderr ?? '',
  }
}

function runSetup(dataDir: string, req: SetupRequest) {
  const inv = buildSetupInvocation(req, baseEnv)
  const envArgs = Object.entries(inv.env).flatMap(([k, v]) => [
    '-e',
    `${k}=${v}`,
  ])
  const [bin, ...args] = inv.command
  const r = docker(
    [
      'run',
      '--rm',
      '-i',
      '--network',
      'none',
      '-v',
      `${dataDir}:/data`,
      ...envArgs,
      '--entrypoint',
      bin,
      image!,
      ...args,
    ],
    inv.input,
  )
  // The secret must never come back on either stream.
  const { descriptor } = req
  assert.ok(
    !r.stdout.includes(descriptor.trim()),
    'descriptor echoed on stdout',
  )
  assert.ok(
    !r.stderr.includes(descriptor.trim()),
    'descriptor echoed on stderr',
  )
  return { raw: r, outcome: interpretSetupResult(r, descriptor) }
}

async function freshDataDir() {
  const dir = await mkdtemp(
    join(process.env.TMPDIR ?? tmpdir(), 'friglet-data-'),
  )
  // What main.ts / the action do before friglet-ui touches /data.
  await ensureOwnership(dir, frigletUid, frigletGid)
  return dir
}

test('setup action: stage, refuse, replace, reject', { skip }, async () => {
  const dir = await freshDataDir()
  try {
    const first = randomDescriptor()
    const ok = runSetup(dir, {
      descriptor: first.descriptor,
      birthday: 840000,
      node: null,
      replace: false,
    })
    assert.equal(ok.raw.exitCode, 0, ok.raw.stdout + ok.raw.stderr)
    assert.ok(ok.outcome.ok && ok.outcome.staged, JSON.stringify(ok.outcome))
    assert.ok(ok.outcome.ok && ok.outcome.spAddress?.startsWith('sp1'))

    const files = await readdir(dir)
    assert.ok(
      files.includes('config.toml') && files.includes('scan.key'),
      files.join(','),
    )
    const key = await stat(join(dir, 'scan.key'))
    assert.equal(key.mode & 0o777, 0o600)
    assert.equal(key.uid, frigletUid)
    assert.equal((await stat(dir)).mode & 0o777, 0o700)

    const again = runSetup(dir, {
      descriptor: first.descriptor,
      birthday: 840000,
      node: null,
      replace: false,
    })
    assert.equal(again.raw.exitCode, 0, 'same wallet and birthday is a no-op')

    const second = randomDescriptor()
    const exists = runSetup(dir, {
      descriptor: second.descriptor,
      birthday: 850000,
      node: null,
      replace: false,
    })
    assert.equal(exists.raw.exitCode, 7)
    assert.ok(!exists.outcome.ok && exists.outcome.failure === 'wallet-exists')

    const replaced = runSetup(dir, {
      descriptor: second.descriptor,
      birthday: 850000,
      node: null,
      replace: true,
    })
    assert.equal(
      replaced.raw.exitCode,
      0,
      replaced.raw.stdout + replaced.raw.stderr,
    )
    assert.ok(replaced.outcome.ok)

    const bad = runSetup(dir, {
      descriptor: 'sp(spscan1notakey)',
      birthday: 850000,
      node: null,
      replace: true,
    })
    assert.equal(bad.raw.exitCode, 3)
    assert.ok(!bad.outcome.ok && bad.outcome.failure === 'descriptor')

    const early = runSetup(dir, {
      descriptor: randomDescriptor().descriptor,
      birthday: 100,
      node: null,
      replace: true,
    })
    assert.equal(early.raw.exitCode, 4)
    assert.ok(!early.outcome.ok && early.outcome.failure === 'birthday')

    const testnet = runSetup(dir, {
      descriptor: randomDescriptor('test').descriptor,
      birthday: 850000,
      node: null,
      replace: true,
    })
    assert.equal(testnet.raw.exitCode, 5)
    assert.ok(!testnet.outcome.ok && testnet.outcome.failure === 'network')

    const node = runSetup(dir, {
      descriptor: randomDescriptor().descriptor,
      birthday: 850000,
      node: '127.0.0.1:1',
      replace: true,
    })
    assert.equal(node.raw.exitCode, 6)
    assert.ok(!node.outcome.ok && node.outcome.failure === 'node')
  } finally {
    await rm(dir, { recursive: true, force: true })
  }
})

test(
  'daemon: entrypoint, env, ports, health probes, stop',
  { skip },
  async () => {
    const dir = await freshDataDir()
    const name = `friglet-startos-test-${process.pid}`
    try {
      const before = docker([
        'run',
        '--rm',
        '-v',
        `${dir}:/data`,
        '--entrypoint',
        FRIGLET_UI_BIN,
        image!,
        'status',
        '--json',
      ])
      const unconfigured = scanHealth(1, before.exitCode, before.stdout)
      assert.equal(unconfigured.kind, 'needs-setup')

      const setup = runSetup(dir, {
        descriptor: randomDescriptor().descriptor,
        birthday: 840000,
        node: null,
        replace: false,
      })
      assert.equal(setup.raw.exitCode, 0)

      const env = { ...baseEnv, FRIGLET_UI_NODE: '127.0.0.1:8333' }
      const envArgs = Object.entries(env).flatMap(([k, v]) => [
        '-e',
        `${k}=${v}`,
      ])
      assert.equal(env.FRIGLET_UI_AUTH, 'password')

      // auth=password without the file: friglet-ui must refuse to start.
      const noFile = docker([
        'run',
        '--rm',
        '-v',
        `${dir}:/data`,
        ...envArgs,
        image!,
      ])
      assert.notEqual(noFile.exitCode, 0, 'started without a password file')

      // What main.ts does before the daemon starts (store -> password file).
      const pw1 = randomBytes(16).toString('hex')
      const pwFile = join(dir, 'ui-password')
      await writePasswordFile(pwFile, pw1, frigletUid, frigletGid)
      const started = docker([
        'run',
        '-d',
        '--name',
        name,
        '-v',
        `${dir}:/data`,
        ...envArgs,
        image!,
      ])
      assert.equal(started.exitCode, 0, started.stderr)

      const listening = (port: number) => {
        const hex = port.toString(16).toUpperCase().padStart(4, '0')
        const r = docker([
          'exec',
          name,
          'cat',
          '/proc/net/tcp',
          '/proc/net/tcp6',
        ])
        return r.stdout
          .split('\n')
          .some((l) => l.includes(`:${hex} `) && / 0A /.test(l))
      }
      let up = false
      for (let i = 0; i < 30 && !up; i++) {
        up = listening(uiPort)
        if (!up) spawnSync('sleep', ['1'])
      }
      assert.ok(up, `nothing listening on ${uiPort}`)
      assert.ok(listening(electrumPort), `nothing listening on ${electrumPort}`)

      const health = docker([
        'exec',
        ...envArgs,
        name,
        FRIGLET_UI_BIN,
        'health',
      ])
      const status = docker([
        'exec',
        ...envArgs,
        name,
        FRIGLET_UI_BIN,
        'status',
        '--json',
      ])
      const h = scanHealth(health.exitCode, status.exitCode, status.stdout)
      assert.ok(['success', 'loading'].includes(h.result), JSON.stringify(h))

      // "Reset web UI password": new file, then a restart picks it up.
      const pw2 = randomBytes(16).toString('hex')
      assert.equal(
        await writePasswordFile(pwFile, pw2, frigletUid, frigletGid),
        true,
      )
      assert.equal(docker(['restart', '-t', '150', name]).exitCode, 0)
      spawnSync('sleep', ['2'])

      const t0 = Date.now()
      const stopped = docker(['stop', '-t', '150', name])
      assert.equal(stopped.exitCode, 0)
      assert.ok(Date.now() - t0 < 150_000)
      const logs = docker(['logs', name])
      const text = logs.stdout + logs.stderr
      assert.ok(!/spscan1/.test(text), 'descriptor in daemon logs')
      assert.ok(!text.includes(pw1) && !text.includes(pw2), 'password in logs')
      assert.equal(
        text.match(/password changed/g)?.length,
        2,
        'friglet-ui should take the password at first start and after the reset',
      )
    } finally {
      docker(['rm', '-f', name])
      await rm(dir, { recursive: true, force: true })
    }
  },
)
