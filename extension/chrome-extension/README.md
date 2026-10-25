# DVIDM Link Helper (Chrome extension)

Sends links and pages to **DV Internet Download Manager** over a localhost-only endpoint.

## Install (unpacked)

1. Start DVIDM **0.2.9** or newer.
2. Open `chrome://extensions`.
3. Enable **Developer mode**.
4. Click **Load unpacked** and select this `chrome-extension` folder.
5. Optional: pin the extension from the puzzle-piece menu.

## Usage

- **Ctrl+click or Alt+click a file link** (common download extensions such as
  `.zip`, `.rar`, `.exe`, `.msi`, `.iso`, `.pdf`, `.mp4`, or any link with a
  `download` attribute) to add it to DVIDM and start downloading immediately.
  Normal page links are left alone, so Ctrl+click still opens a new tab.
  Ctrl+Shift+click keeps Chrome's native behavior.
- **Right-click a link** → “Download link with DVIDM”.
- **Select text/links on a page**, right-click → “Send selected links to DVIDM”.
  All links inside the selection (anchors and plain URLs) are collected.
- **Select text/links on a page**, right-click → “Copy selected links” copies
  the same set to the clipboard, one URL per line.
- **Right-click the page** → “Send this page to DVIDM”.
- **Toolbar popup**:
  - click “Send selected links on page” to send the selection to DVIDM;
  - or click “Copy selected links” to copy every selected link to the clipboard,
    one URL per line.
- The badge shows `OK` (green), `!` (red) or `0` (no selected links).

**Links sent from menus and the popup are not added directly.** DVIDM opens the
same review window used by “Get from clipboard” (titled **From Chrome**), where
you can uncheck items, pick a destination list and press **Add**. Only the
Ctrl/Alt+click shortcut uses the direct path.

## How it works

DVIDM listens on `http://127.0.0.1:52137`:

| Method | Path    | Notes |
|--------|---------|-------|
| GET    | `/ping` | Returns `DVIDM` when the app is running |
| POST   | `/add`  | JSON `{"urls":["https://...", ...], "direct":false}` plus header `X-DVIDM: chrome` (`{"url":"..."}` is also accepted). `"direct": true` skips the review window and starts the downloads. |

Only `chrome-extension://` origins are accepted for cross-origin requests; requests
without an `Origin` header (local tools like curl) are also allowed. Nothing leaves
the machine.

## Troubleshooting

- **Offline**: DVIDM is not running, or another program is using port 52137.
- After updating DVIDM, the extension needs no update.
- The extension declares `http://127.0.0.1:52137/*` in `host_permissions`.
- If the context menu is missing after reloading, disable/enable the extension once;
  menus are recreated on install.
