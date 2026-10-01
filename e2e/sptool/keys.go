package main

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"os"
	"strings"

	"github.com/btcsuite/btcd/btcec/v2"
	"github.com/btcsuite/btcd/btcutil/bech32"
)

// Keys is the content of keys.json. It holds secrets: written 0600, never printed whole.
type Keys struct {
	Network     string `json:"network"`
	ScanSecret  string `json:"scan_secret"`
	SpendSecret string `json:"spend_secret"`
	ScanPubKey  string `json:"scan_pubkey"`
	SpendPubKey string `json:"spend_pubkey"`
	// Address uses the HRP friglet reports for the network (sprt on regtest).
	Address string `json:"address"`
	// Descriptor is BIP-392 sp(<spscan key>) with its BIP-380 checksum.
	Descriptor string `json:"descriptor"`
	// HexEntry is friglet-ui's alternative input "hex:<scan secret>:<spend pubkey>".
	HexEntry string `json:"hex_entry"`
}

// addressHRP follows bdk-sp, whose encoding friglet reports (sp, tsp, sprt).
func addressHRP(network string) (string, error) {
	switch network {
	case "bitcoin", "mainnet":
		return "sp", nil
	case "signet", "testnet", "testnet4":
		return "tsp", nil
	case "regtest":
		return "sprt", nil
	}
	return "", fmt.Errorf("unknown network %q", network)
}

// scanKeyHRP follows BIP-392: spscan on mainnet, tspscan on every test network.
func scanKeyHRP(network string) string {
	if network == "bitcoin" || network == "mainnet" {
		return "spscan"
	}
	return "tspscan"
}

func randomSecret() ([32]byte, error) {
	for {
		var k [32]byte
		if _, err := rand.Read(k[:]); err != nil {
			return k, err
		}
		var s btcec.ModNScalar
		if overflow := s.SetBytes(&k); overflow == 0 && !s.IsZero() {
			return k, nil
		}
	}
}

// encodeVersioned is bech32m(hrp, [version 0] || convertbits(payload, 8, 5)), no length limit,
// the layout of both BIP-352 addresses and BIP-392 spscan keys.
func encodeVersioned(hrp string, payload []byte) (string, error) {
	conv, err := bech32.ConvertBits(payload, 8, 5, true)
	if err != nil {
		return "", err
	}
	return bech32.EncodeM(hrp, append([]byte{0}, conv...))
}

func deriveKeys(network string, scanSec, spendSec [32]byte) (*Keys, error) {
	hrp, err := addressHRP(network)
	if err != nil {
		return nil, err
	}
	_, scanPub := btcec.PrivKeyFromBytes(scanSec[:])
	_, spendPub := btcec.PrivKeyFromBytes(spendSec[:])
	scanPubB, spendPubB := scanPub.SerializeCompressed(), spendPub.SerializeCompressed()
	addr, err := encodeVersioned(hrp, append(append([]byte{}, scanPubB...), spendPubB...))
	if err != nil {
		return nil, err
	}
	scanKey, err := encodeVersioned(scanKeyHRP(network), append(append([]byte{}, scanSec[:]...), spendPubB...))
	if err != nil {
		return nil, err
	}
	desc, err := descsumCreate("sp(" + scanKey + ")")
	if err != nil {
		return nil, err
	}
	return &Keys{
		Network:     network,
		ScanSecret:  hex.EncodeToString(scanSec[:]),
		SpendSecret: hex.EncodeToString(spendSec[:]),
		ScanPubKey:  hex.EncodeToString(scanPubB),
		SpendPubKey: hex.EncodeToString(spendPubB),
		Address:     addr,
		Descriptor:  desc,
		HexEntry:    "hex:" + hex.EncodeToString(scanSec[:]) + ":" + hex.EncodeToString(spendPubB),
	}, nil
}

func cmdKeygen(args []string) error {
	fs := flag.NewFlagSet("keygen", flag.ExitOnError)
	network := fs.String("network", "regtest", "bitcoin, signet, testnet4 or regtest")
	out := fs.String("out", "", "keys.json to write (mode 0600); must not exist")
	_ = fs.Parse(args)
	if *out == "" {
		return errors.New("-out is required")
	}
	scanSec, err := randomSecret()
	if err != nil {
		return err
	}
	spendSec, err := randomSecret()
	if err != nil {
		return err
	}
	k, err := deriveKeys(*network, scanSec, spendSec)
	if err != nil {
		return err
	}
	b, err := json.MarshalIndent(k, "", "  ")
	if err != nil {
		return err
	}
	f, err := os.OpenFile(*out, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return err
	}
	if _, err := f.Write(append(b, '\n')); err != nil {
		f.Close()
		return err
	}
	if err := f.Close(); err != nil {
		return err
	}
	fmt.Println(k.Address)
	return nil
}

func readKeys(path string) (*Keys, error) {
	b, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var k Keys
	if err := json.Unmarshal(b, &k); err != nil {
		return nil, fmt.Errorf("%s: %w", path, err)
	}
	return &k, nil
}

func cmdGet(args []string) error {
	fs := flag.NewFlagSet("get", flag.ExitOnError)
	path := fs.String("keys", "", "keys.json")
	_ = fs.Parse(args)
	if *path == "" || fs.NArg() != 1 {
		return errors.New("usage: sptool get -keys keys.json FIELD")
	}
	k, err := readKeys(*path)
	if err != nil {
		return err
	}
	var m map[string]string
	b, _ := json.Marshal(k)
	_ = json.Unmarshal(b, &m)
	v, ok := m[fs.Arg(0)]
	if !ok {
		names := make([]string, 0, len(m))
		for n := range m {
			names = append(names, n)
		}
		return fmt.Errorf("no field %q (have %s)", fs.Arg(0), strings.Join(names, ", "))
	}
	fmt.Println(v)
	return nil
}

// BIP-380 descriptor checksum.
const (
	descInputCharset    = "0123456789()[],'/*abcdefgh@:$%{}IJKLMNOPQRSTUVWXYZ&+-.;<=>?!^_|~ijklmnopqrstuvwxyzABCDEFGH`#\"\\ "
	descChecksumCharset = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"
)

func descsumPolymod(symbols []uint64) uint64 {
	gen := [5]uint64{0xf5dee51989, 0xa9fdca3312, 0x1bab10e32d, 0x3706b1677a, 0x644d626ffd}
	chk := uint64(1)
	for _, v := range symbols {
		top := chk >> 35
		chk = (chk&0x7ffffffff)<<5 ^ v
		for i := 0; i < 5; i++ {
			if (top>>i)&1 == 1 {
				chk ^= gen[i]
			}
		}
	}
	return chk
}

func descsumCreate(desc string) (string, error) {
	var symbols, groups []uint64
	for _, c := range desc {
		v := strings.IndexRune(descInputCharset, c)
		if v < 0 {
			return "", fmt.Errorf("character %q not allowed in a descriptor", c)
		}
		symbols = append(symbols, uint64(v&31))
		groups = append(groups, uint64(v>>5))
		if len(groups) == 3 {
			symbols = append(symbols, groups[0]*9+groups[1]*3+groups[2])
			groups = groups[:0]
		}
	}
	switch len(groups) {
	case 1:
		symbols = append(symbols, groups[0])
	case 2:
		symbols = append(symbols, groups[0]*3+groups[1])
	}
	symbols = append(symbols, 0, 0, 0, 0, 0, 0, 0, 0)
	chk := descsumPolymod(symbols) ^ 1
	var sb strings.Builder
	sb.WriteString(desc)
	sb.WriteByte('#')
	for i := 0; i < 8; i++ {
		sb.WriteByte(descChecksumCharset[(chk>>(5*(7-i)))&31])
	}
	return sb.String(), nil
}
