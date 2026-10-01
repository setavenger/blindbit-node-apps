export const short = {
  en_US: 'Electrum server with Silent Payments scanning',
  es_ES: 'Servidor Electrum con escaneo de Pagos Silenciosos',
  de_DE: 'Electrum-Server mit Silent-Payments-Scan',
  pl_PL: 'Serwer Electrum ze skanowaniem Silent Payments',
  fr_FR: 'Serveur Electrum avec analyse des Silent Payments',
}

export const long = {
  en_US:
    'Frigate is an experimental Electrum server from the Sparrow Wallet team. It scans the blockchain for Silent Payments (BIP352) on behalf of wallets, using ephemeral client keys so the server never learns which payments belong to whom, and can offload the scan to a GPU. Pair it with Fulcrum or Electrs to serve ordinary address lookups alongside Silent Payments.',
  es_ES:
    'Frigate es un servidor Electrum experimental del equipo de Sparrow Wallet. Escanea la cadena de bloques en busca de Pagos Silenciosos (BIP352) en nombre de las carteras, usando claves de cliente efímeras para que el servidor nunca sepa qué pagos pertenecen a quién, y puede delegar el escaneo a una GPU. Combínalo con Fulcrum o Electrs para atender también las búsquedas de direcciones habituales.',
  de_DE:
    'Frigate ist ein experimenteller Electrum-Server vom Team hinter Sparrow Wallet. Er durchsucht die Blockchain im Auftrag von Wallets nach Silent Payments (BIP352) und nutzt dabei kurzlebige Client-Schlüssel, sodass der Server nie erfährt, welche Zahlung zu wem gehört; der Scan kann auf eine GPU ausgelagert werden. In Kombination mit Fulcrum oder Electrs beantwortet er zusätzlich gewöhnliche Adressabfragen.',
  pl_PL:
    'Frigate to eksperymentalny serwer Electrum od zespołu Sparrow Wallet. Skanuje łańcuch bloków w poszukiwaniu Silent Payments (BIP352) w imieniu portfeli, używając efemerycznych kluczy klienta, dzięki czemu serwer nigdy nie dowiaduje się, do kogo należą płatności, a samo skanowanie może przenieść na GPU. W połączeniu z Fulcrum lub Electrs obsługuje także zwykłe wyszukiwanie adresów.',
  fr_FR:
    "Frigate est un serveur Electrum expérimental de l'équipe de Sparrow Wallet. Il analyse la chaîne de blocs à la recherche de Silent Payments (BIP352) pour le compte des portefeuilles, à l'aide de clés client éphémères pour que le serveur n'apprenne jamais à qui appartient un paiement, et peut confier l'analyse à un GPU. Associé à Fulcrum ou Electrs, il répond en plus aux recherches d'adresses classiques.",
}

export const bitcoindDescription = {
  en_US: 'Provides the blocks Frigate indexes, and new block events over ZMQ.',
  es_ES:
    'Proporciona los bloques que Frigate indexa y los eventos de nuevos bloques por ZMQ.',
  de_DE:
    'Liefert die Blöcke, die Frigate indiziert, sowie neue Blockereignisse über ZMQ.',
  pl_PL:
    'Dostarcza bloki indeksowane przez Frigate oraz zdarzenia nowych bloków przez ZMQ.',
  fr_FR:
    'Fournit les blocs que Frigate indexe ainsi que les événements de nouveaux blocs via ZMQ.',
}

export const electrumBackendDescription = {
  en_US: 'Serves ordinary address lookups alongside Silent Payments.',
  es_ES:
    'Atiende las búsquedas de direcciones habituales junto a los Pagos Silenciosos.',
  de_DE:
    'Beantwortet gewöhnliche Adressabfragen zusätzlich zu Silent Payments.',
  pl_PL: 'Obsługuje zwykłe wyszukiwanie adresów obok Silent Payments.',
  fr_FR:
    "Répond aux recherches d'adresses classiques en plus des Silent Payments.",
}
