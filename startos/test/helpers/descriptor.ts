// Generates a fresh, well-formed Silent Payments watch-only descriptor at test
// time (BIP-392 `sp(spscan1q…)`: bech32m, version q, payload = 32-byte scan
// secret || 33-byte spend public key). Random keys every run; nothing
// secret is committed.
import { createECDH } from 'node:crypto'

const CHARSET = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l'
const BECH32M_CONST = 0x2bc830a3

function polymod(values: number[]): number {
  const GEN = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]
  let chk = 1
  for (const v of values) {
    const top = chk >>> 25
    chk = ((chk & 0x1ffffff) << 5) ^ v
    for (let i = 0; i < 5; i++) if ((top >>> i) & 1) chk ^= GEN[i]
  }
  return chk >>> 0
}

function hrpExpand(hrp: string): number[] {
  const out: number[] = []
  for (const c of hrp) out.push(c.charCodeAt(0) >>> 5)
  out.push(0)
  for (const c of hrp) out.push(c.charCodeAt(0) & 31)
  return out
}

function toWords(bytes: Uint8Array): number[] {
  let acc = 0
  let bits = 0
  const out: number[] = []
  for (const b of bytes) {
    acc = (acc << 8) | b
    bits += 8
    while (bits >= 5) {
      bits -= 5
      out.push((acc >>> bits) & 31)
    }
  }
  if (bits > 0) out.push((acc << (5 - bits)) & 31)
  return out
}

export function bech32m(hrp: string, data: number[]): string {
  const values = [...hrpExpand(hrp), ...data, 0, 0, 0, 0, 0, 0]
  const mod = polymod(values) ^ BECH32M_CONST
  const checksum = [0, 1, 2, 3, 4, 5].map((i) => (mod >>> (5 * (5 - i))) & 31)
  return `${hrp}1${[...data, ...checksum].map((d) => CHARSET[d]).join('')}`
}

/** `sp(spscan1q…)` for mainnet, `sp(tspscan1q…)` for test networks. */
export function randomDescriptor(network: 'main' | 'test' = 'main'): {
  descriptor: string
  scanSecretHex: string
} {
  const scan = createECDH('secp256k1')
  scan.generateKeys()
  const spend = createECDH('secp256k1')
  spend.generateKeys()
  const scanSecret = scan.getPrivateKey()
  // getPrivateKey() drops leading zero bytes; pad back to 32.
  const scan32 = Buffer.concat([
    Buffer.alloc(32 - scanSecret.length),
    scanSecret,
  ])
  const spendPub = spend.getPublicKey(null, 'compressed')
  const payload = Buffer.concat([scan32, spendPub])
  const hrp = network === 'main' ? 'spscan' : 'tspscan'
  const key = bech32m(hrp, [0, ...toWords(payload)])
  return { descriptor: `sp(${key})`, scanSecretHex: scan32.toString('hex') }
}
