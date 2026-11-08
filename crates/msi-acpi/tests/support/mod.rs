//! In-memory EC that answers `MSI_ACPI` calls the way MS-15K1 firmware does.
//!
//! Other models are emulated by giving it another firmware string: the RAM stays the MS-15K1
//! dump, so tests set the registers they read.

use std::cell::{Cell, RefCell};

use msi_acpi::{Method, PACKAGE_LEN, Package, Result, Transport};

/// Real EC RAM of a Cyborg 15 A12VF (EC 15K1IMS1.113), dumped with `msi-probe`.
const DUMP: &str = include_str!("../fixtures/cyborg15-a12vf-ec.txt");

/// Offset between CPU and GPU fan/temperature blocks.
const GPU_OFFSET: usize = 0x18;
const CPU_TEMP: usize = 0x68;
const CPU_DUTY: usize = 0x71;
const TACH: usize = 0xC8;
/// Where `Get_EC` reads the firmware string from.
const FIRMWARE: usize = 0xA0;

pub struct FakeEc {
    pub ram: RefCell<[u8; 256]>,
    pub wmi_version: (u8, u8),
    /// Method that answers with status flag 0.
    pub reject: Cell<Option<Method>>,
}

impl FakeEc {
    pub fn cyborg15() -> Self {
        let mut ram = [0u8; 256];
        for line in DUMP.lines().skip(1) {
            let (row, bytes) = line.split_once(':').expect("dump row");
            let base = usize::from_str_radix(row.trim(), 16).expect("row address");
            for (i, b) in bytes.split_whitespace().enumerate() {
                ram[base + i] = u8::from_str_radix(b, 16).expect("hex byte");
            }
        }
        Self { ram: RefCell::new(ram), wmi_version: (2, 8), reject: Cell::new(None) }
    }

    /// The MS-15K1 dump reporting EC firmware `firmware` (12 characters, like `15K1IMS1.113`).
    pub fn with_firmware(firmware: &str) -> Self {
        assert_eq!(firmware.len(), 12, "{firmware:?} is not a 12-character EC firmware name");
        let ec = Self::cyborg15();
        ec.ram.borrow_mut()[FIRMWARE..FIRMWARE + 12].copy_from_slice(firmware.as_bytes());
        ec
    }

    pub fn get(&self, addr: u8) -> u8 {
        self.ram.borrow()[usize::from(addr)]
    }

    pub fn put(&self, addr: u8, value: u8) {
        self.ram.borrow_mut()[usize::from(addr)] = value;
    }

    /// Temperature/curve block of `Get_Temperature(1|2)`: EC `[base+1, base+7, base+8, base+2..=base+6]`.
    fn curve_temps(ram: &[u8; 256], base: usize) -> [u8; 8] {
        [
            ram[base + 1],
            ram[base + 7],
            ram[base + 8],
            ram[base + 2],
            ram[base + 3],
            ram[base + 4],
            ram[base + 5],
            ram[base + 6],
        ]
    }
}

impl Transport for FakeEc {
    fn call(&self, method: Method, index: u8, payload: &[u8]) -> Result<Package> {
        let mut out = [0u8; PACKAGE_LEN];
        if self.reject.get() == Some(method) {
            return Ok(out);
        }
        out[0] = 1;
        let mut ram = self.ram.borrow_mut();
        let fan_base = |fan: u8| CPU_DUTY + GPU_OFFSET * usize::from(fan - 1);
        let body: Vec<u8> = match (method, index) {
            (Method::Get_WMI, _) => vec![self.wmi_version.0, self.wmi_version.1],
            (Method::Get_EC, _) => {
                let mut v = vec![0x81];
                v.extend_from_slice(&ram[FIRMWARE..0xBC]);
                v
            }
            (Method::Get_Data, addr) => vec![ram[usize::from(addr)]],
            (Method::Set_Data, addr) => {
                ram[usize::from(addr)] = payload[0];
                vec![]
            }
            (Method::Get_Temperature, 0) => vec![ram[CPU_TEMP], ram[CPU_TEMP + GPU_OFFSET], 0],
            (Method::Get_Temperature, fan @ 1..=2) => {
                Self::curve_temps(&ram, CPU_TEMP + GPU_OFFSET * usize::from(fan - 1)).to_vec()
            }
            (Method::Get_Fan, 0) => ram[TACH..TACH + 8].to_vec(),
            (Method::Get_Fan, fan @ 1..=2) => ram[fan_base(fan)..fan_base(fan) + 8].to_vec(),
            (Method::Set_Fan, fan @ 1..=2) => {
                // Firmware takes the 7 speeds; byte 0 (live duty) is read-only.
                let base = fan_base(fan);
                ram[base + 1..base + 8].copy_from_slice(&payload[1..8]);
                vec![]
            }
            _ => {
                out[0] = 0;
                vec![]
            }
        };
        out[1..=body.len()].copy_from_slice(&body);
        Ok(out)
    }
}
