# Troubleshooting

<!-- Generated from friglet-ui/states.toml by friglet-ui's tests. Do not edit by hand.
     Regenerate: FRIGLET_UI_REGEN_DOCS=1 cargo test -p friglet-ui troubleshooting -->

Every state the friglet status page, the Umbrel widget and the StartOS health check can show, with what friglet-ui saw and what to do. Anchors are the state ids (`#node-pruned`).

| State | Severity | Retry | Widget |
|---|---|---|---|
| [`crash-loop`](#crash-loop) | error | automatic | Needs attention |
| [`identity-mismatch`](#identity-mismatch) | error | after you act | Needs attention |
| [`needs-setup`](#needs-setup) | info | - | Set up |
| [`node-pruned`](#node-pruned) | error | after you act | Needs attention |
| [`node-syncing`](#node-syncing) | warn | automatic | Needs attention |
| [`oracle-dropped-resuming`](#oracle-dropped-resuming) | warn | automatic | Needs attention |
| [`oracle-unreachable`](#oracle-unreachable) | warn | automatic | Needs attention |
| [`paused`](#paused) | info | - | Paused |
| [`peer-unreachable`](#peer-unreachable) | warn | automatic | Needs attention |
| [`port-in-use`](#port-in-use) | error | after you act | Needs attention |
| [`rescanning-after-reset`](#rescanning-after-reset) | info | - | Scanning |
| [`scan-stopped`](#scan-stopped) | warn | automatic | Needs attention |
| [`scanning`](#scanning) | ok | - | Scanning |
| [`starting`](#starting) | info | - | Starting |
| [`ui-disconnected`](#ui-disconnected) | warn | automatic | Needs attention |
| [`unknown-exit`](#unknown-exit) | warn | automatic | Needs attention |
| [`unsupported-network`](#unsupported-network) | error | after you act | Needs attention |
| [`up-to-date`](#up-to-date) | ok | - | Up to date |
| [`verifying-identity`](#verifying-identity) | info | - | Starting |

## States

### crash-loop

**friglet keeps stopping**

friglet stopped several times in a few minutes. friglet-ui keeps restarting it with longer pauses.

- Severity: error (transient).
- How friglet-ui detects it: Five or more unexpected exits within ten minutes.
- Fix: Open Diagnostics; the restart history and the last error are listed there.
- Offered action: Retry now.

### identity-mismatch

**friglet is scanning a different wallet**

The address friglet reports is not the address of the descriptor you entered, so friglet was stopped and Electrum stays closed. Enter the descriptor again or start fresh.

- Severity: error (permanent).
- How friglet-ui detects it: GetStatus.sp_address differs from the address derived from config.toml and scan.key (usually a state file made with another key).
- Fix: Re-enter the descriptor, restore the previous wallet, or forget the wallet and set it up again.
- Offered action: Change wallet.

### needs-setup

**Set up your wallet**

Paste the Silent Payments descriptor from Sparrow and the wallet's birthday height to start scanning.

- Severity: info (setup).
- How friglet-ui detects it: No wallet is configured in the data directory.
- Fix: Open the web interface and complete setup (StartOS: run the Set up wallet action).
- Offered action: Set up.

### node-pruned

**Your node is pruned**

friglet downloads old blocks from your node, and a pruned node has deleted them. friglet needs an unpruned node.

- Severity: error (permanent).
- How friglet-ui detects it: The node's version message has NODE_NETWORK_LIMITED without NODE_NETWORK.
- Fix: Switch pruning off in your Bitcoin node settings (StartOS: prune=0; RaspiBlitz and myNode: remove prune= from bitcoin.conf) and let it resync.
- Offered action: Retry.

### node-syncing

**Your node is still syncing**

Your node is behind the oracle, so friglet can only scan up to your node's height. Scanning continues as your node catches up.

- Severity: warn (transient).
- How friglet-ui detects it: Node tip from the handshake is below the oracle tip, or below the block friglet asked for.
- Fix: Wait for the node to finish syncing.

### oracle-dropped-resuming

**Oracle connection dropped, resuming**

The connection to the oracle broke during a scan. friglet is restarted and resumes from the last saved block.

- Severity: warn (transient).
- How friglet-ui detects it: friglet aborted (exit 134) or its scan task ended with a gRPC stream error.
- Fix: Usually none. Frequent drops point at an unstable internet connection.

### oracle-unreachable

**Cannot reach the oracle**

friglet could not connect to the BlindBit oracle, a third-party service it needs for block filters. friglet-ui keeps retrying.

- Severity: warn (transient).
- How friglet-ui detects it: friglet exited at startup with an oracle transport error, or GetStatus reports the oracle unreachable.
- Fix: Check the box's internet connection; the oracle URL is shown on the status page.
- Offered action: Retry now.

### paused

**Scanning paused**

You paused scanning. Electrum still serves what was found so far.

- Severity: info (progress).
- How friglet-ui detects it: The user paused scanning (friglet Stop).
- Fix: Resume scanning.
- Offered action: Resume scanning.

### peer-unreachable

**Cannot reach your Bitcoin node**

friglet-ui could not complete a P2P handshake with your node. It re-resolves the node address and retries.

- Severity: warn (transient).
- How friglet-ui detects it: Resolving FRIGLET_UI_NODE failed, the TCP connection was refused or timed out, or friglet reported a P2P connection error.
- Fix: Check that the Bitcoin node app runs. Compose: check FRIGLET_UI_NODE.
- Offered action: Retry now.

### port-in-use

**A port friglet needs is taken**

Another program already uses one of friglet's ports, so friglet cannot start.

- Severity: error (permanent).
- How friglet-ui detects it: friglet failed to bind 127.0.0.1:50001 (Electrum), 127.0.0.1:8095 (HTTP) or its control socket, or friglet-ui failed to bind an Electrum listener.
- Fix: Stop the other program or change its port, then retry.
- Offered action: Retry.

### rescanning-after-reset

**Rescanning after a reset**

friglet's scanned height went backwards, so it is scanning those blocks again. Payments found before reappear as it catches up.

- Severity: info (progress).
- How friglet-ui detects it: Scanned height fell below the highest height seen for this wallet.
- Fix: Nothing to do.

### scan-stopped

**Scanning stopped, restarting it**

friglet's scan task ended with an error while friglet kept running. friglet-ui restarts the scan automatically.

- Severity: warn (transient).
- How friglet-ui detects it: GetStatus: scanning=false with last_error set, and the user did not pause.
- Fix: Usually none: the scan restarts with backoff. If it keeps stopping, open Diagnostics.
- Offered action: Retry now.

### scanning

**Scanning**

friglet is scanning blocks for your payments. Sparrow already sees payments up to the scanned block; more appear as scanning continues.

- Severity: ok (progress).
- How friglet-ui detects it: Identity verified, scan task running, scanned height below the tip.
- Fix: Nothing to do.
- Offered action: Connect a wallet.

### starting

**Starting friglet**

friglet is starting and connecting to the oracle and your node.

- Severity: info (progress).
- How friglet-ui detects it: friglet was started and its control socket has not answered yet.
- Fix: Wait. If this lasts more than a few minutes, open Diagnostics.

### ui-disconnected

**Lost connection to friglet-ui** (shown by the web page itself)

The page cannot reach friglet-ui. It retries every few seconds.

- Severity: warn (transient).
- How friglet-ui detects it: The browser's GET /api/state failed.
- Fix: Check that the app is running.

### unknown-exit

**friglet stopped unexpectedly**

friglet exited for a reason friglet-ui does not recognise. It is restarted automatically.

- Severity: warn (transient).
- How friglet-ui detects it: Any exit not matched by another state.
- Fix: Usually none. If it repeats, open Diagnostics.
- Offered action: Retry now.

### unsupported-network

**This network is not supported**

friglet scans bitcoin (mainnet), signet, testnet4 and regtest. This node runs another network, so friglet is not started.

- Severity: error (permanent).
- How friglet-ui detects it: FRIGLET_UI_NETWORK is missing or not one of bitcoin, signet, testnet4, regtest (Umbrel passes testnet for testnet3).
- Fix: Switch the Bitcoin node to a supported network, or uninstall friglet.

### up-to-date

**Caught up with your node**

Every block from your birthday to your node's tip has been scanned. New blocks are scanned as they arrive.

- Severity: ok (progress).
- How friglet-ui detects it: Identity verified, scan task running, scanned height at min(node tip, oracle tip) and at or above the birthday.
- Fix: Nothing to do.
- Offered action: Connect a wallet.

### verifying-identity

**Checking the wallet**

friglet-ui is comparing the address friglet scans for with the address from your descriptor. Electrum stays closed until they match.

- Severity: info (progress).
- How friglet-ui detects it: friglet runs but has not reported a Silent Payments address that was checked yet.
- Fix: Wait. If friglet reports no address within 5 minutes, open Diagnostics.

## Refusals

### stray-friglet-variable

**friglet-ui refuses to start: a FRIGLET_* variable is set**

- Cause: An environment variable starting with FRIGLET_ but not FRIGLET_UI_ is set. friglet-ui starts friglet with a cleared environment, so such a variable would silently do nothing (or, set by hand on friglet, override the managed configuration).
- Fix: Remove the variable from the container or service environment; use the matching FRIGLET_UI_* setting (docs/contract.md).

### missing-ui-password

**friglet-ui refuses to start: no UI password**

- Cause: FRIGLET_UI_AUTH=password (the default outside Umbrel) and no password hash is stored in the data directory yet.
- Fix: Compose: set FRIGLET_UI_PASSWORD. Native installs and StartOS: write the password to the file named by FRIGLET_UI_PASSWORD_FILE (read on every start). Or run `friglet-ui password set` with the password on stdin.

### wallet-exists

**Setup refused: a different wallet is already set up**

- Cause: `friglet-ui setup` (exit code 7) or POST /api/setup (409 wallet-exists) was given another descriptor or birthday than the configured wallet.
- Fix: Confirm the replacement: `friglet-ui setup --replace`, or confirm the change in the web UI. The old wallet is kept in prev/ for 24 hours and can be restored.
