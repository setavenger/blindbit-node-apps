# friglet-ui design

The web assets in `ui/` are static HTML, CSS and JavaScript with no build step. friglet-ui
embeds them and serves them as they are. They talk only to the JSON API in `docs/api.md`. This
file records the visual system (taste TD3: the tray's look plus a light theme), the components
and the layout of each screen. `docs/plan.md` (design block) is the source of the requirements.

## Files

| Path | What |
|---|---|
| `ui/index.html` | One page holding every screen as a `<section class="view">`; only one is visible |
| `ui/app.js` | Routing, polling, rendering, forms. One classic script, no modules, no dependencies |
| `ui/style.css` | Tokens and components below |
| `ui/favicon.svg` | Orange dot on the dark background |
| `ui/fonts/` | IBM Plex Sans 400/600 and Plex Mono 400, Latin1 `woff2` subsets, plus the OFL `LICENSE.txt` |

Fonts come from the IBM/plex releases `@ibm/plex-sans@1.1.0` and `@ibm/plex-mono@2.5.0`
(`fonts/split/woff2/*-Latin1.woff2`, retrieved 2026-10-01), SIL Open Font License 1.1. The three
files total 61 KB. Characters outside Latin1 fall back to the system font.

The page works under a strict Content Security Policy:
`default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'; font-src 'self';
connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'`. It uses no
inline script, no inline style attributes and no `data:` URLs. Requests use
`referrerPolicy: same-origin`, so the browser sends `Origin` on every POST for the backend's
same-origin check.

## Tokens

All colours are CSS variables on `:root`. Dark is the default (the tray palette).
`@media (prefers-color-scheme: light)` swaps in the light set.

### Colour

| Token | Dark | Light | Use |
|---|---|---|---|
| `--bg` | `#12161d` | `#f6f7f9` | Page background |
| `--surface` | `#1b2432` | `#ffffff` | Inputs, preview and confirm panels |
| `--code-bg` | `#1b2432` | `#e9edf2` | Read-only values with a copy button |
| `--text` | `#e8eef7` | `#141a22` | Body text |
| `--muted` | `#8b98ab` | `#4d5868` | Labels, hints, `dt` |
| `--accent` | `#f7931a` | `#f7931a` | Primary button fill, progress value, current nav underline |
| `--accent-strong` | `#f7931a` | `#a85400` | Primary button border (gives 3:1 on the light background) |
| `--on-accent` | `#1a1206` | `#1a1206` | Text on the accent |
| `--link` | `#f7a645` | `#9a4a00` | Links and disclosure summaries |
| `--good` | `#34c07a` | `#137148` | Verified, copied, test passed |
| `--bad` | `#f07272` | `#b3261e` | Errors, danger buttons |
| `--warn` | `#f2c14e` | `#8a5300` | Warnings, "Third-party service" tag |
| `--info` | `#7cb7ff` | `#1f5fae` | In-progress severity, transaction panel |
| `--*-tint` | rgba of the above | rgba of the above | Pill and banner backgrounds |
| `--border` | `#2a3547` | `#d6dce4` | Dividers only (decorative) |
| `--control` | `#6f7f96` | `#737e8e` | Borders of inputs and secondary buttons |
| `--focus` | `#f7931a` | `#a85400` | Focus ring |
| `--track` | `#2a3547` | `#d6dce4` | Progress track |

Measured contrast (WCAG 2 ratio):

| Pair | Dark | Light | Needed |
|---|---|---|---|
| `--text` on `--bg` / `--surface` | 15.5 / 13.4 | 16.3 / 17.5 | 4.5 |
| `--muted` on `--bg` / `--surface` | 6.2 / 5.3 | 6.7 / 7.2 | 4.5 |
| `--link` on `--bg` | 9.1 | 5.8 | 4.5 |
| `--good` / `--bad` / `--warn` / `--info` on `--bg` | 7.7 / 6.4 / 10.8 / 8.7 | 5.6 / 6.1 / 5.9 / 5.9 | 4.5 |
| Same on `--code-bg` (lowest) | 5.3 (muted) | 5.1 (good) | 4.5 |
| `--on-accent` on `--accent` | 8.1 | 8.1 | 4.5 |
| `--control` against `--bg` / `--surface` | 4.4 / 3.8 | 3.8 / 4.1 | 3 |
| `--focus` against `--bg` / `--surface` | 7.9 / 6.8 | 5.0 / 5.3 | 3 |
| `--accent-strong` border against `--bg` | 7.9 | 5.0 | 3 |
| Solid danger button text (`--bg` on `--bad`) | 6.4 | 6.1 | 4.5 |

axe-core 4 found no contrast violations on any screen in either theme (see Verification).

### Type

IBM Plex Sans for text, IBM Plex Mono for addresses, hosts, ports, fingerprints, commands and
the descriptor field. Numbers use `font-variant-numeric: tabular-nums` (`.num`).

| Token | Size | Weight | Use |
|---|---|---|---|
| `--fs-h1` | 24 px (1.5rem), line height 1.25 | 600 | One per screen; on Status it is the state headline |
| `--fs-h2` | 18 px, line height 1.3 | 600 | Section titles, brand |
| `--fs-h3` | 16 px | 600 | Panel titles, labels of fields |
| `--fs-body` | 16 px, line height 1.5 | 400 | Body |
| `--fs-small` | 14 px | 400 | Hints, labels, mono values, pills |

### Space, shape, size

| Token | Value |
|---|---|
| Spacing `--s1`..`--s6` | 4, 8, 12, 16, 24, 32 px |
| `--radius` | 6 px everywhere (pills use 999 px) |
| Focus ring | 2 px solid `--focus`, offset 2 px (1 px on inputs); `:focus-visible` only |
| `--target` | 44 px minimum height and width for buttons, inputs, nav links and summaries |
| `--reading` | 760 px content width; 16 px side padding |

Motion: only the progress value (0.4 s) and the disclosure triangle (0.15 s) animate.
`prefers-reduced-motion: reduce` sets every transition and animation to none. `forced-colors`
adds borders to pills, banners, panels and the progress bar.

## Components

- **Header**: brand (dot + "friglet", links to Status or Setup), main nav as plain links with
  `aria-current="page"` and a 2 px accent underline, "Sign out" (password mode only). Below
  640 px the nav wraps under the brand. The nav is hidden until signed in, and before setup it
  shows only Setup and Diagnostics.
- **Banners** (top of `<main>`): `ui-disconnected` (error colours; last update time; "keeps trying
  every 2 seconds"), `auth_mode=none` (warn colours; permanent), and `session.banner` text
  from the backend (info colours). Banners are not live regions.
- **Severity pill**: text, never colour alone. `ok` "All good", `info` "In progress",
  `warn` "Check this", `error` "Needs attention".
- **Progress**: native `<progress max="1">` with a `<label>`, percentage, "Scanned to block X of
  Y, starting from B", and "About N left" only when the backend sends `eta_seconds`. Hidden when
  `progress.fraction` is null. There is no spinner or indeterminate state anywhere.
- **Facts list**: `<dl class="facts">`, stacked below 560 px, two columns (11 rem labels) above.
- **Copy row**: mono value on `--code-bg` that wraps anywhere, plus a "Copy" button whose name
  includes what it copies. The copy uses `navigator.clipboard`, then `execCommand("copy")`. If
  both fail, it selects the value and the button reads "Selected: press Ctrl+C" (Cmd on Mac).
- **Field**: label, hint (`aria-describedby`), input, error paragraph. On error the error id is
  put first in `aria-describedby`, `aria-invalid="true"` is set, and the first invalid field gets
  focus. Errors without a `field` go to the form error paragraph, which gets focus. A field's
  error clears as soon as it is edited. Errors are prefixed "Error:" so they don't rely on colour.
- **Secret input**: `type="password"`, `autocomplete="off"`, `autocapitalize="off"`,
  `autocorrect="off"`, `spellcheck="false"`, password-manager ignore attributes, and a
  Show/Hide button whose `aria-label` names the field. Values are cleared after a successful
  submit and are never written to storage, the URL or the console.
- **Disclosure**: native `<details>`/`<summary>` with a CSS triangle; the body is indented with a
  2 px rule.
- **Buttons**: primary (accent fill), secondary (outline), quiet (header), danger (red outline)
  and solid danger (only the final "Forget this wallet").
- **Inline confirmation**: a bordered panel that replaces the button that opened it. Its heading
  or first paragraph gets focus. Back or Cancel returns focus to the opener. There are no modals.
- **Transaction panel**: info-bordered, or red when the state severity is `error`. It shows the
  kind, current or failed step (humanised from `state.transaction.step`) and start time. The
  next-action button sits inside the panel on the Wallet page. Its content comes from
  `/api/state` on every poll, so it survives reloads.
- **Live region**: exactly one, `aria-live="polite"`, visually hidden. It announces only state
  changes (state id or transaction step, including `ui-disconnected`), never the first render,
  copy feedback or form results. Results of user actions (test, restore) get focus instead.

## Behaviour

- Hash routes (`#/setup`, `#/status`, `#/connect`, `#/wallet`, `#/diagnostics`; login is shown
  in place), so any static server works and no SPA fallback is needed. The backend's
  `action.target` paths such as `/connect` map to `#/connect`.
- Route guard: not signed in shows Sign in (password) or a platform-login note (gateway). Not
  configured shows Setup (Diagnostics stays reachable). Configured hides Setup.
- `GET /api/state` every 2 s while the page is visible, chained with `setTimeout` so requests
  never overlap. Polling stops when the page is hidden and resumes at once when it is shown. A
  network error or 5xx gives `ui-disconnected` while keeping the last data with its time; 401
  sends the user back to Sign in.
- Every POST sends `X-CSRF-Token` from `/api/session`. On a 403 with a `csrf*` code the page
  reloads the session and retries once.
- Every state id is rendered from the payload: headline, detail, severity, `next_retry_at`
  ("friglet-ui tries again automatically in about N (at T)") and `action` (`link` becomes an
  accent link-button, `post` a button that POSTs to an `/api/` target, `none` or null shows
  nothing). A small fallback table supplies a headline only when the backend sends none.
  `needs-setup` is shown as the Setup screen.
- `platform`: on `startos` the Setup screen and "Change wallet" explain the StartOS action and
  hide the forms. Sign in shows where the password lives per platform. Diagnostics lists this
  platform's log location first.

## Screens

Every screen is one reading column inside `<main>` with one `h1`, which gets focus on navigation.

1. **Sign in**: password field, platform hint, Sign in. 401 is a field error; 429 is a form error
   with the wait time, and the page refuses further attempts until it has passed.
2. **Setup** (first run): lede; descriptor (masked, Show); "Enter the keys as hex instead"
   disclosure (scan secret masked, spend pubkey); birthday height asking for the height
   Sparrow shows; read-only network and node; identity preview after inline
   `POST /api/setup/validate` (on change, both fields filled, no focus moves); oracle
   disclosure line directly above "Save and start scanning". 409 `wallet-exists` explains and
   links to Change wallet.
3. **Status**: severity pill, state headline (`h1`), detail, retry time, "Scanned X–Y, no payments
   found" with a birthday link when up to date with nothing found, progress, transaction panel,
   Wallet (SP address with copy, the instruction to compare with Sparrow, address check, key
   fingerprint), one next action, then Details: payments found, birthday, scanned (and saved)
   height, node tip, oracle tip, network, node peer, oracle with "Third-party service", versions,
   restarts, last update. No balance (BR3).
4. **Connect**: gate notice while `listeners_open` is false; partial-history note while
   scanning; numbered steps: (1) separate Sparrow, with the required sentence and per-OS launch
   commands in a disclosure; (2) recommended endpoint as Sparrow's fields: URL and Port with copy,
   Use SSL as an instruction, the combined `host:port:s` string with copy, the note, and the
   TLS fingerprint with copy; (3) other endpoints collapsed; (4) who can read this port
   (`exposure_text`); (5) "Test from this node", then laptop-side troubleshooting. No QR code.
5. **Wallet**: transaction panel; current wallet; Change wallet (same fields as Setup, then
   "Review change" validates and shows a confirmation that names the current and new wallet
   with fingerprint and birthday, the rescan height and the Sparrow birthday change);
   Restore previous wallet (confirm; 404 is explained); Forget wallet (confirm naming the
   wallet; then Setup).
6. **Diagnostics** (placeholder): planned download; per-platform log locations with the
   txid/amount/IP warning; state id, platform, versions and restarts to quote when asking for help.

## Verification

`friglet-ui/ui` was checked against a mock of every `docs/api.md` endpoint (all state ids, both
auth modes plus `none`, all five platforms, origin and CSRF enforced, strict CSP) with
Playwright 1.55 (Chromium) and axe-core 4. The run covered 247 checks and 85 screenshots, and
axe reported 0 violations on 25 screen/theme combinations. The mock and the tests are not
part of this repository.
