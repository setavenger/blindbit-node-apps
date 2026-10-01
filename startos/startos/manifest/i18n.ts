export const short = {
  en_US: 'Silent Payments scanner and Electrum server for one wallet',
  es_ES: 'Escáner de Pagos Silenciosos y servidor Electrum para una cartera',
  de_DE: 'Silent-Payments-Scanner und Electrum-Server für eine Wallet',
  pl_PL: 'Skaner Silent Payments i serwer Electrum dla jednego portfela',
  fr_FR:
    'Analyseur de Silent Payments et serveur Electrum pour un portefeuille',
}

export const long = {
  en_US:
    "friglet, from the BlindBit project, finds the Silent Payments (BIP352) received by one wallet and serves them to Sparrow over the Electrum protocol. Paste the wallet descriptor Sparrow shows and its birthday height once; friglet fetches blocks from your own Bitcoin node, which must be unpruned. Scanning uses tweak data from a BlindBit oracle run by a third party (oracle.setor.dev): the oracle sees this server's IP address and which block ranges it scans, never your keys. The Electrum port serves this wallet's history to anyone who can reach it. Use a separate Sparrow instance for this wallet: Sparrow's other wallets show no history while it points at friglet.",
  es_ES:
    'friglet, del proyecto BlindBit, encuentra los Pagos Silenciosos (BIP352) que recibe una cartera y se los sirve a Sparrow mediante el protocolo Electrum. Pegue una vez el descriptor de la cartera que muestra Sparrow y su altura de nacimiento; friglet obtiene los bloques de su propio nodo Bitcoin, que no debe estar podado. El escaneo usa datos de ajuste de un oráculo BlindBit operado por un tercero (oracle.setor.dev): el oráculo ve la dirección IP de este servidor y qué rangos de bloques escanea, nunca sus claves. El puerto Electrum entrega el historial de esta cartera a cualquiera que pueda alcanzarlo. Use una instancia separada de Sparrow para esta cartera: las demás carteras de Sparrow no muestran historial mientras apunte a friglet.',
  de_DE:
    'friglet aus dem BlindBit-Projekt findet die Silent Payments (BIP352), die eine Wallet empfängt, und stellt sie Sparrow über das Electrum-Protokoll bereit. Fügen Sie einmal den Wallet-Deskriptor aus Sparrow und die Geburtshöhe ein; friglet lädt die Blöcke von Ihrem eigenen Bitcoin-Knoten, der nicht beschnitten (pruned) sein darf. Der Scan nutzt Tweak-Daten eines BlindBit-Orakels, das ein Dritter betreibt (oracle.setor.dev): Das Orakel sieht die IP-Adresse dieses Servers und welche Blockbereiche er scannt, nie Ihre Schlüssel. Der Electrum-Port liefert den Verlauf dieser Wallet an jeden, der ihn erreichen kann. Nutzen Sie für diese Wallet eine eigene Sparrow-Instanz: Solange Sparrow auf friglet zeigt, zeigen seine anderen Wallets keinen Verlauf.',
  pl_PL:
    'friglet z projektu BlindBit wyszukuje płatności Silent Payments (BIP352) otrzymane przez jeden portfel i udostępnia je Sparrow przez protokół Electrum. Wystarczy raz wkleić deskryptor portfela pokazany w Sparrow oraz wysokość urodzin portfela; friglet pobiera bloki z Twojego własnego węzła Bitcoin, który nie może być przycięty (pruned). Skanowanie korzysta z danych tweak z wyroczni BlindBit prowadzonej przez stronę trzecią (oracle.setor.dev): wyrocznia widzi adres IP tego serwera i skanowane zakresy bloków, nigdy Twoich kluczy. Port Electrum udostępnia historię tego portfela każdemu, kto może się z nim połączyć. Używaj osobnej instancji Sparrow dla tego portfela: pozostałe portfele Sparrow nie pokazują historii, dopóki wskazuje on na friglet.',
  fr_FR:
    "friglet, du projet BlindBit, trouve les Silent Payments (BIP352) reçus par un portefeuille et les sert à Sparrow via le protocole Electrum. Collez une fois le descripteur du portefeuille affiché par Sparrow et sa hauteur de naissance ; friglet récupère les blocs depuis votre propre nœud Bitcoin, qui ne doit pas être élagué (pruned). L'analyse utilise les données de tweak d'un oracle BlindBit exploité par un tiers (oracle.setor.dev) : l'oracle voit l'adresse IP de ce serveur et les plages de blocs analysées, jamais vos clés. Le port Electrum sert l'historique de ce portefeuille à quiconque peut l'atteindre. Utilisez une instance Sparrow distincte pour ce portefeuille : les autres portefeuilles de Sparrow n'affichent aucun historique tant qu'il pointe vers friglet.",
}

export const bitcoindDescription = {
  en_US:
    'Supplies the blocks friglet scans, over a whitelisted peer-to-peer connection. Must be unpruned.',
  es_ES:
    'Proporciona los bloques que friglet escanea, mediante una conexión entre pares en lista blanca. No debe estar podado.',
  de_DE:
    'Liefert die Blöcke, die friglet scannt, über eine freigegebene Peer-to-Peer-Verbindung. Darf nicht beschnitten (pruned) sein.',
  pl_PL:
    'Dostarcza bloki skanowane przez friglet przez uprzywilejowane połączenie peer-to-peer. Nie może być przycięty (pruned).',
  fr_FR:
    'Fournit les blocs que friglet analyse, via une connexion pair-à-pair sur liste blanche. Ne doit pas être élagué (pruned).',
}
