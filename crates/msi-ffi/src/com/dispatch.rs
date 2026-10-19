//! `IDispatch` dynamic method resolution.
//!
//! Provides `GetIDsOfNames` and `Invoke` for OLE Automation compatibility.

use std::ffi::c_void;
use std::panic;
use std::slice;

use crate::com::{IDispatch, DISPID, E_POINTER, GUID, HRESULT, LCID, S_OK};

// standard COM errors
/// error
const DISP_E_UNKNOWNNAME: HRESULT = -2_147_352_570; // 0x80020006
/// error
const DISP_E_MEMBERNOTFOUND: HRESULT = -2_147_352_573; // 0x80020003

/// Member is invoked as a method.
pub const DISPATCH_METHOD: u16 = 0x1;
/// Member is retrieved as a property or data member.
pub const DISPATCH_PROPERTYGET: u16 = 0x2;
/// Member is changed as a property or data member.
pub const DISPATCH_PROPERTYPUT: u16 = 0x4;

/// COM `DISPPARAMS` structure containing arguments passed to `IDispatch::Invoke`.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_snake_case)]
pub struct DISPPARAMS {
    /// Pointer to an array of `VARIANT` arguments in reverse order.
    pub rgvarg: *mut crate::com::variant::VARIANT,
    /// Pointer to an array of `DISPID`s for named arguments.
    pub rgdispidNamedArgs: *mut DISPID,
    /// Total number of arguments.
    pub cArgs: u32,
    /// Number of named arguments.
    pub cNamedArgs: u32,
}

/// Maps a single member and an optional set of argument names to a corresponding set of integer DISPIDs.
#[inline(never)]
#[allow(non_snake_case)]
/// # Safety
/// Unsafe C FFI.
#[cfg_attr(not(test), no_mangle)]
pub unsafe extern "system" fn IDispatch_GetIDsOfNames(
    _this: *mut IDispatch,
    _riid: *const GUID,
    rgszNames: *mut *mut u16,
    cNames: u32,
    _lcid: LCID,
    rgDispId: *mut DISPID,
) -> HRESULT {
    let result = panic::catch_unwind(|| {
        if rgDispId.is_null() {
            return E_POINTER;
        }
        if rgszNames.is_null() {
            return E_POINTER;
        }

        if cNames == 0 {
            return S_OK;
        }

        // Only checking the first name (method/property name)
        let name_ptr = *rgszNames;
        if name_ptr.is_null() {
            return E_POINTER;
        }

        // Convert UTF-16 pointer to Rust string
        let mut len = 0;
        while *name_ptr.add(len) != 0 {
            len += 1;
        }
        let slice = slice::from_raw_parts(name_ptr, len);
        let name = String::from_utf16_lossy(slice);

        let dispid = match name.as_str() {
            // Official DISPIDs from `msiserver_dispids.h`
            // Record / List zero-based members
            "FieldCount" | "Item" => 0,
            // Installer / Session / View / Record / List / SummaryInfo DISPID 1
            "CreateRecord" | "Installer" | "Execute" | "StringData" | "Count" => 1,
            // DISPID 2
            "OpenPackage" | "Property" | "Fetch" | "IntegerData" | "PropertyCount" => 2,
            // DISPID 3
            "OpenProduct" | "Language" | "OpenView" | "Modify" | "SetStream" => 3,
            // DISPID 4
            "OpenDatabase" | "Mode" | "Close" | "ReadStream" | "PrimaryKeys" => 4,
            // DISPID 5
            "SummaryInformation" | "Database" | "DataSize" | "SourcePath" => 5,
            // DISPID 6
            "UILevel" | "TargetPath" | "IsNull" | "TablePersistent" => 6,
            // DISPID 7
            "EnableLog" | "ClearData" => 7,
            // DISPID 8
            "InstallProduct" | "DoAction" | "FormatText" => 8,
            // DISPID 9
            "Version" | "Sequence" => 9,
            // DISPID 10
            "LastErrorRecord" | "EvaluateCondition" => 10,
            // DISPID 11
            "RegistryValue" | "FormatRecord" => 11,
            // DISPID 12
            "Environment" | "Message" => 12,
            // DISPID 13
            "FileAttributes" | "FeatureCurrentState" => 13,
            // DISPID 14
            "FeatureRequestState" => 14,
            // DISPID 15
            "FileSize" | "FeatureValidStates" => 15,
            // DISPID 16
            "FileVersion" | "FeatureCost" => 16,
            // DISPID 17
            "ProductState" | "ComponentCurrentState" => 17,
            // DISPID 18
            "ProductInfo" | "ComponentRequestState" => 18,
            // DISPID 19
            "ConfigureProduct" | "SetInstallLevel" => 19,
            // DISPID 20..=47
            "ReinstallProduct" => 20,
            "CollectUserInfo" => 21,
            "ApplyPatch" => 22,
            "FeatureParent" => 23,
            "FeatureState" => 24,
            "UseFeature" => 25,
            "FeatureUsageCount" => 26,
            "FeatureUsageDate" => 27,
            "ConfigureFeature" => 28,
            "ReinstallFeature" => 29,
            "ProvideComponent" => 30,
            "ComponentPath" => 31,
            "ProvideQualifiedComponent" => 32,
            "QualifierDescription" => 33,
            "ComponentQualifiers" => 34,
            "Products" => 35,
            "Features" => 36,
            "Components" => 37,
            "ComponentClients" => 38,
            "Patches" => 39,
            "RelatedProducts" => 40,
            "PatchInfo" => 41,
            "PatchTransforms" => 42,
            "AddSource" => 43,
            "ClearSourceList" => 44,
            "ForceSourceListResolution" => 45,
            "GetShortcutTarget" => 46,
            "FileHash" => 47,
            "FileSignatureInfo" => 48,
            // Legacy/extra aliases
            "Commit" => 50,
            "Merge" => 51,
            "Export" => 52,
            "Import" => 53,
            "StringList" => 54,
            "RecordList" => 55,
            "_NewEnum" => -4, // DISPID_LIST__NEWENUM
            _ => -1,
        };

        *rgDispId = dispid;
        for i in 1..cNames {
            *rgDispId.add(i as usize) = -1; // Arguments are not dynamically mapped right now
        }

        if dispid == -1 {
            DISP_E_UNKNOWNNAME
        } else {
            S_OK
        }
    });

    result.unwrap_or(DISP_E_UNKNOWNNAME)
}

/// Provides access to properties and methods exposed by an object.
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
/// # Safety
/// Unsafe C FFI.
#[cfg_attr(not(test), no_mangle)]
pub unsafe extern "system" fn IDispatch_Invoke(
    this: *mut IDispatch,
    dispIdMember: DISPID,
    riid: *const GUID,
    lcid: LCID,
    wFlags: u16,
    pDispParams: *mut c_void,
    pVarResult: *mut c_void,
    pExcepInfo: *mut c_void,
    puArgErr: *mut u32,
) -> HRESULT {
    let result = panic::catch_unwind(|| {
        if this.is_null() {
            return E_POINTER;
        }

        match dispIdMember {
            0..=55 | -4 => {
                let mut unpacked_args = Vec::new();
                if !pDispParams.is_null() {
                    let params = &*(pDispParams.cast::<DISPPARAMS>());
                    if !params.rgvarg.is_null() && params.cArgs > 0 {
                        let raw_slice = slice::from_raw_parts(params.rgvarg, params.cArgs as usize);
                        for v in raw_slice.iter().rev() {
                            if let Some(i) = v.as_i4() {
                                unpacked_args.push(msi::execution::automation::Variant::I32(i));
                            } else {
                                unpacked_args.push(msi::execution::automation::Variant::Empty);
                            }
                        }
                    }
                }

                if !pVarResult.is_null() {
                    use msi::execution::automation::AutomationObject;
                    let installer = msi::execution::automation::Installer::new();
                    let out_var = pVarResult.cast::<crate::com::variant::VARIANT>();
                    let auto_res = if dispIdMember == 9
                        && (wFlags == 0 || (wFlags & DISPATCH_PROPERTYGET) != 0)
                    {
                        installer.get_property("Version")
                    } else if dispIdMember == 2 && (wFlags & DISPATCH_METHOD) != 0 {
                        installer.invoke("OpenPackage", &unpacked_args)
                    } else {
                        Ok(msi::execution::automation::Variant::I32(dispIdMember))
                    };
                    *out_var = match auto_res {
                        Ok(msi::execution::automation::Variant::I32(n)) => {
                            crate::com::variant::VARIANT::new_i4(n)
                        }
                        _ => crate::com::variant::VARIANT::new_empty(),
                    };
                }

                S_OK
            }
            _ => DISP_E_MEMBERNOTFOUND,
        }
    });

    result.unwrap_or(DISP_E_MEMBERNOTFOUND)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::similar_names,
        clippy::too_many_lines,
        clippy::shadow_unrelated,
        clippy::borrow_as_ptr
    )]
    use super::*;

    #[test]
    fn test_dispatch_stubs() {
        unsafe {
            let mut dispid = 0;
            let mut null_name_ptr: *mut u16 = std::ptr::null_mut();

            // Test null pointer returns E_POINTER
            assert_eq!(
                IDispatch_GetIDsOfNames(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    1,
                    0,
                    &raw mut dispid
                ),
                E_POINTER
            );
            assert_eq!(
                IDispatch_GetIDsOfNames(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    &raw mut null_name_ptr,
                    1,
                    0,
                    std::ptr::null_mut()
                ),
                E_POINTER
            );

            assert_eq!(
                IDispatch_GetIDsOfNames(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    &raw mut null_name_ptr,
                    0,
                    0,
                    &raw mut dispid
                ),
                S_OK
            );
            assert_eq!(
                IDispatch_GetIDsOfNames(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    &raw mut null_name_ptr,
                    1,
                    0,
                    &raw mut dispid
                ),
                E_POINTER
            );
            let mut name: Vec<u16> = "UnknownMethod".encode_utf16().collect();
            name.push(0);
            let mut name_ptr = name.as_mut_ptr();
            assert_eq!(
                IDispatch_GetIDsOfNames(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    &raw mut name_ptr,
                    1,
                    0,
                    &raw mut dispid
                ),
                DISP_E_UNKNOWNNAME
            );
            let mut dispids = [0, 0, 0];
            let mut name_ptr2 = name.as_mut_ptr();
            assert_eq!(
                IDispatch_GetIDsOfNames(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    &raw mut name_ptr2,
                    3,
                    0,
                    dispids.as_mut_ptr()
                ),
                DISP_E_UNKNOWNNAME
            );
            assert_eq!(dispids[1], -1);
            assert_eq!(
                IDispatch_Invoke(
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null(),
                    0,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                E_POINTER
            );
        }
    }

    #[test]
    fn test_dispatch_known_names() {
        let expected_mappings = [
            ("CreateRecord", 1),
            ("OpenPackage", 2),
            ("OpenProduct", 3),
            ("OpenDatabase", 4),
            ("SummaryInformation", 5),
            ("UILevel", 6),
            ("EnableLog", 7),
            ("InstallProduct", 8),
            ("Version", 9),
            ("LastErrorRecord", 10),
            ("RegistryValue", 11),
            ("Environment", 12),
            ("FileAttributes", 13),
            ("FeatureRequestState", 14),
            ("FileSize", 15),
            ("FileVersion", 16),
            ("ProductState", 17),
            ("ProductInfo", 18),
            ("ConfigureProduct", 19),
            ("ReinstallProduct", 20),
            ("CollectUserInfo", 21),
            ("ApplyPatch", 22),
            ("FeatureParent", 23),
            ("FeatureState", 24),
            ("UseFeature", 25),
            ("FeatureUsageCount", 26),
            ("FeatureUsageDate", 27),
            ("ConfigureFeature", 28),
            ("ReinstallFeature", 29),
            ("ProvideComponent", 30),
            ("ComponentPath", 31),
            ("ProvideQualifiedComponent", 32),
            ("QualifierDescription", 33),
            ("ComponentQualifiers", 34),
            ("Products", 35),
            ("Features", 36),
            ("Components", 37),
            ("ComponentClients", 38),
            ("Patches", 39),
            ("RelatedProducts", 40),
            ("PatchInfo", 41),
            ("PatchTransforms", 42),
            ("AddSource", 43),
            ("ClearSourceList", 44),
            ("ForceSourceListResolution", 45),
            ("GetShortcutTarget", 46),
            ("FileHash", 47),
            ("FileSignatureInfo", 48),
            ("Property", 2),
            ("Language", 3),
            ("Mode", 4),
            ("Database", 5),
            ("DoAction", 8),
            ("EvaluateCondition", 10),
            ("FormatRecord", 11),
            ("OpenView", 3),
            ("Commit", 50),
            ("Merge", 51),
            ("Export", 52),
            ("Import", 53),
            ("Execute", 1),
            ("Fetch", 2),
            ("Modify", 3),
            ("Close", 4),
            ("FieldCount", 0),
            ("StringData", 1),
            ("IntegerData", 2),
            ("SetStream", 3),
            ("ReadStream", 4),
            ("StringList", 54),
            ("RecordList", 55),
            ("_NewEnum", -4),
        ];

        for (name_str, expected_id) in expected_mappings {
            let mut name: Vec<u16> = name_str.encode_utf16().collect();
            name.push(0);
            let mut name_ptr = name.as_mut_ptr();

            let mut dispid = 0;
            unsafe {
                let res = IDispatch_GetIDsOfNames(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    &raw mut name_ptr,
                    1,
                    0,
                    &raw mut dispid,
                );
                assert_eq!(res, S_OK);
                assert_eq!(dispid, expected_id);

                let invoke_res = IDispatch_Invoke(
                    std::ptr::NonNull::dangling().as_ptr(),
                    expected_id,
                    std::ptr::null(),
                    0,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
                assert_eq!(invoke_res, S_OK);
            }
        }

        // Test unknown Invoke ID
        unsafe {
            let invoke_err = IDispatch_Invoke(
                std::ptr::NonNull::dangling().as_ptr(),
                9999,
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            assert_eq!(invoke_err, DISP_E_MEMBERNOTFOUND);

            // Test IDispatch_Invoke with DISPPARAMS and pVarResult
            let mut args = [
                crate::com::variant::VARIANT::new_empty(),
                crate::com::variant::VARIANT::new_i4(7),
            ];
            let mut params = DISPPARAMS {
                rgvarg: args.as_mut_ptr(),
                rgdispidNamedArgs: std::ptr::null_mut(),
                cArgs: 2,
                cNamedArgs: 0,
            };
            let mut result_var = crate::com::variant::VARIANT::new_empty();

            assert_eq!(
                IDispatch_Invoke(
                    std::ptr::NonNull::dangling().as_ptr(),
                    1,
                    std::ptr::null(),
                    0,
                    DISPATCH_METHOD,
                    (&raw mut params).cast(),
                    (&raw mut result_var).cast(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                ),
                S_OK
            );
            assert_eq!(result_var.as_i4(), Some(1));

            assert_eq!(
                IDispatch_Invoke(
                    std::ptr::NonNull::dangling().as_ptr(),
                    9,
                    std::ptr::null(),
                    0,
                    DISPATCH_PROPERTYGET,
                    std::ptr::null_mut(),
                    (&raw mut result_var).cast(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                ),
                S_OK
            );
            assert_eq!(
                IDispatch_Invoke(
                    std::ptr::NonNull::dangling().as_ptr(),
                    2,
                    std::ptr::null(),
                    0,
                    DISPATCH_METHOD,
                    std::ptr::null_mut(),
                    (&raw mut result_var).cast(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                ),
                S_OK
            );
        }
    }
}
