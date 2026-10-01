# Unraid Templates
Docker container templates for Unraid.

## Templates
* [bitcoind](bitcoind.xml)
  * Don't trust, verify! A full Bitcoin Core node to maintain your autonomy
  * Based on trustless [docker-bitcoind](https://github.com/ofawx/docker-bitcoind/)
* [electrs](electrs.xml)
  * A lightweight personal Electrum server
  * Based on [docker-electrs](https://github.com/ofawx/docker-electrs/)
* [electrumx](electrumx.xml)
  * A high-performing Electrum server
  * Based on [docker-electrumx](https://github.com/ofawx/docker-electrumx/)

## FAQ
* Should I use ElectrumX or electrs?
  * ElectrumX is a high-performance server appropriate for serving many clients, including yourself and optionally the wider public. It is faster (<1 second responses), but consumes more resources.
  * electrs is a low-footprint server that is appropriate for personal use or with a small number of trusted friends. Requests are served more slowly (3+ seconds), but the server consumes fewer resources.
  * Or run both! I run a public ElectrumX server to support the network, and retain a personal electrs server to ensure uptime and privacy for myself.

## Support
[Support Thread](https://forums.unraid.net/topic/112959-support-ofawx-templates-repo/)