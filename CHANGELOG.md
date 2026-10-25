# Changelog

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
