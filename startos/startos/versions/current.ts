import { IMPOSSIBLE, VersionInfo } from '@start9labs/start-sdk'

// friglet has no release tags yet (BR11). The upstream part is friglet's crate
// version at the pinned blindbit-rs commit (versions.env: BLINDBIT_RS_REV
// 93f6a92…, friglet/Cargo.toml 0.1.0); the image tag in manifest/image.ts
// carries the commit. Bump the downstream revision for packaging-only changes.
export const current = VersionInfo.of({
  version: '0.1.0:0',
  releaseNotes: {
    en_US:
      'First release: friglet scans one Silent Payments wallet against your Bitcoin node and serves it to Sparrow over Electrum.',
    es_ES:
      'Primera versión: friglet escanea una cartera de Pagos Silenciosos con su nodo Bitcoin y se la sirve a Sparrow mediante Electrum.',
    de_DE:
      'Erste Version: friglet scannt eine Silent-Payments-Wallet mit Ihrem Bitcoin-Knoten und stellt sie Sparrow über Electrum bereit.',
    pl_PL:
      'Pierwsze wydanie: friglet skanuje jeden portfel Silent Payments przy użyciu Twojego węzła Bitcoin i udostępnia go Sparrow przez Electrum.',
    fr_FR:
      "Première version : friglet analyse un portefeuille Silent Payments à l'aide de votre nœud Bitcoin et le sert à Sparrow via Electrum.",
  },
  migrations: {
    up: async ({ effects }) => {},
    down: IMPOSSIBLE,
  },
})
