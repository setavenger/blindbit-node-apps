// Command sptool is the regtest helper of the friglet-node end-to-end test.
//
//	sptool keygen  -network regtest -out keys.json
//	    Fresh random scan and spend keys. Writes keys.json (mode 0600) with the secrets, the
//	    silent-payment address and the BIP-392 descriptor; prints only the address.
//	sptool get     -keys keys.json FIELD
//	    Prints one field of keys.json (address, descriptor, scan_secret, spend_pubkey, ...),
//	    for piping into `friglet-ui setup` or a key file. Never used on a terminal by the harness.
//	sptool pay     -rpc URL -rpc-user U -rpc-pass P -wallet W -to ADDRESS -amount SATS
//	    Funds a fresh BIP-86 key from the node wallet, builds a BIP-352 payment to ADDRESS
//	    with go-bip352 (as blindbit-testkit's cmd/bootstrap does), broadcasts and confirms it.
//	    Prints {"txid","vout","output_key","amount","height"}.
//	sptool electrum-check -server HOST:PORT -txid TXID -timeout 5m
//	    Waits until the Electrum server lists TXID in blockchain.silentpayments.subscribe and
//	    the payment output's scripthash history contains it. Prints a JSON summary.
//	sptool widget-check -url http://friglet-node:8097/api/widget -min-height H
//	    Waits until friglet-ui's widget reports scanning/up-to-date with scanned_height >= H.
//
// Regtest-only. The keys are throwaway test keys generated at run time.
package main

import (
	"fmt"
	"os"
)

func main() {
	if len(os.Args) < 2 {
		usage()
	}
	var err error
	switch os.Args[1] {
	case "keygen":
		err = cmdKeygen(os.Args[2:])
	case "get":
		err = cmdGet(os.Args[2:])
	case "pay":
		err = cmdPay(os.Args[2:])
	case "electrum-check":
		err = cmdElectrumCheck(os.Args[2:])
	case "widget-check":
		err = cmdWidgetCheck(os.Args[2:])
	default:
		usage()
	}
	if err != nil {
		fmt.Fprintf(os.Stderr, "sptool %s: %v\n", os.Args[1], err)
		os.Exit(1)
	}
}

func usage() {
	fmt.Fprintln(os.Stderr, "usage: sptool keygen|get|pay|electrum-check|widget-check [flags]")
	os.Exit(2)
}
