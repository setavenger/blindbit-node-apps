export const DEFAULT_LANG = 'en_US'

const dict = {
  // actions/setupWallet.ts
  "friglet asks a BlindBit oracle run by a third party (oracle.setor.dev) for scan data. The oracle sees this server's IP address and which block ranges it scans, never your keys.": 0,
  'Silent Payments descriptor': 1,
  "The Silent Payments descriptor Sparrow shows for this wallet, starting with sp(. It holds the wallet's scan key: friglet keeps it on this server and never shows it again.": 2,
  'Starts with sp( and contains no spaces.': 3,
  'Wallet birthday height': 4,
  'The birthday block height your Sparrow wallet uses; once the wallet has received a payment, Sparrow adds it to the descriptor as ?bh=<height>. friglet scans from this block and does not find payments before it. If you are unsure, pick an earlier block.': 5,
  'This is not a valid Silent Payments descriptor. Copy the descriptor Sparrow shows for the wallet; it starts with sp(.': 6,
  'This birthday height is not valid. Use the birthday height your Sparrow wallet uses: at or after block 709632 and not above the tip of your Bitcoin node.': 7,
  'This descriptor belongs to a different network. friglet on StartOS scans Bitcoin mainnet; use a mainnet wallet (spscan1…).': 8,
  'friglet could not use your Bitcoin node. Make sure Bitcoin is running and not pruned, then run this action again.': 9,
  'A different wallet is already set up on this server. Run Change wallet to replace it.': 10,
  'friglet could not write its files. Check that the server has free disk space, then run this action again.': 11,
  'Setting up the wallet took longer than 60 seconds and was stopped. Nothing was changed. Try again.': 12,
  'Setting up the wallet failed unexpectedly. Nothing was changed. The service log may say more.': 13,
  'Change wallet': 14,
  'Replace the wallet friglet scans for with another one. friglet rescans from the new birthday; set the same birthday in Sparrow.': 15,
  "This replaces the current wallet. Sparrow pointed at this server will show the new wallet's history only, starting from its birthday.": 16,
  Wallet: 17,
  'Set up wallet': 18,
  'Tell friglet which Silent Payments wallet to scan for: paste its descriptor and birthday height from Sparrow.': 19,
  'Wallet changed': 20,
  'Wallet set up': 21,
  'Saved. friglet checks the wallet when the service starts and opens the Electrum port once the address it derives matches. Compare the address below with the Silent Payments receive address in Sparrow.': 22,
  'friglet is switching to this wallet now and opens the Electrum port once the address it derives matches. Compare the address below with the Silent Payments receive address in Sparrow.': 23,
  'Silent Payments address': 24,
  // actions/uiPassword.ts
  Username: 25,
  Password: 26,
  'Show web UI password': 27,
  'Show the username and password your browser asks for when you open the Web UI.': 28,
  'Web UI': 29,
  'No web UI password is set yet. Run Reset web UI password.': 30,
  'Web UI login': 31,
  'Use these credentials when your browser asks to sign in.': 32,
  'Reset web UI password': 33,
  'Generate a new random password for the Web UI. The old password stops working.': 34,
  'Replaces the current web UI password.': 35,
  'The new password is active. Use these credentials when your browser asks to sign in.': 36,
  // dependencies.ts
  'friglet downloads every block from your wallet birthday onward from this node, so pruning must be off.': 37,
  // init/setupTask.ts
  'Paste your Silent Payments descriptor and wallet birthday from Sparrow so friglet knows what to scan for.': 38,
  // interfaces.ts
  'Wallet status, scan progress and connection details. Sign in as "friglet" with the password from the Show web UI password action.': 39,
  'Electrum (SSL)': 40,
  "Electrum endpoint for Sparrow, served over SSL. Anyone who can reach it can read this wallet's history.": 41,
  // main.ts
  'Block ${scanned} of ${tip}.': 42,
  'Caught up with your node at block ${height}.': 43,
  'Caught up with your node.': 44,
  'Scanning.': 45,
  'No wallet is set up. Run Set up wallet.': 46,
  'friglet is starting.': 47,
  'Catching up.': 48,
  'Scanning is paused in the web UI.': 49,
  'friglet is not answering yet.': 50,
  'friglet needs attention (${state}). Open the web UI for details.': 51,
  'Starting friglet': 52,
  'Bitcoin node': 53,
  'Bitcoin does not offer the whitelisted peer port friglet fetches blocks from. Update Bitcoin.': 54,
  'Bitcoin is not installed. Install Bitcoin to use friglet.': 55,
  'The web UI is ready': 56,
  'The web UI is not ready': 57,
  Scan: 58,
  Electrum: 59,
  'Electrum is accepting connections': 60,
  'Electrum opens once a wallet is set up and friglet has verified its address.': 61,
} as const

/**
 * Plumbing. DO NOT EDIT.
 */
export type I18nKey = keyof typeof dict
export type LangDict = Record<(typeof dict)[I18nKey], string>
export default dict
