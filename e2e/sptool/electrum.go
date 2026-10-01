package main

import (
	"bufio"
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"net"
	"os"
	"time"

	"github.com/btcsuite/btcd/wire"
)

func stdout() io.Writer { return os.Stdout }

// electrum is a minimal newline-delimited JSON-RPC client. friglet pushes notifications on
// the same connection, so replies are matched by id and everything else is skipped.
type electrum struct {
	conn net.Conn
	r    *bufio.Reader
	id   int
}

func dialElectrum(addr string) (*electrum, error) {
	conn, err := net.DialTimeout("tcp", addr, 5*time.Second)
	if err != nil {
		return nil, err
	}
	return &electrum{conn: conn, r: bufio.NewReaderSize(conn, 1<<20)}, nil
}

func (e *electrum) close() { _ = e.conn.Close() }

type rpcMsg struct {
	ID     *int            `json:"id"`
	Method string          `json:"method"`
	Params json.RawMessage `json:"params"`
	Result json.RawMessage `json:"result"`
	Error  json.RawMessage `json:"error"`
}

// call sends one request and returns its result plus every notification seen meanwhile.
func (e *electrum) call(method string, params ...any) (json.RawMessage, []rpcMsg, error) {
	if params == nil {
		params = []any{}
	}
	e.id++
	id := e.id
	b, _ := json.Marshal(map[string]any{"jsonrpc": "2.0", "id": id, "method": method, "params": params})
	_ = e.conn.SetDeadline(time.Now().Add(30 * time.Second))
	if _, err := e.conn.Write(append(b, '\n')); err != nil {
		return nil, nil, err
	}
	var notes []rpcMsg
	for {
		line, err := e.r.ReadBytes('\n')
		if err != nil {
			return nil, notes, err
		}
		var m rpcMsg
		if err := json.Unmarshal(bytes.TrimSpace(line), &m); err != nil {
			return nil, notes, fmt.Errorf("bad line from server: %.200s", line)
		}
		if m.ID == nil {
			notes = append(notes, m)
			continue
		}
		if *m.ID != id {
			continue
		}
		if len(m.Error) > 0 && string(m.Error) != "null" {
			return nil, notes, fmt.Errorf("%s: %s", method, m.Error)
		}
		return m.Result, notes, nil
	}
}

// readNotes collects notifications that arrive within d (subscribe pushes its history).
func (e *electrum) readNotes(d time.Duration) []rpcMsg {
	var notes []rpcMsg
	_ = e.conn.SetReadDeadline(time.Now().Add(d))
	for {
		line, err := e.r.ReadBytes('\n')
		if err != nil {
			return notes
		}
		var m rpcMsg
		if json.Unmarshal(bytes.TrimSpace(line), &m) == nil && m.ID == nil {
			notes = append(notes, m)
		}
	}
}

type spHistoryItem struct {
	Height int64  `json:"height"`
	TxHash string `json:"tx_hash"`
}

type checkResult struct {
	Tip          int64   `json:"tip"`
	Address      string  `json:"address"`
	StartHeight  int64   `json:"start_height"`
	Progress     float64 `json:"progress"`
	Found        bool    `json:"found"`
	Height       int64   `json:"height"`
	Scripthash   string  `json:"scripthash"`
	HistoryCount int     `json:"history_count"`
}

func checkOnce(server, txid string) (*checkResult, error) {
	e, err := dialElectrum(server)
	if err != nil {
		return nil, err
	}
	defer e.close()
	res := &checkResult{}

	raw, _, err := e.call("blockchain.headers.subscribe")
	if err != nil {
		return nil, err
	}
	var hdr struct {
		Height int64 `json:"height"`
	}
	if err := json.Unmarshal(raw, &hdr); err != nil {
		return nil, fmt.Errorf("headers.subscribe: %s", raw)
	}
	res.Tip = hdr.Height

	raw, notes, err := e.call("blockchain.silentpayments.subscribe")
	if err != nil {
		return nil, err
	}
	var sub struct {
		Address     string `json:"address"`
		StartHeight int64  `json:"start_height"`
	}
	_ = json.Unmarshal(raw, &sub)
	res.Address, res.StartHeight = sub.Address, sub.StartHeight
	notes = append(notes, e.readNotes(3*time.Second)...)
	for _, n := range notes {
		if n.Method != "blockchain.silentpayments.subscribe" {
			continue
		}
		var params []json.RawMessage
		if json.Unmarshal(n.Params, &params) != nil || len(params) < 3 {
			continue
		}
		_ = json.Unmarshal(params[1], &res.Progress)
		var items []spHistoryItem
		_ = json.Unmarshal(params[2], &items)
		for _, it := range items {
			if it.TxHash == txid {
				res.Found, res.Height = true, it.Height
			}
		}
	}
	if !res.Found {
		return res, nil
	}

	// The payment output must also show in its scripthash history (how Sparrow sees it).
	raw, _, err = e.call("blockchain.transaction.get", txid)
	if err != nil {
		return res, err
	}
	var txHex string
	if err := json.Unmarshal(raw, &txHex); err != nil {
		return res, err
	}
	txBytes, err := hex.DecodeString(txHex)
	if err != nil {
		return res, err
	}
	var tx wire.MsgTx
	if err := tx.Deserialize(bytes.NewReader(txBytes)); err != nil {
		return res, err
	}
	if len(tx.TxOut) == 0 {
		return res, errors.New("transaction has no outputs")
	}
	sum := sha256.Sum256(tx.TxOut[0].PkScript)
	for i, j := 0, len(sum)-1; i < j; i, j = i+1, j-1 {
		sum[i], sum[j] = sum[j], sum[i]
	}
	res.Scripthash = hex.EncodeToString(sum[:])
	raw, _, err = e.call("blockchain.scripthash.get_history", res.Scripthash)
	if err != nil {
		return res, err
	}
	var hist []spHistoryItem
	if err := json.Unmarshal(raw, &hist); err != nil {
		return res, err
	}
	for _, h := range hist {
		if h.TxHash == txid {
			res.HistoryCount++
		}
	}
	return res, nil
}

func cmdElectrumCheck(args []string) error {
	fs := flag.NewFlagSet("electrum-check", flag.ExitOnError)
	server := fs.String("server", "", "Electrum TCP server host:port")
	txid := fs.String("txid", "", "transaction that must be found")
	timeout := fs.Duration("timeout", 5*time.Minute, "how long to wait")
	_ = fs.Parse(args)
	if *server == "" || *txid == "" {
		return errors.New("-server and -txid are required")
	}
	deadline := time.Now().Add(*timeout)
	var last error
	for {
		res, err := checkOnce(*server, *txid)
		if err == nil && res.Found && res.HistoryCount > 0 {
			return json.NewEncoder(stdout()).Encode(res)
		}
		switch {
		case err != nil:
			last = err
		case !res.Found:
			last = fmt.Errorf("tx not in silentpayments history yet (tip %d, progress %.3f)", res.Tip, res.Progress)
		default:
			last = fmt.Errorf("tx listed but scripthash %s has no history", res.Scripthash)
		}
		if time.Now().After(deadline) {
			return fmt.Errorf("gave up after %s: %w", *timeout, last)
		}
		time.Sleep(3 * time.Second)
	}
}
