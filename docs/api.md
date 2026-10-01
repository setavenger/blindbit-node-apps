# friglet-ui JSON API (internal contract between backend and web assets)

All endpoints are under the friglet-ui listener. Static UI files are served with correct
MIME types (`text/javascript`, `text/css`, `font/woff2`, `image/svg+xml`) and the CSP recorded in
`friglet-ui/DESIGN.md`; the UI uses hash routes, so no SPA fallback. A stale or missing CSRF
token is `403` with `error.code = "csrf-invalid"`. A 429 carries `error.retry_after_seconds`
and a `Retry-After` header. In `gateway` mode `/api/session.authenticated` is always `true`
(the platform gateway has already authenticated the request). JSON in and out. Every `POST` needs the
header `X-CSRF-Token: <token from /api/session>` and a Host/Origin allowlist match. Errors are
`{"error":{"code":"<machine id>","message":"<human text>","field":"<form field>|null"}}` with
4xx/5xx status. The web UI polls `GET /api/state` every 2 s while visible.

## Session and auth

- `GET /api/session` → `{"authenticated":bool,"auth_mode":"password|gateway|none","csrf_token":str,"platform":"umbrel|startos|raspiblitz|mynode|compose","banner":str|null}`
- `POST /api/login` `{"password":str}` → 204 and HttpOnly SameSite=Strict cookie; 401 bad password; 429 rate limited (`retry_after_seconds`).
- `POST /api/logout` → 204.

## State

`GET /api/state` →

```json
{
  "configured": true,
  "network": "bitcoin",
  "state": {"id": "scanning", "severity": "ok|info|warn|error", "headline": "…", "detail": "…",
            "action": {"label": "…", "kind": "link|post|none", "target": "/connect|/api/…"} ,
            "next_retry_at": "RFC3339|null"},
  "wallet": {"sp_address": "sp1…", "key_fingerprint": "ab12cd34", "network": "bitcoin",
             "birthday_height": 840000, "identity_verified": true,
             "reported_sp_address": "sp1… (what friglet reports; differs only in identity-mismatch)"},
  "heights": {"birthday": 840000, "birthday_time": "RFC3339|null", "scanned": 850000, "persisted": 849990, "node_tip": 900000,
              "oracle_tip": 900000, "updated_at": "RFC3339"},
  "progress": {"fraction": 0.42, "eta_seconds": 1200},
  "payments_found": 3,
  "node": {"peer": "10.21.21.8:8335", "configured": "bitcoind:8333", "user_agent": "/Satoshi:31.1.0/"},
  "oracle": {"url": "https://oracle.setor.dev", "third_party": true},
  "versions": {"friglet": "93f6a92", "friglet_ui": "0.1.0", "dev_build": false},
  "restarts": 0,
  "paused": false,
  "transaction": {"kind": "replace|forget|restore", "step": "intent|stopping|snapshot|writing|starting|verifying|persisting|done",
                  "failed": false, "error": "…|null", "started_at": "RFC3339"},
  "previous": {"exists": true, "expires_at": "RFC3339|null"}
}
```

`network` is the box's configured network and is present before setup. Any field may be
`null` when unknown (`birthday_time` stays `null` until the backend can read the header). `wallet` is `null` before setup. State ids are the
catalog in `friglet-ui/states.toml` (`needs-setup`, `starting`, `verifying-identity`,
`identity-mismatch`, `scanning`, `up-to-date`, `scan-stopped`, `paused`, `oracle-unreachable`,
`oracle-dropped-resuming`, `peer-unreachable`, `node-pruned`, `node-syncing`, `port-in-use`,
`rescanning-after-reset`, `crash-loop`, `unknown-exit`, `unsupported-network`); the browser adds `ui-disconnected`.

## Setup and wallet

- `POST /api/setup/validate` `{"descriptor":str,"birthday_height":int}` → `{"sp_address":str,"network":str,"key_fingerprint":str}` or a field error. No side effects.
- `POST /api/setup` `{"descriptor":str,"birthday_height":int,"replace":bool}` → 202 `{"state":…}`; 409 `wallet-exists` when a different wallet is configured and `replace` is false.
- `POST /api/wallet/forget` → 202. `POST /api/wallet/restore-previous` → 202 or 404.
  `POST /api/wallet/discard-previous` → 204 or 404 (deletes `prev/` after the user confirms).
- Every 202 body is the full `GET /api/state` document.
- `POST /api/scan/pause`, `POST /api/scan/resume` → 202.

Hex entry: `descriptor` may instead be `"hex:<scan_secret_64hex>:<spend_pubkey_66hex>"`.

## Connect

`GET /api/connect` →

```json
{
  "recommended": {"label": "LAN, encrypted", "host": "umbrel.local", "port": 50032,
                  "protocol": "ssl", "string": "umbrel.local:50032:s", "note": "…"},
  "others": [{"label": "Tor", "host": "…onion", "port": 50031, "protocol": "tcp", "string": "…:50031:t", "note": "…"}],
  "tls_fingerprint_sha256": "AB:CD:…|null",
  "exposure_text": "Anyone who can reach this port can read this wallet's history.",
  "listeners_open": true
}
```

- `POST /api/connect/test` → `{"ok":bool,"detail":str}` (server.version against the advertised LAN address, run from the box).

## Unauthenticated

- `GET /api/widget` → Umbrel `text-with-progress` payload plus `state`, `scanned_height`, `tip_height` (exact shape in docs/contract.md).
- `GET /healthz` → 200 `ok`.
