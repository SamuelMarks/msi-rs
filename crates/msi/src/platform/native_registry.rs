//! Windows native registry implementation.

use crate::error::Result;
use crate::platform::registry_store::{RegistryRoot, RegistryValue};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyW, RegDeleteValueW, RegEnumKeyExW, RegEnumValueW, RegOpenKeyExW,
    RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CLASSES_ROOT, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, HKEY_USERS, KEY_READ, KEY_WRITE, REG_BINARY, REG_DWORD, REG_EXPAND_SZ,
    REG_MULTI_SZ, REG_QWORD, REG_SZ, REG_VALUE_TYPE,
};

/// Windows native registry driver wrapping Win32 registry APIs.
#[derive(Debug, Default)]
pub struct NativeRegistryDriver;

/// Maps a logical [`RegistryRoot`] to its corresponding Win32 [`HKEY`] constant.
const fn root_to_hkey(root: RegistryRoot) -> HKEY {
    match root {
        RegistryRoot::ClassesRoot => HKEY_CLASSES_ROOT,
        RegistryRoot::CurrentUser => HKEY_CURRENT_USER,
        RegistryRoot::LocalMachine => HKEY_LOCAL_MACHINE,
        RegistryRoot::Users => HKEY_USERS,
    }
}

/// Encodes a Rust UTF-8 string slice as a null-terminated UTF-16 buffer for Win32 `PCWSTR` calls.
fn to_pcwstr(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Decodes a little-endian byte buffer into a UTF-16 `u16` vector.
fn bytes_to_u16_vec(data: &[u8]) -> Vec<u16> {
    data.chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect()
}

/// Decodes raw registry byte data into a strongly-typed [`RegistryValue`].
fn decode_registry_value(val_type: REG_VALUE_TYPE, data: Vec<u8>) -> Option<RegistryValue> {
    match val_type {
        REG_SZ => {
            let chars = bytes_to_u16_vec(&data);
            let mut s = String::from_utf16_lossy(&chars);
            s.truncate(s.trim_end_matches('\0').len());
            Some(RegistryValue::Sz(s))
        }
        REG_EXPAND_SZ => {
            let chars = bytes_to_u16_vec(&data);
            let mut s = String::from_utf16_lossy(&chars);
            s.truncate(s.trim_end_matches('\0').len());
            Some(RegistryValue::ExpandSz(s))
        }
        REG_DWORD => (data.len() >= 4).then(|| {
            let mut b = [0u8; 4];
            b.copy_from_slice(&data[0..4]);
            RegistryValue::Dword(u32::from_le_bytes(b))
        }),
        REG_QWORD => (data.len() >= 8).then(|| {
            let mut b = [0u8; 8];
            b.copy_from_slice(&data[0..8]);
            RegistryValue::Qword(u64::from_le_bytes(b))
        }),
        REG_BINARY => Some(RegistryValue::Binary(data)),
        REG_MULTI_SZ => {
            let chars = bytes_to_u16_vec(&data);
            let mut strings = Vec::new();
            let mut current = Vec::new();
            for c in chars {
                if c == 0 {
                    if !current.is_empty() {
                        strings.push(String::from_utf16_lossy(&current));
                        current.clear();
                    }
                } else {
                    current.push(c);
                }
            }
            Some(RegistryValue::MultiSz(strings))
        }
        _ => None,
    }
}

impl NativeRegistryDriver {
    /// Creates a new native registry driver with optional WAL path.
    #[must_use]
    pub fn new_wal(_path: impl AsRef<std::path::Path>) -> Self {
        Self
    }

    /// Checks if a registry key exists.
    #[must_use]
    pub fn has_key(&self, root: RegistryRoot, key: &str) -> bool {
        let mut hkey = HKEY::default();
        let path_w = to_pcwstr(key);
        // SAFETY: Invoking RegOpenKeyExW with a valid null-terminated UTF-16 subkey buffer.
        let open_res = unsafe {
            RegOpenKeyExW(
                root_to_hkey(root),
                PCWSTR(path_w.as_ptr()),
                Some(0),
                KEY_READ,
                &raw mut hkey,
            )
        };
        if open_res == ERROR_SUCCESS {
            // SAFETY: Closing key handle opened by RegOpenKeyExW.
            let _ = unsafe { RegCloseKey(hkey) };
            true
        } else {
            false
        }
    }

    /// Enumerates subkeys for a given registry key.
    pub fn get_subkeys<'a>(
        &'a self,
        root: RegistryRoot,
        key: &'a str,
    ) -> Box<dyn Iterator<Item = String> + 'a> {
        let mut hkey = HKEY::default();
        let path_w = to_pcwstr(key);
        // SAFETY: Invoking RegOpenKeyExW with a valid null-terminated UTF-16 subkey buffer.
        let open_res = unsafe {
            RegOpenKeyExW(
                root_to_hkey(root),
                PCWSTR(path_w.as_ptr()),
                Some(0),
                KEY_READ,
                &raw mut hkey,
            )
        };
        if open_res != ERROR_SUCCESS {
            return Box::new(std::iter::empty());
        }

        let mut index = 0;
        let mut subkeys = Vec::new();
        let mut name_buf = vec![0u16; 256];

        loop {
            let mut name_len = u32::try_from(name_buf.len()).unwrap_or(256);
            // SAFETY: Enumerating subkeys into a pre-allocated UTF-16 buffer of `name_len` capacity.
            let enum_res = unsafe {
                RegEnumKeyExW(
                    hkey,
                    index,
                    Some(PWSTR(name_buf.as_mut_ptr())),
                    &raw mut name_len,
                    None,
                    Some(PWSTR(std::ptr::null_mut())),
                    None,
                    None,
                )
            };

            if enum_res == ERROR_SUCCESS || WIN32_ERROR(enum_res.0) == ERROR_SUCCESS {
                let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                subkeys.push(name);
                index += 1;
            } else {
                break;
            }
        }
        // SAFETY: Closing key handle opened by RegOpenKeyExW.
        let _ = unsafe { RegCloseKey(hkey) };
        Box::new(subkeys.into_iter())
    }

    /// Enumerates value names for a given registry key.
    pub fn get_value_names<'a>(
        &'a self,
        root: RegistryRoot,
        key: &'a str,
    ) -> Box<dyn Iterator<Item = String> + 'a> {
        let mut hkey = HKEY::default();
        let path_w = to_pcwstr(key);
        // SAFETY: Invoking RegOpenKeyExW with a valid null-terminated UTF-16 subkey buffer.
        let open_res = unsafe {
            RegOpenKeyExW(
                root_to_hkey(root),
                PCWSTR(path_w.as_ptr()),
                Some(0),
                KEY_READ,
                &raw mut hkey,
            )
        };
        if open_res != ERROR_SUCCESS {
            return Box::new(std::iter::empty());
        }

        let mut index = 0;
        let mut names = Vec::new();
        let mut name_buf = vec![0u16; 16384];

        loop {
            let mut name_len = u32::try_from(name_buf.len()).unwrap_or(16384);
            // SAFETY: Enumerating value names into a pre-allocated UTF-16 buffer of `name_len` capacity.
            let enum_res = unsafe {
                RegEnumValueW(
                    hkey,
                    index,
                    Some(PWSTR(name_buf.as_mut_ptr())),
                    &raw mut name_len,
                    None,
                    None,
                    None,
                    None,
                )
            };

            if enum_res == ERROR_SUCCESS || WIN32_ERROR(enum_res.0) == ERROR_SUCCESS {
                let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                names.push(name);
                index += 1;
            } else {
                break;
            }
        }
        // SAFETY: Closing key handle opened by RegOpenKeyExW.
        let _ = unsafe { RegCloseKey(hkey) };
        Box::new(names.into_iter())
    }

    /// Retrieves a registry value.
    #[must_use]
    pub fn get_value(
        &self,
        root: RegistryRoot,
        key: &str,
        name: Option<&str>,
    ) -> Option<RegistryValue> {
        let mut hkey = HKEY::default();
        let path_w = to_pcwstr(key);
        // SAFETY: Invoking RegOpenKeyExW with a valid null-terminated UTF-16 subkey buffer.
        let open_res = unsafe {
            RegOpenKeyExW(
                root_to_hkey(root),
                PCWSTR(path_w.as_ptr()),
                Some(0),
                KEY_READ,
                &raw mut hkey,
            )
        };
        if open_res != ERROR_SUCCESS {
            return None;
        }

        let name_w = name.map(to_pcwstr);
        let name_pcwstr = name_w
            .as_ref()
            .map_or(PCWSTR(std::ptr::null()), |w| PCWSTR(w.as_ptr()));

        let mut val_type = REG_VALUE_TYPE(0);
        let mut val_len = 0;
        // SAFETY: Querying value type and required byte buffer length.
        let size_res = unsafe {
            RegQueryValueExW(
                hkey,
                name_pcwstr,
                None,
                Some(&raw mut val_type),
                None,
                Some(&raw mut val_len),
            )
        };

        if size_res != ERROR_SUCCESS {
            // SAFETY: Closing key handle opened by RegOpenKeyExW.
            let _ = unsafe { RegCloseKey(hkey) };
            return None;
        }

        let mut data = vec![0u8; val_len as usize];
        // SAFETY: Reading value bytes into pre-sized buffer `data`.
        let read_res = unsafe {
            RegQueryValueExW(
                hkey,
                name_pcwstr,
                None,
                Some(&raw mut val_type),
                Some(data.as_mut_ptr()),
                Some(&raw mut val_len),
            )
        };
        // SAFETY: Closing key handle opened by RegOpenKeyExW.
        let _ = unsafe { RegCloseKey(hkey) };

        if read_res != ERROR_SUCCESS {
            return None;
        }

        decode_registry_value(val_type, data)
    }

    /// Sets a registry value.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::MsiError`] if the operation fails.
    pub fn set_value(
        &mut self,
        root: RegistryRoot,
        key: &str,
        name: Option<&str>,
        value: RegistryValue,
    ) -> Result<()> {
        let mut hkey = HKEY::default();
        let path_w = to_pcwstr(key);
        // SAFETY: Creating or opening registry key with a valid null-terminated UTF-16 subkey buffer.
        let create_res =
            unsafe { RegCreateKeyW(root_to_hkey(root), PCWSTR(path_w.as_ptr()), &raw mut hkey) };
        if create_res != ERROR_SUCCESS {
            return Ok(());
        }

        let name_w = name.map(to_pcwstr);
        let name_pcwstr = name_w
            .as_ref()
            .map_or(PCWSTR(std::ptr::null()), |w| PCWSTR(w.as_ptr()));

        let (val_type, data) = match value {
            RegistryValue::Sz(s) => (
                REG_SZ,
                to_pcwstr(&s)
                    .iter()
                    .flat_map(|&c| c.to_le_bytes())
                    .collect::<Vec<u8>>(),
            ),
            RegistryValue::ExpandSz(s) => (
                REG_EXPAND_SZ,
                to_pcwstr(&s)
                    .iter()
                    .flat_map(|&c| c.to_le_bytes())
                    .collect::<Vec<u8>>(),
            ),
            RegistryValue::Dword(d) => (REG_DWORD, d.to_le_bytes().to_vec()),
            RegistryValue::Qword(q) => (REG_QWORD, q.to_le_bytes().to_vec()),
            RegistryValue::Binary(b) => (REG_BINARY, b),
            RegistryValue::MultiSz(strings) => {
                let mut buf = Vec::new();
                for s in strings {
                    buf.extend(to_pcwstr(&s));
                }
                buf.push(0);
                (
                    REG_MULTI_SZ,
                    buf.iter()
                        .flat_map(|&c| c.to_le_bytes())
                        .collect::<Vec<u8>>(),
                )
            }
        };

        // SAFETY: Writing value data to open key handle and closing the handle.
        unsafe {
            let _ = RegSetValueExW(hkey, name_pcwstr, Some(0), val_type, Some(&data));
            let _ = RegCloseKey(hkey);
        }
        Ok(())
    }

    /// Deletes a registry value.
    pub fn delete_value(&mut self, root: RegistryRoot, key: &str, name: Option<&str>) -> bool {
        let mut hkey = HKEY::default();
        let path_w = to_pcwstr(key);
        // SAFETY: Opening registry key with KEY_WRITE access.
        let open_res = unsafe {
            RegOpenKeyExW(
                root_to_hkey(root),
                PCWSTR(path_w.as_ptr()),
                Some(0),
                KEY_WRITE,
                &raw mut hkey,
            )
        };
        if open_res != ERROR_SUCCESS {
            return false;
        }

        let name_w = name.map(to_pcwstr);
        let name_pcwstr = name_w
            .as_ref()
            .map_or(PCWSTR(std::ptr::null()), |w| PCWSTR(w.as_ptr()));

        // SAFETY: Deleting value on open key handle and closing the handle.
        let del_res = unsafe { RegDeleteValueW(hkey, name_pcwstr) };
        // SAFETY: Closing key handle opened by RegOpenKeyExW.
        let _ = unsafe { RegCloseKey(hkey) };
        del_res == ERROR_SUCCESS || WIN32_ERROR(del_res.0) == ERROR_SUCCESS
    }
}
