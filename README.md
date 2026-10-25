<div align="center">

<img src="assets/logo.png" width="104" alt="DV Internet Download Manager" />

# DV Internet Download Manager

**Fast, segmented downloads for Windows — with download lists, schedules, speed control and Chrome integration.**

<img src="https://img.shields.io/badge/version-0.5.29-58a6ff?style=for-the-badge" alt="Version 0.5.29" />
<img src="https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078D4?style=for-the-badge" alt="Windows 10 and 11" />
<img src="https://img.shields.io/badge/browser-Chrome%20%7C%20Edge-4285F4?style=for-the-badge" alt="Chrome and Edge" />
<img src="https://img.shields.io/badge/license-Freeware-12ec91?style=for-the-badge" alt="Freeware" />

<br />

[**Download DVIDM**](https://github.com/davoodtaba-glitch/DVIDM/releases/latest/download/DVIDM-Setup.msi) &nbsp;·&nbsp;
[English guide](docs/USER-GUIDE.pdf) &nbsp;·&nbsp;
[راهنمای فارسی](docs/USER-GUIDE.fa.pdf) &nbsp;·&nbsp;
[Extension video](docs/install%20extension%20guide.mp4)

</div>

---

![DVIDM main window](screenshots/main.png)

## What is DVIDM?

DVIDM is a desktop download manager for Windows built around **segmented, multi-connection transfers**. It splits a file across up to 32 connections, resumes paused downloads, retries broken parts, and keeps everything organized in lists, categories and schedules — all in a compact dark interface.

The package includes a **Chrome / Edge extension** that sends selected links straight to the app, plus **English and Persian user guides** and a short **extension install video**.

---

## Highlights

| | |
|---|---|
| **Segmented downloads** | Up to 32 parallel connections per file, with automatic part retry and resume. |
| **Download lists** | Colored lists, per-list parallelism, time-of-day schedules and optional shutdown when a list window closes. |
| **Queue control** | Start, pause and re-order downloads; hold new downloads with *Don't start immediately*. |
| **Speed limit** | Global bandwidth cap from 0 to 1 GB/s. |
| **Browser integration** | Send links, media and pages from Chrome or Edge; right-click menus and clipboard picker included. |
| **Smart organization** | Categories, search, sortable columns and a **Last date** column for finished downloads. |
| **Ready-to-use defaults** | Saves to your Windows **Downloads** folder, partial files in `%TEMP%\DVIDM`, and starts with Windows on first launch. |
| **Tray and notifications** | Close to tray, single instance, desktop completion cards with an option to turn them off. |
| **Bilingual help** | Full guides in **English** and **Persian**, plus a video walkthrough for the extension. |

---

## Download and install

| File | Description |
|---|---|
| [`installer/DVIDM-Setup.msi`](https://github.com/davoodtaba-glitch/DVIDM/releases/latest/download/DVIDM-Setup.msi) | Windows installer (64-bit, no administrator rights required) — also attached to [Releases](https://github.com/davoodtaba-glitch/DVIDM/releases) |
| [`extension/chrome-extension/`](extension/chrome-extension) | Unpacked Chrome / Edge extension |
| [`extension/DVIDM-chrome-extension.zip`](extension/DVIDM-chrome-extension.zip) | Same extension as a zip archive |
| [`docs/`](docs) | English and Persian guides, and the extension video |

### Install DVIDM

1. Download **`DVIDM-Setup.msi`** and run it.
2. If **Windows protected your PC** appears, choose **More info → Run anyway** (the build is not code-signed).
3. Follow the wizard and launch DVIDM from the desktop or Start menu.
4. On first launch, DVIDM saves to `Downloads` and uses `%TEMP%\DVIDM` for partial files. Both folders can be changed later in **Settings**.

> Requires Windows 10 or Windows 11, 64-bit. The installer is per-user and installs to `%LOCALAPPDATA%\Programs\DVIDM`.

---

## Browser extension

The extension talks to the running app over a local connection (`127.0.0.1:52137`). Nothing is sent to the internet.

1. Keep **DVIDM running**.
2. Open `chrome://extensions` (or `edge://extensions`).
3. Turn on **Developer mode**.
4. Click **Load unpacked** and select:
   ```
   %LOCALAPPDATA%\Programs\DVIDM\chrome-extension
   ```
5. Pin the extension to the toolbar. Select links on a page and choose **Send selected links**, or use the right-click menu. Copied links can also be sent from DVIDM's **Clipboard** button.

<div align="center">

[**Watch the extension video (MP4)**](docs/install%20extension%20guide.mp4)

</div>

---

## User guides

| Language | Guide |
|---|---|
| English | [`docs/USER-GUIDE.pdf`](docs/USER-GUIDE.pdf) |
| Persian (فارسی) | [`docs/USER-GUIDE.fa.pdf`](docs/USER-GUIDE.fa.pdf) |
| Video | [`docs/install extension guide.mp4`](docs/install%20extension%20guide.mp4) |

The same guides and video are installed with the app and available from the **User guide** menu behind the DV logo.

![User guide menu](screenshots/user-guide.png)

---

## Frequently asked questions

**Windows shows “Windows protected your PC”.**
The application is not code-signed yet. Click **More info**, then **Run anyway**.

**The extension says it cannot reach DVIDM.**
Start DVIDM and keep it running. Only one instance can run at a time, and the extension connects to the local port `52137`.

**My download or temp folder disappeared (external drive removed).**
DVIDM detects this at startup and falls back to the Windows **Downloads** folder and `%TEMP%\DVIDM`, then remembers the change.

**Can I turn off completion notifications?**
Yes — **Settings → Notifications**.

**Does DVIDM send telemetry?**
No. Downloads go directly to the source server. The extension only communicates with the local app.

**How do I uninstall it?**
Use **Settings → Apps → Installed apps → DV Internet Download Manager → Uninstall**. Downloaded files are not removed.

---

## Version

Current release: **0.5.29** — see [CHANGELOG.md](CHANGELOG.md) for user-facing changes.

---

## License

DVIDM is **freeware**. See [LICENSE](LICENSE) for the end-user license terms.

Bundled interface icons are from [Untitled UI](https://www.untitledui.com/) and remain under their own license.

<div align="center">
<sub>Built for Windows 10 and 11. Download responsibly and respect the terms of the sites you use.</sub>
</div>
