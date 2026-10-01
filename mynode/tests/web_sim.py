#!/usr/bin/env python3
#
# Loads the app the way MyNode's own code does, using functions taken from the checkout's
# application_info.py (via ast, with the device-specific helpers stubbed), then:
#   - fills the app JSON with MyNode's defaults
#   - writes the Tor hidden service config and lists the firewall rules MyNode makes from it
#   - registers the app's Flask blueprint with MyNode's loader (the short name has a "-")
#   - renders the app page with MyNode's templates for several device states
# Started by check.sh in python:3.8-slim (MyNode runs Python 3.8) with Flask 2.2, as root.
# Expects /mynode (a mynodebtc/mynode checkout) and /app (the app folder).

import ast
import copy
import http.server
import importlib
import json
import os
import shutil
import sys
import threading
import types

APP = "blindbit-friglet"
MYNODE = "/mynode/rootfs/standard"
FAILED = []


def check(what, ok):
    print(("PASS: " if ok else "FAIL: ") + what)
    if not ok:
        FAILED.append(what)


def load_functions(names, namespace):
    # Take only the named top-level functions from application_info.py
    path = MYNODE + "/var/pynode/application_info.py"
    tree = ast.parse(open(path).read(), path)
    nodes = [n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name in names]
    assert sorted(n.name for n in nodes) == sorted(names), "missing functions in application_info.py"
    exec(compile(ast.Module(body=nodes, type_ignores=[]), path, "exec"), namespace)
    return namespace


# --- device state the stubs report --------------------------------------------------------
DEVICE = {"premium": True, "tor_remote_access": True, "onion": "a" * 56 + ".onion", "enabled": True}
COMMANDS = []

app_info = load_functions([
    "initialize_application_defaults", "replace_app_info_variables", "create_application_tor_service",
    "open_application_ports", "register_dynamic_app_flask_blueprints",
], {
    "os": os, "importlib": importlib,
    "get_app_screenshots": lambda name: [],
    "is_tor_remote_access_enabled": lambda: DEVICE["tor_remote_access"],
    "get_onion_url_for_service": lambda name: DEVICE["onion"] if DEVICE["premium"] else "not_available",
    "get_app_current_version_from_file": lambda name: "93f6a92-1",
    "get_app_latest_version_from_file": lambda app: app["latest_version"],
    "has_custom_app_version": lambda name: False,
    "is_installed": lambda name: True,
    "is_service_enabled": lambda name: DEVICE["enabled"],
    "get_application_log_file": lambda name: "",
    "get_local_ip": lambda: "192.168.1.50",
    "run_linux_cmd": lambda cmd, *a: COMMANDS.append(cmd),
    "log_message": lambda msg: COMMANDS.append("log: " + msg),
    "trigger_application_refresh": lambda: None,
    "get_dynamic_app_names": lambda: [APP],
})

raw = json.load(open("/app/{}.json".format(APP)))


def load_app():
    # MyNode fills the defaults (tor_address among them) whenever it reloads the app list
    return app_info["initialize_application_defaults"](copy.deepcopy(raw))


app = load_app()

# --- app definition -----------------------------------------------------------------------
check("short_name matches the folder", app["short_name"] == APP)
check("runs as bitcoin", app["linux_user"] == "bitcoin")
check("can be uninstalled", app["can_uninstall"] is True)
check("tile opens the app page", app["has_info_page"] and app["tile_shows_open_and_info"])
check("Open button goes to HTTPS 8098 only", app["http_port"] is None and app["https_port"] == 8098)
check("Reset Data is off (it would delete the scan key)", app["data_manageable"] is False)
check("page fallback text is filled in",
      "https://192.168.1.50:8098" in app["app_page_content"][0]["content"][0])

# --- Tor and firewall ---------------------------------------------------------------------
os.makedirs("/etc/torrc.d", exist_ok=True)
app_info["create_application_tor_service"](app)
torrc = open("/etc/torrc.d/" + APP).read()
print("--- generated /etc/torrc.d/{}\n{}---".format(APP, torrc))
check("onion service directory", "HiddenServiceDir /var/lib/tor/mynode_{}/\n".format(APP) in torrc)
check("onion port 443 -> nginx 8098", "HiddenServicePort 443 127.0.0.1:8098\n" in torrc)
check("onion port 50031 -> Electrum", "HiddenServicePort 50031 127.0.0.1:50031\n" in torrc)
check("onion port 50032 -> Electrum TLS", "HiddenServicePort 50032 127.0.0.1:50032\n" in torrc)
check("no onion port for the plain web interface", "HiddenServicePort 80 " not in torrc)

ufw = []
app_info["open_application_ports"].__globals__["get_all_applications"] = lambda: [app]
real_system = os.system
os.system = lambda cmd: ufw.append(cmd) or 0
try:
    app_info["open_application_ports"]()
finally:
    os.system = real_system
print("--- firewall rules\n" + "\n".join(ufw) + "\n---")
check("ufw opens exactly 8098, 50031, 50032",
      sorted(c.split()[2] for c in ufw) == ["50031", "50032", "8098"])

# --- web: install the app's www files where init_dynamic_app puts them ---------------------
WWW = "/var/www/mynode"
TEMPLATES = WWW + "/templates"
if os.path.isdir(WWW):
    shutil.rmtree(WWW)
shutil.copytree(MYNODE + "/var/www/mynode/templates", TEMPLATES)
os.makedirs("{}/app/{}".format(WWW, APP))
os.makedirs("{}/app/{}".format(TEMPLATES, APP))
for f in os.listdir("/app/www/python"):
    shutil.copy("/app/www/python/" + f, "{}/app/{}/{}".format(WWW, APP, f))
for f in os.listdir("/app/www/templates"):
    if f.endswith(".html"):
        shutil.copy("/app/www/templates/" + f, "{}/app/{}/{}".format(TEMPLATES, APP, f))
sys.path.insert(0, WWW)

# Stand-ins for the MyNode modules the blueprint imports
APP_PASSWORD_USER_SET = "User configured (not managed by MyNode)"
PASSWORD = {"value": "Abc123def456GHI789jkl012"}
stubs = {
    "user_management": {"check_logged_in": lambda: True},
    "application_info": {
        "get_application": lambda name: copy.deepcopy(app) if name == APP else None,
        "get_application_status": lambda name: "Running",
        "get_application_status_color": lambda name: "green",
        "get_application_password": lambda name: PASSWORD["value"],
        "APP_PASSWORD_USER_SET": APP_PASSWORD_USER_SET,
    },
    "device_info": {
        "read_ui_settings": lambda: {"darkmode": False, "background": "none", "reseller": False},
        "get_local_ip": lambda: "192.168.1.50",
        "get_onion_url_for_service": lambda name: DEVICE["onion"] if DEVICE["premium"] else "not_available",
        "is_community_edition": lambda: not DEVICE["premium"],
        "is_tor_remote_access_enabled": lambda: DEVICE["tor_remote_access"],
    },
}
for name, attrs in stubs.items():
    module = types.ModuleType(name)
    module.__dict__.update(attrs)
    sys.modules[name] = module

from flask import Flask, Blueprint  # noqa: E402

flask_app = Flask("mynode", template_folder=TEMPLATES, static_folder=MYNODE + "/var/www/mynode/static")
# MyNode's catch-all page for apps without their own (generic_app.py)
generic = Blueprint("mynode_generic_app", "generic")
generic.add_url_rule("/app/<name>/info", "app_generic_info_page", lambda name: "GENERIC PAGE")
flask_app.register_blueprint(generic)
app_info["register_dynamic_app_flask_blueprints"](flask_app)
errors = [c for c in COMMANDS if "error" in c]
check("MyNode's loader imports the blueprint without errors: {}".format(errors), not errors)
rules = {r.rule: r.endpoint for r in flask_app.url_map.iter_rules()}
check("blueprint serves /app/{}/info".format(APP),
      rules.get("/app/{}/info".format(APP)) == "mynode_blindbit_friglet.blindbit_friglet_page")
client = flask_app.test_client()


# --- friglet-ui's widget endpoint, as a stand-in on 127.0.0.1:8097 --------------------------
class Widget(http.server.BaseHTTPRequestHandler):
    body = b""

    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(Widget.body)

    def log_message(self, *args):
        pass


server = http.server.HTTPServer(("127.0.0.1", 8097), Widget)
threading.Thread(target=server.serve_forever, daemon=True).start()


def page():
    response = client.get("/app/{}/info".format(APP))
    return response.status_code, response.get_data(as_text=True)


print("== page: Premium, Tor on, scanning")
Widget.body = json.dumps({"state": "scanning", "scanned_height": 850000, "tip_height": 900000}).encode()
status, html = page()
check("page renders (200), not MyNode's generic page", status == 200 and "GENERIC PAGE" not in html)
check("shows the HTTPS URL", "https://192.168.1.50:8098" in html)
check("shows the scan state", "Scanning" in html and "(scanning)" in html)
check("shows scanned and tip heights", "850,000 of 900,000" in html)
check("shows the TLS Electrum endpoint", "192.168.1.50:50032:s" in html)
check("shows the onion Electrum endpoint", DEVICE["onion"] + ":50031:t" in html)
check("shows the onion web URL", "https://" + DEVICE["onion"] in html)
check("password is on the page, hidden until clicked",
      'id="friglet_password" class="friglet_value" style="display: none;' in html and PASSWORD["value"] in html)
check("tells to use a separate Sparrow instance", "separate Sparrow instance" in html)
check("discloses the oracle", "oracle.setor.dev" in html and "third-party" in html)
check("says who can read the port", "can read this wallet" in html)
check("says what uninstall deletes", "/mnt/hdd/mynode/blindbit-friglet" in html and "scan key" in html)
check("Open button is HTTPS-only", "open_app_in_new_tab('None', '8098'" in html)

print("== page: Community Edition")
DEVICE["premium"] = False
app = load_app()
status, html = page()
check("Community Edition: no onion, says Premium", "Premium feature" in html and ".onion" not in html)
DEVICE["premium"] = True
app = load_app()

print("== page: Tor remote access off")
DEVICE["tor_remote_access"] = False
app = load_app()
status, html = page()
check("Tor off: says so, no onion", "Tor remote access is disabled" in html and ".onion" not in html)
DEVICE["tor_remote_access"] = True
app = load_app()

print("== page: friglet-ui answers garbage")
Widget.body = json.dumps({"state": "<script>alert(1)</script>", "scanned_height": "x"}).encode()
status, html = page()
check("unexpected widget data is not shown", status == 200 and "alert(1)" not in html and "Not reachable yet" in html)
Widget.body = b"{" + b" " * 5000 + b"}"
status, html = page()
check("oversized widget answer is ignored", status == 200 and "Not reachable yet" in html)
Widget.body = json.dumps({"state": "node-pruned", "scanned_height": None, "tip_height": None}).encode()
status, html = page()
check("attention states are flagged", "friglet_attention" in html and "Bitcoin node is pruned" in html)

print("== page: friglet-ui not running")
server.shutdown()
server.server_close()
status, html = page()
check("unreachable friglet-ui still renders", status == 200 and "Not reachable yet" in html)

print("== page: no password yet, app disabled")
PASSWORD["value"] = ""
DEVICE["enabled"] = False
app = load_app()
status, html = page()
check("missing password says when it appears", "Created when Friglet first starts" in html)
check("disabled app offers Enable", "toggleEnabled('{}', 'Friglet', true)".format(APP) in html)

if FAILED:
    print("WEB: {} check(s) FAILED".format(len(FAILED)))
    sys.exit(1)
print("WEB: all checks passed")
