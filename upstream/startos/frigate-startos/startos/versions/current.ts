import { IMPOSSIBLE, VersionInfo } from '@start9labs/start-sdk'

export const FRIGATE_VERSION = '1.5.3.3'

export const current = VersionInfo.of({
  version: '1.5.3:8',
  releaseNotes: {
    en_US:
      'First release on the Start9 Community Registry. Fixes the plaintext Electrum port, which asked for the same external port as the SSL one; marks Electrum addresses as `tcp` and `ssl` so they can be pasted straight into a wallet; raises the Bitcoin version floor to one that actually offers the ZMQ setting Frigate needs; and drops a "Start Indexing on Launch" toggle that never did anything.',
    es_ES:
      'Primera publicación en el Registro Comunitario de Start9. Corrige el puerto Electrum en texto plano, que pedía el mismo puerto externo que el de SSL; marca las direcciones Electrum como `tcp` y `ssl` para poder pegarlas directamente en una cartera; eleva la versión mínima de Bitcoin a una que realmente ofrezca la opción ZMQ que Frigate necesita; y elimina un interruptor «Iniciar la indexación al arrancar» que nunca hizo nada.',
    de_DE:
      'Erste Veröffentlichung in der Start9 Community Registry. Korrigiert den unverschlüsselten Electrum-Port, der denselben externen Port anforderte wie der SSL-Port; kennzeichnet Electrum-Adressen als `tcp` bzw. `ssl`, sodass sie direkt in eine Wallet eingefügt werden können; hebt die Bitcoin-Mindestversion auf eine an, die die von Frigate benötigte ZMQ-Einstellung tatsächlich anbietet; und entfernt den Schalter „Indizierung beim Start beginnen“, der nie etwas bewirkt hat.',
    pl_PL:
      'Pierwsze wydanie w Rejestrze Społeczności Start9. Naprawia nieszyfrowany port Electrum, który żądał tego samego portu zewnętrznego co port SSL; oznacza adresy Electrum jako `tcp` i `ssl`, dzięki czemu można je wkleić wprost do portfela; podnosi minimalną wersję Bitcoina do takiej, która faktycznie udostępnia ustawienie ZMQ wymagane przez Frigate; oraz usuwa przełącznik „Rozpocznij indeksowanie przy starcie”, który nigdy nic nie robił.',
    fr_FR:
      "Première publication sur le Registre Communautaire Start9. Corrige le port Electrum en clair, qui demandait le même port externe que celui en SSL ; marque les adresses Electrum comme `tcp` et `ssl` afin de pouvoir les coller directement dans un portefeuille ; relève la version minimale de Bitcoin à une version qui propose réellement le réglage ZMQ dont Frigate a besoin ; et supprime un interrupteur « Démarrer l'indexation au lancement » qui n'a jamais rien fait.",
  },
  migrations: {
    up: async ({ effects }) => {
      const { ensureStore } = await import('../fileModels/store.json')
      await ensureStore(effects)
    },
    down: IMPOSSIBLE,
  },
})
