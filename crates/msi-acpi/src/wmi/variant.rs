//! `VARIANT` / `SAFEARRAY` plumbing for WMI property access.

use std::mem::ManuallyDrop;

use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Ole::{
    SafeArrayAccessData, SafeArrayCreateVector, SafeArrayGetLBound, SafeArrayGetUBound, SafeArrayUnaccessData,
};
use windows::Win32::System::Variant::{
    VARENUM, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_ARRAY, VT_BSTR, VT_UI1, VT_UNKNOWN,
    VariantClear,
};
use windows::Win32::System::Wmi::IWbemClassObject;
use windows::core::{BSTR, HSTRING, IUnknown};

use crate::error::WmiContext;
use crate::{Error, Result};

fn variant(vt: VARENUM, value: VARIANT_0_0_0) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: value,
            }),
        },
    }
}

/// Wraps an embedded object; the VARIANT owns the reference (free with `VariantClear`).
pub(super) fn from_unknown(unk: IUnknown) -> VARIANT {
    variant(VT_UNKNOWN, VARIANT_0_0_0 { punkVal: ManuallyDrop::new(Some(unk)) })
}

/// Builds a `uint8[]` VARIANT; the caller frees it with `VariantClear`.
pub(super) fn from_bytes(bytes: &[u8]) -> Result<VARIANT> {
    let len = u32::try_from(bytes.len()).map_err(|_| Error::InvalidArgument("array too long".into()))?;
    // SAFETY: creates a new one-dimensional VT_UI1 array of `len` elements.
    let sa = unsafe { SafeArrayCreateVector(VT_UI1, 0, len) };
    if sa.is_null() {
        return Err(Error::Unavailable("SafeArrayCreateVector failed".into()));
    }
    let mut data = std::ptr::null_mut();
    // SAFETY: `sa` is a valid array; while locked, `data` points to `len` writable bytes.
    unsafe {
        SafeArrayAccessData(sa, &mut data).ctx(|| "SafeArrayAccessData".into())?;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data.cast::<u8>(), bytes.len());
        SafeArrayUnaccessData(sa).ctx(|| "SafeArrayUnaccessData".into())?;
    }
    Ok(variant(VT_ARRAY | VT_UI1, VARIANT_0_0_0 { parray: sa }))
}

/// Copies a `uint8[]` VARIANT into a `Vec`. Does not free `v`.
pub(super) fn to_bytes(v: &VARIANT) -> Result<Vec<u8>> {
    // SAFETY: reading the discriminant of an initialised VARIANT.
    let inner = unsafe { &v.Anonymous.Anonymous };
    if inner.vt != (VT_ARRAY | VT_UI1) {
        return Err(Error::Unavailable(format!("expected uint8[], got vt={}", inner.vt.0)));
    }
    // SAFETY: vt says the union holds a SAFEARRAY of bytes.
    let sa: *mut SAFEARRAY = unsafe { inner.Anonymous.parray };
    // SAFETY: `sa` is a valid one-dimensional array; while locked, `data` covers `len` bytes.
    unsafe {
        let lo = SafeArrayGetLBound(sa, 1).ctx(|| "SafeArrayGetLBound".into())?;
        let hi = SafeArrayGetUBound(sa, 1).ctx(|| "SafeArrayGetUBound".into())?;
        let len = usize::try_from(hi - lo + 1).unwrap_or(0);
        let mut data = std::ptr::null_mut();
        SafeArrayAccessData(sa, &mut data).ctx(|| "SafeArrayAccessData".into())?;
        let out = std::slice::from_raw_parts(data.cast::<u8>(), len).to_vec();
        SafeArrayUnaccessData(sa).ctx(|| "SafeArrayUnaccessData".into())?;
        Ok(out)
    }
}

/// Reads an embedded-object property.
pub(super) fn get_unknown(obj: &IWbemClassObject, name: &str) -> Result<IUnknown> {
    let mut v = VARIANT::default();
    // SAFETY: `v` is a valid out VARIANT and is cleared on every path below.
    unsafe {
        obj.Get(&HSTRING::from(name), 0, &mut v, None, None).ctx(|| format!("Get({name})"))?;
        let vt = v.Anonymous.Anonymous.vt;
        let unk = (vt == VT_UNKNOWN).then(|| (*v.Anonymous.Anonymous.Anonymous.punkVal).clone()).flatten();
        VariantClear(&mut v).ctx(|| "VariantClear".into())?;
        unk.ok_or_else(|| Error::Unavailable(format!("{name}: expected an embedded object, got vt={}", vt.0)))
    }
}

/// Reads a string property.
pub(super) fn get_bstr(obj: &IWbemClassObject, name: &str) -> Result<BSTR> {
    let mut v = VARIANT::default();
    // SAFETY: as in `get_unknown`.
    unsafe {
        obj.Get(&HSTRING::from(name), 0, &mut v, None, None).ctx(|| format!("Get({name})"))?;
        let vt = v.Anonymous.Anonymous.vt;
        let s = (vt == VT_BSTR).then(|| (*v.Anonymous.Anonymous.Anonymous.bstrVal).clone());
        VariantClear(&mut v).ctx(|| "VariantClear".into())?;
        s.ok_or_else(|| Error::Unavailable(format!("{name}: expected a string, got vt={}", vt.0)))
    }
}
