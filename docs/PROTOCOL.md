# MSI_ACPI protocol notes

How MSI laptops expose fan, performance and battery control on Windows, as used by
`crates/msi-acpi`. Everything here was derived by observing MSI Center 2.0.75 on a Cyborg 15
A12VF (board MS-15K1, EC firmware `15K1IMS1.113`, MSI WMI interface 2.8): its own call log
(`WmiAcpi2.log`), its public .NET metadata, live probing with `msi-probe`, and cross-checking
against the Linux [`msi-ec`](https://github.com/BeardOverflow/msi-ec) driver (`CONF_G2_1`).

No MSI code is included in this repository.

## Transport

MSI firmware implements an ACPI-WMI bridge (`PNP0C14`) surfaced as the WMI class
`root\WMI:MSI_ACPI` (instance `ACPI\PNP0C14\0_0`). Access requires Administrator rights.

Every method takes and returns one embedded `Package_32` object whose only property is
`uint8 Bytes[32]`:

| Direction | `Bytes[0]` | `Bytes[1..]` |
|---|---|---|
| request | sub-function index, or EC address for `Get_Data`/`Set_Data` | payload |
| response | status flag, `1` = success, `0` = rejected | payload |

`Get_WMI`, `Get_EC` take no input package.

| Method | Index | Payload | Meaning |
|---|---|---|---|
| `Get_WMI` | — | `[major, minor]` | interface version; only **2.x** is implemented |
| `Get_EC` | — | `[ram_ver, "15K1IMS1.113", "MMDDYYYY", "HH:MM:SS"]` | firmware id |
| `Get_Data` | EC address | `[value]` | read one EC RAM byte |
| `Set_Data` | EC address | `[value]` in | write one EC RAM byte |
| `Get_Temperature` | 0 | `[cpu °C, gpu °C, 0]` | `0`/`255` = sensor unavailable (e.g. dGPU asleep) |
| `Get_Temperature` | 1 / 2 | 8 bytes, see fan curves | CPU / GPU curve temperatures |
| `Get_Fan` | 0 | 4 × big-endian u16 tach | `rpm = 480000 / raw`, `raw = 0` → 0 rpm |
| `Get_Fan` | 1 / 2 | `[live duty %, speed0..speed6]` | CPU / GPU curve speeds |
| `Set_Fan` | 1 / 2 | same 8 bytes in | write curve speeds; byte 0 is ignored |

Other getters (`Get_Thermal`, `Get_AP`, `Get_Device`, `Get_Power`) mirror further EC bytes and
are dumped by `msi-probe` for new-model reports.

## EC RAM map (WMI 2.x, `CONF_G2_1` family)

All writes are read-modify-write of one byte; bits not listed are preserved.

| Address | Bits | Meaning |
|---|---|---|
| `0x2E` | bit1 | webcam on |
| `0x2F` | bit1 | webcam switch supported |
| `0x68` / `0x80` | byte | CPU / GPU temperature °C |
| `0x6A..=0x6F` | bytes | CPU curve breakpoints °C (GPU `+0x18`) |
| `0x71` | byte | CPU live duty % (GPU `0x89`) |
| `0x72..=0x78` | bytes | CPU curve speeds % (GPU `0x8A..=0x90`) |
| `0x98` | bit7 | Cooler Boost |
| `0xBF` | bit3 / bit5 | USB power share supported / on |
| `0xC8..=0xCB` | 2 × u16 BE | CPU / GPU fan tach |
| `0xD2` | byte | shift mode `0xC0 \| mode`: `C4` turbo, `C1` comfort, `C2` eco |
| `0xD3` | `0x80 \| level` | keyboard backlight 0..=3 (MSI writes `(v & 0x80) \| level`) |
| `0xD4` | bit4 / bit7 | fan mode: silent / advanced (user curve); `0x0D` = auto |
| `0xD7` | `0x80 \| pct` | battery stop-charge threshold; `0x80` or `0xE4` (100 %) = no limit |
| `0xD9` | bit1 | **cleared** = Fn Lock engaged; read-only (see below) |
| `0xE8` | bit4 | Win/Fn swap, **cleared** = swapped |

### Fan curves

Seven speeds and six breakpoints per fan. Speed *i* applies from breakpoint *i − 1* upwards;
speed 0 applies from 0 °C.

`Get_Temperature(1)` returns EC `[0x69, 0x6F, 0x70, 0x6A, 0x6B, 0x6C, 0x6D, 0x6E]`, so
MSI Center pairs temperatures `[p0, p3, p4, p5, p6, p7]` with speeds 0..=5. It never edits the
seventh (top) speed or any breakpoint; neither does this project.

Example (stock, CPU): breakpoints 51/58/65/73/78/83 °C, speeds 38/40/45/54/60/68/78 %.

### Scenarios

MSI Center keeps the selected scenario in the registry; on non-TDP models it only writes:

| Scenario | `0xD2` | `0xD4` bit4 |
|---|---|---|
| Extreme Performance | `0xC4` | 0 |
| Balanced | `0xC1` | 0 |
| Silent | `0xC1` | 1 |
| Super Battery | `0xC2` | 0 |

The scenario is therefore recovered from these two registers.

### Fn Lock and touchpad

Fn Lock is owned by the EC: Fn+Esc toggles `0xD9` bit1 and MSI Center only reads it. The other
bits of `0xD9` are written by MSI's service, so this project never writes the byte.

The touchpad has no EC register. MSI Center toggles it by sending the Precision Touchpad hotkey
Ctrl+Win+F24; the resulting state is
`HKCU\Software\Microsoft\Windows\CurrentVersion\PrecisionTouchPad\Status\Enabled` (DWORD).

## Known side effects

- *MSI Center Service* re-applies its own scenario at boot and after resume from sleep. The app
  re-applies the user's profile for 30 s after start, wake (a gap between polls) and a power-source
  change, then adopts whatever the user changes.
- Super Battery in MSI Center also switches the Windows power mode; this project does not.

## Unsupported on MS-15K1

Touchpad LED (`Get_AP(3)` rejected), GPU MUX (`0x2F` bit6 clear), USB LED, panel overdrive
(the EC bit exists, MSI Center excludes the panel).
