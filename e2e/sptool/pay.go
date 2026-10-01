package main

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	"github.com/btcsuite/btcd/btcec/v2"
	"github.com/btcsuite/btcd/btcec/v2/schnorr"
	"github.com/btcsuite/btcd/btcutil"
	"github.com/btcsuite/btcd/chaincfg"
	"github.com/btcsuite/btcd/chaincfg/chainhash"
	"github.com/btcsuite/btcd/txscript"
	"github.com/btcsuite/btcd/wire"
	"github.com/setavenger/go-bip352"
)

const feeSats = 2000

type rpcClient struct {
	url, user, pass, wallet string
	http                    *http.Client
}

// call runs one JSON-RPC request; wallet calls go to /wallet/<name>.
func (c *rpcClient) call(wallet bool, method string, params ...any) (json.RawMessage, error) {
	if params == nil {
		params = []any{}
	}
	body, _ := json.Marshal(map[string]any{"jsonrpc": "1.0", "id": "sptool", "method": method, "params": params})
	url := c.url
	if wallet {
		url = strings.TrimRight(url, "/") + "/wallet/" + c.wallet
	}
	req, err := http.NewRequest(http.MethodPost, url, bytes.NewReader(body))
	if err != nil {
		return nil, err
	}
	req.SetBasicAuth(c.user, c.pass)
	req.Header.Set("Content-Type", "application/json")
	resp, err := c.http.Do(req)
	if err != nil {
		return nil, fmt.Errorf("%s: %w", method, err)
	}
	defer resp.Body.Close()
	raw, _ := io.ReadAll(resp.Body)
	var out struct {
		Result json.RawMessage `json:"result"`
		Error  *struct {
			Code    int    `json:"code"`
			Message string `json:"message"`
		} `json:"error"`
	}
	if err := json.Unmarshal(raw, &out); err != nil {
		return nil, fmt.Errorf("%s: HTTP %d: %s", method, resp.StatusCode, strings.TrimSpace(string(raw)))
	}
	if out.Error != nil {
		return nil, fmt.Errorf("%s: %d %s", method, out.Error.Code, out.Error.Message)
	}
	return out.Result, nil
}

func (c *rpcClient) mineOne() (string, error) {
	var addr string
	raw, err := c.call(true, "getnewaddress")
	if err != nil {
		return "", err
	}
	if err := json.Unmarshal(raw, &addr); err != nil {
		return "", err
	}
	var hashes []string
	raw, err = c.call(false, "generatetoaddress", 1, addr)
	if err != nil {
		return "", err
	}
	if err := json.Unmarshal(raw, &hashes); err != nil || len(hashes) != 1 {
		return "", fmt.Errorf("generatetoaddress: %s", raw)
	}
	return hashes[0], nil
}

type rawTx struct {
	Txid string `json:"txid"`
	Vout []struct {
		Value        float64 `json:"value"`
		N            uint32  `json:"n"`
		ScriptPubKey struct {
			Hex string `json:"hex"`
		} `json:"scriptPubKey"`
	} `json:"vout"`
}

func cmdPay(args []string) error {
	fs := flag.NewFlagSet("pay", flag.ExitOnError)
	rpcURL := fs.String("rpc", "http://bitcoind:18443", "bitcoind RPC URL")
	user := fs.String("rpc-user", "bbtk", "RPC user")
	pass := fs.String("rpc-pass", "bbtk-regtest-only", "RPC password")
	wallet := fs.String("wallet", "bbtk", "funding wallet loaded in bitcoind")
	to := fs.String("to", "", "silent-payment address (sprt1… or tsp1…)")
	amount := fs.Uint64("amount", 100_000, "amount in sats")
	_ = fs.Parse(args)
	if *to == "" {
		return errors.New("-to is required")
	}
	c := &rpcClient{url: *rpcURL, user: *user, pass: *pass, wallet: *wallet, http: &http.Client{Timeout: 60 * time.Second}}

	// 1. A fresh BIP-86 key-path P2TR output, funded by the node wallet and confirmed.
	fundSec, err := randomSecret()
	if err != nil {
		return err
	}
	priv, _ := btcec.PrivKeyFromBytes(fundSec[:])
	outKey := txscript.ComputeTaprootKeyNoScript(priv.PubKey())
	pkScript, err := txscript.PayToTaprootScript(outKey)
	if err != nil {
		return err
	}
	fundAddr, err := btcutil.NewAddressTaproot(schnorr.SerializePubKey(outKey), &chaincfg.RegressionNetParams)
	if err != nil {
		return err
	}
	fundValue := *amount + feeSats + 10_000
	var fundTxid string
	raw, err := c.call(true, "sendtoaddress", fundAddr.EncodeAddress(), btcutil.Amount(fundValue).ToBTC())
	if err != nil {
		return err
	}
	if err := json.Unmarshal(raw, &fundTxid); err != nil {
		return err
	}
	blockHash, err := c.mineOne()
	if err != nil {
		return err
	}
	raw, err = c.call(false, "getrawtransaction", fundTxid, true, blockHash)
	if err != nil {
		return err
	}
	var ftx rawTx
	if err := json.Unmarshal(raw, &ftx); err != nil {
		return err
	}
	vout, value := uint32(0), uint64(0)
	found := false
	for _, o := range ftx.Vout {
		if o.ScriptPubKey.Hex == hex.EncodeToString(pkScript) {
			vout, value, found = o.N, uint64(o.Value*1e8+0.5), true
		}
	}
	if !found {
		return fmt.Errorf("funding tx %s has no output to %s", fundTxid, fundAddr)
	}

	// 2. BIP-352 output for the recipient, from the input's tweaked (BIP-86) secret key.
	txidBytes, err := hex.DecodeString(fundTxid)
	if err != nil || len(txidBytes) != 32 {
		return fmt.Errorf("bad txid %q", fundTxid)
	}
	var txidArr [32]byte
	copy(txidArr[:], txidBytes)
	tweaked := txscript.TweakTaprootPrivKey(*priv, []byte{})
	var tweakedSec [32]byte
	copy(tweakedSec[:], tweaked.Serialize())
	vins := []*bip352.Vin{{Txid: txidArr, Vout: vout, Amount: value, SecretKey: &tweakedSec, Taproot: true}}
	recipients := []*bip352.Recipient{{SilentPaymentAddress: *to, Amount: *amount}}
	if err := bip352.SenderCreateOutputs(recipients, vins, false, false); err != nil {
		return fmt.Errorf("derive silent-payment output: %w", err)
	}
	xonly := recipients[0].Output

	// 3. Spend: SP output first, change back to the funding script; key-path signature.
	tx := wire.NewMsgTx(2)
	prevHash, err := chainhash.NewHashFromStr(fundTxid)
	if err != nil {
		return err
	}
	tx.AddTxIn(wire.NewTxIn(wire.NewOutPoint(prevHash, vout), nil, nil))
	spScript, err := txscript.NewScriptBuilder().AddOp(txscript.OP_1).AddData(xonly[:]).Script()
	if err != nil {
		return err
	}
	tx.AddTxOut(wire.NewTxOut(int64(*amount), spScript))
	if change := int64(value) - int64(*amount) - feeSats; change > 1000 {
		tx.AddTxOut(wire.NewTxOut(change, pkScript))
	}
	fetcher := txscript.NewCannedPrevOutputFetcher(pkScript, int64(value))
	witness, err := txscript.TaprootWitnessSignature(tx, txscript.NewTxSigHashes(tx, fetcher), 0, int64(value), pkScript, txscript.SigHashDefault, priv)
	if err != nil {
		return fmt.Errorf("sign: %w", err)
	}
	tx.TxIn[0].Witness = witness
	var buf bytes.Buffer
	if err := tx.Serialize(&buf); err != nil {
		return err
	}
	raw, err = c.call(false, "sendrawtransaction", hex.EncodeToString(buf.Bytes()))
	if err != nil {
		return err
	}
	var txid string
	if err := json.Unmarshal(raw, &txid); err != nil {
		return err
	}
	if _, err := c.mineOne(); err != nil {
		return err
	}
	var height int64
	raw, err = c.call(false, "getblockcount")
	if err != nil {
		return err
	}
	_ = json.Unmarshal(raw, &height)
	return json.NewEncoder(stdout()).Encode(map[string]any{
		"txid": txid, "vout": 0, "output_key": hex.EncodeToString(xonly[:]), "amount": *amount, "height": height,
	})
}
