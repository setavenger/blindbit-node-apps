import { test } from 'node:test'
import assert from 'node:assert/strict'
import { parseStatus, scanHealth } from '../startos/health/scan.ts'

test('status parser accepts the api.md shapes', () => {
  assert.deepEqual(
    parseStatus(
      '{"state":{"id":"scanning","headline":"Scanning"},"heights":{"scanned":5,"node_tip":9}}',
    ),
    { stateId: 'scanning', headline: 'Scanning', scanned: 5, tip: 9 },
  )
  assert.deepEqual(
    parseStatus('{"state":"up-to-date","scanned_height":9,"tip_height":9}'),
    {
      stateId: 'up-to-date',
      headline: null,
      scanned: 9,
      tip: 9,
    },
  )
  assert.deepEqual(parseStatus('garbage'), {
    stateId: null,
    headline: null,
    scanned: null,
    tip: null,
  })
})

test('health exit 0 is success when caught up, loading while scanning', () => {
  assert.equal(scanHealth(0, 0, '{"state":"up-to-date"}').result, 'success')
  assert.equal(scanHealth(0, 0, '{"state":"scanning"}').result, 'loading')
  assert.equal(scanHealth(0, 0, '').result, 'success')
})

test('not configured is a failure that names the action', () => {
  const h = scanHealth(1, 2, '{"state":"needs-setup"}')
  assert.equal(h.result, 'failure')
  assert.equal(h.kind, 'needs-setup')
})

test('transient and permanent states', () => {
  assert.equal(
    scanHealth(1, 1, '{"state":"verifying-identity"}').result,
    'starting',
  )
  assert.equal(scanHealth(1, 1, '{"state":"node-syncing"}').result, 'loading')
  assert.equal(scanHealth(1, 1, '{"state":"paused"}').result, 'disabled')
  for (const id of [
    'identity-mismatch',
    'node-pruned',
    'oracle-unreachable',
    'crash-loop',
    'port-in-use',
  ]) {
    const h = scanHealth(1, 1, `{"state":{"id":"${id}","headline":"x"}}`)
    assert.equal(h.result, 'failure', id)
    assert.equal(h.kind, 'attention', id)
  }
  assert.equal(scanHealth(null, null, '').result, 'starting')
})
