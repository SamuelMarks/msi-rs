//! Windows native registry implementation.

#![cfg(windows)]

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
pub struct NativeRegistryDriver {}

fn root_to_hkey(root: RegistryRoot) -> HKEY {
    match root {
        RegistryRoot::ClassesRoot => HKEY_CLASSES_ROOT,
        RegistryRoot::CurrentUser => HKEY_CURRENT_USER,
        RegistryRoot::LocalMachine => HKEY_LOCAL_MACHINE,
        RegistryRoot::Users => HKEY_USERS,
    }
}

fn to_pcwstr(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

impl NativeRegistryDriver {
    /// Creates a new native registry driver with optional WAL path.
    pub fn new_wal(_path: impl AsRef<std::path::Path>) -> Self {
        Self {}
    }

    /// Checks if a registry key exists.
    pub fn has_key(&self, root: RegistryRoot, key: &str) -> bool {
        let mut hkey = HKEY::default();
        let path_w = to_pcwstr(key);
        let res = unsafe {
            RegOpenKeyExW(
                root_to_hkey(root),
                PCWSTR(path_w.as_ptr()),
                Some(0),
                KEY_READ,
                &mut hkey,
            )
        };
        if res == ERROR_SUCCESS {
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
        let res = unsafe {
            RegOpenKeyExW(
                root_to_hkey(root),
                PCWSTR(path_w.as_ptr()),
                Some(0),
                KEY_READ,
                &mut hkey,
            )
        };
        if res != ERROR_SUCCESS {
            return Box::new(std::iter::empty());
        }

        let mut index = 0;
        let mut subkeys = Vec::new();
        let mut name_buf = vec![0u16; 256];

        loop {
            let mut name_len = name_buf.len() as u32;
            let res = unsafe {
                RegEnumKeyExW(
                    hkey,
                    index,
                    Some(PWSTR(name_buf.as_mut_ptr())),
                    &mut name_len,
                    None,
                    Some(PWSTR(std::ptr::null_mut())),
                    None,
                    None,
                )
            };

            if res == ERROR_SUCCESS || WIN32_ERROR(res.0) == ERROR_SUCCESS {
                let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                subkeys.push(name);
                index += 1;
            } else {
                break;
            }
        }
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
        let res = unsafe {
            RegOpenKeyExW(
                root_to_hkey(root),
                PCWSTR(path_w.as_ptr()),
                Some(0),
                KEY_READ,
                &mut hkey,
            )
        };
        if res != ERROR_SUCCESS {
            return Box::new(std::iter::empty());
        }

        let mut index = 0;
        let mut names = Vec::new();
        let mut name_buf = vec![0u16; 16384];

        loop {
            let mut name_len = name_buf.len() as u32;
            let res = unsafe {
                RegEnumValueW(
                    hkey,
                    index,
                    Some(PWSTR(name_buf.as_mut_ptr())),
                    &mut name_len,
                    None,
                    None,
                    None,
                    None,
                )
            };

            if res == ERROR_SUCCESS || WIN32_ERROR(res.0) == ERROR_SUCCESS {
                let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                names.push(name);
                index += 1;
            } else {
                break;
            }
        }
        let _ = unsafe { RegCloseKey(hkey) };
        Box::new(names.into_iter())
    }

    /// Retrieves a registry value.
    pub fn get_value(
        &self,
        root: RegistryRoot,
        key: &str,
        name: Option<&str>,
    ) -> Option<RegistryValue> {
        let mut hkey = HKEY::default();
        let path_w = to_pcwstr(key);
        let res = unsafe {
            RegOpenKeyExW(
                root_to_hkey(root),
                PCWSTR(path_w.as_ptr()),
                Some(0),
                KEY_READ,
                &mut hkey,
            )
        };
        if res != ERROR_SUCCESS {
            return None;
        }

        let name_w = name.map(to_pcwstr);
        let name_pcwstr = name_w
            .as_ref()
            .map(|w| PCWSTR(w.as_ptr()))
            .unwrap_or(PCWSTR(std::ptr::null()));

        let mut val_type = REG_VALUE_TYPE(0);
        let mut val_len = 0;
        let res = unsafe {
            RegQueryValueExW(
                hkey,
                name_pcwstr,
                None,
                Some(&mut val_type),
                None,
                Some(&mut val_len),
            )
        };

        if res != ERROR_SUCCESS {
            let _ = unsafe { RegCloseKey(hkey) };
            return None;
        }

        let mut data = vec![0u8; val_len as usize];
        let res = unsafe {
            RegQueryValueExW(
                hkey,
                name_pcwstr,
                None,
                Some(&mut val_type),
                Some(data.as_mut_ptr()),
                Some(&mut val_len),
            )
        };
        let _ = unsafe { RegCloseKey(hkey) };

        if res != ERROR_SUCCESS {
            return None;
        }

        match val_type {
            REG_SZ => {
                let chars: &[u16] = unsafe {
                    std::slice::from_raw_parts(data.as_ptr() as *const u16, data.len() / 2)
                };
                let mut s = String::from_utf16_lossy(chars);
                s.truncate(s.trim_end_matches('\0').len());
                Some(RegistryValue::Sz(s))
            }
            REG_EXPAND_SZ => {
                let chars: &[u16] = unsafe {
                    std::slice::from_raw_parts(data.as_ptr() as *const u16, data.len() / 2)
                };
                let mut s = String::from_utf16_lossy(chars);
                s.truncate(s.trim_end_matches('\0').len());
                Some(RegistryValue::ExpandSz(s))
            }
            REG_DWORD => {
                if data.len() >= 4 {
                    let mut b = [0u8; 4];
                    b.copy_from_slice(&data[0..4]);
                    Some(RegistryValue::Dword(u32::from_le_bytes(b)))
                } else {
                    None
                }
            }
            REG_QWORD => {
                if data.len() >= 8 {
                    let mut b = [0u8; 8];
                    b.copy_from_slice(&data[0..8]);
                    Some(RegistryValue::Qword(u64::from_le_bytes(b)))
                } else {
                    None
                }
            }
            REG_BINARY => Some(RegistryValue::Binary(data)),
            REG_MULTI_SZ => {
                let chars: &[u16] = unsafe {
                    std::slice::from_raw_parts(data.as_ptr() as *const u16, data.len() / 2)
                };
                let mut strings = Vec::new();
                let mut current = Vec::new();
                for &c in chars {
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

    /// Sets a registry value.
    pub fn set_value(
        &mut self,
        root: RegistryRoot,
        key: &str,
        name: Option<&str>,
        value: RegistryValue,
    ) -> Result<()> {
        let mut hkey = HKEY::default();
        let path_w = to_pcwstr(key);
        let res = unsafe { RegCreateKeyW(root_to_hkey(root), PCWSTR(path_w.as_ptr()), &mut hkey) };
        if res != ERROR_SUCCESS {
            return Ok(());
        }

        let name_w = name.map(to_pcwstr);
        let name_pcwstr = name_w
            .as_ref()
            .map(|w| PCWSTR(w.as_ptr()))
            .unwrap_or(PCWSTR(std::ptr::null()));

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
        let res = unsafe {
            RegOpenKeyExW(
                root_to_hkey(root),
                PCWSTR(path_w.as_ptr()),
                Some(0),
                KEY_WRITE,
                &mut hkey,
            )
        };
        if res != ERROR_SUCCESS {
            return false;
        }

        let name_w = name.map(to_pcwstr);
        let name_pcwstr = name_w
            .as_ref()
            .map(|w| PCWSTR(w.as_ptr()))
            .unwrap_or(PCWSTR(std::ptr::null()));

        let res = unsafe { RegDeleteValueW(hkey, name_pcwstr) };
        let _ = unsafe { RegCloseKey(hkey) };
        res == ERROR_SUCCESS || WIN32_ERROR(res.0) == ERROR_SUCCESS
    }
}
