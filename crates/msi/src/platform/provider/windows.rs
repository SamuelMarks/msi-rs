//! Windows platform provider implementation.

use super::PlatformProvider;
use crate::error::MsiError;
use std::path::Path;

/// Windows-specific `PlatformProvider`.
#[derive(Debug, Clone, Default)]
pub struct WindowsProvider;

impl PlatformProvider for WindowsProvider {
    fn os_name(&self) -> &'static str {
        "windows"
    }

    fn read_registry_string(
        &self,
        _root: u32,
        _path: &str,
        _name: &str,
    ) -> Result<Option<String>, MsiError> {
        // TODO: Map to actual Win32 RegGetValueW
        Ok(None)
    }

    fn write_registry_string(
        &self,
        _root: u32,
        _path: &str,
        _name: &str,
        _value: &str,
    ) -> Result<(), MsiError> {
        // TODO: Map to actual Win32 RegSetKeyValueW
        Ok(())
    }

    fn service_exists(&self, _name: &str) -> Result<bool, MsiError> {
        // TODO: Map to actual Win32 OpenServiceW
        Ok(false)
    }

    fn create_shortcut(&self, target: &Path, shortcut_path: &Path) -> Result<(), MsiError> {
        #[cfg(windows)]
        {
            use windows::core::PCWSTR;
            use windows::Win32::System::Com::{
                CoCreateInstance, CoInitialize, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
            };
            use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

            unsafe {
                let _ = CoInitialize(None);

                let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                    .map_err(|e| MsiError::ComRpcError(e.message().to_string()))?;

                let mut target_w: Vec<u16> = target.to_string_lossy().encode_utf16().collect();
                target_w.push(0);

                link.SetPath(PCWSTR(target_w.as_ptr()))
                    .map_err(|e| MsiError::ComRpcError(e.message().to_string()))?;

                let persist: IPersistFile = link
                    .cast()
                    .map_err(|e| MsiError::ComRpcError(e.message().to_string()))?;

                let mut shortcut_w: Vec<u16> =
                    shortcut_path.to_string_lossy().encode_utf16().collect();
                shortcut_w.push(0);

                persist
                    .Save(PCWSTR(shortcut_w.as_ptr()), true)
                    .map_err(|e| MsiError::ComRpcError(e.message().to_string()))?;

                CoUninitialize();
            }
        }

        #[cfg(not(windows))]
        {
            let _ = target;
            let _ = shortcut_path;
        }

        Ok(())
    }

    fn register_file_association(
        &self,
        _extension: &str,
        _prog_id: &str,
        _description: &str,
        _executable: &Path,
    ) -> Result<(), MsiError> {
        Ok(())
    }

    fn register_com_class(
        &self,
        _clsid: &str,
        _context: &str,
        _executable: &Path,
    ) -> Result<(), MsiError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_provider_defaults() {
        let provider = WindowsProvider;

        assert_eq!(format!("{provider:?}"), "WindowsProvider");

        assert_eq!(provider.os_name(), "windows");

        let read_res = provider.read_registry_string(0, "", "");
        assert!(read_res.is_ok());
        assert!(read_res.unwrap_or_default().is_none());

        let write_res = provider.write_registry_string(0, "", "", "");
        assert!(write_res.is_ok());

        let svc_res = provider.service_exists("");
        assert!(svc_res.is_ok());
        assert!(!svc_res.unwrap_or(true));

        let shortcut_res = provider.create_shortcut(Path::new(""), Path::new(""));
        assert!(shortcut_res.is_ok());
    }

    #[test]
    fn test_shell_registration_stubs() {
        let provider = WindowsProvider;
        assert!(provider
            .register_file_association(".txt", "txtfile", "Text File", Path::new(""))
            .is_ok());
        assert!(provider
            .register_com_class("clsid", "context", Path::new(""))
            .is_ok());
    }
}
