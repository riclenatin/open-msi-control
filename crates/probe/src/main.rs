//! `msi-probe`: read-only dump of everything `MSI_ACPI` exposes. Run from an elevated shell.
//!
//! The output is what a bug report for a new model needs: interface version, firmware string,
//! the EC layout the app would use, the raw replies of every getter and the full EC RAM.

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    match probe::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("msi-probe: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("msi-probe only runs on Windows");
    std::process::exit(1);
}

#[cfg(windows)]
mod probe {
    use msi_acpi::{Laptop, Method, Result, Transport, WmiTransport};

    /// Getters and the indices worth dumping, with payload length shown.
    const GETTERS: &[(Method, &[u8], usize)] = &[
        (Method::Get_Temperature, &[0, 1, 2], 8),
        (Method::Get_Fan, &[0, 1, 2, 3], 8),
        (Method::Get_Thermal, &[0, 1, 2, 3], 7),
        (Method::Get_AP, &[0, 1, 2, 3], 8),
        (Method::Get_Device, &[0, 1], 6),
        (Method::Get_Power, &[0], 3),
    ];

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
    }

    pub(crate) fn run() -> Result<()> {
        let wmi = WmiTransport::connect()?;
        let laptop = Laptop::new(&wmi)?;
        let info = laptop.info();
        println!("MSI WMI {}.{}", info.wmi_version.0, info.wmi_version.1);
        println!("EC firmware {} (build {})", info.ec_firmware, info.ec_build);
        match info.ec_config {
            Some(config) => println!("EC layout: msi-ec {config}"),
            None => println!("EC layout: built-in MS-15K1 (firmware not in the msi-ec table)"),
        }
        print!("{}", laptop.layout());
        println!();

        for &(method, indices, len) in GETTERS {
            for &index in indices {
                match wmi.call(method, index, &[]) {
                    Ok(p) => println!("{method}({index}) = {}", hex(&p[..=len])),
                    Err(e) => println!("{method}({index}) error: {e}"),
                }
            }
        }

        println!("\nEC RAM (Get_Data):");
        println!("    {}", (0..16).map(|c| format!("{c:02X}")).collect::<Vec<_>>().join(" "));
        for row in 0..16u8 {
            let cells: Vec<String> = (0..16u8)
                .map(|col| match laptop.read_ec(row << 4 | col) {
                    Ok(v) => format!("{v:02X}"),
                    Err(_) => "??".to_owned(),
                })
                .collect();
            println!("{:02X}: {}", row << 4, cells.join(" "));
        }
        Ok(())
    }
}
