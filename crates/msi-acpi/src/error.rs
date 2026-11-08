use crate::transport::Method;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A COM/WMI call failed. `context` names the operation.
    #[cfg(windows)]
    #[error("{context}: {source}")]
    Wmi {
        context: String,
        #[source]
        source: windows::core::Error,
    },
    /// The `MSI_ACPI` class or instance is missing, or the process is not elevated.
    #[error("MSI_ACPI is unavailable: {0}")]
    Unavailable(String),
    /// Firmware answered with status flag 0.
    #[error("{method}(0x{index:02X}) was rejected by the firmware")]
    Rejected { method: Method, index: u8 },
    /// The firmware speaks an interface version this crate does not implement.
    #[error("unsupported MSI WMI interface {major}.{minor} (only 2.x is implemented)")]
    UnsupportedInterface { major: u8, minor: u8 },
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    /// The model's EC layout has no register for this feature; nothing was written.
    #[error("{0} is not supported on this model")]
    Unsupported(&'static str),
    /// The hardware worker thread has exited.
    #[error("hardware worker stopped")]
    WorkerStopped,
}

#[cfg(windows)]
pub(crate) trait WmiContext<T> {
    fn ctx(self, context: impl FnOnce() -> String) -> Result<T>;
}

#[cfg(windows)]
impl<T> WmiContext<T> for windows::core::Result<T> {
    fn ctx(self, context: impl FnOnce() -> String) -> Result<T> {
        self.map_err(|source| Error::Wmi { context: context(), source })
    }
}
