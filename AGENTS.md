# AGENTS.md

Guidance for coding agents (and humans in a hurry) working in this repository.

## What this is

A Windows desktop app (Rust + [gpui](https://gpui.rs)) that replaces the cooling, performance and
battery parts of MSI Center on MSI laptops. It talks to the firmware through the
`root\WMI:MSI_ACPI` interface. Protocol facts live in [`docs/PROTOCOL.md`](docs/PROTOCOL.md).

## Layout

| Path | Role |
|---|---|
| `crates/msi-acpi` | Library: `Transport` trait, `WmiTransport` (COM/WMI), `Laptop` feature layer, `HwWorker` thread. No UI. |
| `crates/msi-acpi/src/ec.rs` | The only place EC addresses and bit masks are defined. |
| `crates/msi-acpi/tests` | Behaviour tests against `FakeEc`, seeded from a real EC dump in `tests/fixtures`. |
| `crates/app` | The gpui application (`open-msi-control.exe`). |
| `crates/app/src/app.rs` | Root view `MsiApp`: all UI state, polling, writes, intents. |
| `crates/app/src/views/` | One file per card (single narrow column); each adds `render_*` methods to `MsiApp`. |
| `crates/app/src/ui/` | Stateless components (card/inset surfaces, button, segmented, switch, pill, drag). |
| `crates/app/src/charts/` | Canvas-painted charts (timeline, fan curve, tick gauge, meters). |
| `crates/app/src/theme/` | OKLCH tokens + WCAG contrast tests. |
| `crates/app/src/motion.rs` | Easing curves, durations, interruptible `Transitions`. |
| `crates/app/src/tray.rs` | Tray icon (Shell_NotifyIcon) on a hidden window; events reach the app over a channel. |
| `crates/app/src/autostart.rs` | Start-with-Windows Task Scheduler task (`--tray`, highest privileges). |
| `crates/app/src/i18n.rs` | RU/EN string tables. |
| `crates/probe` | `msi-probe.exe`: read-only dump for new-model reports. |
| `assets/` | Bundled Geist fonts (OFL) and Phosphor icons (MIT). |

## Commands

```sh
cargo run                         # the app (debug); elevation prompt appears
cargo run --release
cargo run -p msi-probe            # from an elevated terminal
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all
```

Release builds need `fxc.exe` from the Windows 10/11 SDK (gpui compiles its shaders with it).

## Rules

- **Hardware safety first.** Every EC write is a read-modify-write of one byte through
  `Laptop::update_ec`/`set_bits`. Never write a whole byte you did not read, never add a register
  that is not confirmed on real hardware, and keep new registers in `ec.rs` + `docs/PROTOCOL.md`.
- Only WMI interface 2.x is supported; `Laptop::new` rejects anything else. Do not loosen that
  without a probe dump from the target model.
- `WmiTransport` is not `Send`: COM proxies stay on the `HwWorker` thread. UI code reaches the
  hardware only via `HwWorker::run` from a background executor, never on the UI thread.
- New `unsafe` blocks need a `// SAFETY:` comment (clippy `undocumented_unsafe_blocks`).
- Library errors are `msi_acpi::Error` (thiserror); `anyhow` is for the app edge only.

## UI conventions

- Colours come from `theme()`; add tokens to `theme::palette` in OKLCH and extend the contrast
  tests. Text must meet WCAG AA (4.5:1) on every surface it can sit on; focus ring 3:1.
- Layering is borderless: one `card` per section, `inset` for grouped rows, separated by
  lightness only. The window is narrow (≈460 px) and single-column; keep cards dense.
- Segmented controls have equal-width segments (measured from the longest label, or filling
  the parent) so the sliding pill always matches its label.
- Motion follows Emil Kowalski's rules: animate only with a purpose, `motion::EASE_OUT` for
  enter/feedback, `EASE_IN_OUT` for on-screen movement, durations ≤ 240 ms, keyboard-initiated
  changes snap (`snap_frame`), reduced motion (Windows "Animation effects" off) drops movement but
  keeps fades. Use `Transitions::animate` for state transitions so they are interruptible.
- Data the user reads (temperatures, rpm) is not animated for style; only gauges ease between
  readings to avoid jumps.
- Every interactive element is reachable with Tab, has a visible focus ring while navigating by
  keyboard, and responds to Enter/Space or arrows.
- All UI text lives in `crates/app/src/i18n.rs` (`Strings` table per language; a missing
  translation is a compile error). Add every new string to both `RU` and `EN`, keep them short,
  sentence case. Explanations go into (i) tooltips (`ui::info`), not into extra visible text.

## Tests

Test behaviour a user would notice: register encodings, preserved bits, bounds, rejection paths,
contrast, easing correctness. Do not add tests that restate implementation or only check that
something does not panic.
