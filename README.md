# open-msi-control

A small, fast replacement for the cooling, performance and battery parts of **MSI Center** on
MSI laptops. Native Rust, GPU-rendered UI ([gpui](https://gpui.rs)), no background services.

<p align="center"><img src="docs/screenshot.png" width="420" alt="open-msi-control window"></p>

## Features

- **Live telemetry** — CPU/GPU temperature gauges, 5-minute temperature timeline (red above 90 °C) with hover
  readout, fan RPM and EC target duty.
- **Scenarios** — Extreme Performance, Balanced, Silent, Super Battery (the same registers MSI
  Center writes).
- **Fan curves** — drag the six editable points per fan (CPU and GPU), apply or revert; curves
  stay monotonic and never exceed the firmware's top point. Auto mode and Cooler Boost.
- **Battery charge limit** — slider (40–100 %) and the Battery Master presets (100/80/60 %).
- **System** — keyboard backlight, webcam kill switch, Win/Fn swap, USB power share.
- **Tray** — close to tray, left click shows/hides the window, right click opens quick actions
  (scenarios, Cooler Boost, quit); the icon tooltip shows CPU/GPU temperatures. One instance only.
- **Start with Windows** — a Task Scheduler logon task starts the app elevated in the tray without
  a UAC prompt (it runs on battery too).
- Russian and English UI (follows the Windows display language, switchable in System), (i) hints
  explaining every setting.
- Keyboard navigation (Tab, arrows, Enter/Space) with focus rings, WCAG AA contrast, honours the
  Windows "Animation effects" setting.

## Supported hardware

Developed and verified on **MSI Cyborg 15 A12VF** (board MS-15K1, EC `15K1IMS1.113`). It should
work on MSI laptops whose firmware reports MSI WMI interface **2.x** and uses the same EC layout
as the Linux `msi-ec` `CONF_G2_1` family; the app refuses to start on other interface versions.

Got another model? Run `msi-probe.exe` (read-only) and open a *Support a new laptop model* issue.

## Install / run

Requires Windows 10/11 x64 and Administrator rights (the firmware interface is restricted to
Administrators; the executable requests elevation).

```sh
cargo run --release
```

Building needs Rust stable, Visual Studio Build Tools (C++ workload) and the Windows SDK
(`fxc.exe`, used by gpui to compile shaders).

> MSI Center can stay installed, but its service re-applies its own scenario at boot and after
> sleep. Either uninstall it or re-apply your settings afterwards.

## How it works

The firmware exposes an ACPI-WMI bridge, `root\WMI:MSI_ACPI`. The app reads sensors and writes a
handful of embedded-controller bytes through it, exactly like MSI Center does, always as
read-modify-write of single bytes. See [`docs/PROTOCOL.md`](docs/PROTOCOL.md) for the register
map and how it was derived.

| Crate | Purpose |
|---|---|
| `crates/msi-acpi` | Library: WMI transport, EC feature layer, hardware thread |
| `crates/app` | The `open-msi-control` desktop app |
| `crates/probe` | `msi-probe` diagnostic dump |

## Disclaimer

Not affiliated with MSI. Writing to the embedded controller is at your own risk; the app only
touches registers verified on real hardware and keeps all other bits intact.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
Bundled fonts: Geist (SIL OFL 1.1). Icons: Phosphor (MIT).
