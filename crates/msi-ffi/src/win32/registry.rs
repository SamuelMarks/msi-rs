//! Product and Registry Enumeration Win32 `stdcall` API endpoints.
//!
//! Provides `MsiEnumProducts`, `MsiEnumFeatures`, `MsiEnumComponents`,
//! `MsiQueryProductState`, `MsiQueryFeatureState`, and `MsiGetProductInfo`.

use std::panic;

use crate::handles::MsiHandle;
use crate::win32::strings::{
    lpcstr_to_string, lpcwstr_to_string, string_to_lpstr, string_to_lpwstr,
};
use crate::win32::{
    Dword, Lpcstr, Lpcwstr, Lpstr, Lpwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_PARAMETER,
    ERROR_SUCCESS,
};
use msi::platform::registry_store::{RegistryRoot, RegistryValue};
use msi::platform::squid::{decode_squid, encode_squid};

// The Windows Installer uses standard return codes for enumeration
/// No more items available.
const ERROR_NO_MORE_ITEMS: Uint = 259;

/// INSTALLSTATE enum
const INSTALLSTATE_UNKNOWN: i32 = -1;

/// Helper decoding a SQUID subkey name into a formatted GUID string if possible.
fn subkey_to_guid(subkey: &str) -> String {
    let dec = decode_squid(subkey).unwrap_or_default();
    if dec.is_empty() { subkey } else { &dec }.to_string()
}

/// Helper encoding a GUID string into a SQUID or returning the input string if not a valid GUID.
fn guid_to_squid_or_raw(guid: &str) -> String {
    let enc = encode_squid(guid).unwrap_or_default();
    if enc.is_empty() { guid } else { &enc }.to_string()
}

/// Helper normalizing an optional user SID into a non-empty SID string (`S-1-5-18` default).
fn resolve_user_sid(user_sid: Option<&str>) -> &str {
    let sid = user_sid.unwrap_or("S-1-5-18");
    if sid.is_empty() {
        "S-1-5-18"
    } else {
        sid
    }
}

/// Writes `dw_context` (defaulting `0` to `4` / `MSIINSTALLCONTEXT_MACHINE`) to `pdw_out` if non-null.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
fn write_opt_context(pdw_out: *mut Dword, dw_context: Dword) {
    if pdw_out.is_null() {
        return;
    }
    let val = if dw_context == 0 { 4 } else { dw_context };
    // SAFETY: Pointer checked for null.
    unsafe {
        *pdw_out = val;
    }
}

/// Lists all installed product SQUIDs under `UserData\S-1-5-18\Products` and `HKCR\Installer\Products`.
fn list_product_squids() -> Vec<String> {
    crate::win32::registry_backend::with_registry(|reg| {
        let mut items = reg.list_subkeys(
            RegistryRoot::LocalMachine,
            r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products",
        );
        for k in reg.list_subkeys(RegistryRoot::ClassesRoot, r"Installer\Products") {
            if !items.contains(&k) {
                items.push(k);
            }
        }
        items
    })
}

/// Lists all installed component SQUIDs under `UserData\S-1-5-18\Components`.
fn list_component_squids() -> Vec<String> {
    crate::win32::registry_backend::with_registry(|reg| {
        reg.list_subkeys(
            RegistryRoot::LocalMachine,
            r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Components",
        )
    })
}

/// Enumerates through all the products currently advertised or installed (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumProductsW(
    iProductIndex: Dword,
    lpProductBuf: Lpwstr, // 39 characters for GUID
) -> Uint {
    let result = panic::catch_unwind(|| {
        if lpProductBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let prods = list_product_squids();
        let Some(squid) = prods.get(iProductIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let guid = subkey_to_guid(squid);
        let mut cch: Dword = 39;
        string_to_lpwstr(&guid, lpProductBuf, &raw mut cch)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates through all the products currently advertised or installed (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumProductsA(
    iProductIndex: Dword,
    lpProductBuf: Lpstr, // 39 characters for GUID
) -> Uint {
    let result = panic::catch_unwind(|| {
        if lpProductBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let prods = list_product_squids();
        let Some(squid) = prods.get(iProductIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let guid = subkey_to_guid(squid);
        let mut cch: Dword = 39;
        string_to_lpstr(&guid, lpProductBuf, &raw mut cch)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Lists `(feature_id, parent_feature_id)` pairs for a product from `HKCR\Installer\Features\<SQUID>` or `UserData\S-1-5-18\Products\<SQUID>\Features`.
fn list_product_features(product: &str) -> Vec<(String, String)> {
    let prod_id = guid_to_squid_or_raw(product);
    let hkcr_key = format!(r"Installer\Features\{prod_id}");
    let hklm_key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products\{prod_id}\Features"
    );
    crate::win32::registry_backend::with_registry(|reg| {
        let mut vals = reg.list_values(RegistryRoot::ClassesRoot, &hkcr_key);
        if vals.is_empty() {
            vals = reg.list_values(RegistryRoot::LocalMachine, &hklm_key);
        }
        let mut out = Vec::new();
        for (name, val) in vals {
            if let (Some(feat), RegistryValue::Sz(parent)) = (name, val) {
                out.push((feat, parent));
            }
        }
        out.sort();
        out
    })
}

/// Enumerates the published features for a given product (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumFeaturesW(
    szProduct: Lpcwstr,
    iFeatureIndex: Dword,
    lpFeatureBuf: Lpwstr,
    lpParentBuf: Lpwstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let (Some(prod), false) = (lpcwstr_to_string(szProduct), lpFeatureBuf.is_null()) else {
            return ERROR_INVALID_PARAMETER;
        };
        let features = list_product_features(&prod);
        let Some((feat, parent)) = features.get(iFeatureIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let mut cch: Dword = 256;
        let _ = string_to_lpwstr(feat, lpFeatureBuf, &raw mut cch);
        if !lpParentBuf.is_null() {
            let mut pcch: Dword = 256;
            let _ = string_to_lpwstr(parent, lpParentBuf, &raw mut pcch);
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the published features for a given product (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumFeaturesA(
    szProduct: Lpcstr,
    iFeatureIndex: Dword,
    lpFeatureBuf: Lpstr,
    lpParentBuf: Lpstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let (Some(prod), false) = (lpcstr_to_string(szProduct), lpFeatureBuf.is_null()) else {
            return ERROR_INVALID_PARAMETER;
        };
        let features = list_product_features(&prod);
        let Some((feat, parent)) = features.get(iFeatureIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let mut cch: Dword = 256;
        let _ = string_to_lpstr(feat, lpFeatureBuf, &raw mut cch);
        if !lpParentBuf.is_null() {
            let mut pcch: Dword = 256;
            let _ = string_to_lpstr(parent, lpParentBuf, &raw mut pcch);
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the installed components (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumComponentsW(
    iComponentIndex: Dword,
    lpComponentBuf: Lpwstr, // 39 chars GUID
) -> Uint {
    let result = panic::catch_unwind(|| {
        if lpComponentBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let comps = list_component_squids();
        let Some(squid) = comps.get(iComponentIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let guid = subkey_to_guid(squid);
        let mut cch: Dword = 39;
        string_to_lpwstr(&guid, lpComponentBuf, &raw mut cch)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the installed components (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumComponentsA(
    iComponentIndex: Dword,
    lpComponentBuf: Lpstr, // 39 chars GUID
) -> Uint {
    let result = panic::catch_unwind(|| {
        if lpComponentBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let comps = list_component_squids();
        let Some(squid) = comps.get(iComponentIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let guid = subkey_to_guid(squid);
        let mut cch: Dword = 39;
        string_to_lpstr(&guid, lpComponentBuf, &raw mut cch)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Shared helper querying product state in the registry (`5` = `INSTALLSTATE_DEFAULT`, `1` = `INSTALLSTATE_ADVERTISED`, `-1` = `INSTALLSTATE_UNKNOWN`).
fn query_product_state_in_registry(product: &str) -> i32 {
    let prod_id = encode_squid(product).unwrap_or_else(|_| product.to_string());
    let hklm_key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products\{prod_id}"
    );
    let hkcr_key = format!(r"Installer\Products\{prod_id}");
    crate::win32::registry_backend::with_registry(|reg| {
        if reg.has_key(RegistryRoot::LocalMachine, &hklm_key) {
            5 // INSTALLSTATE_DEFAULT
        } else if reg.has_key(RegistryRoot::ClassesRoot, &hkcr_key) {
            1 // INSTALLSTATE_ADVERTISED
        } else {
            INSTALLSTATE_UNKNOWN
        }
    })
}

/// Returns the installed state for a product (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiQueryProductStateW(szProduct: Lpcwstr) -> i32 {
    let result = panic::catch_unwind(|| {
        let Some(prod) = lpcwstr_to_string(szProduct) else {
            return INSTALLSTATE_UNKNOWN;
        };
        query_product_state_in_registry(&prod)
    });

    result.unwrap_or(INSTALLSTATE_UNKNOWN)
}

/// Returns the installed state for a product (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiQueryProductStateA(szProduct: Lpcstr) -> i32 {
    let result = panic::catch_unwind(|| {
        let Some(prod) = lpcstr_to_string(szProduct) else {
            return INSTALLSTATE_UNKNOWN;
        };
        query_product_state_in_registry(&prod)
    });

    result.unwrap_or(INSTALLSTATE_UNKNOWN)
}

/// Shared helper querying feature state in the registry (`3` = `INSTALLSTATE_LOCAL`, `1` = `INSTALLSTATE_ADVERTISED`, `-1` = `INSTALLSTATE_UNKNOWN`).
fn query_feature_state_in_registry(
    product: &str,
    feature: &str,
    user_sid: Option<&str>,
) -> Option<i32> {
    let sid = resolve_user_sid(user_sid);
    let prod_id = guid_to_squid_or_raw(product);
    let hklm_key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\{sid}\Products\{prod_id}\Features"
    );
    let hkcr_key = format!(r"Installer\Features\{prod_id}");
    crate::win32::registry_backend::with_registry(|reg| {
        if reg
            .get_value(RegistryRoot::LocalMachine, &hklm_key, Some(feature))
            .is_some()
        {
            Some(3) // INSTALLSTATE_LOCAL
        } else if reg
            .get_value(RegistryRoot::ClassesRoot, &hkcr_key, Some(feature))
            .is_some()
        {
            Some(1) // INSTALLSTATE_ADVERTISED
        } else {
            None
        }
    })
}

/// Returns the installed state for a product feature (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiQueryFeatureStateW(szProduct: Lpcwstr, szFeature: Lpcwstr) -> i32 {
    let result = panic::catch_unwind(|| {
        let (Some(prod), Some(feat)) = (lpcwstr_to_string(szProduct), lpcwstr_to_string(szFeature))
        else {
            return INSTALLSTATE_UNKNOWN;
        };
        query_feature_state_in_registry(&prod, &feat, None).unwrap_or(INSTALLSTATE_UNKNOWN)
    });

    result.unwrap_or(INSTALLSTATE_UNKNOWN)
}

/// Returns the installed state for a product feature (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiQueryFeatureStateA(szProduct: Lpcstr, szFeature: Lpcstr) -> i32 {
    let result = panic::catch_unwind(|| {
        let (Some(prod), Some(feat)) = (lpcstr_to_string(szProduct), lpcstr_to_string(szFeature))
        else {
            return INSTALLSTATE_UNKNOWN;
        };
        query_feature_state_in_registry(&prod, &feat, None).unwrap_or(INSTALLSTATE_UNKNOWN)
    });

    result.unwrap_or(INSTALLSTATE_UNKNOWN)
}

/// Shared helper querying product info from `InstallProperties` or `HKCR\Installer\Products\<SQUID>`.
fn query_product_info_in_registry(product: &str, attribute: &str) -> Option<String> {
    let prod_id = guid_to_squid_or_raw(product);
    let prop_key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products\{prod_id}\InstallProperties"
    );
    let hkcr_key = format!(r"Installer\Products\{prod_id}");
    crate::win32::registry_backend::with_registry(|reg| {
        if let Some(RegistryValue::Sz(s)) =
            reg.get_value(RegistryRoot::LocalMachine, &prop_key, Some(attribute))
        {
            return Some(s);
        }
        if let Some(RegistryValue::Sz(s)) =
            reg.get_value(RegistryRoot::ClassesRoot, &hkcr_key, Some(attribute))
        {
            return Some(s);
        }
        None
    })
}

/// Returns product information (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetProductInfoW(
    szProduct: Lpcwstr,
    szAttribute: Lpcwstr,
    lpValueBuf: Lpwstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let (Some(prod), Some(attr)) =
            (lpcwstr_to_string(szProduct), lpcwstr_to_string(szAttribute))
        else {
            return ERROR_INVALID_PARAMETER;
        };
        if let Some(val) = query_product_info_in_registry(&prod, &attr) {
            return string_to_lpwstr(&val, lpValueBuf, pcchValueBuf);
        }
        crate::win32::ERROR_INVALID_DATA
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Returns product information (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetProductInfoA(
    szProduct: Lpcstr,
    szAttribute: Lpcstr,
    lpValueBuf: Lpstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let (Some(prod), Some(attr)) = (lpcstr_to_string(szProduct), lpcstr_to_string(szAttribute))
        else {
            return ERROR_INVALID_PARAMETER;
        };
        if let Some(val) = query_product_info_in_registry(&prod, &attr) {
            return string_to_lpstr(&val, lpValueBuf, pcchValueBuf);
        }
        crate::win32::ERROR_INVALID_DATA
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Lists all client product GUIDs registered for a component.
fn list_component_clients(component: &str, user_sid: Option<&str>) -> Vec<String> {
    let sid = resolve_user_sid(user_sid);
    let comp_id = guid_to_squid_or_raw(component);
    let key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\{sid}\Components\{comp_id}"
    );
    crate::win32::registry_backend::with_registry(|reg| {
        let mut clients: Vec<String> = reg
            .list_values(RegistryRoot::LocalMachine, &key)
            .into_iter()
            .filter_map(|(name, _)| {
                let n = name?;
                Some(subkey_to_guid(&n))
            })
            .collect();
        clients.sort();
        clients
    })
}

/// Enumerates the clients for a given component (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumClientsW(
    szComponent: Lpcwstr,
    iProductIndex: Dword,
    lpProductBuf: Lpwstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let (Some(comp), false) = (lpcwstr_to_string(szComponent), lpProductBuf.is_null()) else {
            return ERROR_INVALID_PARAMETER;
        };
        let clients = list_component_clients(&comp, None);
        let Some(prod_guid) = clients.get(iProductIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let mut cch: Dword = 39;
        string_to_lpwstr(prod_guid, lpProductBuf, &raw mut cch)
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the clients for a given component (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumClientsA(
    szComponent: Lpcstr,
    iProductIndex: Dword,
    lpProductBuf: Lpstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let (Some(comp), false) = (lpcstr_to_string(szComponent), lpProductBuf.is_null()) else {
            return ERROR_INVALID_PARAMETER;
        };
        let clients = list_component_clients(&comp, None);
        let Some(prod_guid) = clients.get(iProductIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let mut cch: Dword = 39;
        string_to_lpstr(prod_guid, lpProductBuf, &raw mut cch)
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the clients for a given component with advanced options (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumClientsExW(
    szComponent: Lpcwstr,
    szUserSid: Lpcwstr,
    dwContext: Dword,
    dwProductIndex: Dword,
    szProductBuf: Lpwstr,
    pdwContext: *mut Dword,
    szSidBuf: Lpwstr,
    pcchSidBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let Some(comp) = lpcwstr_to_string(szComponent) else {
            return ERROR_INVALID_PARAMETER;
        };
        let sid = lpcwstr_to_string(szUserSid);
        let clients = list_component_clients(&comp, sid.as_deref());
        let Some(prod_guid) = clients.get(dwProductIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let mut cch: Dword = 39;
        let _ = string_to_lpwstr(prod_guid, szProductBuf, &raw mut cch);
        write_opt_context(pdwContext, dwContext);
        let sid_str = sid.as_deref().unwrap_or("S-1-5-18");
        let _ = string_to_lpwstr(sid_str, szSidBuf, pcchSidBuf);
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the clients for a given component with advanced options (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumClientsExA(
    szComponent: Lpcstr,
    szUserSid: Lpcstr,
    dwContext: Dword,
    dwProductIndex: Dword,
    szProductBuf: Lpstr,
    pdwContext: *mut Dword,
    szSidBuf: Lpstr,
    pcchSidBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let Some(comp) = lpcstr_to_string(szComponent) else {
            return ERROR_INVALID_PARAMETER;
        };
        let sid = lpcstr_to_string(szUserSid);
        let clients = list_component_clients(&comp, sid.as_deref());
        let Some(prod_guid) = clients.get(dwProductIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let mut cch: Dword = 39;
        let _ = string_to_lpstr(prod_guid, szProductBuf, &raw mut cch);
        write_opt_context(pdwContext, dwContext);
        let sid_str = sid.as_deref().unwrap_or("S-1-5-18");
        let _ = string_to_lpstr(sid_str, szSidBuf, pcchSidBuf);
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Lists `(qualifier, app_data)` pairs for a qualified component under `HKCR\Installer\Components\<SQUID>`.
fn list_component_qualifiers(component: &str) -> Vec<(String, String)> {
    let comp_id = guid_to_squid_or_raw(component);
    let key = format!(r"Installer\Components\{comp_id}");
    crate::win32::registry_backend::with_registry(|reg| {
        let mut out = Vec::new();
        for (name, val) in reg.list_values(RegistryRoot::ClassesRoot, &key) {
            if let (Some(q), RegistryValue::Sz(data)) = (name, val) {
                out.push((q, data));
            }
        }
        out.sort();
        out
    })
}

/// Enumerates the qualifiers for a given component (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumComponentQualifiersW(
    szComponent: Lpcwstr,
    iIndex: Dword,
    lpQualifierBuf: Lpwstr,
    pcchQualifierBuf: *mut Dword,
    lpApplicationDataBuf: Lpwstr,
    pcchApplicationDataBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let Some(comp) = lpcwstr_to_string(szComponent) else {
            return ERROR_INVALID_PARAMETER;
        };
        let quals = list_component_qualifiers(&comp);
        let Some((qual, app_data)) = quals.get(iIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let _ = string_to_lpwstr(qual, lpQualifierBuf, pcchQualifierBuf);
        string_to_lpwstr(app_data, lpApplicationDataBuf, pcchApplicationDataBuf)
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the qualifiers for a given component (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumComponentQualifiersA(
    szComponent: Lpcstr,
    iIndex: Dword,
    lpQualifierBuf: Lpstr,
    pcchQualifierBuf: *mut Dword,
    lpApplicationDataBuf: Lpstr,
    pcchApplicationDataBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let Some(comp) = lpcstr_to_string(szComponent) else {
            return ERROR_INVALID_PARAMETER;
        };
        let quals = list_component_qualifiers(&comp);
        let Some((qual, app_data)) = quals.get(iIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let _ = string_to_lpstr(qual, lpQualifierBuf, pcchQualifierBuf);
        string_to_lpstr(app_data, lpApplicationDataBuf, pcchApplicationDataBuf)
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Lists `(patch_guid, transforms)` applied to a product under `UserData\<SID>\Products\<SQUID>\Patches`.
fn list_product_patches(product: &str, user_sid: Option<&str>) -> Vec<(String, String)> {
    let sid = resolve_user_sid(user_sid);
    let prod_id = guid_to_squid_or_raw(product);
    let patches_key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\{sid}\Products\{prod_id}\Patches"
    );
    crate::win32::registry_backend::with_registry(|reg| {
        let subkeys = reg.list_subkeys(RegistryRoot::LocalMachine, &patches_key);
        subkeys
            .into_iter()
            .map(|patch_squid| {
                let patch_guid = subkey_to_guid(&patch_squid);
                let sub = format!(r"{patches_key}\{patch_squid}");
                let mut transforms = String::new();
                if let Some(RegistryValue::Sz(s)) =
                    reg.get_value(RegistryRoot::LocalMachine, &sub, Some("Transforms"))
                {
                    transforms = s;
                }
                (patch_guid, transforms)
            })
            .collect()
    })
}

/// Enumerates the patches for a given product (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumPatchesW(
    szProduct: Lpcwstr,
    iPatchIndex: Dword,
    lpPatchBuf: Lpwstr,
    lpTransformsBuf: Lpwstr,
    pcchTransformsBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let (Some(prod), false) = (lpcwstr_to_string(szProduct), lpPatchBuf.is_null()) else {
            return ERROR_INVALID_PARAMETER;
        };
        let patches = list_product_patches(&prod, None);
        let Some((patch_guid, transforms)) = patches.get(iPatchIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let mut cch: Dword = 39;
        let _ = string_to_lpwstr(patch_guid, lpPatchBuf, &raw mut cch);
        string_to_lpwstr(transforms, lpTransformsBuf, pcchTransformsBuf)
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the patches for a given product (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumPatchesA(
    szProduct: Lpcstr,
    iPatchIndex: Dword,
    lpPatchBuf: Lpstr,
    lpTransformsBuf: Lpstr,
    pcchTransformsBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let (Some(prod), false) = (lpcstr_to_string(szProduct), lpPatchBuf.is_null()) else {
            return ERROR_INVALID_PARAMETER;
        };
        let patches = list_product_patches(&prod, None);
        let Some((patch_guid, transforms)) = patches.get(iPatchIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let mut cch: Dword = 39;
        let _ = string_to_lpstr(patch_guid, lpPatchBuf, &raw mut cch);
        string_to_lpstr(transforms, lpTransformsBuf, pcchTransformsBuf)
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the patches with advanced options (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumPatchesExW(
    szProductCode: Lpcwstr,
    szUserSid: Lpcwstr,
    dwContext: Dword,
    dwFilter: Dword,
    dwIndex: Dword,
    szPatchCode: Lpwstr,
    szTargetProductCode: Lpwstr,
    pdwTargetProductContext: *mut Dword,
    szTargetUserSid: Lpwstr,
    pcchTargetUserSid: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let Some(prod) = lpcwstr_to_string(szProductCode) else {
            return ERROR_INVALID_PARAMETER;
        };
        let sid = lpcwstr_to_string(szUserSid);
        let patches = list_product_patches(&prod, sid.as_deref());
        let Some((patch_guid, _)) = patches.get(dwIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let mut cch: Dword = 39;
        let _ = string_to_lpwstr(patch_guid, szPatchCode, &raw mut cch);
        let mut pcch: Dword = 39;
        let _ = string_to_lpwstr(&prod, szTargetProductCode, &raw mut pcch);
        write_opt_context(pdwTargetProductContext, dwContext);
        let sid_str = sid.as_deref().unwrap_or("S-1-5-18");
        let _ = string_to_lpwstr(sid_str, szTargetUserSid, pcchTargetUserSid);
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the patches with advanced options (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumPatchesExA(
    szProductCode: Lpcstr,
    szUserSid: Lpcstr,
    dwContext: Dword,
    dwFilter: Dword,
    dwIndex: Dword,
    szPatchCode: Lpstr,
    szTargetProductCode: Lpstr,
    pdwTargetProductContext: *mut Dword,
    szTargetUserSid: Lpstr,
    pcchTargetUserSid: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let Some(prod) = lpcstr_to_string(szProductCode) else {
            return ERROR_INVALID_PARAMETER;
        };
        let sid = lpcstr_to_string(szUserSid);
        let patches = list_product_patches(&prod, sid.as_deref());
        let Some((patch_guid, _)) = patches.get(dwIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let mut cch: Dword = 39;
        let _ = string_to_lpstr(patch_guid, szPatchCode, &raw mut cch);
        let mut pcch: Dword = 39;
        let _ = string_to_lpstr(&prod, szTargetProductCode, &raw mut pcch);
        write_opt_context(pdwTargetProductContext, dwContext);
        let sid_str = sid.as_deref().unwrap_or("S-1-5-18");
        let _ = string_to_lpstr(sid_str, szTargetUserSid, pcchTargetUserSid);
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Lists all product GUIDs registered under `HKCR\Installer\UpgradeCodes\<SQUID>` or `HKLM\Software\Microsoft\Windows\CurrentVersion\Installer\UpgradeCodes\<SQUID>`.
fn list_related_products(upgrade_code: &str) -> Vec<String> {
    let upg_id = guid_to_squid_or_raw(upgrade_code);
    let hkcr_key = format!(r"Installer\UpgradeCodes\{upg_id}");
    let hklm_key =
        format!(r"Software\Microsoft\Windows\CurrentVersion\Installer\UpgradeCodes\{upg_id}");
    crate::win32::registry_backend::with_registry(|reg| {
        let mut vals = reg.list_values(RegistryRoot::ClassesRoot, &hkcr_key);
        if vals.is_empty() {
            vals = reg.list_values(RegistryRoot::LocalMachine, &hklm_key);
        }
        let mut prods: Vec<String> = vals
            .into_iter()
            .filter_map(|(name, _)| {
                let prod_squid = name?;
                Some(subkey_to_guid(&prod_squid))
            })
            .collect();
        prods.sort();
        prods
    })
}

/// Enumerates the related products (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumRelatedProductsW(
    szUpgradeCode: Lpcwstr,
    dwReserved: Dword,
    iProductIndex: Dword,
    lpProductBuf: Lpwstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let (Some(upg), false) = (lpcwstr_to_string(szUpgradeCode), lpProductBuf.is_null()) else {
            return ERROR_INVALID_PARAMETER;
        };
        let prods = list_related_products(&upg);
        let Some(prod_guid) = prods.get(iProductIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let mut cch: Dword = 39;
        string_to_lpwstr(prod_guid, lpProductBuf, &raw mut cch)
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the related products (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumRelatedProductsA(
    szUpgradeCode: Lpcstr,
    dwReserved: Dword,
    iProductIndex: Dword,
    lpProductBuf: Lpstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let (Some(upg), false) = (lpcstr_to_string(szUpgradeCode), lpProductBuf.is_null()) else {
            return ERROR_INVALID_PARAMETER;
        };
        let prods = list_related_products(&upg);
        let Some(prod_guid) = prods.get(iProductIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let mut cch: Dword = 39;
        string_to_lpstr(prod_guid, lpProductBuf, &raw mut cch)
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the products with advanced options (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumProductsExW(
    szProductCode: Lpcwstr,
    szUserSid: Lpcwstr,
    dwContext: Dword,
    dwIndex: Dword,
    szInstalledProductCode: Lpwstr,
    pdwInstalledContext: *mut Dword,
    szSid: Lpwstr,
    pcchSid: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let prods = list_product_squids();
        let Some(squid) = prods.get(dwIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let guid = subkey_to_guid(squid);
        let mut cch: Dword = 39;
        let _ = string_to_lpwstr(&guid, szInstalledProductCode, &raw mut cch);
        write_opt_context(pdwInstalledContext, dwContext);
        let sid = lpcwstr_to_string(szUserSid).unwrap_or_else(|| "S-1-5-18".to_string());
        let _ = string_to_lpwstr(&sid, szSid, pcchSid);
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the products with advanced options (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumProductsExA(
    szProductCode: Lpcstr,
    szUserSid: Lpcstr,
    dwContext: Dword,
    dwIndex: Dword,
    szInstalledProductCode: Lpstr,
    pdwInstalledContext: *mut Dword,
    szSid: Lpstr,
    pcchSid: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let prods = list_product_squids();
        let Some(squid) = prods.get(dwIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let guid = subkey_to_guid(squid);
        let mut cch: Dword = 39;
        let _ = string_to_lpstr(&guid, szInstalledProductCode, &raw mut cch);
        write_opt_context(pdwInstalledContext, dwContext);
        let sid = lpcstr_to_string(szUserSid).unwrap_or_else(|| "S-1-5-18".to_string());
        let _ = string_to_lpstr(&sid, szSid, pcchSid);
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the components with advanced options (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumComponentsExW(
    szUserSid: Lpcwstr,
    dwContext: Dword,
    dwIndex: Dword,
    szInstalledComponentCode: Lpwstr,
    pdwInstalledContext: *mut Dword,
    szSid: Lpwstr,
    pcchSid: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let comps = list_component_squids();
        let Some(squid) = comps.get(dwIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let guid = subkey_to_guid(squid);
        let mut cch: Dword = 39;
        let _ = string_to_lpwstr(&guid, szInstalledComponentCode, &raw mut cch);
        write_opt_context(pdwInstalledContext, dwContext);
        let sid = lpcwstr_to_string(szUserSid).unwrap_or_else(|| "S-1-5-18".to_string());
        let _ = string_to_lpwstr(&sid, szSid, pcchSid);
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the components with advanced options (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumComponentsExA(
    szUserSid: Lpcstr,
    dwContext: Dword,
    dwIndex: Dword,
    szInstalledComponentCode: Lpstr,
    pdwInstalledContext: *mut Dword,
    szSid: Lpstr,
    pcchSid: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let comps = list_component_squids();
        let Some(squid) = comps.get(dwIndex as usize) else {
            return ERROR_NO_MORE_ITEMS;
        };
        let guid = subkey_to_guid(squid);
        let mut cch: Dword = 39;
        let _ = string_to_lpstr(&guid, szInstalledComponentCode, &raw mut cch);
        write_opt_context(pdwInstalledContext, dwContext);
        let sid = lpcstr_to_string(szUserSid).unwrap_or_else(|| "S-1-5-18".to_string());
        let _ = string_to_lpstr(&sid, szSid, pcchSid);
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Advanced feature state check (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiQueryFeatureStateExW(
    szProductCode: Lpcwstr,
    szUserSid: Lpcwstr,
    dwContext: Dword,
    szFeature: Lpcwstr,
    pdwState: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let (Some(prod), Some(feat)) = (
            lpcwstr_to_string(szProductCode),
            lpcwstr_to_string(szFeature),
        ) else {
            return ERROR_INVALID_PARAMETER;
        };
        let sid = lpcwstr_to_string(szUserSid);
        if let Some(st) = query_feature_state_in_registry(&prod, &feat, sid.as_deref()) {
            if let Some(out) = unsafe { pdwState.as_mut() } {
                *out = st as Dword;
            }
            return ERROR_SUCCESS;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Advanced feature state check (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiQueryFeatureStateExA(
    szProductCode: Lpcstr,
    szUserSid: Lpcstr,
    dwContext: Dword,
    szFeature: Lpcstr,
    pdwState: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let (Some(prod), Some(feat)) =
            (lpcstr_to_string(szProductCode), lpcstr_to_string(szFeature))
        else {
            return ERROR_INVALID_PARAMETER;
        };
        let sid = lpcstr_to_string(szUserSid);
        if let Some(st) = query_feature_state_in_registry(&prod, &feat, sid.as_deref()) {
            if let Some(out) = unsafe { pdwState.as_mut() } {
                *out = st as Dword;
            }
            return ERROR_SUCCESS;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Helper to query installed and action states for a feature or component on `h_install`.
fn query_session_item_state(
    h_install: MsiHandle,
    prefix: &str,
    name: &str,
    pi_installed: *mut i32,
    pi_action: *mut i32,
) -> Uint {
    if h_install == 0 {
        return ERROR_NO_MORE_ITEMS;
    }
    let inst_key = format!("_MSI_{prefix}_INSTALLED_{name}");
    let act_key = format!("_MSI_{prefix}_ACTION_{name}");
    crate::handles::with_handle(h_install, |obj| {
        if let crate::handles::MsiObject::Database(db) = obj {
            let installed = db
                .inner
                .get_property_value(&inst_key)
                .unwrap_or("2")
                .parse::<i32>()
                .unwrap_or(2);
            let action = db
                .inner
                .get_property_value(&act_key)
                .unwrap_or("3")
                .parse::<i32>()
                .unwrap_or(3);
            if let Some(out) = unsafe { pi_installed.as_mut() } {
                *out = installed;
            }
            if let Some(out) = unsafe { pi_action.as_mut() } {
                *out = action;
            }
            ERROR_SUCCESS
        } else {
            ERROR_NO_MORE_ITEMS
        }
    })
    .unwrap_or(ERROR_NO_MORE_ITEMS)
}

/// Get current and action state for component (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetComponentStateW(
    hInstall: MsiHandle,
    szComponent: Lpcwstr,
    piInstalled: *mut i32,
    piAction: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(comp) = lpcwstr_to_string(szComponent) else {
            return ERROR_INVALID_PARAMETER;
        };
        query_session_item_state(hInstall, "COMP", &comp, piInstalled, piAction)
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Get current and action state for component (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetComponentStateA(
    hInstall: MsiHandle,
    szComponent: Lpcstr,
    piInstalled: *mut i32,
    piAction: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(comp) = lpcstr_to_string(szComponent) else {
            return ERROR_INVALID_PARAMETER;
        };
        query_session_item_state(hInstall, "COMP", &comp, piInstalled, piAction)
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Get current and action state for feature (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFeatureStateW(
    hInstall: MsiHandle,
    szFeature: Lpcwstr,
    piInstalled: *mut i32,
    piAction: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(feat) = lpcwstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        query_session_item_state(hInstall, "FEAT", &feat, piInstalled, piAction)
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Get current and action state for feature (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFeatureStateA(
    hInstall: MsiHandle,
    szFeature: Lpcstr,
    piInstalled: *mut i32,
    piAction: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(feat) = lpcstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        query_session_item_state(hInstall, "FEAT", &feat, piInstalled, piAction)
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Shared helper resolving component keypath for a product and component in the registry.
fn lookup_component_path_in_registry(
    product: &str,
    component: &str,
    user_sid: Option<&str>,
) -> Option<String> {
    let sid = resolve_user_sid(user_sid);
    let prod_id = guid_to_squid_or_raw(product);
    let comp_id = guid_to_squid_or_raw(component);
    let key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\{sid}\Components\{comp_id}"
    );
    crate::win32::registry_backend::with_registry(|reg| {
        if let Some(RegistryValue::Sz(path)) =
            reg.get_value(RegistryRoot::LocalMachine, &key, Some(&prod_id))
        {
            Some(path)
        } else {
            None
        }
    })
}

/// Locate component file path (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetComponentPathExW(
    szProductCode: Lpcwstr,
    szComponentCode: Lpcwstr,
    szUserSid: Lpcwstr,
    dwContext: Dword,
    lpOutPathBuffer: Lpwstr,
    pcchOutPathBuffer: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let (Some(prod), Some(comp)) = (
            lpcwstr_to_string(szProductCode),
            lpcwstr_to_string(szComponentCode),
        ) else {
            return ERROR_INVALID_PARAMETER;
        };
        let sid = lpcwstr_to_string(szUserSid);
        if let Some(path) = lookup_component_path_in_registry(&prod, &comp, sid.as_deref()) {
            let _ = string_to_lpwstr(&path, lpOutPathBuffer, pcchOutPathBuffer);
            return 3; // INSTALLSTATE_LOCAL
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Locate component file path (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetComponentPathExA(
    szProductCode: Lpcstr,
    szComponentCode: Lpcstr,
    szUserSid: Lpcstr,
    dwContext: Dword,
    lpOutPathBuffer: Lpstr,
    pcchOutPathBuffer: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let (Some(prod), Some(comp)) = (
            lpcstr_to_string(szProductCode),
            lpcstr_to_string(szComponentCode),
        ) else {
            return ERROR_INVALID_PARAMETER;
        };
        let sid = lpcstr_to_string(szUserSid);
        if let Some(path) = lookup_component_path_in_registry(&prod, &comp, sid.as_deref()) {
            let _ = string_to_lpstr(&path, lpOutPathBuffer, pcchOutPathBuffer);
            return 3; // INSTALLSTATE_LOCAL
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Locate component file path for the current user (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetComponentPathW(
    szProduct: Lpcwstr,
    szComponent: Lpcwstr,
    lpPathBuf: Lpwstr,
    pcchBuf: *mut Dword,
) -> i32 {
    let result = panic::catch_unwind(|| {
        let (Some(prod), Some(comp)) =
            (lpcwstr_to_string(szProduct), lpcwstr_to_string(szComponent))
        else {
            return INSTALLSTATE_UNKNOWN;
        };
        if let Some(path) = lookup_component_path_in_registry(&prod, &comp, None) {
            let _ = string_to_lpwstr(&path, lpPathBuf, pcchBuf);
            return 3; // INSTALLSTATE_LOCAL
        }
        1 // INSTALLSTATE_LOCAL fallback
    });
    result.unwrap_or(INSTALLSTATE_UNKNOWN)
}

/// Locate component file path for the current user (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetComponentPathA(
    szProduct: Lpcstr,
    szComponent: Lpcstr,
    lpPathBuf: Lpstr,
    pcchBuf: *mut Dword,
) -> i32 {
    let result = panic::catch_unwind(|| {
        let (Some(prod), Some(comp)) = (lpcstr_to_string(szProduct), lpcstr_to_string(szComponent))
        else {
            return INSTALLSTATE_UNKNOWN;
        };
        if let Some(path) = lookup_component_path_in_registry(&prod, &comp, None) {
            let _ = string_to_lpstr(&path, lpPathBuf, pcchBuf);
            return 3; // INSTALLSTATE_LOCAL
        }
        1 // INSTALLSTATE_LOCAL fallback
    });
    result.unwrap_or(INSTALLSTATE_UNKNOWN)
}

/// Retrieve valid installation states for feature (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFeatureValidStatesW(
    hInstall: MsiHandle,
    szFeature: Lpcwstr,
    dwInstallStates: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        if hInstall != 0 && !dwInstallStates.is_null() {
            unsafe {
                *dwInstallStates = (1 << 2) | (1 << 3);
            }
            return ERROR_SUCCESS;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieve valid installation states for feature (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFeatureValidStatesA(
    hInstall: MsiHandle,
    szFeature: Lpcstr,
    dwInstallStates: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        if hInstall != 0 && !dwInstallStates.is_null() {
            unsafe {
                *dwInstallStates = (1 << 2) | (1 << 3);
            }
            return ERROR_SUCCESS;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Calculate feature disk cost (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFeatureCostW(
    hInstall: MsiHandle,
    szFeature: Lpcwstr,
    iCostTree: i32,
    iState: i32,
    piCost: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        if hInstall != 0 && !piCost.is_null() {
            unsafe {
                *piCost = 0;
            }
            return ERROR_SUCCESS;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Calculate feature disk cost (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFeatureCostA(
    hInstall: MsiHandle,
    szFeature: Lpcstr,
    iCostTree: i32,
    iState: i32,
    piCost: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        if hInstall != 0 && !piCost.is_null() {
            unsafe {
                *piCost = 0;
            }
            return ERROR_SUCCESS;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerate drive costs for component (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumComponentCostsW(
    hInstall: MsiHandle,
    szComponent: Lpcwstr,
    dwIndex: Dword,
    iState: i32,
    szDriveBuf: Lpwstr,
    pcchDriveBuf: *mut Dword,
    piCost: *mut i32,
    piTempCost: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerate drive costs for component (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumComponentCostsA(
    hInstall: MsiHandle,
    szComponent: Lpcstr,
    dwIndex: Dword,
    iState: i32,
    szDriveBuf: Lpstr,
    pcchDriveBuf: *mut Dword,
    piCost: *mut i32,
    piTempCost: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Parse MSI component descriptor (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiDecomposeDescriptorW(
    szDescriptor: Lpcwstr,
    szProductCode: Lpwstr,
    szFeatureId: Lpwstr,
    szComponentCode: Lpwstr,
    pdwArgsOffset: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szDescriptor.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Parse MSI component descriptor (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiDecomposeDescriptorA(
    szDescriptor: Lpcstr,
    szProductCode: Lpstr,
    szFeatureId: Lpstr,
    szComponentCode: Lpstr,
    pdwArgsOffset: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szDescriptor.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Extract PE file version (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFileVersionW(
    szFilePath: Lpcwstr,
    szVersionPath: Lpwstr,
    pcchVersionPath: *mut Dword,
    szLangPath: Lpwstr,
    pcchLangPath: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let Some(path) = lpcwstr_to_string(szFilePath) else {
            return ERROR_INVALID_PARAMETER;
        };
        if let Some((ver, lang)) = std::fs::read(&path)
            .ok()
            .and_then(|bytes| msi::wix::linker::inspect_pe_version(&bytes))
        {
            let _ = string_to_lpwstr(&ver, szVersionPath, pcchVersionPath);
            return string_to_lpwstr(&lang, szLangPath, pcchLangPath);
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Extract PE file version (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFileVersionA(
    szFilePath: Lpcstr,
    szVersionPath: Lpstr,
    pcchVersionPath: *mut Dword,
    szLangPath: Lpstr,
    pcchLangPath: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let Some(path) = lpcstr_to_string(szFilePath) else {
            return ERROR_INVALID_PARAMETER;
        };
        if let Some((ver, lang)) = std::fs::read(&path)
            .ok()
            .and_then(|bytes| msi::wix::linker::inspect_pe_version(&bytes))
        {
            let _ = string_to_lpstr(&ver, szVersionPath, pcchVersionPath);
            return string_to_lpstr(&lang, szLangPath, pcchLangPath);
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Obtains user information for a product (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiCollectUserInfoW(szProduct: Lpcwstr) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Obtains user information for a product (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiCollectUserInfoA(szProduct: Lpcstr) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Shared helper retrieving `(RegOwner, RegCompany, ProductID)` from `UserData\S-1-5-18\Products\<SQUID>\InstallProperties`.
fn lookup_user_info_in_registry(product: &str) -> Option<(String, String, String)> {
    let prod_id = guid_to_squid_or_raw(product);
    let key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products\{prod_id}\InstallProperties"
    );
    crate::win32::registry_backend::with_registry(|reg| {
        if !reg.has_key(RegistryRoot::LocalMachine, &key) {
            return None;
        }
        let get_sz = |name: &str| {
            let mut out = String::new();
            if let Some(RegistryValue::Sz(s)) =
                reg.get_value(RegistryRoot::LocalMachine, &key, Some(name))
            {
                out = s;
            }
            out
        };
        Some((
            get_sz("RegOwner"),
            get_sz("RegCompany"),
            get_sz("ProductID"),
        ))
    })
}

/// Retrieves registered user information (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetUserInfoW(
    szProduct: Lpcwstr,
    lpUserNameBuf: Lpwstr,
    pcchUserNameBuf: *mut Dword,
    lpOrgNameBuf: Lpwstr,
    pcchOrgNameBuf: *mut Dword,
    lpSerialBuf: Lpwstr,
    pcchSerialBuf: *mut Dword,
) -> i32 {
    let result = panic::catch_unwind(|| {
        let Some(prod) = lpcwstr_to_string(szProduct) else {
            return INSTALLSTATE_UNKNOWN;
        };
        if let Some((owner, org, serial)) = lookup_user_info_in_registry(&prod) {
            let _ = string_to_lpwstr(&owner, lpUserNameBuf, pcchUserNameBuf);
            let _ = string_to_lpwstr(&org, lpOrgNameBuf, pcchOrgNameBuf);
            let _ = string_to_lpwstr(&serial, lpSerialBuf, pcchSerialBuf);
            return 0; // USERINFOSTATE_PRESENT
        }
        INSTALLSTATE_UNKNOWN
    });
    result.unwrap_or(INSTALLSTATE_UNKNOWN)
}

/// Retrieves registered user information (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetUserInfoA(
    szProduct: Lpcstr,
    lpUserNameBuf: Lpstr,
    pcchUserNameBuf: *mut Dword,
    lpOrgNameBuf: Lpstr,
    pcchOrgNameBuf: *mut Dword,
    lpSerialBuf: Lpstr,
    pcchSerialBuf: *mut Dword,
) -> i32 {
    let result = panic::catch_unwind(|| {
        let Some(prod) = lpcstr_to_string(szProduct) else {
            return INSTALLSTATE_UNKNOWN;
        };
        if let Some((owner, org, serial)) = lookup_user_info_in_registry(&prod) {
            let _ = string_to_lpstr(&owner, lpUserNameBuf, pcchUserNameBuf);
            let _ = string_to_lpstr(&org, lpOrgNameBuf, pcchOrgNameBuf);
            let _ = string_to_lpstr(&serial, lpSerialBuf, pcchSerialBuf);
            return 0; // USERINFOSTATE_PRESENT
        }
        INSTALLSTATE_UNKNOWN
    });
    result.unwrap_or(INSTALLSTATE_UNKNOWN)
}

#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
/// Migrates cached packages from Windows Installer 1.0 format.
///
/// This is a legacy API that is no longer used by modern installers. It is safely stubbed to return `ERROR_SUCCESS`.
///
/// # Arguments
/// * `szProductCode` - Product code to migrate.
/// * `szUserSid` - User SID to migrate.
/// * `szPreflightCheck` - Preflight check string.
/// * `dwReserved` - Reserved parameter.
///
/// # Returns
/// `ERROR_SUCCESS`.
#[cfg_attr(not(test), no_mangle)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn Migrate10CachedPackagesW(
    szProductCode: Lpcwstr,
    szUserSid: Lpcwstr,
    szPreflightCheck: Lpcwstr,
    dwReserved: Dword,
) -> Uint {
    let result = panic::catch_unwind(|| ERROR_SUCCESS); // No-op
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::cognitive_complexity)]
mod tests {
    #[test]
    fn test_more_registry_branches() {
        let mut buf_a = [0i8; 39];
        let mut buf_w = [0u16; 39];
        let valid_w: Vec<u16> = "prod".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"prod\0";
        let mut pcch = 0;

        // MsiEnumFeaturesW/A
        assert_eq!(
            MsiEnumFeaturesW(
                valid_w.as_ptr(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumFeaturesW(
                std::ptr::null(),
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumFeaturesA(
                valid_a.as_ptr().cast(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumFeaturesA(
                std::ptr::null(),
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );

        // MsiQueryFeatureStateW/A
        assert_eq!(
            MsiQueryFeatureStateW(valid_w.as_ptr(), std::ptr::null()),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiQueryFeatureStateW(std::ptr::null(), valid_w.as_ptr()),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiQueryFeatureStateA(valid_a.as_ptr().cast(), std::ptr::null()),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiQueryFeatureStateA(std::ptr::null(), valid_a.as_ptr().cast()),
            INSTALLSTATE_UNKNOWN
        );

        // MsiGetProductInfoW/A
        assert_eq!(
            MsiGetProductInfoW(
                std::ptr::null(),
                std::ptr::null(),
                buf_w.as_mut_ptr(),
                &raw mut pcch
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetProductInfoW(
                valid_w.as_ptr(),
                std::ptr::null(),
                buf_w.as_mut_ptr(),
                &raw mut pcch
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetProductInfoW(
                std::ptr::null(),
                valid_w.as_ptr(),
                buf_w.as_mut_ptr(),
                &raw mut pcch
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetProductInfoA(
                std::ptr::null(),
                std::ptr::null(),
                buf_a.as_mut_ptr(),
                &raw mut pcch
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetProductInfoA(
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                buf_a.as_mut_ptr(),
                &raw mut pcch
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetProductInfoA(
                std::ptr::null(),
                valid_a.as_ptr().cast(),
                buf_a.as_mut_ptr(),
                &raw mut pcch
            ),
            ERROR_INVALID_PARAMETER
        );

        // MsiEnumClientsW/A
        assert_eq!(
            MsiEnumClientsW(valid_w.as_ptr(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumClientsW(std::ptr::null(), 0, buf_w.as_mut_ptr()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumClientsA(valid_a.as_ptr().cast(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumClientsA(std::ptr::null(), 0, buf_a.as_mut_ptr()),
            ERROR_INVALID_PARAMETER
        );

        // MsiEnumPatchesW/A
        assert_eq!(
            MsiEnumPatchesW(
                valid_w.as_ptr(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesW(
                std::ptr::null(),
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesA(
                valid_a.as_ptr().cast(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesA(
                std::ptr::null(),
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );

        // MsiEnumRelatedProductsW/A
        assert_eq!(
            MsiEnumRelatedProductsW(valid_w.as_ptr(), 0, 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumRelatedProductsW(std::ptr::null(), 0, 0, buf_w.as_mut_ptr()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumRelatedProductsA(valid_a.as_ptr().cast(), 0, 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumRelatedProductsA(std::ptr::null(), 0, 0, buf_a.as_mut_ptr()),
            ERROR_INVALID_PARAMETER
        );

        // MsiQueryFeatureStateExW/A
        assert_eq!(
            MsiQueryFeatureStateExW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryFeatureStateExW(
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryFeatureStateExW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                valid_w.as_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryFeatureStateExA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryFeatureStateExA(
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryFeatureStateExA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                valid_a.as_ptr().cast(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );

        // MsiGetComponentPathExW/A
        assert_eq!(
            MsiGetComponentPathExW(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetComponentPathExW(
                valid_w.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetComponentPathExW(
                std::ptr::null(),
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetComponentPathExA(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetComponentPathExA(
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetComponentPathExA(
                std::ptr::null(),
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );

        // MsiGetComponentPathW/A
        assert_eq!(
            MsiGetComponentPathW(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiGetComponentPathW(
                valid_w.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiGetComponentPathW(
                std::ptr::null(),
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiGetComponentPathA(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiGetComponentPathA(
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiGetComponentPathA(
                std::ptr::null(),
                valid_a.as_ptr().cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );
    }
    use super::*;
    use crate::locator::{MsiGetShortcutTargetA, MsiGetShortcutTargetW};

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_registry_stubs() {
        let mut buf_a = [0i8; 39];
        let mut buf_w = [0u16; 39];
        assert_eq!(
            MsiEnumProductsW(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumProductsA(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(MsiEnumProductsW(0, buf_w.as_mut_ptr()), ERROR_NO_MORE_ITEMS);

        assert_eq!(
            MsiQueryProductStateW(std::ptr::null()),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiQueryProductStateA(std::ptr::null()),
            INSTALLSTATE_UNKNOWN
        );

        assert_eq!(
            MsiQueryFeatureStateW(std::ptr::null(), std::ptr::null()),
            INSTALLSTATE_UNKNOWN
        );

        assert_eq!(
            MsiGetProductInfoW(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(MsiEnumProductsA(0, buf_a.as_mut_ptr()), ERROR_NO_MORE_ITEMS);

        let valid_w: Vec<u16> = "prod".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"prod\0";

        assert_eq!(
            MsiEnumFeaturesW(
                valid_w.as_ptr(),
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumFeaturesA(
                valid_a.as_ptr().cast::<i8>(),
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );

        assert_eq!(
            MsiEnumComponentsW(0, buf_w.as_mut_ptr()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsA(0, buf_a.as_mut_ptr()),
            ERROR_NO_MORE_ITEMS
        );

        assert_eq!(
            MsiQueryProductStateW(valid_w.as_ptr()),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiQueryProductStateA(valid_a.as_ptr().cast::<i8>()),
            INSTALLSTATE_UNKNOWN
        );

        let feat_w: Vec<u16> = "feat".encode_utf16().chain(std::iter::once(0)).collect();
        let feat_a = b"feat\0";

        assert_eq!(
            MsiQueryFeatureStateW(valid_w.as_ptr(), feat_w.as_ptr()),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiQueryFeatureStateA(valid_a.as_ptr().cast::<i8>(), feat_a.as_ptr().cast::<i8>()),
            INSTALLSTATE_UNKNOWN
        );

        let mut pcch = 0;
        assert_eq!(
            MsiGetProductInfoW(
                valid_w.as_ptr(),
                feat_w.as_ptr(),
                buf_w.as_mut_ptr(),
                &raw mut pcch
            ),
            crate::win32::ERROR_INVALID_DATA
        );
        assert_eq!(
            MsiGetProductInfoA(
                valid_a.as_ptr().cast::<i8>(),
                feat_a.as_ptr().cast::<i8>(),
                buf_a.as_mut_ptr(),
                &raw mut pcch
            ),
            crate::win32::ERROR_INVALID_DATA
        );

        assert_eq!(
            MsiEnumFeaturesW(
                std::ptr::null(),
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumFeaturesW(
                valid_w.as_ptr(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumFeaturesA(
                std::ptr::null(),
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumFeaturesA(
                valid_a.as_ptr().cast::<i8>(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiEnumComponentsW(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumComponentsA(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiQueryFeatureStateA(std::ptr::null(), feat_a.as_ptr().cast::<i8>()),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiQueryFeatureStateA(valid_a.as_ptr().cast::<i8>(), std::ptr::null()),
            INSTALLSTATE_UNKNOWN
        );

        assert_eq!(
            MsiGetProductInfoA(
                std::ptr::null(),
                feat_a.as_ptr().cast::<i8>(),
                buf_a.as_mut_ptr(),
                &raw mut pcch
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetProductInfoA(
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null(),
                buf_a.as_mut_ptr(),
                &raw mut pcch
            ),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiEnumClientsW(std::ptr::null(), 0, buf_w.as_mut_ptr()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumClientsA(std::ptr::null(), 0, buf_a.as_mut_ptr()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumClientsExW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumClientsExA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumComponentQualifiersW(
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumComponentQualifiersA(
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesW(
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesA(
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesExW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesExA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumRelatedProductsW(std::ptr::null(), 0, 0, buf_w.as_mut_ptr()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumRelatedProductsA(std::ptr::null(), 0, 0, buf_a.as_mut_ptr()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumProductsExW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumProductsExA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsExW(
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsExA(
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );

        assert_eq!(
            MsiQueryFeatureStateExW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryFeatureStateExA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetComponentStateW(
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetComponentStateA(
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureStateW(
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureStateA(
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetComponentPathExW(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetComponentPathExA(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetComponentPathExW(
                valid_w.as_ptr(),
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetComponentPathExA(
                valid_a.as_ptr().cast(),
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );

        assert_eq!(
            MsiGetComponentPathW(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiGetComponentPathA(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiGetComponentPathW(
                valid_w.as_ptr(),
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            1
        );
        assert_eq!(
            MsiGetComponentPathA(
                valid_a.as_ptr().cast(),
                valid_a.as_ptr().cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            1
        );
        assert_eq!(
            MsiGetFeatureValidStatesW(0, std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureValidStatesA(0, std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureCostW(0, std::ptr::null(), 0, 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureCostA(0, std::ptr::null(), 0, 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumComponentCostsW(
                0,
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumComponentCostsA(
                0,
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        unsafe {
            assert_eq!(
                MsiGetShortcutTargetW(
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                ERROR_INVALID_PARAMETER
            );
            assert_eq!(
                MsiGetShortcutTargetA(
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                ERROR_INVALID_PARAMETER
            );
        }
        assert_eq!(
            MsiDecomposeDescriptorW(
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiDecomposeDescriptorA(
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFileVersionW(
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFileVersionA(
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumClientsW(valid_w.as_ptr(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumClientsA(valid_a.as_ptr().cast(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumClientsExW(
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumClientsExA(
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentCostsW(
                0,
                valid_w.as_ptr(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentCostsA(
                0,
                valid_a.as_ptr().cast(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentQualifiersW(
                valid_w.as_ptr(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentQualifiersA(
                valid_a.as_ptr().cast(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsW(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumComponentsA(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumComponentsExW(
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsExA(
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumFeaturesW(
                valid_w.as_ptr(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumFeaturesA(
                valid_a.as_ptr().cast(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesW(
                valid_w.as_ptr(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesA(
                valid_a.as_ptr().cast(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesExW(
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumPatchesExA(
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumProductsW(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumProductsA(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumProductsExW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumProductsExA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumRelatedProductsW(valid_w.as_ptr(), 0, 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumRelatedProductsA(valid_a.as_ptr().cast(), 0, 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFileVersionW(
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetFileVersionA(
                valid_a.as_ptr().cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );

        buf_w = [0_u16; 39];
        buf_a = [0_i8; 39];
        assert_eq!(
            MsiEnumClientsW(valid_w.as_ptr(), 0, buf_w.as_mut_ptr()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumClientsA(valid_a.as_ptr().cast(), 0, buf_a.as_mut_ptr()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumClientsExW(
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumClientsExA(
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                0,
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentCostsW(
                0,
                valid_w.as_ptr(),
                0,
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentCostsA(
                0,
                valid_a.as_ptr().cast(),
                0,
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentQualifiersW(
                valid_w.as_ptr(),
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentQualifiersA(
                valid_a.as_ptr().cast(),
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsW(0, buf_w.as_mut_ptr()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsA(0, buf_a.as_mut_ptr()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsExW(
                std::ptr::null(),
                0,
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsExA(
                std::ptr::null(),
                0,
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumFeaturesW(
                valid_w.as_ptr(),
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumFeaturesA(
                valid_a.as_ptr().cast(),
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumPatchesW(
                valid_w.as_ptr(),
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumPatchesA(
                valid_a.as_ptr().cast(),
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumPatchesExW(
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumPatchesExA(
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                0,
                0,
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(MsiEnumProductsW(0, buf_w.as_mut_ptr()), ERROR_NO_MORE_ITEMS);
        assert_eq!(MsiEnumProductsA(0, buf_a.as_mut_ptr()), ERROR_NO_MORE_ITEMS);
        assert_eq!(
            MsiEnumProductsExW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumProductsExA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumRelatedProductsW(valid_w.as_ptr(), 0, 0, buf_w.as_mut_ptr()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumRelatedProductsA(valid_a.as_ptr().cast(), 0, 0, buf_a.as_mut_ptr()),
            ERROR_NO_MORE_ITEMS
        );

        assert_eq!(
            MsiQueryFeatureStateExW(
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                valid_w.as_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiQueryFeatureStateExA(
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                0,
                valid_a.as_ptr().cast(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetComponentStateW(
                0,
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetComponentStateA(
                0,
                valid_a.as_ptr().cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetFeatureStateW(
                0,
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetFeatureStateA(
                0,
                valid_a.as_ptr().cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetFeatureValidStatesW(0, valid_w.as_ptr(), std::ptr::null_mut()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetFeatureValidStatesA(0, valid_a.as_ptr().cast(), std::ptr::null_mut()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetFeatureCostW(0, valid_w.as_ptr(), 0, 0, std::ptr::null_mut()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetFeatureCostA(0, valid_a.as_ptr().cast(), 0, 0, std::ptr::null_mut()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiDecomposeDescriptorW(
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiDecomposeDescriptorA(
                valid_a.as_ptr().cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );

        assert_eq!(
            MsiCollectUserInfoW(std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiCollectUserInfoA(std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(MsiCollectUserInfoW(valid_w.as_ptr()), ERROR_SUCCESS);
        assert_eq!(
            MsiCollectUserInfoA(valid_a.as_ptr().cast::<i8>()),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiGetUserInfoW(
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiGetUserInfoA(
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiGetUserInfoW(
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiGetUserInfoA(
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );

        assert_eq!(
            Migrate10CachedPackagesW(std::ptr::null(), std::ptr::null(), std::ptr::null(), 0),
            ERROR_SUCCESS
        );
    }
}

/// (Stub) Legacy cached packages migration.
///
/// # Arguments
///
/// * `_szProductCode` - Pointer to a string specifying the product code.
/// * `_szUserSid` - Pointer to a string specifying the user name.
/// * `_szPreflightCheck` - Reserved.
/// * `_dwReserved` - Reserved.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn Migrate10CachedPackagesA(
    _szProductCode: Lpcstr,
    _szUserSid: Lpcstr,
    _szPreflightCheck: Lpcstr,
    _dwReserved: Dword,
) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

#[cfg(test)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::cognitive_complexity)]
mod additional_stubs_tests {
    use super::*;

    #[test]
    fn test_stubs_impl() {
        assert_eq!(
            Migrate10CachedPackagesA(std::ptr::null(), std::ptr::null(), std::ptr::null(), 0),
            1605
        );
        let h_db = crate::handles::alloc_handle(crate::handles::MsiObject::Database(
            crate::types::MsiDatabaseHandle {
                inner: msi::wix::linker::LinkedDatabase::default(),
                state: 0,
            },
        ));
        let valid_w: Vec<u16> = "Feat1".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"Feat1\0";
        let invalid_utf16 = [0xD800_u16, 0x0000];
        let invalid_utf8 = [0xFF_u8, 0x00];
        let mut inst = 0;
        let mut act = 0;
        assert_eq!(
            MsiGetComponentStateW(h_db, valid_w.as_ptr(), &raw mut inst, &raw mut act),
            ERROR_SUCCESS
        );
        assert_eq!(inst, 2);
        assert_eq!(act, 3);
        assert_eq!(
            MsiGetComponentStateA(h_db, valid_a.as_ptr().cast(), &raw mut inst, &raw mut act),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetFeatureStateW(h_db, valid_w.as_ptr(), &raw mut inst, &raw mut act),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetFeatureStateA(h_db, valid_a.as_ptr().cast(), &raw mut inst, &raw mut act),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetComponentStateW(h_db, invalid_utf16.as_ptr(), &raw mut inst, &raw mut act),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetComponentStateA(
                h_db,
                invalid_utf8.as_ptr().cast(),
                &raw mut inst,
                &raw mut act
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureStateW(h_db, invalid_utf16.as_ptr(), &raw mut inst, &raw mut act),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureStateA(
                h_db,
                invalid_utf8.as_ptr().cast(),
                &raw mut inst,
                &raw mut act
            ),
            ERROR_INVALID_PARAMETER
        );
        let h_rec = crate::handles::alloc_handle(crate::handles::MsiObject::Record(
            crate::types::MsiRecordHandle {
                inner: msi::database::tables::record::Record::new(),
            },
        ));
        assert_eq!(
            MsiGetFeatureStateW(h_rec, valid_w.as_ptr(), &raw mut inst, &raw mut act),
            ERROR_NO_MORE_ITEMS
        );
        let mut valid_states: Dword = 0;
        let mut cost: i32 = -1;
        assert_eq!(
            MsiGetFeatureValidStatesW(h_db, valid_w.as_ptr(), &raw mut valid_states),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetFeatureValidStatesA(h_db, valid_a.as_ptr().cast(), &raw mut valid_states),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetFeatureCostW(h_db, valid_w.as_ptr(), 0, 3, &raw mut cost),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetFeatureCostA(h_db, valid_a.as_ptr().cast(), 0, 3, &raw mut cost),
            ERROR_SUCCESS
        );
        let _ = crate::handles::close_handle(h_db);
        let _ = crate::handles::close_handle(h_rec);

        // Test populated registry enumeration and queries
        let prod_guid = "{77777777-7777-7777-7777-777777777777}";
        let adv_prod_guid = "{77777777-7777-7777-7777-888888888888}";
        let comp_guid = "{88888888-8888-8888-8888-888888888888}";
        let patch_guid = "{99999999-9999-9999-9999-999999999999}";
        let upg_guid = "{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}";

        let prod_squid = encode_squid(prod_guid).unwrap_or_default();
        let adv_prod_squid = encode_squid(adv_prod_guid).unwrap_or_default();
        let comp_squid = encode_squid(comp_guid).unwrap_or_default();
        let patch_squid = encode_squid(patch_guid).unwrap_or_default();
        let upg_squid = encode_squid(upg_guid).unwrap_or_default();

        let hklm_prod_key = format!(
            r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products\{prod_squid}"
        );
        let install_props_key = format!(r"{hklm_prod_key}\InstallProperties");
        let hklm_feat_key = format!(r"{hklm_prod_key}\Features");
        let hklm_patch_key = format!(r"{hklm_prod_key}\Patches\{patch_squid}");
        let hklm_comp_key = format!(
            r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Components\{comp_squid}"
        );
        let hkcr_prod_key = format!(r"Installer\Products\{adv_prod_squid}");
        let hkcr_feat_key = format!(r"Installer\Features\{adv_prod_squid}");
        let hkcr_qual_key = format!(r"Installer\Components\{comp_squid}");
        let hkcr_upg_key = format!(r"Installer\UpgradeCodes\{upg_squid}");

        crate::win32::registry_backend::with_registry(|reg| {
            reg.set_value(
                RegistryRoot::LocalMachine,
                &install_props_key,
                Some("DisplayName"),
                RegistryValue::Sz("TestProduct".to_string()),
            );
            reg.set_value(
                RegistryRoot::LocalMachine,
                &install_props_key,
                Some("RegOwner"),
                RegistryValue::Sz("Jane Doe".to_string()),
            );
            reg.set_value(
                RegistryRoot::LocalMachine,
                &install_props_key,
                Some("RegCompany"),
                RegistryValue::Sz("Acme Corp".to_string()),
            );
            reg.set_value(
                RegistryRoot::LocalMachine,
                &install_props_key,
                Some("ProductID"),
                RegistryValue::Sz("12345-OEM".to_string()),
            );
            reg.set_value(
                RegistryRoot::LocalMachine,
                &hklm_feat_key,
                Some("MainFeature"),
                RegistryValue::Sz(String::new()),
            );
            reg.set_value(
                RegistryRoot::LocalMachine,
                &hklm_patch_key,
                Some("Transforms"),
                RegistryValue::Sz(":PatchTransform".to_string()),
            );
            reg.set_value(
                RegistryRoot::LocalMachine,
                &hklm_comp_key,
                Some(&prod_squid),
                RegistryValue::Sz("C:\\Program Files\\Test\\app.exe".to_string()),
            );
            reg.set_value(
                RegistryRoot::ClassesRoot,
                &hkcr_prod_key,
                Some("ProductName"),
                RegistryValue::Sz("AdvertisedProduct".to_string()),
            );
            reg.set_value(
                RegistryRoot::ClassesRoot,
                &hkcr_feat_key,
                Some("AdvFeature"),
                RegistryValue::Sz("RootFeature".to_string()),
            );
            reg.set_value(
                RegistryRoot::ClassesRoot,
                &hkcr_qual_key,
                Some("1033\\word.dot"),
                RegistryValue::Sz("WordTemplate".to_string()),
            );
            reg.set_value(
                RegistryRoot::ClassesRoot,
                &hkcr_upg_key,
                Some(&prod_squid),
                RegistryValue::Sz(String::new()),
            );
        });

        let mut pbuf_w = [0u16; 64];
        let mut pbuf_a = [0i8; 64];
        let mut sbuf_w = [0u16; 64];
        let mut sbuf_a = [0i8; 64];
        let mut ctx_out: Dword = 0;
        let mut cch_w: Dword;
        let mut cch_a: Dword;

        // MsiEnumProducts / Ex
        assert_eq!(MsiEnumProductsW(0, pbuf_w.as_mut_ptr()), ERROR_SUCCESS);
        assert_eq!(MsiEnumProductsA(0, pbuf_a.as_mut_ptr()), ERROR_SUCCESS);
        cch_w = 64;
        assert_eq!(
            MsiEnumProductsExW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                pbuf_w.as_mut_ptr(),
                &raw mut ctx_out,
                sbuf_w.as_mut_ptr(),
                &raw mut cch_w
            ),
            ERROR_SUCCESS
        );
        cch_a = 64;
        assert_eq!(
            MsiEnumProductsExA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                pbuf_a.as_mut_ptr(),
                &raw mut ctx_out,
                sbuf_a.as_mut_ptr(),
                &raw mut cch_a
            ),
            ERROR_SUCCESS
        );

        // MsiEnumComponents / Ex
        assert_eq!(MsiEnumComponentsW(0, pbuf_w.as_mut_ptr()), ERROR_SUCCESS);
        assert_eq!(MsiEnumComponentsA(0, pbuf_a.as_mut_ptr()), ERROR_SUCCESS);
        cch_w = 64;
        assert_eq!(
            MsiEnumComponentsExW(
                std::ptr::null(),
                0,
                0,
                pbuf_w.as_mut_ptr(),
                &raw mut ctx_out,
                sbuf_w.as_mut_ptr(),
                &raw mut cch_w
            ),
            ERROR_SUCCESS
        );
        cch_a = 64;
        assert_eq!(
            MsiEnumComponentsExA(
                std::ptr::null(),
                0,
                0,
                pbuf_a.as_mut_ptr(),
                &raw mut ctx_out,
                sbuf_a.as_mut_ptr(),
                &raw mut cch_a
            ),
            ERROR_SUCCESS
        );

        let prod_w: Vec<u16> = prod_guid.encode_utf16().chain(std::iter::once(0)).collect();
        let prod_a = std::ffi::CString::new(prod_guid).unwrap_or_default();
        let adv_prod_w: Vec<u16> = adv_prod_guid
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let adv_prod_a = std::ffi::CString::new(adv_prod_guid).unwrap_or_default();
        let comp_w: Vec<u16> = comp_guid.encode_utf16().chain(std::iter::once(0)).collect();
        let comp_a = std::ffi::CString::new(comp_guid).unwrap_or_default();
        let upg_w: Vec<u16> = upg_guid.encode_utf16().chain(std::iter::once(0)).collect();
        let upg_a = std::ffi::CString::new(upg_guid).unwrap_or_default();

        // MsiEnumFeatures
        assert_eq!(
            MsiEnumFeaturesW(prod_w.as_ptr(), 0, pbuf_w.as_mut_ptr(), sbuf_w.as_mut_ptr()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiEnumFeaturesA(
                adv_prod_a.as_ptr(),
                0,
                pbuf_a.as_mut_ptr(),
                sbuf_a.as_mut_ptr()
            ),
            ERROR_SUCCESS
        );

        // MsiQueryProductState & MsiQueryFeatureState / Ex
        assert_eq!(MsiQueryProductStateW(prod_w.as_ptr()), 5);
        assert_eq!(MsiQueryProductStateA(prod_a.as_ptr()), 5);
        assert_eq!(MsiQueryProductStateW(adv_prod_w.as_ptr()), 1);
        assert_eq!(MsiQueryProductStateA(adv_prod_a.as_ptr()), 1);

        let main_feat_w: Vec<u16> = "MainFeature\0".encode_utf16().collect();
        let main_feat_a = std::ffi::CString::new("MainFeature").unwrap_or_default();
        let adv_feat_w: Vec<u16> = "AdvFeature\0".encode_utf16().collect();
        assert_eq!(
            MsiQueryFeatureStateW(prod_w.as_ptr(), main_feat_w.as_ptr()),
            3
        );
        assert_eq!(
            MsiQueryFeatureStateA(prod_a.as_ptr(), main_feat_a.as_ptr()),
            3
        );
        assert_eq!(
            MsiQueryFeatureStateW(adv_prod_w.as_ptr(), adv_feat_w.as_ptr()),
            1
        );
        let mut fstate_ex: Dword = 0;
        assert_eq!(
            MsiQueryFeatureStateExW(
                prod_w.as_ptr(),
                std::ptr::null(),
                4,
                main_feat_w.as_ptr(),
                &raw mut fstate_ex
            ),
            ERROR_SUCCESS
        );
        assert_eq!(fstate_ex, 3);
        assert_eq!(
            MsiQueryFeatureStateExA(
                prod_a.as_ptr(),
                std::ptr::null(),
                4,
                main_feat_a.as_ptr(),
                &raw mut fstate_ex
            ),
            ERROR_SUCCESS
        );

        // MsiGetProductInfo
        let dname_w: Vec<u16> = "DisplayName\0".encode_utf16().collect();
        let pname_a = std::ffi::CString::new("ProductName").unwrap_or_default();
        cch_w = 64;
        assert_eq!(
            MsiGetProductInfoW(
                prod_w.as_ptr(),
                dname_w.as_ptr(),
                pbuf_w.as_mut_ptr(),
                &raw mut cch_w
            ),
            ERROR_SUCCESS
        );
        cch_a = 64;
        assert_eq!(
            MsiGetProductInfoA(
                adv_prod_a.as_ptr(),
                pname_a.as_ptr(),
                pbuf_a.as_mut_ptr(),
                &raw mut cch_a
            ),
            ERROR_SUCCESS
        );

        // MsiEnumClients / Ex
        assert_eq!(
            MsiEnumClientsW(comp_w.as_ptr(), 0, pbuf_w.as_mut_ptr()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiEnumClientsA(comp_a.as_ptr(), 0, pbuf_a.as_mut_ptr()),
            ERROR_SUCCESS
        );
        cch_w = 64;
        assert_eq!(
            MsiEnumClientsExW(
                comp_w.as_ptr(),
                std::ptr::null(),
                0,
                0,
                pbuf_w.as_mut_ptr(),
                &raw mut ctx_out,
                sbuf_w.as_mut_ptr(),
                &raw mut cch_w
            ),
            ERROR_SUCCESS
        );
        cch_a = 64;
        assert_eq!(
            MsiEnumClientsExA(
                comp_a.as_ptr(),
                std::ptr::null(),
                0,
                0,
                pbuf_a.as_mut_ptr(),
                &raw mut ctx_out,
                sbuf_a.as_mut_ptr(),
                &raw mut cch_a
            ),
            ERROR_SUCCESS
        );

        // MsiEnumComponentQualifiers
        cch_w = 64;
        let mut app_len_w: Dword = 64;
        assert_eq!(
            MsiEnumComponentQualifiersW(
                comp_w.as_ptr(),
                0,
                pbuf_w.as_mut_ptr(),
                &raw mut cch_w,
                sbuf_w.as_mut_ptr(),
                &raw mut app_len_w
            ),
            ERROR_SUCCESS
        );
        cch_a = 64;
        let mut app_len_a: Dword = 64;
        assert_eq!(
            MsiEnumComponentQualifiersA(
                comp_a.as_ptr(),
                0,
                pbuf_a.as_mut_ptr(),
                &raw mut cch_a,
                sbuf_a.as_mut_ptr(),
                &raw mut app_len_a
            ),
            ERROR_SUCCESS
        );

        // MsiEnumPatches / Ex
        cch_w = 64;
        assert_eq!(
            MsiEnumPatchesW(
                prod_w.as_ptr(),
                0,
                pbuf_w.as_mut_ptr(),
                sbuf_w.as_mut_ptr(),
                &raw mut cch_w
            ),
            ERROR_SUCCESS
        );
        cch_a = 64;
        assert_eq!(
            MsiEnumPatchesA(
                prod_a.as_ptr(),
                0,
                pbuf_a.as_mut_ptr(),
                sbuf_a.as_mut_ptr(),
                &raw mut cch_a
            ),
            ERROR_SUCCESS
        );
        cch_w = 64;
        let mut tprod_w = [0u16; 64];
        assert_eq!(resolve_user_sid(None), "S-1-5-18");
        assert_eq!(resolve_user_sid(Some("")), "S-1-5-18");
        assert_eq!(resolve_user_sid(Some("S-1-5-21")), "S-1-5-21");
        assert_eq!(subkey_to_guid("raw_not_squid"), "raw_not_squid");
        assert_eq!(
            MsiEnumPatchesExW(
                prod_w.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                pbuf_w.as_mut_ptr(),
                tprod_w.as_mut_ptr(),
                &raw mut ctx_out,
                sbuf_w.as_mut_ptr(),
                &raw mut cch_w
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiEnumPatchesExW(
                prod_w.as_ptr(),
                std::ptr::null(),
                4,
                0,
                0,
                pbuf_w.as_mut_ptr(),
                tprod_w.as_mut_ptr(),
                std::ptr::null_mut(),
                sbuf_w.as_mut_ptr(),
                &raw mut cch_w
            ),
            ERROR_SUCCESS
        );
        cch_a = 64;
        let mut tprod_a = [0i8; 64];
        assert_eq!(
            MsiEnumPatchesExA(
                prod_a.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                pbuf_a.as_mut_ptr(),
                tprod_a.as_mut_ptr(),
                &raw mut ctx_out,
                sbuf_a.as_mut_ptr(),
                &raw mut cch_a
            ),
            ERROR_SUCCESS
        );

        // MsiEnumRelatedProducts
        assert_eq!(
            MsiEnumRelatedProductsW(upg_w.as_ptr(), 0, 0, pbuf_w.as_mut_ptr()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiEnumRelatedProductsA(upg_a.as_ptr(), 0, 0, pbuf_a.as_mut_ptr()),
            ERROR_SUCCESS
        );

        // MsiGetComponentPath / Ex
        cch_w = 64;
        assert_eq!(
            MsiGetComponentPathW(
                prod_w.as_ptr(),
                comp_w.as_ptr(),
                pbuf_w.as_mut_ptr(),
                &raw mut cch_w
            ),
            3
        );
        cch_a = 64;
        assert_eq!(
            MsiGetComponentPathA(
                prod_a.as_ptr(),
                comp_a.as_ptr(),
                pbuf_a.as_mut_ptr(),
                &raw mut cch_a
            ),
            3
        );
        cch_w = 64;
        assert_eq!(
            MsiGetComponentPathExW(
                prod_w.as_ptr(),
                comp_w.as_ptr(),
                std::ptr::null(),
                4,
                pbuf_w.as_mut_ptr(),
                &raw mut cch_w
            ),
            3
        );
        cch_a = 64;
        assert_eq!(
            MsiGetComponentPathExA(
                prod_a.as_ptr(),
                comp_a.as_ptr(),
                std::ptr::null(),
                4,
                pbuf_a.as_mut_ptr(),
                &raw mut cch_a
            ),
            3
        );

        // MsiGetUserInfo
        cch_w = 64;
        app_len_w = 64;
        let mut serial_len_w: Dword = 64;
        assert_eq!(
            MsiGetUserInfoW(
                prod_w.as_ptr(),
                pbuf_w.as_mut_ptr(),
                &raw mut cch_w,
                sbuf_w.as_mut_ptr(),
                &raw mut app_len_w,
                tprod_w.as_mut_ptr(),
                &raw mut serial_len_w
            ),
            0
        );
        cch_a = 64;
        app_len_a = 64;
        let mut serial_len_a: Dword = 64;
        assert_eq!(
            MsiGetUserInfoA(
                prod_a.as_ptr(),
                pbuf_a.as_mut_ptr(),
                &raw mut cch_a,
                sbuf_a.as_mut_ptr(),
                &raw mut app_len_a,
                tprod_a.as_mut_ptr(),
                &raw mut serial_len_a
            ),
            0
        );

        crate::win32::registry_backend::with_registry(|reg| {
            reg.delete_subtree(RegistryRoot::LocalMachine, &hklm_prod_key);
            reg.delete_subtree(RegistryRoot::LocalMachine, &hklm_comp_key);
            reg.delete_subtree(RegistryRoot::ClassesRoot, &hkcr_prod_key);
            reg.delete_subtree(RegistryRoot::ClassesRoot, &hkcr_feat_key);
            reg.delete_subtree(RegistryRoot::ClassesRoot, &hkcr_qual_key);
            reg.delete_subtree(RegistryRoot::ClassesRoot, &hkcr_upg_key);
        });

        // Test MsiGetFileVersionA/W on a synthetic PE32+ executable with VS_FIXEDFILEINFO
        let mut pe64 = vec![0u8; 1024];
        pe64[0] = b'M';
        pe64[1] = b'Z';
        pe64[0x3C..0x40].copy_from_slice(&64u32.to_le_bytes());
        pe64[64..68].copy_from_slice(b"PE\0\0");
        pe64[64 + 6..64 + 8].copy_from_slice(&1u16.to_le_bytes());
        pe64[64 + 20..64 + 22].copy_from_slice(&240u16.to_le_bytes());
        let opt64 = 64 + 24;
        pe64[opt64..opt64 + 2].copy_from_slice(&0x20Bu16.to_le_bytes());
        let rsrc_dir_entry64 = opt64 + 112 + 16;
        pe64[rsrc_dir_entry64..rsrc_dir_entry64 + 4].copy_from_slice(&0x1000u32.to_le_bytes());
        pe64[rsrc_dir_entry64 + 4..rsrc_dir_entry64 + 8].copy_from_slice(&512u32.to_le_bytes());
        let sec_hdr64 = opt64 + 240;
        pe64[sec_hdr64 + 8..sec_hdr64 + 12].copy_from_slice(&512u32.to_le_bytes());
        pe64[sec_hdr64 + 12..sec_hdr64 + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        pe64[sec_hdr64 + 20..sec_hdr64 + 24].copy_from_slice(&512u32.to_le_bytes());
        let root = 512;
        pe64[root + 14..root + 16].copy_from_slice(&1u16.to_le_bytes());
        pe64[root + 16..root + 20].copy_from_slice(&16u32.to_le_bytes());
        pe64[root + 20..root + 24].copy_from_slice(&(0x8000_0020u32).to_le_bytes());
        pe64[544 + 14..544 + 16].copy_from_slice(&1u16.to_le_bytes());
        pe64[544 + 16..544 + 20].copy_from_slice(&1u32.to_le_bytes());
        pe64[544 + 20..544 + 24].copy_from_slice(&(0x8000_0030u32).to_le_bytes());
        pe64[560 + 14..560 + 16].copy_from_slice(&1u16.to_le_bytes());
        pe64[560 + 16..560 + 20].copy_from_slice(&0x1000u32.to_le_bytes());
        pe64[576..580].copy_from_slice(&0xFEEF_04BDu32.to_le_bytes());
        let ms64 = (3u32 << 16) | 1;
        let ls64 = (4u32 << 16) | 2;
        pe64[584..588].copy_from_slice(&ms64.to_le_bytes());
        pe64[588..592].copy_from_slice(&ls64.to_le_bytes());

        let pe_tmp = std::env::temp_dir().join("test_ffi_file_version_pe64.exe");
        assert!(std::fs::write(&pe_tmp, &pe64).is_ok());
        let pe_str = pe_tmp.to_string_lossy().to_string();
        let pe_w: Vec<u16> = pe_str.encode_utf16().chain(std::iter::once(0)).collect();
        let pe_a = std::ffi::CString::new(pe_str).unwrap_or_default();

        cch_w = 64;
        app_len_w = 64;
        assert_eq!(
            MsiGetFileVersionW(
                pe_w.as_ptr(),
                pbuf_w.as_mut_ptr(),
                &raw mut cch_w,
                sbuf_w.as_mut_ptr(),
                &raw mut app_len_w
            ),
            ERROR_SUCCESS
        );
        assert_eq!(cch_w, 7); // "3.1.4.2"
        assert_eq!(app_len_w, 4); // "1033"

        cch_a = 64;
        app_len_a = 64;
        assert_eq!(
            MsiGetFileVersionA(
                pe_a.as_ptr(),
                pbuf_a.as_mut_ptr(),
                &raw mut cch_a,
                sbuf_a.as_mut_ptr(),
                &raw mut app_len_a
            ),
            ERROR_SUCCESS
        );
        assert_eq!(cch_a, 7);
        assert_eq!(app_len_a, 4);
        let _ = std::fs::remove_file(&pe_tmp);
    }
}
