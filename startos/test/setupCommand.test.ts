// Run with: npm test  (node --experimental-strip-types --test test/*.test.ts)
import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  buildSetupInvocation,
  descriptorLooksValid,
  FRIGLET_UI_BIN,
  interpretSetupResult,
  parseSetupJson,
  redactSecrets,
} from '../startos/setup/command.ts'
import { baseEnv } from '../startos/utils.ts'
import { bech32m, randomDescriptor } from './helpers/descriptor.ts'

test('bech32m helper matches the BIP-350 vector', () => {
  assert.equal(bech32m('a', []), 'a1lqfn3a')
})

test('descriptor travels on stdin only, never argv or env', () => {
  const { descriptor, scanSecretHex } = randomDescriptor()
  const inv = buildSetupInvocation(
    {
      descriptor: `  ${descriptor}\n`,
      birthday: 840000,
      node: '10.0.3.1:58334',
      replace: false,
    },
    baseEnv,
  )
  assert.equal(inv.input, `${descriptor}\n`)
  const outside = JSON.stringify([inv.command, inv.env])
  assert.ok(!outside.includes(descriptor))
  assert.ok(!outside.includes(scanSecretHex))
  assert.ok(!outside.includes('spscan1'))
})

test('setup command follows the contract', () => {
  const inv = buildSetupInvocation(
    {
      descriptor: 'sp(spscan1qx)',
      birthday: 840000,
      node: '10.0.3.1:58334',
      replace: true,
    },
    baseEnv,
  )
  assert.deepEqual(inv.command, [
    FRIGLET_UI_BIN,
    'setup',
    '--network',
    'bitcoin',
    '--birthday',
    '840000',
    '--json',
    '--node',
    '10.0.3.1:58334',
    '--replace',
  ])
  assert.equal(inv.env.FRIGLET_UI_NODE, '10.0.3.1:58334')
  assert.equal(inv.env.FRIGLET_UI_PLATFORM, 'startos')
  assert.equal(inv.env.FRIGLET_UI_ELECTRUM_TLS, 'off')
  // friglet-ui refuses any FRIGLET_* variable that is not FRIGLET_UI_*.
  for (const k of Object.keys(inv.env)) assert.match(k, /^FRIGLET_UI_/)
})

test('no node and no replace: those flags are left out', () => {
  const inv = buildSetupInvocation(
    {
      descriptor: 'sp(spscan1qx)',
      birthday: 709632,
      node: null,
      replace: false,
    },
    baseEnv,
  )
  assert.ok(!inv.command.includes('--node'))
  assert.ok(!inv.command.includes('--replace'))
  assert.equal(inv.env.FRIGLET_UI_NODE, undefined)
})

test('descriptor shape check accepts what Sparrow shows', () => {
  const { descriptor } = randomDescriptor()
  assert.ok(descriptorLooksValid(descriptor))
  assert.ok(
    descriptorLooksValid(
      `sp([0f056943/352h/0h/0h]${descriptor.slice(3)}?bh=850000#7eve6al9`,
    ),
  )
  assert.ok(descriptorLooksValid(` ${descriptor} `))
  assert.ok(!descriptorLooksValid(''))
  assert.ok(!descriptorLooksValid('xpub6CUGRUo…'))
  assert.ok(!descriptorLooksValid('sp(spscan1 qqq)'))
  assert.ok(!descriptorLooksValid(`${descriptor}\nsp(spscan1q)`))
})

test('exit codes map to failures', () => {
  const cases: [number | null, string | null, string][] = [
    [2, null, 'usage'],
    [3, null, 'descriptor'],
    [4, null, 'birthday'],
    [5, null, 'network'],
    [6, null, 'node'],
    [7, null, 'wallet-exists'],
    [8, null, 'io'],
    [1, null, 'unknown'],
    [null, 'SIGKILL', 'timeout'],
    [null, null, 'unknown'],
  ]
  for (const [exitCode, exitSignal, failure] of cases) {
    const r = interpretSetupResult({ exitCode, exitSignal, stdout: '' })
    assert.equal(r.ok, false)
    if (!r.ok) assert.equal(r.failure, failure, `exit ${exitCode}`)
  }
})

test('success: staged vs handed over, address taken from --json', () => {
  const staged = interpretSetupResult({
    exitCode: 0,
    stdout:
      'noise\n{"ok":true,"code":0,"state":"staged","sp_address":"sp1qabc","message":"staged, verification pending"}\n',
  })
  assert.deepEqual(staged, {
    ok: true,
    staged: true,
    state: 'staged',
    spAddress: 'sp1qabc',
    message: 'staged, verification pending',
  })
  const live = interpretSetupResult({
    exitCode: 0,
    stdout:
      '{"ok":true,"code":0,"state":"verifying-identity","sp_address":"sp1qabc","message":null}',
  })
  assert.equal(live.ok && live.staged, false)
  const bare = interpretSetupResult({ exitCode: 0, stdout: '' })
  assert.deepEqual(bare, {
    ok: true,
    staged: false,
    state: null,
    spAddress: null,
    message: null,
  })
})

test('messages from friglet-ui are scrubbed of anything secret-looking', () => {
  const { descriptor, scanSecretHex } = randomDescriptor()
  const json = JSON.stringify({
    ok: false,
    code: 3,
    message: `bad ${descriptor} key ${scanSecretHex} and ${descriptor.slice(3, -1)}`,
  })
  const r = interpretSetupResult({ exitCode: 3, stdout: json }, descriptor)
  assert.equal(r.ok, false)
  if (!r.ok) {
    assert.ok(r.message)
    assert.ok(!r.message.includes(scanSecretHex))
    assert.ok(!r.message.includes('spscan1'))
    assert.ok(!r.message.includes(descriptor))
  }
  assert.equal(redactSecrets('plain text'), 'plain text')
})

test('parseSetupJson takes the last JSON object line', () => {
  assert.equal(parseSetupJson('a\n{"code":1}\n{"code":2}\nb')?.code, 2)
  assert.equal(parseSetupJson('[1,2]\nnot json'), null)
})
