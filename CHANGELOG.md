# Changelog

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
