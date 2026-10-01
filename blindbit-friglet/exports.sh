# shellcheck shell=bash
# Fixed address of the friglet-node container. Tor's HiddenServicePort (torrc.template) needs a
# literal target address, as in the electrs and fulcrum apps. Unused in getumbrel/umbrel-apps@01be6c3.
export APP_BLINDBIT_FRIGLET_NODE_IP="10.21.21.104"

# Onion hostname for the plaintext Electrum port 50031, written by the tor service.
# Empty until Tor has created it; hooks/pre-start waits for it on first start.
blindbit_friglet_onion_file="${EXPORTS_TOR_DATA_DIR}/app-${EXPORTS_APP_ID}-electrum/hostname"
APP_BLINDBIT_FRIGLET_ELECTRUM_HIDDEN_SERVICE="$(cat "${blindbit_friglet_onion_file}" 2>/dev/null || echo "")"
export APP_BLINDBIT_FRIGLET_ELECTRUM_HIDDEN_SERVICE
