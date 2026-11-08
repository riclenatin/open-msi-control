//! Read and control MSI laptop firmware through the `root\WMI:MSI_ACPI` interface.
//!
//! The crate is split into a transport ([`Transport`], implemented for real hardware by
//! `WmiTransport` on Windows) and the feature layer ([`Laptop`]) that knows which embedded
//! controller (EC) registers mean what. See `docs/PROTOCOL.md` in the repository for the
//! register map and how it was derived.
//!
//! Only firmware that reports MSI WMI interface **2.x** is supported.

mod ec;
mod error;
mod laptop;
mod model;
mod transport;
#[cfg(windows)]
mod wmi;
#[cfg(windows)]
mod worker;

pub use error::{Error, Result};
pub use laptop::Laptop;
pub use model::{CURVE_POINTS, Fan, FanCurve, FanStatus, Info, Scenario, Snapshot};
pub use transport::{Method, PACKAGE_LEN, Package, Transport};
#[cfg(windows)]
pub use wmi::WmiTransport;
#[cfg(windows)]
pub use worker::HwWorker;

/// A laptop reached through the real WMI interface.
#[cfg(windows)]
pub type MsiLaptop = Laptop<WmiTransport>;
