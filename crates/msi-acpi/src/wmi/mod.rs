//! [`Transport`] over the real `root\WMI:MSI_ACPI` class.
//!
//! Every method takes and returns one embedded `Package_32 { uint8 Bytes[32] }` object.

mod variant;

use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoInitializeSecurity,
    CoSetProxyBlanket, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL, RPC_C_AUTHN_LEVEL_DEFAULT,
    RPC_C_IMP_LEVEL_IMPERSONATE,
};
use windows::Win32::System::Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE};
use windows::Win32::System::Variant::{VARIANT, VariantClear};
use windows::Win32::System::Wmi::{
    IEnumWbemClassObject, IWbemClassObject, IWbemLocator, IWbemServices, WBEM_FLAG_FORWARD_ONLY,
    WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_FLAG_RETURN_WBEM_COMPLETE, WBEM_GENERIC_FLAG_TYPE, WBEM_INFINITE,
    WbemLocator,
};
use windows::core::{BSTR, HSTRING, IUnknown, Interface, PCWSTR, w};

use crate::error::WmiContext;
use crate::transport::{Method, PACKAGE_LEN, Package, Transport};
use crate::{Error, Result};

/// Connection to the single `MSI_ACPI` instance.
///
/// Not `Send`: the COM proxies belong to the multithreaded apartment of the thread that created
/// them. Use [`crate::HwWorker`] to own one on a dedicated thread.
#[derive(Debug)]
pub struct WmiTransport {
    svc: IWbemServices,
    acpi_class: IWbemClassObject,
    package_class: IWbemClassObject,
    instance_path: BSTR,
}

impl WmiTransport {
    /// Initialises COM (MTA) on the calling thread and binds to `MSI_ACPI`.
    pub fn connect() -> Result<Self> {
        // SAFETY: plain COM initialisation; S_FALSE (already initialised) is accepted by `.ok()`.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok().ctx(|| "CoInitializeEx".into())?;
        // SAFETY: process-wide security defaults; RPC_E_TOO_LATE means someone already set them,
        // which is fine, so the result is ignored.
        let _ = unsafe {
            CoInitializeSecurity(
                None,
                -1,
                None,
                None,
                RPC_C_AUTHN_LEVEL_DEFAULT,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
                None,
            )
        };
        // SAFETY: COM is initialised on this thread.
        let locator: IWbemLocator = unsafe { CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER) }
            .ctx(|| "CoCreateInstance(WbemLocator)".into())?;
        let empty = BSTR::new();
        // SAFETY: all arguments are valid BSTRs owned for the duration of the call.
        let svc = unsafe {
            locator.ConnectServer(&BSTR::from("root\\WMI"), &empty, &empty, &empty, 0, &empty, None)
        }
        .ctx(|| "ConnectServer(root\\WMI)".into())?;
        // SAFETY: `svc` is a live proxy obtained above.
        unsafe {
            CoSetProxyBlanket(
                &svc,
                RPC_C_AUTHN_WINNT,
                RPC_C_AUTHZ_NONE,
                PCWSTR::null(),
                RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
            )
        }
        .ctx(|| "CoSetProxyBlanket".into())?;

        let acpi_class = get_object(&svc, "MSI_ACPI")?;
        let package_class = get_object(&svc, "Package_32")?;
        let instance_path = first_instance_path(&svc, "MSI_ACPI")?;
        Ok(Self { svc, acpi_class, package_class, instance_path })
    }

    fn input_params(&self, method: Method, bytes: &Package) -> Result<Option<IWbemClassObject>> {
        if method.is_output_only() {
            return Ok(None);
        }
        let mut sig: Option<IWbemClassObject> = None;
        // SAFETY: `sig` is a valid out pointer; the output-signature pointer may be null.
        unsafe {
            self.acpi_class.GetMethod(&HSTRING::from(method.name()), 0, &mut sig, std::ptr::null_mut())
        }
        .ctx(|| format!("GetMethod({method})"))?;
        let sig = sig.ok_or_else(|| Error::Unavailable(format!("{method} has no input signature")))?;

        // SAFETY: both class objects are live; SpawnInstance returns an owned instance.
        let (params, package) = unsafe { (sig.SpawnInstance(0), self.package_class.SpawnInstance(0)) };
        let (params, package) =
            (params.ctx(|| format!("{method}: spawn params"))?, package.ctx(|| "Package_32: spawn".into())?);
        let mut v_bytes = variant::from_bytes(bytes)?;
        // SAFETY: the VARIANT is fully initialised; Put copies it, then we free our copy.
        let put = unsafe { package.Put(w!("Bytes"), 0, &v_bytes, 0) };
        // SAFETY: `v_bytes` owns its SAFEARRAY and is not used afterwards.
        unsafe { VariantClear(&mut v_bytes) }.ctx(|| "VariantClear".into())?;
        put.ctx(|| "Package_32.Put(Bytes)".into())?;

        let unknown = package.cast::<IUnknown>().ctx(|| "Package_32 as IUnknown".into())?;
        let mut v_pkg = variant::from_unknown(unknown);
        // SAFETY: as above, Put copies (AddRefs) the embedded object.
        let put = unsafe { params.Put(w!("Data"), 0, &v_pkg, 0) };
        // SAFETY: releases our reference; `v_pkg` is not used afterwards.
        unsafe { VariantClear(&mut v_pkg) }.ctx(|| "VariantClear".into())?;
        put.ctx(|| format!("{method}: Put(Data)"))?;
        Ok(Some(params))
    }
}

impl Transport for WmiTransport {
    fn call(&self, method: Method, index: u8, payload: &[u8]) -> Result<Package> {
        if payload.len() > PACKAGE_LEN - 1 {
            return Err(Error::InvalidArgument(format!(
                "{method}: payload of {} bytes exceeds the package",
                payload.len()
            )));
        }
        let mut bytes = [0; PACKAGE_LEN];
        bytes[0] = index;
        bytes[1..=payload.len()].copy_from_slice(payload);
        let params = self.input_params(method, &bytes)?;

        let mut out: Option<IWbemClassObject> = None;
        // SAFETY: all references outlive the synchronous call; `out` is a valid out pointer.
        unsafe {
            self.svc.ExecMethod(
                &self.instance_path,
                &BSTR::from(method.name()),
                WBEM_GENERIC_FLAG_TYPE(0),
                None,
                params.as_ref(),
                Some(&mut out),
                None,
            )
        }
        .ctx(|| format!("ExecMethod({method}, 0x{index:02X})"))?;
        let out = out.ok_or_else(|| Error::Unavailable(format!("{method} returned no output")))?;

        let data: IWbemClassObject =
            variant::get_unknown(&out, "Data")?.cast().ctx(|| format!("{method}: Data object"))?;
        let mut v = VARIANT::default();
        // SAFETY: `v` is a valid out VARIANT, cleared below.
        unsafe { data.Get(w!("Bytes"), 0, &mut v, None, None) }.ctx(|| format!("{method}: Get(Bytes)"))?;
        let res = variant::to_bytes(&v);
        // SAFETY: frees the SAFEARRAY copied out by Get.
        unsafe { VariantClear(&mut v) }.ctx(|| "VariantClear".into())?;
        let res = res?;

        let mut pkg = [0; PACKAGE_LEN];
        let n = res.len().min(PACKAGE_LEN);
        pkg[..n].copy_from_slice(&res[..n]);
        Ok(pkg)
    }
}

fn get_object(svc: &IWbemServices, path: &str) -> Result<IWbemClassObject> {
    let mut obj: Option<IWbemClassObject> = None;
    // SAFETY: `obj` is a valid out pointer for the synchronous call.
    unsafe { svc.GetObject(&BSTR::from(path), WBEM_FLAG_RETURN_WBEM_COMPLETE, None, Some(&mut obj), None) }
        .ctx(|| format!("GetObject({path})"))?;
    obj.ok_or_else(|| Error::Unavailable(format!("class {path} not found")))
}

fn first_instance_path(svc: &IWbemServices, class: &str) -> Result<BSTR> {
    // SAFETY: query strings are owned BSTRs alive for the call.
    let it: IEnumWbemClassObject = unsafe {
        svc.ExecQuery(
            &BSTR::from("WQL"),
            &BSTR::from(format!("SELECT * FROM {class}")),
            WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
            None,
        )
    }
    .ctx(|| format!("ExecQuery({class})"))?;
    let mut row = [None];
    let mut returned = 0;
    // SAFETY: `row` and `returned` are valid out buffers of matching length.
    unsafe { it.Next(WBEM_INFINITE, &mut row, &mut returned) }
        .ok()
        .map_err(|e| Error::Unavailable(format!("cannot enumerate {class} (run as Administrator): {e}")))?;
    let obj = row[0]
        .take()
        .ok_or_else(|| Error::Unavailable(format!("no {class} instance; not an MSI laptop?")))?;
    variant::get_bstr(&obj, "__RELPATH")
}
