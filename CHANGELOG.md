# Changelog

All notable changes are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [SemVer](https://semver.org).

## [Unreleased]

### Added

- `msi-acpi` library: `MSI_ACPI` WMI transport, feature layer for MSI WMI 2.x firmware
  (scenarios, fan curves, Cooler Boost, battery limit, webcam, Win/Fn swap, keyboard backlight,
  USB power share), dedicated hardware thread, tests against an emulated MS-15K1 EC.
- `open-msi-control` app (gpui): compact single-column window with a dithered header and live
  CPU/GPU gauges, 5-minute temperature
  timeline with hover readout, draggable fan curve editor for CPU and GPU, scenario picker,
  battery limit slider with presets, system toggles; keyboard navigation with focus rings,
  OKLCH theme checked against WCAG AA, interruptible motion that honours Windows reduced-motion,
  Russian and English UI with (i) explanations, temperature timeline turning red above 90 °C,
  tray icon with quick-action menu and close-to-tray, start with Windows (Task Scheduler),
  single instance.
- `msi-probe` diagnostic dump for new-model reports.
- Protocol documentation in `docs/PROTOCOL.md`.
- Profile restore: the scenario, fan curves, Cooler Boost and charge limit are re-applied after
  boot, wake and power-source changes; optional separate profiles on AC and battery.
- Tray flyout (left click): temperatures, scenarios, fan mode, Cooler Boost, charge presets.
- Touchpad switch (Precision Touchpad hotkey) and read-only Fn Lock state.
- Temperature timeline ranges of 5 min, 1 h and 24 h; CSV export of the 24 h history.
- Daily update check against GitHub Releases; installer and portable zip.
- Settings stored in `%APPDATA%\open-msi-control\settings.json`.
- Licensed under GPL-2.0-or-later.
