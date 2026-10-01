/**
 * Pure helpers for the "Set up wallet" action: build the `friglet-ui setup`
 * invocation (docs/contract.md, "friglet-ui setup CLI") and interpret what it
 * returns. No SDK imports, so `test/*.test.ts` loads this file directly.
 *
 * The descriptor carries the scan secret. It only ever travels on stdin: never
 * argv, never env, never a file on the volume, never an action result.
 */

/** Absolute path from the image contract, so PATH in the image does not matter. */
export const FRIGLET_UI_BIN = '/usr/local/bin/friglet-ui'

/** bitcoind on StartOS is mainnet; friglet-ui calls it `bitcoin`. */
export const NETWORK = 'bitcoin'

/** Mainnet taproot activation: the earliest block a Silent Payment can exist in. */
export const TAPROOT_ACTIVATION_HEIGHT = 709_632

/** docs/plan.md: "Action timeout 60 s". */
export const SETUP_TIMEOUT_MS = 60_000

export type SetupRequest = {
  descriptor: string
  birthday: number
  /** bitcoind's whitelisted P2P bridge address (`host:port`), or null when unknown. */
  node: string | null
  /** Replace a different wallet that is already set up. */
  replace: boolean
}

export type SetupInvocation = {
  command: string[]
  env: Record<string, string>
  /** Written to the process's stdin. */
  input: string
}

/** Descriptor without surrounding whitespace. */
export function normalizeDescriptor(descriptor: string): string {
  return descriptor.trim()
}

/**
 * Cheap shape check before anything runs: friglet-ui does the real parsing.
 * Rejects what would break the stdin framing (whitespace inside) or obviously
 * is not a descriptor.
 */
export function descriptorLooksValid(descriptor: string): boolean {
  const d = normalizeDescriptor(descriptor)
  return (
    d.length > 0 &&
    d.length <= 4096 &&
    !/\s/.test(d) &&
    // sp([origin]spscan1…)?bh=N#checksum (BIP-392/393/380), as Sparrow shows it
    (/^sp\(.+\)\S*$/i.test(d) || /^hex:[0-9a-f]+:[0-9a-f]+$/i.test(d))
  )
}

export function buildSetupInvocation(
  req: SetupRequest,
  baseEnv: Readonly<Record<string, string>>,
): SetupInvocation {
  const command = [
    FRIGLET_UI_BIN,
    'setup',
    '--network',
    NETWORK,
    '--birthday',
    String(req.birthday),
    '--json',
  ]
  if (req.node) command.push('--node', req.node)
  if (req.replace) command.push('--replace')
  const env: Record<string, string> = { ...baseEnv }
  if (req.node) env.FRIGLET_UI_NODE = req.node
  return { command, env, input: `${normalizeDescriptor(req.descriptor)}\n` }
}

/** The `--json` object friglet-ui prints (docs/contract.md). */
export type SetupJson = {
  ok?: boolean
  code?: number
  state?: string | null
  sp_address?: string | null
  message?: string | null
}

export type SetupFailure =
  | 'usage'
  | 'descriptor'
  | 'birthday'
  | 'network'
  | 'node'
  | 'wallet-exists'
  | 'io'
  | 'timeout'
  | 'unknown'

export type SetupOutcome =
  | {
      ok: true
      /** True when friglet-ui staged the files offline (no supervisor running). */
      staged: boolean
      state: string | null
      spAddress: string | null
      message: string | null
    }
  | {
      ok: false
      failure: SetupFailure
      exitCode: number | null
      message: string | null
    }

const FAILURE_BY_EXIT: Record<number, SetupFailure> = {
  2: 'usage',
  3: 'descriptor',
  4: 'birthday',
  5: 'network',
  6: 'node',
  7: 'wallet-exists',
  8: 'io',
}

/**
 * Defensive scrub for anything friglet-ui prints back: drop descriptors, scan
 * keys and long hex runs even though friglet-ui is not supposed to echo them.
 */
export function redactSecrets(text: string, descriptor?: string): string {
  let out = text
  const d = descriptor === undefined ? '' : normalizeDescriptor(descriptor)
  if (d.length >= 8) out = out.split(d).join('[redacted]')
  return out
    .replace(/sp\([^)\s]*\)\S*/gi, '[redacted]')
    .replace(/(spscan|tspscan|sprtscan)1[0-9a-z]{20,}/gi, '[redacted]')
    .replace(/hex:[0-9a-f:]{20,}/gi, '[redacted]')
    .replace(/\b[0-9a-f]{64,}\b/gi, '[redacted]')
}

/** Last line of stdout that parses as a JSON object, if any. */
export function parseSetupJson(stdout: string): SetupJson | null {
  const lines = stdout
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean)
  for (let i = lines.length - 1; i >= 0; i--) {
    try {
      const parsed: unknown = JSON.parse(lines[i])
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        return parsed as SetupJson
      }
    } catch {
      // not JSON; keep looking
    }
  }
  return null
}

function str(value: unknown): string | null {
  return typeof value === 'string' && value.length > 0 ? value : null
}

export function interpretSetupResult(
  result: {
    exitCode: number | null
    exitSignal?: string | null
    stdout: string | Buffer
  },
  descriptor?: string,
): SetupOutcome {
  const json = parseSetupJson(result.stdout.toString())
  const message = str(json?.message)
  const safeMessage =
    message === null ? null : redactSecrets(message, descriptor)

  if (result.exitCode === 0) {
    const state = str(json?.state)
    const spAddress = str(json?.sp_address)
    return {
      ok: true,
      staged: state !== null && /^staged/i.test(state),
      state,
      spAddress:
        spAddress === null ? null : redactSecrets(spAddress, descriptor),
      message: safeMessage,
    }
  }

  let failure: SetupFailure = 'unknown'
  if (result.exitCode === null) {
    // Killed: StartOS's exec timeout SIGKILLs the process.
    failure = result.exitSignal ? 'timeout' : 'unknown'
  } else if (FAILURE_BY_EXIT[result.exitCode]) {
    failure = FAILURE_BY_EXIT[result.exitCode]
  }
  return { ok: false, failure, exitCode: result.exitCode, message: safeMessage }
}
