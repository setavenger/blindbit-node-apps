from flask import Blueprint, render_template
from user_management import check_logged_in
from application_info import get_application, get_application_status, get_application_status_color, \
    get_application_password, APP_PASSWORD_USER_SET
from device_info import read_ui_settings, get_local_ip, get_onion_url_for_service, is_community_edition, \
    is_tor_remote_access_enabled
import json
import re
import urllib.request


SHORT_NAME = "blindbit-friglet"
HTTPS_PORT = 8098
ELECTRUM_TCP_PORT = 50031
ELECTRUM_TLS_PORT = 50032
# Unauthenticated, read-only status of friglet-ui: {"state": id, "scanned_height": n, "tip_height": n}
WIDGET_URL = "http://127.0.0.1:8097/api/widget"
WIDGET_MAX_BYTES = 4096

mynode_blindbit_friglet = Blueprint("mynode_blindbit_friglet", __name__)
# MyNode looks the blueprint up as "mynode_<short_name>", which has a "-" in it
globals()["mynode_" + SHORT_NAME] = mynode_blindbit_friglet

STATE_LABELS = {
    "needs-setup": "Waiting for wallet setup",
    "starting": "Starting",
    "verifying-identity": "Checking the wallet",
    "identity-mismatch": "Needs attention: wallet does not match",
    "scanning": "Scanning",
    "up-to-date": "Up to date",
    "scan-stopped": "Needs attention: scanning stopped",
    "paused": "Paused",
    "oracle-unreachable": "Needs attention: oracle unreachable",
    "oracle-dropped-resuming": "Reconnecting to the oracle",
    "peer-unreachable": "Needs attention: Bitcoin node unreachable",
    "node-pruned": "Needs attention: Bitcoin node is pruned",
    "node-syncing": "Waiting for the Bitcoin node to sync",
    "port-in-use": "Needs attention: port in use",
    "rescanning-after-reset": "Scanning again",
    "crash-loop": "Needs attention: keeps restarting",
    "unknown-exit": "Needs attention: stopped unexpectedly",
}


def _height(value):
    if isinstance(value, int) and not isinstance(value, bool) and 0 <= value < 100000000:
        return value
    return None


def get_scan_status():
    # Ask friglet-ui directly (never through a proxy) and accept only the fixed widget schema
    try:
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        with opener.open(WIDGET_URL, timeout=1.5) as response:
            raw = response.read(WIDGET_MAX_BYTES + 1)
        if len(raw) > WIDGET_MAX_BYTES:
            return None
        data = json.loads(raw.decode("utf-8"))
    except Exception:
        return None
    if not isinstance(data, dict):
        return None
    state = data.get("state")
    if not isinstance(state, str) or not re.match(r"^[a-z][a-z-]{0,39}$", state):
        return None
    return {
        "state": state,
        "label": STATE_LABELS.get(state, state),
        "attention": STATE_LABELS.get(state, "").startswith("Needs attention"),
        "scanned_height": _height(data.get("scanned_height")),
        "tip_height": _height(data.get("tip_height")),
    }


def get_onion_info():
    # Onion addresses are a Premium feature and need Tor remote access (as on the Electrum Server page)
    if is_community_edition():
        return {"available": False, "reason": "Premium feature"}
    if not is_tor_remote_access_enabled():
        return {"available": False, "reason": "Tor remote access is disabled"}
    onion = get_onion_url_for_service(SHORT_NAME)
    if not re.match(r"^[a-z2-7]{56}\.onion$", onion or ""):
        return {"available": False, "reason": "Not created yet (restart Friglet after Tor has started)"}
    return {"available": True, "host": onion}


@mynode_blindbit_friglet.route("/info")
def blindbit_friglet_page():
    check_logged_in()

    app = get_application(SHORT_NAME)
    app_status = get_application_status(SHORT_NAME)
    app_status_color = get_application_status_color(SHORT_NAME)

    password = get_application_password(SHORT_NAME)
    if password == APP_PASSWORD_USER_SET:
        password = ""

    local_ip = get_local_ip()
    template_data = {
        "title": "MyNode - " + app["name"],
        "ui_settings": read_ui_settings(),
        "app_status": app_status,
        "app_status_color": app_status_color,
        "app": app,
        "friglet_password": password,
        "scan_status": get_scan_status() if app["is_enabled"] else None,
        "local_ip": local_ip,
        "web_url": "https://{}:{}".format(local_ip, HTTPS_PORT),
        "electrum_tls": "{}:{}:s".format(local_ip, ELECTRUM_TLS_PORT),
        "electrum_tls_port": ELECTRUM_TLS_PORT,
        "electrum_tcp_port": ELECTRUM_TCP_PORT,
        "onion": get_onion_info(),
    }
    return render_template("/app/blindbit-friglet/blindbit-friglet.html", **template_data)
