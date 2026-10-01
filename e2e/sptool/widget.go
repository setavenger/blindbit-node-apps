package main

import (
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"net/http"
	"time"
)

// cmdWidgetCheck waits until friglet-ui's unauthenticated /api/widget reports a healthy state
// with scanned_height >= -min-height, then prints the widget JSON.
func cmdWidgetCheck(args []string) error {
	fs := flag.NewFlagSet("widget-check", flag.ExitOnError)
	url := fs.String("url", "http://friglet-node:8097/api/widget", "widget endpoint")
	minHeight := fs.Int64("min-height", 0, "scanned_height must reach this")
	timeout := fs.Duration("timeout", 5*time.Minute, "how long to wait")
	_ = fs.Parse(args)
	client := &http.Client{Timeout: 10 * time.Second}
	deadline := time.Now().Add(*timeout)
	var last error
	for {
		var w struct {
			State         string `json:"state"`
			ScannedHeight *int64 `json:"scanned_height"`
			TipHeight     *int64 `json:"tip_height"`
		}
		resp, err := client.Get(*url)
		if err == nil {
			body, _ := io.ReadAll(resp.Body)
			resp.Body.Close()
			switch {
			case resp.StatusCode != http.StatusOK:
				last = fmt.Errorf("HTTP %d: %.200s", resp.StatusCode, body)
			case json.Unmarshal(body, &w) != nil:
				last = fmt.Errorf("not JSON: %.200s", body)
			case w.State != "scanning" && w.State != "up-to-date":
				last = fmt.Errorf("state %q", w.State)
			case w.ScannedHeight == nil || *w.ScannedHeight < *minHeight:
				last = fmt.Errorf("state %q, scanned_height %v < %d", w.State, w.ScannedHeight, *minHeight)
			default:
				fmt.Println(string(body))
				return nil
			}
		} else {
			last = err
		}
		if time.Now().After(deadline) {
			if last == nil {
				last = errors.New("no answer")
			}
			return fmt.Errorf("gave up after %s: %w", *timeout, last)
		}
		time.Sleep(3 * time.Second)
	}
}
