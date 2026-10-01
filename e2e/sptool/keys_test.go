package main

import (
	"encoding/hex"
	"strings"
	"testing"

	"github.com/setavenger/go-bip352"
)

func TestDescsumVector(t *testing.T) {
	// BIP-380 test vector.
	got, err := descsumCreate("raw(deadbeef)")
	if err != nil || got != "raw(deadbeef)#89f8spxm" {
		t.Fatalf("descsum: got %q, %v", got, err)
	}
}

func TestAddressMatchesGoBip352(t *testing.T) {
	var scan, spend [32]byte
	scan[31], spend[31] = 1, 2
	k, err := deriveKeys("signet", scan, spend)
	if err != nil {
		t.Fatal(err)
	}
	scanPub, spendPub := bip352.PubKeyFromSecKey(&scan), bip352.PubKeyFromSecKey(&spend)
	want, err := bip352.CreateAddress(scanPub, spendPub, false, 0)
	if err != nil {
		t.Fatal(err)
	}
	if k.Address != want {
		t.Fatalf("address %s, go-bip352 %s", k.Address, want)
	}
	if hex.EncodeToString(spendPub[:]) != k.SpendPubKey {
		t.Fatal("spend pubkey mismatch")
	}
}

func TestRegtestShapes(t *testing.T) {
	var scan, spend [32]byte
	scan[31], spend[31] = 3, 4
	k, err := deriveKeys("regtest", scan, spend)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(k.Address, "sprt1q") {
		t.Fatalf("regtest address %s", k.Address)
	}
	// "sp(" + "tspscan1" + version q + 104 payload + 6 checksum + ")" + "#" 8 = 3+8+1+104+6+1+9.
	if !strings.HasPrefix(k.Descriptor, "sp(tspscan1q") || len(k.Descriptor) != 132 {
		t.Fatalf("descriptor shape: %d chars", len(k.Descriptor))
	}
	// go-bip352 accepts the sprt HRP off mainnet, which `pay` relies on.
	if _, _, err := bip352.DecodeSilentPaymentAddressToKeys(k.Address, false); err != nil {
		t.Fatalf("go-bip352 rejects %s: %v", k.Address, err)
	}
}
