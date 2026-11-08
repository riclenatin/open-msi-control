use std::fmt;

use crate::Result;

/// Size of the `Package_32` buffer every `MSI_ACPI` method exchanges.
pub const PACKAGE_LEN: usize = 32;

/// Raw request/response buffer.
///
/// Request: `[0]` = sub-function index (or EC address for `Get_Data`/`Set_Data`), `[1..]` = payload.
/// Response: `[0]` = status flag (`1` = OK), `[1..]` = payload.
pub type Package = [u8; PACKAGE_LEN];

/// `MSI_ACPI` methods used by this crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types, reason = "names mirror the WMI method names")]
pub enum Method {
    Get_WMI,
    Get_EC,
    Get_Temperature,
    Get_Thermal,
    Get_Fan,
    Set_Fan,
    Get_Device,
    Get_Power,
    Get_AP,
    Get_Data,
    Set_Data,
}

impl Method {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Get_WMI => "Get_WMI",
            Self::Get_EC => "Get_EC",
            Self::Get_Temperature => "Get_Temperature",
            Self::Get_Thermal => "Get_Thermal",
            Self::Get_Fan => "Get_Fan",
            Self::Set_Fan => "Set_Fan",
            Self::Get_Device => "Get_Device",
            Self::Get_Power => "Get_Power",
            Self::Get_AP => "Get_AP",
            Self::Get_Data => "Get_Data",
            Self::Set_Data => "Set_Data",
        }
    }

    /// Methods invoked without an input package.
    pub const fn is_output_only(self) -> bool {
        matches!(self, Self::Get_WMI | Self::Get_EC)
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Executes one `MSI_ACPI` method call and returns the raw response package.
///
/// Implementations do not interpret the status flag; [`crate::Laptop`] does.
pub trait Transport {
    fn call(&self, method: Method, index: u8, payload: &[u8]) -> Result<Package>;
}

impl<T: Transport + ?Sized> Transport for &T {
    fn call(&self, method: Method, index: u8, payload: &[u8]) -> Result<Package> {
        (**self).call(method, index, payload)
    }
}
