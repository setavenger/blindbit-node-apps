/**
 * Pure mapping from `friglet-ui health` / `friglet-ui status --json`
 * (docs/contract.md) to a StartOS health result. No SDK imports, so
 * `test/*.test.ts` loads this file directly.
 *
 * Contract: `health` exits 0 only in `scanning` / `up-to-date`; `status --json`
 * exits 0 healthy, 1 attention, 2 not configured and prints the state id and
 * heights. The exact JSON layout is not pinned down yet, so the parser accepts
 * the shapes docs/api.md uses (`state` as an id or as `{id, headline}`,
 * heights flat or under `heights`) and ignores anything else.
 */

export type FrigletStatus = {
  stateId: string | null
  headline: string | null
  scanned: number | null
  tip: number | null
}

export type ScanHealthKind =
  | 'up-to-date'
  | 'scanning'
  | 'needs-setup'
  | 'starting'
  | 'catching-up'
  | 'paused'
  | 'attention'
  | 'unavailable'

export type ScanHealth = {
  result: 'success' | 'loading' | 'starting' | 'disabled' | 'failure'
  kind: ScanHealthKind
  status: FrigletStatus
}

const num = (v: unknown): number | null =>
  typeof v === 'number' && Number.isSafeInteger(v) && v >= 0 ? v : null
const text = (v: unknown): string | null =>
  typeof v === 'string' && v.trim().length > 0 ? v.trim() : null

export function parseStatus(stdout: string): FrigletStatus {
  const empty: FrigletStatus = {
    stateId: null,
    headline: null,
    scanned: null,
    tip: null,
  }
  const lines = stdout
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean)
  let json: Record<string, any> | null = null
  for (let i = lines.length - 1; i >= 0 && !json; i--) {
    try {
      const parsed: unknown = JSON.parse(lines[i])
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        json = parsed as Record<string, any>
      }
    } catch {
      // keep looking
    }
  }
  if (!json) return empty

  const state = json.state
  const stateObj = state && typeof state === 'object' ? state : null
  const heights =
    json.heights && typeof json.heights === 'object' ? json.heights : {}
  return {
    stateId: text(stateObj ? stateObj.id : state) ?? text(json.state_id),
    headline: text(stateObj?.headline) ?? text(json.headline),
    scanned:
      num(heights.scanned) ?? num(json.scanned_height) ?? num(json.scanned),
    tip:
      num(heights.node_tip) ??
      num(heights.tip) ??
      num(json.tip_height) ??
      num(json.node_tip) ??
      num(json.tip),
  }
}

const STARTING = new Set(['starting', 'verifying-identity'])
const CATCHING_UP = new Set([
  'rescanning-after-reset',
  'oracle-dropped-resuming',
  'node-syncing',
])

export function scanHealth(
  healthExit: number | null,
  statusExit: number | null,
  statusStdout: string,
): ScanHealth {
  const status = parseStatus(statusStdout)
  const id = status.stateId

  if (healthExit === 0) {
    if (id === 'up-to-date' || (id === null && statusExit === 0)) {
      return { result: 'success', kind: 'up-to-date', status }
    }
    return { result: 'loading', kind: 'scanning', status }
  }
  if (healthExit === null && statusExit === null) {
    return { result: 'starting', kind: 'unavailable', status }
  }
  if (statusExit === 2 || id === 'needs-setup') {
    return { result: 'failure', kind: 'needs-setup', status }
  }
  if (id !== null && STARTING.has(id)) {
    return { result: 'starting', kind: 'starting', status }
  }
  if (id !== null && CATCHING_UP.has(id)) {
    return { result: 'loading', kind: 'catching-up', status }
  }
  if (id === 'paused') {
    return { result: 'disabled', kind: 'paused', status }
  }
  return { result: 'failure', kind: 'attention', status }
}
