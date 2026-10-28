# Changelog

## 0.5.77

- **Custom list colours:** pick a palette colour or choose any shade with the new colour picker.
- **Your table:** show or hide columns, drag the dividers to resize them, and keep the layout.
- **Per-download folder:** change the save folder of one download from Properties; partial data moves with it.
- **Faster selection:** click a card to select it, drag across cards, and click empty space to clear. Right-click clears the selection first, then opens Properties.
- **Quick appearance:** switch light/dark and the interface language from the main window.
- Fixed the small square that could stay around list controls.
- Rebuilt installer and portable package.

## 0.5.71

- **A cleaner Settings window:** options are grouped into General, Downloads, and Browser & network, and arranged in two columns so the page uses the available space.
- Related options now sit together: appearance, language and font; startup, notifications and scrolling; connections and retries.
- The default file-type list is shown on two lines.
- Long Persian hints in Settings now wrap cleanly.
- Rebuilt installer and portable package.

## 0.5.69

- **Open folder:** opening a download's folder now selects the file in Windows Explorer when it exists.
- Rebuilt installer and portable package with extension 0.5.16.

## 0.5.68

- **Safer delete:** the row delete button now asks first and offers to delete the completed file from disk as well.

## 0.5.67

- **Startup update notice:** when a newer version is available, a fading message appears at the bottom-right when DVIDM starts.

## 0.5.66

- **List controls:** every download list in the sidebar now has a small play/pause button. Start or pause the whole list without opening it.
- **Downloading indicator:** while the current list is downloading, the **Start list** button shows a small pulsing dot.
- Rebuilt installer and portable package with extension 0.5.16.

## 0.5.65

- **Persian and right-to-left support:** choose System, English, فارسی or العربية under **Settings → Appearance**. Persian and Arabic flip the whole interface right-to-left, with Persian wording, readable numbers, and normal text editing.
- **Proxy support:** connect through the Windows proxy, or enter an HTTP or SOCKS5 proxy with an optional username and password.
- **Better browser downloads (extension 0.5.16):** pick your own shortcut key, add more file types (including `.safetensors`), and download from sites that need an active browser session. If the browser cannot start the download, the link is still sent to DVIDM; if DVIDM is closed, Chrome keeps the file.
- **Browser quick menu:** selecting text opens a compact DVIDM menu near the pointer for sending or copying the selected links, downloading the selected link, or sending the page. A checkbox in the menu turns it off.
- **Smoother downloads:** the download engine has been reworked for steadier speeds, a clearer two-decimal speed display, and fewer interruptions. New downloads no longer inherit partial data from another file with the same name.
- **Interface polish:** short three-letter file badges, more space between Progress and Speed, and windows that block clicks to the list behind them.
- Downloads are saved into folders named after the download list and category.
- Rebuilt installer and portable package, including language files and extension 0.5.16.

## 0.5.35

- **Chrome extension 0.2.8:** Ctrl+click or Alt+click a file link to add it to DVIDM and start it directly. Ordinary page links are untouched, Ctrl+Shift+click stays with Chrome, and “Don’t start immediately” is respected.
- New downloads are scrolled into view automatically; turn this off in **Settings → Behavior**.
- In-app messages moved to the bottom-right corner with a fade in/out and auto-hide; they no longer dim or block the window.
- Download-complete and update-available desktop cards now fade in and out.
- **Last date** now records the last start time (or the add time before the first start). The column stays brief and hovering shows the complete date and time.
- **File Properties** adds **Download time** and **Average speed**; paused time is excluded from both.
- **Download installer** in the update dialog now adds the MSI to DVIDM’s own downloads instead of opening the browser.
- Rebuilt installer and portable package, with the extension files included for manual loading.

## 0.5.33

- **List options now shows a live schedule preview** at the bottom: “Downloads will start in …” and “Downloads will run for …”. Both lines recalculate as the start or end time is typed, dim when the schedule is switched off, and use correct singular/plural units.
- Added breathing room above and below the **Save list… / Load list…** buttons and a clearer gap under the divider.
- Downloads held by a schedule now show a **yellow “Scheduled” badge**.
- Rebuilt installer and portable package for 0.5.33.

## 0.5.32

- The schedule tooltip on a list’s clock now reads **“Downloads will start in …”** and **“Downloads will run for …”**, with natural hour/minute wording and correct singular/plural units. The start time is calculated from the saved schedule and the current clock, and both values refresh when the schedule changes.
- Rebuilt installer and portable package for 0.5.32.

## 0.5.31

- Fixed the **Check for update** dialog getting stuck on “Checking…”. The request now runs silently and the result dialog opens when it arrives.
- Added a **background update check every two days**. When a newer release exists, the app shows its desktop notification (once per version); clicking it opens the installer.
- The last check time and the announced version are saved in settings.
- Rebuilt installer and portable package for 0.5.31.

## 0.5.30

- **Check for update now uses GitHub**: the logo menu asks the public release feed for the newest version, compares it with the running build, and shows either an up-to-date message or a download dialog with **Download installer** and **Release page** actions. Errors offer **Try again**.
- Rebuilt installer and portable package for 0.5.30.

## 0.5.29

- **User guide menu**: the DV logo menu now opens a guide dialog with the English PDF, the Persian PDF and the extension install video.
- **Start with Windows by default**: new installs and existing settings that never made this choice enable the startup entry on first launch. The setting can still be turned off in **Settings → Startup**.
- The installer now ships `install extension guide.mp4` alongside both PDF guides.
- Added a **portable zip** with the same app, extension, guides and video — extract and run, no installation.

## Earlier highlights

- Download lists with colors, per-list parallelism, schedules and optional shutdown.
- Sortable **Last date** column and persisted sort state.
- Desktop completion notifications, with a setting to disable them.
- Custom resizable sidebar, logo menu with About and full exit.
- Redownload action that keeps the existing file.
- Save and load download lists to a text file.
- Automatic fallback to the Windows Downloads and temp folders when a saved path is unavailable.
- Per-user MSI installer with desktop and Start menu shortcuts.
