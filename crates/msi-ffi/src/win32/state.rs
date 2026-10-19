//! Feature and component state endpoints.

use crate::win32::strings::{lpcstr_to_string, lpcwstr_to_string};
use crate::win32::{
    Bool, Dword, Lpcstr, Lpcwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_PARAMETER,
    ERROR_SUCCESS, FALSE,
};
use std::panic;

/// Determines whether the product is installed with elevated privileges.
///
/// # Arguments
///
/// * `szProduct` - Product code.
/// * `pfElevated` - Pointer to a boolean that receives the result.
///
/// # Returns
///
/// `ERROR_SUCCESS` or `ERROR_INVALID_PARAMETER`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiIsProductElevatedW(szProduct: Lpcwstr, pfElevated: *mut Bool) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || pfElevated.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_product) = lpcwstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };

        // TODO: Real elevation check (e.g., euid == 0 on POSIX or Token check on Windows).
        // For now, mock as false.
        unsafe {
            *pfElevated = FALSE;
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Determines whether the product is installed with elevated privileges (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiIsProductElevatedA(szProduct: Lpcstr, pfElevated: *mut Bool) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || pfElevated.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_product) = lpcstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };

        unsafe {
            *pfElevated = FALSE;
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieves the internal engine mode flags (`MSIRUNMODE_*`) for an active installation session.
///
/// # Arguments
///
/// * `hInstall` - Handle to the installation session.
/// * `eRunMode` - Run mode index to query (`0..=18`, e.g., `MSIRUNMODE_REBOOTATEND`).
///
/// # Returns
///
/// `TRUE` (`1`) if the mode is set on `hInstall`, or `FALSE` (`0`) otherwise.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiGetMode(hInstall: crate::handles::MsiHandle, eRunMode: Uint) -> Bool {
    let result = panic::catch_unwind(|| {
        if hInstall == 0 || eRunMode > 31 {
            return FALSE;
        }
        let key = format!("_MSIRUNMODE_{eRunMode}");
        crate::handles::with_handle(hInstall, |obj| match obj {
            crate::handles::MsiObject::Database(db) => {
                if db.inner.get_property_value(&key) == Some("1") {
                    crate::win32::TRUE
                } else {
                    FALSE
                }
            }
            _ => FALSE,
        })
        .unwrap_or(FALSE)
    });

    result.unwrap_or(FALSE)
}

/// Sets an internal engine mode flag (`MSIRUNMODE_*`) for an active installation session.
///
/// # Arguments
///
/// * `hInstall` - Handle to the installation session.
/// * `eRunMode` - Run mode index to update (`0..=31`).
/// * `fState` - Non-zero (`TRUE`) to enable the mode, or `0` (`FALSE`) to disable it.
///
/// # Returns
///
/// `ERROR_SUCCESS` on success, or `ERROR_INVALID_PARAMETER` if `eRunMode > 31`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiSetMode(
    hInstall: crate::handles::MsiHandle,
    eRunMode: Uint,
    fState: Bool,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if eRunMode > 31 {
            return ERROR_INVALID_PARAMETER;
        }
        if hInstall != 0 {
            let key = format!("_MSIRUNMODE_{eRunMode}");
            let val = if fState != 0 { "1" } else { "" };
            crate::handles::with_handle_mut(hInstall, |obj| {
                if let crate::handles::MsiObject::Database(db) = obj {
                    db.inner.set_property_value(&key, val);
                }
            });
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets the installation level (`INSTALLLEVEL`) for a product installation session.
///
/// # Arguments
///
/// * `hInstall` - Handle to the installation session.
/// * `iInstallLevel` - Requested installation level (`>= 0`).
///
/// # Returns
///
/// `ERROR_SUCCESS` or `ERROR_INVALID_PARAMETER` if `iInstallLevel < 0`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiSetInstallLevel(
    hInstall: crate::handles::MsiHandle,
    iInstallLevel: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if iInstallLevel < 0 {
            return ERROR_INVALID_PARAMETER;
        }
        if hInstall != 0 {
            let val = if iInstallLevel == 0 {
                "1".to_string()
            } else {
                iInstallLevel.to_string()
            };
            crate::handles::with_handle_mut(hInstall, |obj| {
                if let crate::handles::MsiObject::Database(db) = obj {
                    db.inner.set_property_value("INSTALLLEVEL", &val);
                }
            });
        }
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Configures the installed state for a product feature (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiConfigureFeatureW(
    szProduct: Lpcwstr,
    szFeature: Lpcwstr,
    eInstallState: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(_product) = lpcwstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };
        let Some(_feature) = lpcwstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Configures the installed state for a product feature (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiConfigureFeatureA(
    szProduct: Lpcstr,
    szFeature: Lpcstr,
    eInstallState: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(_product) = lpcstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };
        let Some(_feature) = lpcstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Reinstalls a feature (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiReinstallFeatureW(
    szProduct: Lpcwstr,
    szFeature: Lpcwstr,
    dwReinstallMode: Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(_product) = lpcwstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };
        let Some(_feature) = lpcwstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Reinstalls a feature (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiReinstallFeatureA(
    szProduct: Lpcstr,
    szFeature: Lpcstr,
    dwReinstallMode: Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(_product) = lpcstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };
        let Some(_feature) = lpcstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Modifies the runtime attributes of a feature (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiSetFeatureAttributesW(
    hInstall: crate::handles::MsiHandle,
    szFeature: Lpcwstr,
    dwAttributes: Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(feature) = lpcwstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        if hInstall != 0 {
            let key = format!("_MSI_FEAT_ATTR_{feature}");
            crate::handles::with_handle_mut(hInstall, |obj| {
                if let crate::handles::MsiObject::Database(db) = obj {
                    db.inner.set_property_value(&key, &dwAttributes.to_string());
                }
            });
        }
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Modifies the runtime attributes of a feature (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiSetFeatureAttributesA(
    hInstall: crate::handles::MsiHandle,
    szFeature: Lpcstr,
    dwAttributes: Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(feature) = lpcstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        if hInstall != 0 {
            let key = format!("_MSI_FEAT_ATTR_{feature}");
            crate::handles::with_handle_mut(hInstall, |obj| {
                if let crate::handles::MsiObject::Database(db) = obj {
                    db.inner.set_property_value(&key, &dwAttributes.to_string());
                }
            });
        }
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets the installed state for a product feature (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiSetFeatureStateW(
    hInstall: crate::handles::MsiHandle,
    szFeature: Lpcwstr,
    iState: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(feature) = lpcwstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        if hInstall != 0 {
            let key = format!("_MSI_FEAT_ACTION_{feature}");
            crate::handles::with_handle_mut(hInstall, |obj| {
                if let crate::handles::MsiObject::Database(db) = obj {
                    db.inner.set_property_value(&key, &iState.to_string());
                }
            });
        }
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets the installed state for a product feature (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiSetFeatureStateA(
    hInstall: crate::handles::MsiHandle,
    szFeature: Lpcstr,
    iState: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(feature) = lpcstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        if hInstall != 0 {
            let key = format!("_MSI_FEAT_ACTION_{feature}");
            crate::handles::with_handle_mut(hInstall, |obj| {
                if let crate::handles::MsiObject::Database(db) = obj {
                    db.inner.set_property_value(&key, &iState.to_string());
                }
            });
        }
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Shared helper querying component install state from the global registry backend.
fn query_component_state_in_registry(
    product_code: &str,
    user_sid: Option<&str>,
    component_code: &str,
) -> i32 {
    let sid = user_sid.filter(|s| !s.is_empty()).unwrap_or("S-1-5-18");
    let prod_id = msi::platform::squid::encode_squid(product_code)
        .unwrap_or_else(|_| product_code.to_string());
    let comp_id = msi::platform::squid::encode_squid(component_code)
        .unwrap_or_else(|_| component_code.to_string());
    let key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\{sid}\Components\{comp_id}"
    );
    crate::win32::registry_backend::with_registry(|reg| {
        match reg.get_value(
            msi::platform::registry_store::RegistryRoot::LocalMachine,
            &key,
            Some(&prod_id),
        ) {
            Some(msi::platform::registry_store::RegistryValue::Sz(path)) => {
                if path.is_empty() {
                    2 // INSTALLSTATE_ABSENT
                } else {
                    3 // INSTALLSTATE_LOCAL
                }
            }
            _ => 1, // Default fallback
        }
    })
}

/// Queries the installed state of a component.
///
/// # Arguments
///
/// * `szProductCode` - A null-terminated string that specifies the `ProductCode`.
/// * `szUserSid` - A null-terminated string that specifies the user SID.
/// * `dwContext` - A flag that specifies the context.
/// * `szComponentCode` - A null-terminated string that specifies the component code.
/// * `pdwState` - Pointer to a variable that receives the state.
///
/// # Returns
///
/// `ERROR_SUCCESS` or `ERROR_INVALID_PARAMETER`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiQueryComponentStateW(
    szProductCode: Lpcwstr,
    szUserSid: Lpcwstr,
    dwContext: Dword,
    szComponentCode: Lpcwstr,
    pdwState: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProductCode.is_null() || szComponentCode.is_null() || pdwState.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(product_code) = lpcwstr_to_string(szProductCode) else {
            return ERROR_INVALID_PARAMETER;
        };

        let Some(component_code) = lpcwstr_to_string(szComponentCode) else {
            return ERROR_INVALID_PARAMETER;
        };

        let sid = lpcwstr_to_string(szUserSid);
        let state =
            query_component_state_in_registry(&product_code, sid.as_deref(), &component_code);
        // SAFETY: `pdwState` was verified non-null.
        unsafe {
            *pdwState = state;
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Queries the installed state of a component (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiQueryComponentStateA(
    szProductCode: Lpcstr,
    szUserSid: Lpcstr,
    dwContext: Dword,
    szComponentCode: Lpcstr,
    pdwState: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProductCode.is_null() || szComponentCode.is_null() || pdwState.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(product_code) = lpcstr_to_string(szProductCode) else {
            return ERROR_INVALID_PARAMETER;
        };

        let Some(component_code) = lpcstr_to_string(szComponentCode) else {
            return ERROR_INVALID_PARAMETER;
        };

        let sid = lpcstr_to_string(szUserSid);
        let state =
            query_component_state_in_registry(&product_code, sid.as_deref(), &component_code);
        // SAFETY: `pdwState` was verified non-null.
        unsafe {
            *pdwState = state;
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets the installed state of a component.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiSetComponentStateW(
    hInstall: crate::handles::MsiHandle,
    szComponent: Lpcwstr,
    iState: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(component) = lpcwstr_to_string(szComponent) else {
            return ERROR_INVALID_PARAMETER;
        };
        if hInstall != 0 {
            let key = format!("_MSI_COMP_ACTION_{component}");
            crate::handles::with_handle_mut(hInstall, |obj| {
                if let crate::handles::MsiObject::Database(db) = obj {
                    db.inner.set_property_value(&key, &iState.to_string());
                }
            });
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets the installed state of a component (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiSetComponentStateA(
    hInstall: crate::handles::MsiHandle,
    szComponent: Lpcstr,
    iState: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(component) = lpcstr_to_string(szComponent) else {
            return ERROR_INVALID_PARAMETER;
        };
        if hInstall != 0 {
            let key = format!("_MSI_COMP_ACTION_{component}");
            crate::handles::with_handle_mut(hInstall, |obj| {
                if let crate::handles::MsiObject::Database(db) = obj {
                    db.inner.set_property_value(&key, &iState.to_string());
                }
            });
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Shared helper querying feature usage metrics from the registry.
#[allow(clippy::cast_possible_truncation)]
fn query_feature_usage_in_registry(product: &str, feature: &str) -> (Dword, u16) {
    let prod_id =
        msi::platform::squid::encode_squid(product).unwrap_or_else(|_| product.to_string());
    let key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products\{prod_id}\Usage"
    );
    crate::win32::registry_backend::with_registry(|reg| {
        match reg.get_value(
            msi::platform::registry_store::RegistryRoot::LocalMachine,
            &key,
            Some(feature),
        ) {
            Some(msi::platform::registry_store::RegistryValue::Dword(val)) => {
                (val & 0xFFFF, (val >> 16) as u16)
            }
            _ => (0, 0),
        }
    })
}

/// Shared helper incrementing feature usage count in the registry.
fn increment_feature_usage_in_registry(product: &str, feature: &str) {
    let prod_id =
        msi::platform::squid::encode_squid(product).unwrap_or_else(|_| product.to_string());
    let key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products\{prod_id}\Usage"
    );
    crate::win32::registry_backend::with_registry(|reg| {
        let cur = match reg.get_value(
            msi::platform::registry_store::RegistryRoot::LocalMachine,
            &key,
            Some(feature),
        ) {
            Some(msi::platform::registry_store::RegistryValue::Dword(v)) => v,
            _ => 0,
        };
        let count = (cur & 0xFFFF).saturating_add(1);
        let packed = (cur & 0xFFFF_0000) | count;
        reg.set_value(
            msi::platform::registry_store::RegistryRoot::LocalMachine,
            &key,
            Some(feature),
            msi::platform::registry_store::RegistryValue::Dword(packed),
        );
    });
}

/// Retrieves the usage metrics for a product feature.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFeatureUsageW(
    szProduct: Lpcwstr,
    szFeature: Lpcwstr,
    pdwUseCount: *mut Dword,
    pwDateUsed: *mut u16,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(product) = lpcwstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };

        let Some(feature) = lpcwstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };

        let (count, date) = query_feature_usage_in_registry(&product, &feature);

        if !pdwUseCount.is_null() {
            unsafe {
                *pdwUseCount = count;
            }
        }

        if !pwDateUsed.is_null() {
            unsafe {
                *pwDateUsed = date;
            }
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieves the usage metrics for a product feature (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFeatureUsageA(
    szProduct: Lpcstr,
    szFeature: Lpcstr,
    pdwUseCount: *mut Dword,
    pwDateUsed: *mut u16,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(product) = lpcstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };

        let Some(feature) = lpcstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };

        let (count, date) = query_feature_usage_in_registry(&product, &feature);

        if !pdwUseCount.is_null() {
            unsafe {
                *pdwUseCount = count;
            }
        }

        if !pwDateUsed.is_null() {
            unsafe {
                *pwDateUsed = date;
            }
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Increments the usage count for a particular feature and returns the installation state.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiUseFeatureExW(
    szProduct: Lpcwstr,
    szFeature: Lpcwstr,
    dwInstallMode: Dword,
    dwReserved: Dword,
) -> i32 {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return -2; // INSTALLSTATE_INVALIDARG
        }

        let Some(product) = lpcwstr_to_string(szProduct) else {
            return -2;
        };

        let Some(feature) = lpcwstr_to_string(szFeature) else {
            return -2;
        };

        increment_feature_usage_in_registry(&product, &feature);
        1 // INSTALLSTATE_LOCAL
    });

    result.unwrap_or(-1) // INSTALLSTATE_UNKNOWN
}

/// Increments the usage count for a particular feature and returns the installation state (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiUseFeatureExA(
    szProduct: Lpcstr,
    szFeature: Lpcstr,
    dwInstallMode: Dword,
    dwReserved: Dword,
) -> i32 {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return -2; // INSTALLSTATE_INVALIDARG
        }

        let Some(product) = lpcstr_to_string(szProduct) else {
            return -2;
        };

        let Some(feature) = lpcstr_to_string(szFeature) else {
            return -2;
        };

        increment_feature_usage_in_registry(&product, &feature);
        1 // INSTALLSTATE_LOCAL
    });

    result.unwrap_or(-1) // INSTALLSTATE_UNKNOWN
}

/// Installs files that are unexpectedly missing.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiInstallMissingComponentW(
    szProduct: Lpcwstr,
    szComponent: Lpcwstr,
    eInstallState: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_product) = lpcwstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };

        let Some(_component) = lpcwstr_to_string(szComponent) else {
            return ERROR_INVALID_PARAMETER;
        };

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Installs files that are unexpectedly missing (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiInstallMissingComponentA(
    szProduct: Lpcstr,
    szComponent: Lpcstr,
    eInstallState: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_product) = lpcstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };

        let Some(_component) = lpcstr_to_string(szComponent) else {
            return ERROR_INVALID_PARAMETER;
        };

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::cognitive_complexity)]
mod tests {

    #[test]
    fn test_feature_usage_valid() {
        let dummy_w: Vec<u16> = "dummy".encode_utf16().chain(std::iter::once(0)).collect();
        let dummy_a = std::ffi::CString::new("dummy").unwrap_or_default();

        let mut count: Dword = 123;
        let mut date: u16 = 456;

        assert_eq!(
            MsiGetFeatureUsageW(
                dummy_w.as_ptr(),
                dummy_w.as_ptr(),
                &raw mut count,
                &raw mut date
            ),
            ERROR_SUCCESS
        );
        assert_eq!(count, 0);
        assert_eq!(date, 0);

        count = 123;
        date = 456;
        assert_eq!(
            MsiGetFeatureUsageA(
                dummy_a.as_ptr(),
                dummy_a.as_ptr(),
                &raw mut count,
                &raw mut date
            ),
            ERROR_SUCCESS
        );
        assert_eq!(count, 0);
        assert_eq!(date, 0);

        assert_eq!(
            MsiGetFeatureUsageW(
                dummy_w.as_ptr(),
                dummy_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetFeatureUsageW(
                dummy_w.as_ptr(),
                dummy_w.as_ptr(),
                &raw mut count,
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetFeatureUsageW(
                dummy_w.as_ptr(),
                dummy_w.as_ptr(),
                std::ptr::null_mut(),
                &raw mut date
            ),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiGetFeatureUsageA(
                dummy_a.as_ptr(),
                dummy_a.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetFeatureUsageA(
                dummy_a.as_ptr(),
                dummy_a.as_ptr(),
                &raw mut count,
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetFeatureUsageA(
                dummy_a.as_ptr(),
                dummy_a.as_ptr(),
                std::ptr::null_mut(),
                &raw mut date
            ),
            ERROR_SUCCESS
        );

        // Test registry-backed MsiUseFeatureEx and MsiGetFeatureUsage roundtrip
        let prod_u_w: Vec<u16> = "use_prod_1\0".encode_utf16().collect();
        let feat_u_w: Vec<u16> = "use_feat_1\0".encode_utf16().collect();
        assert_eq!(
            MsiUseFeatureExW(prod_u_w.as_ptr(), feat_u_w.as_ptr(), 0, 0),
            1
        );
        assert_eq!(
            MsiUseFeatureExW(prod_u_w.as_ptr(), feat_u_w.as_ptr(), 0, 0),
            1
        );
        assert_eq!(
            MsiGetFeatureUsageW(
                prod_u_w.as_ptr(),
                feat_u_w.as_ptr(),
                &raw mut count,
                &raw mut date
            ),
            ERROR_SUCCESS
        );
        assert_eq!(count, 2);

        // Test registry-backed MsiQueryComponentState with non-empty and empty keypath
        let comp_guid = "{33333333-3333-3333-3333-333333333333}";
        let prod_guid = "{44444444-4444-4444-4444-444444444444}";
        let comp_squid = msi::platform::squid::encode_squid(comp_guid).unwrap_or_default();
        let prod_squid = msi::platform::squid::encode_squid(prod_guid).unwrap_or_default();
        let comp_key = format!(
            r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Components\{comp_squid}"
        );
        crate::win32::registry_backend::with_registry(|reg| {
            reg.set_value(
                msi::platform::registry_store::RegistryRoot::LocalMachine,
                &comp_key,
                Some(&prod_squid),
                msi::platform::registry_store::RegistryValue::Sz("C:\\App\\bin.exe".to_string()),
            );
        });
        let comp_a = std::ffi::CString::new(comp_guid).unwrap_or_default();
        let prod_a = std::ffi::CString::new(prod_guid).unwrap_or_default();
        let mut cstate = 0;
        assert_eq!(
            MsiQueryComponentStateA(
                prod_a.as_ptr(),
                std::ptr::null(),
                4,
                comp_a.as_ptr(),
                &raw mut cstate
            ),
            ERROR_SUCCESS
        );
        assert_eq!(cstate, 3);

        crate::win32::registry_backend::with_registry(|reg| {
            reg.set_value(
                msi::platform::registry_store::RegistryRoot::LocalMachine,
                &comp_key,
                Some(&prod_squid),
                msi::platform::registry_store::RegistryValue::Sz(String::new()),
            );
        });
        assert_eq!(
            MsiQueryComponentStateA(
                prod_a.as_ptr(),
                std::ptr::null(),
                4,
                comp_a.as_ptr(),
                &raw mut cstate
            ),
            ERROR_SUCCESS
        );
        assert_eq!(cstate, 2);

        crate::win32::registry_backend::with_registry(|reg| {
            reg.delete_subtree(
                msi::platform::registry_store::RegistryRoot::LocalMachine,
                &comp_key,
            );
            reg.delete_subtree(
                msi::platform::registry_store::RegistryRoot::LocalMachine,
                r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products\use_prod_1",
            );
        });
    }

    use super::*;
    use crate::win32::TRUE;

    #[test]
    fn test_component_state() {
        let mut state = 0;
        let valid_w: Vec<u16> = "valid".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"valid\0";
        let _ = valid_a;

        assert_eq!(
            MsiQueryComponentStateW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                &raw mut state
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryComponentStateW(
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                &raw mut state
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryComponentStateW(
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                valid_w.as_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryComponentStateA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                &raw mut state
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryComponentStateA(
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                &raw mut state
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryComponentStateA(
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                0,
                valid_a.as_ptr().cast(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiQueryComponentStateW(
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                valid_w.as_ptr(),
                &raw mut state
            ),
            ERROR_SUCCESS
        );
        assert_eq!(state, 1);

        assert_eq!(
            MsiSetComponentStateW(0, std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetComponentStateA(0, std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(MsiSetComponentStateW(0, valid_w.as_ptr(), 1), ERROR_SUCCESS);
        assert_eq!(
            MsiSetComponentStateA(0, valid_a.as_ptr().cast::<i8>(), 1),
            ERROR_SUCCESS
        );
    }

    #[test]
    fn test_feature_usage() {
        let mut count = 0;
        let mut date = 0;

        assert_eq!(
            MsiGetFeatureUsageW(
                std::ptr::null(),
                std::ptr::null(),
                &raw mut count,
                &raw mut date
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureUsageA(
                std::ptr::null(),
                std::ptr::null(),
                &raw mut count,
                &raw mut date
            ),
            ERROR_INVALID_PARAMETER
        );

        let valid_w: Vec<u16> = "valid".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"valid\0";
        let _ = valid_a;

        assert_eq!(
            MsiGetFeatureUsageW(
                valid_w.as_ptr(),
                valid_w.as_ptr(),
                &raw mut count,
                &raw mut date
            ),
            ERROR_SUCCESS
        );
        assert_eq!(count, 0);

        assert_eq!(
            MsiUseFeatureExW(std::ptr::null(), std::ptr::null(), 0, 0),
            -2
        );
        assert_eq!(
            MsiUseFeatureExA(std::ptr::null(), std::ptr::null(), 0, 0),
            -2
        );

        let invalid_utf16 = [0xD800_u16, 0x0000];
        let invalid_utf8 = [0xFF_u8, 0x00];

        assert_eq!(
            MsiUseFeatureExW(valid_w.as_ptr(), valid_w.as_ptr(), 0, 0),
            1
        );
        assert_eq!(
            MsiUseFeatureExA(valid_a.as_ptr().cast(), valid_a.as_ptr().cast(), 0, 0),
            1
        );

        assert_eq!(
            MsiUseFeatureExW(invalid_utf16.as_ptr(), std::ptr::null(), 0, 0),
            -2
        );
        assert_eq!(
            MsiUseFeatureExA(invalid_utf8.as_ptr().cast(), std::ptr::null(), 0, 0),
            -2
        );
        assert_eq!(
            MsiUseFeatureExW(valid_w.as_ptr(), invalid_utf16.as_ptr(), 0, 0),
            -2
        );
        assert_eq!(
            MsiUseFeatureExA(valid_a.as_ptr().cast(), invalid_utf8.as_ptr().cast(), 0, 0),
            -2
        );
        assert_eq!(
            MsiUseFeatureExW(invalid_utf16.as_ptr(), valid_w.as_ptr(), 0, 0),
            -2
        );
        assert_eq!(
            MsiUseFeatureExA(invalid_utf8.as_ptr().cast(), valid_a.as_ptr().cast(), 0, 0),
            -2
        );

        assert_eq!(
            MsiInstallMissingComponentW(std::ptr::null(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiInstallMissingComponentA(std::ptr::null(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiInstallMissingComponentW(valid_w.as_ptr(), valid_w.as_ptr(), 0),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiInstallMissingComponentA(valid_a.as_ptr().cast(), valid_a.as_ptr().cast(), 0),
            ERROR_SUCCESS
        );
    }

    #[test]
    fn test_mode_and_elevation() {
        let mut elevated = TRUE;
        let valid_w: Vec<u16> = "valid".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"valid\0";

        assert_eq!(
            MsiIsProductElevatedW(std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiIsProductElevatedW(valid_w.as_ptr(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiIsProductElevatedA(std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiIsProductElevatedA(valid_a.as_ptr().cast(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiIsProductElevatedW(valid_w.as_ptr(), &raw mut elevated),
            ERROR_SUCCESS
        );
        assert_eq!(elevated, FALSE);
        assert_eq!(
            MsiIsProductElevatedA(valid_a.as_ptr().cast(), &raw mut elevated),
            ERROR_SUCCESS
        );
        assert_eq!(elevated, FALSE);

        let invalid_utf16 = [0xD800_u16, 0x0000];
        let invalid_utf8 = [0xFF_u8, 0x00];

        assert_eq!(
            MsiIsProductElevatedW(invalid_utf16.as_ptr(), &raw mut elevated),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiIsProductElevatedA(invalid_utf8.as_ptr().cast(), &raw mut elevated),
            ERROR_INVALID_PARAMETER
        );

        let mut st = 0;
        assert_eq!(
            MsiQueryComponentStateW(
                valid_w.as_ptr(),
                valid_w.as_ptr(),
                0,
                valid_w.as_ptr(),
                &raw mut st
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiQueryComponentStateA(
                valid_a.as_ptr().cast(),
                valid_a.as_ptr().cast(),
                0,
                valid_a.as_ptr().cast(),
                &raw mut st
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiQueryComponentStateW(
                invalid_utf16.as_ptr(),
                valid_w.as_ptr(),
                0,
                valid_w.as_ptr(),
                &raw mut st
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryComponentStateA(
                invalid_utf8.as_ptr().cast(),
                valid_a.as_ptr().cast(),
                0,
                valid_a.as_ptr().cast(),
                &raw mut st
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryComponentStateW(
                valid_w.as_ptr(),
                valid_w.as_ptr(),
                0,
                invalid_utf16.as_ptr(),
                &raw mut st
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryComponentStateA(
                valid_a.as_ptr().cast(),
                valid_a.as_ptr().cast(),
                0,
                invalid_utf8.as_ptr().cast(),
                &raw mut st
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryComponentStateW(
                invalid_utf16.as_ptr(),
                valid_w.as_ptr(),
                0,
                valid_w.as_ptr(),
                &raw mut st
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryComponentStateA(
                invalid_utf8.as_ptr().cast(),
                valid_a.as_ptr().cast(),
                0,
                valid_a.as_ptr().cast(),
                &raw mut st
            ),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiSetComponentStateW(0, invalid_utf16.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetComponentStateA(0, invalid_utf8.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );

        let mut use_count = 0;
        let mut date_used = 0;
        assert_eq!(
            MsiGetFeatureUsageW(
                valid_w.as_ptr(),
                valid_w.as_ptr(),
                &raw mut use_count,
                &raw mut date_used
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetFeatureUsageA(
                valid_a.as_ptr().cast(),
                valid_a.as_ptr().cast(),
                &raw mut use_count,
                &raw mut date_used
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetFeatureUsageW(
                invalid_utf16.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureUsageA(
                invalid_utf8.as_ptr().cast(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureUsageW(
                valid_w.as_ptr(),
                invalid_utf16.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureUsageA(
                valid_a.as_ptr().cast(),
                invalid_utf8.as_ptr().cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureUsageW(
                invalid_utf16.as_ptr(),
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureUsageA(
                invalid_utf8.as_ptr().cast(),
                valid_a.as_ptr().cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiConfigureFeatureW(invalid_utf16.as_ptr(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiConfigureFeatureA(invalid_utf8.as_ptr().cast(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiConfigureFeatureW(valid_w.as_ptr(), invalid_utf16.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiConfigureFeatureA(valid_a.as_ptr().cast(), invalid_utf8.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiConfigureFeatureW(invalid_utf16.as_ptr(), valid_w.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiConfigureFeatureA(invalid_utf8.as_ptr().cast(), valid_a.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiReinstallFeatureW(invalid_utf16.as_ptr(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiReinstallFeatureA(invalid_utf8.as_ptr().cast(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiReinstallFeatureW(valid_w.as_ptr(), invalid_utf16.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiReinstallFeatureA(valid_a.as_ptr().cast(), invalid_utf8.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiReinstallFeatureW(invalid_utf16.as_ptr(), valid_w.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiReinstallFeatureA(invalid_utf8.as_ptr().cast(), valid_a.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiSetFeatureAttributesW(0, invalid_utf16.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetFeatureAttributesA(0, invalid_utf8.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiSetFeatureStateW(0, invalid_utf16.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetFeatureStateA(0, invalid_utf8.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiInstallMissingComponentW(invalid_utf16.as_ptr(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiInstallMissingComponentA(invalid_utf8.as_ptr().cast(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiInstallMissingComponentW(valid_w.as_ptr(), invalid_utf16.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiInstallMissingComponentA(valid_a.as_ptr().cast(), invalid_utf8.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiInstallMissingComponentW(invalid_utf16.as_ptr(), valid_w.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiInstallMissingComponentA(invalid_utf8.as_ptr().cast(), valid_a.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(MsiGetMode(0, 0), FALSE);
        assert_eq!(MsiGetMode(1, 99), FALSE);
        assert_eq!(MsiSetMode(0, 0, TRUE), ERROR_SUCCESS);
        assert_eq!(MsiSetMode(0, 99, TRUE), ERROR_INVALID_PARAMETER);
        assert_eq!(MsiSetInstallLevel(0, 1), ERROR_SUCCESS);
        assert_eq!(MsiSetInstallLevel(0, -1), ERROR_INVALID_PARAMETER);

        let h_sess = crate::handles::alloc_handle(crate::handles::MsiObject::Database(
            crate::types::MsiDatabaseHandle {
                inner: msi::wix::linker::LinkedDatabase::default(),
                state: 0,
            },
        ));
        assert_eq!(MsiGetMode(h_sess, 6), FALSE);
        assert_eq!(MsiSetMode(h_sess, 6, TRUE), ERROR_SUCCESS);
        assert_eq!(MsiGetMode(h_sess, 6), TRUE);
        assert_eq!(MsiSetMode(h_sess, 6, FALSE), ERROR_SUCCESS);
        assert_eq!(MsiGetMode(h_sess, 6), FALSE);
        assert_eq!(MsiSetInstallLevel(h_sess, 0), ERROR_SUCCESS);
        assert_eq!(MsiSetInstallLevel(h_sess, 100), ERROR_SUCCESS);
        let h_rec = crate::win32::record::MsiCreateRecord(1);
        assert_eq!(MsiGetMode(h_rec, 6), FALSE);
        let _ = crate::handles::close_handle(h_rec);
        let _ = crate::handles::close_handle(h_sess);

        assert_eq!(
            MsiConfigureFeatureW(std::ptr::null(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiConfigureFeatureA(std::ptr::null(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiConfigureFeatureW(valid_w.as_ptr(), valid_w.as_ptr(), 0),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiConfigureFeatureA(
                valid_a.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>(),
                0
            ),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiReinstallFeatureW(std::ptr::null(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiReinstallFeatureA(std::ptr::null(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiReinstallFeatureW(valid_w.as_ptr(), valid_w.as_ptr(), 0),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiReinstallFeatureA(
                valid_a.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>(),
                0
            ),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiSetFeatureAttributesW(0, std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetFeatureAttributesA(0, std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetFeatureAttributesW(0, valid_w.as_ptr(), 0),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiSetFeatureAttributesA(0, valid_a.as_ptr().cast::<i8>(), 0),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiSetFeatureStateW(0, std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetFeatureStateA(0, std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(MsiSetFeatureStateW(0, valid_w.as_ptr(), 0), ERROR_SUCCESS);
        assert_eq!(
            MsiSetFeatureStateA(0, valid_a.as_ptr().cast::<i8>(), 0),
            ERROR_SUCCESS
        );
    }
}

/// Configures a feature from a descriptor (ANSI). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_szDescriptor` - Pointer to a string specifying the descriptor.
/// * `_eInstallLevel` - The installation level.
/// * `_eInstallState` - The installation state.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiConfigureFeatureFromDescriptorA(
    _szDescriptor: Lpcstr,
    _eInstallLevel: i32,
    _eInstallState: i32,
) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Configures a feature from a descriptor (Unicode). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_szDescriptor` - Pointer to a string specifying the descriptor.
/// * `_eInstallLevel` - The installation level.
/// * `_eInstallState` - The installation state.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiConfigureFeatureFromDescriptorW(
    _szDescriptor: Lpcwstr,
    _eInstallLevel: i32,
    _eInstallState: i32,
) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Invalidates the feature cache. This is an unimplemented stub.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiInvalidateFeatureCache() -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Queries a feature state from a descriptor (ANSI). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_szDescriptor` - Pointer to a string specifying the descriptor.
/// * `_piInstallState` - Pointer to an integer to receive the state.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiQueryFeatureStateFromDescriptorA(
    _szDescriptor: Lpcstr,
    _piInstallState: *mut i32,
) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Queries a feature state from a descriptor (Unicode). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_szDescriptor` - Pointer to a string specifying the descriptor.
/// * `_piInstallState` - Pointer to an integer to receive the state.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiQueryFeatureStateFromDescriptorW(
    _szDescriptor: Lpcwstr,
    _piInstallState: *mut i32,
) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Reinstalls a feature from a descriptor (ANSI). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_szDescriptor` - Pointer to a string specifying the descriptor.
/// * `_dwReinstallMode` - The reinstall mode.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiReinstallFeatureFromDescriptorA(
    _szDescriptor: Lpcstr,
    _dwReinstallMode: Dword,
) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Reinstalls a feature from a descriptor (Unicode). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_szDescriptor` - Pointer to a string specifying the descriptor.
/// * `_dwReinstallMode` - The reinstall mode.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub extern "system" fn MsiReinstallFeatureFromDescriptorW(
    _szDescriptor: Lpcwstr,
    _dwReinstallMode: Dword,
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
            MsiConfigureFeatureFromDescriptorA(std::ptr::null(), 0, 0),
            1605
        );
        assert_eq!(
            MsiConfigureFeatureFromDescriptorW(std::ptr::null(), 0, 0),
            1605
        );
        assert_eq!(MsiInvalidateFeatureCache(), 1605);
        assert_eq!(
            MsiQueryFeatureStateFromDescriptorA(std::ptr::null(), std::ptr::null_mut()),
            1605
        );
        let h_db = crate::handles::alloc_handle(crate::handles::MsiObject::Database(
            crate::types::MsiDatabaseHandle {
                inner: msi::wix::linker::LinkedDatabase::default(),
                state: 0,
            },
        ));
        let feat_w: Vec<u16> = "Feat1".encode_utf16().chain(std::iter::once(0)).collect();
        let feat_a = b"Feat1\0";
        assert_eq!(
            MsiSetFeatureAttributesW(h_db, feat_w.as_ptr(), 4),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiSetFeatureAttributesA(h_db, feat_a.as_ptr().cast(), 4),
            ERROR_SUCCESS
        );
        assert_eq!(MsiSetFeatureStateW(h_db, feat_w.as_ptr(), 3), ERROR_SUCCESS);
        assert_eq!(
            MsiSetFeatureStateA(h_db, feat_a.as_ptr().cast(), 3),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiSetComponentStateW(h_db, feat_w.as_ptr(), 3),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiSetComponentStateA(h_db, feat_a.as_ptr().cast(), 3),
            ERROR_SUCCESS
        );
        let _ = crate::handles::close_handle(h_db);
        assert_eq!(
            MsiQueryFeatureStateFromDescriptorW(std::ptr::null(), std::ptr::null_mut()),
            1605
        );
        assert_eq!(
            MsiReinstallFeatureFromDescriptorA(std::ptr::null(), 0),
            1605
        );
        assert_eq!(
            MsiReinstallFeatureFromDescriptorW(std::ptr::null(), 0),
            1605
        );
    }
}
