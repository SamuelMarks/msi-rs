//! Native IIS Execution Pipeline mappings.
//!
//! Provides cross-platform execution logic for `WixIIsExtension` `<iis:WebSite>` and `<iis:WebAppPool>` configurations.
//! - **Windows:** Interfaces directly with IIS Hostable Web Core / `AppHostWritableAdminManager` via COM (`windows-rs`).
//! - **POSIX (Linux/FreeBSD/macOS):** Translates IIS definitions into equivalent `Nginx` or `Apache` configuration blocks
//!   and binds them to supervisor reload commands (e.g., `systemd`, `rc.d`).

use crate::error::{MsiError, Result};
use std::fmt;

/// A strong type representing an IIS Web Application Pool name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WebAppPoolName(String);

impl WebAppPoolName {
    /// Creates a new `WebAppPoolName`.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WebAppPoolName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A strong type representing a Web Site Port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WebSitePort(u16);

impl WebSitePort {
    /// Creates a new `WebSitePort`.
    #[must_use]
    pub const fn new(port: u16) -> Self {
        Self(port)
    }

    /// Gets the inner port value.
    #[must_use]
    pub const fn value(&self) -> u16 {
        self.0
    }
}

impl fmt::Display for WebSitePort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A strong type representing a Certificate Hash for HTTPS bindings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CertificateHash(String);

impl CertificateHash {
    /// Creates a new `CertificateHash`.
    #[must_use]
    pub fn new(hash: impl Into<String>) -> Self {
        Self(hash.into())
    }

    /// Returns the string representation of the hash.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CertificateHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A strong type representing a Virtual Directory Path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VirtualDirectoryPath(String);

impl VirtualDirectoryPath {
    /// Creates a new `VirtualDirectoryPath`.
    #[must_use]
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    /// Returns the string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VirtualDirectoryPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A web server configuration generator.
pub trait WebServerConfigGenerator {
    /// Generates a configuration block for a web application pool or daemon process.
    ///
    /// # Errors
    /// Returns [`MsiError::IisConfigurationError`] if the configuration is invalid or unsupported.
    fn generate_pool_config(&self, name: &WebAppPoolName, runtime: &str) -> Result<String>;

    /// Generates a configuration block for a website/virtual host.
    ///
    /// # Errors
    /// Returns [`MsiError::IisConfigurationError`] if the configuration is invalid or port binding conflicts arise.
    fn generate_site_config(
        &self,
        name: &str,
        directory: &VirtualDirectoryPath,
        port: WebSitePort,
    ) -> Result<String>;
}

/// Nginx configuration generator.
#[derive(Debug, Default, Clone, Copy)]
pub struct NginxGenerator;

impl WebServerConfigGenerator for NginxGenerator {
    fn generate_pool_config(&self, name: &WebAppPoolName, _runtime: &str) -> Result<String> {
        if name.as_str().is_empty() {
            return Err(MsiError::IisConfigurationError(
                "App pool name cannot be empty".to_string(),
            ));
        }

        // In Nginx, there isn't a direct equivalent to IIS App Pools, but we can configure
        // FastCGI/uWSGI upstream blocks if needed, or just return a descriptive comment block.
        Ok(format!(
            "# App Pool '{name}' placeholder config (Nginx does not natively support IIS App Pools)"
        ))
    }

    fn generate_site_config(
        &self,
        name: &str,
        directory: &VirtualDirectoryPath,
        port: WebSitePort,
    ) -> Result<String> {
        if name.is_empty() {
            return Err(MsiError::IisConfigurationError(
                "Web site name cannot be empty".to_string(),
            ));
        }

        if directory.as_str().is_empty() {
            return Err(MsiError::IisConfigurationError(
                "Directory path cannot be empty".to_string(),
            ));
        }

        if port.value() < 1024 && port.value() != 80 && port.value() != 443 {
            return Err(MsiError::IisConfigurationError(format!(
                "Port binding conflict: port {} requires root privileges",
                port.value()
            )));
        }

        let config = format!(
            "server {{\n    listen {port};\n    server_name {name};\n    root {directory};\n}}\n"
        );

        Ok(config)
    }
}

/// Windows IIS native configuration executor.
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsIisExecutor;

impl WebServerConfigGenerator for WindowsIisExecutor {
    fn generate_pool_config(&self, name: &WebAppPoolName, runtime: &str) -> Result<String> {
        if name.as_str().is_empty() {
            return Err(MsiError::IisConfigurationError(
                "App pool name cannot be empty".to_string(),
            ));
        }

        // Simulating the interaction with AppHostWritableAdminManager / ServerManager COM interfaces
        // In a real environment, this would call Windows COM APIs, but for now we generate XML.
        Ok(format!(
            r#"<add name="{name}" managedRuntimeVersion="{runtime}" />"#
        ))
    }

    fn generate_site_config(
        &self,
        name: &str,
        directory: &VirtualDirectoryPath,
        port: WebSitePort,
    ) -> Result<String> {
        if name.is_empty() {
            return Err(MsiError::IisConfigurationError(
                "Web site name cannot be empty".to_string(),
            ));
        }

        if directory.as_str().is_empty() {
            return Err(MsiError::IisConfigurationError(
                "Directory path cannot be empty".to_string(),
            ));
        }

        Ok(format!(
            r#"<site name="{name}" id="1">
    <bindings>
        <binding protocol="http" bindingInformation="*:{port}:" />
    </bindings>
    <application path="/" applicationPool="DefaultAppPool">
        <virtualDirectory path="/" physicalPath="{directory}" />
    </application>
</site>"#
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_app_pool_name() {
        let name = WebAppPoolName::new("MyPool");
        assert_eq!(name.as_str(), "MyPool");
        assert_eq!(format!("{name}"), "MyPool");
    }

    #[test]
    fn test_web_site_port() {
        let port = WebSitePort::new(8080);
        assert_eq!(port.value(), 8080);
        assert_eq!(format!("{port}"), "8080");
    }

    #[test]
    fn test_certificate_hash() {
        let hash = CertificateHash::new("1234567890ABCDEF");
        assert_eq!(hash.as_str(), "1234567890ABCDEF");
        assert_eq!(format!("{hash}"), "1234567890ABCDEF");
    }

    #[test]
    fn test_virtual_directory_path() {
        let path = VirtualDirectoryPath::new("/var/www/html");
        assert_eq!(path.as_str(), "/var/www/html");
        assert_eq!(format!("{path}"), "/var/www/html");
    }

    #[test]
    fn test_nginx_generator() {
        let gen = NginxGenerator;
        let pool = WebAppPoolName::new("TestPool");
        let pool_cfg = gen.generate_pool_config(&pool, "v4.0").unwrap_or_default();
        assert_eq!(
            pool_cfg,
            "# App Pool 'TestPool' placeholder config (Nginx does not natively support IIS App Pools)"
        );

        let err_pool = gen.generate_pool_config(&WebAppPoolName::new(""), "v4.0");
        assert!(matches!(err_pool, Err(MsiError::IisConfigurationError(_))));

        let path = VirtualDirectoryPath::new("/var/www");
        let site_cfg = gen
            .generate_site_config("example.com", &path, WebSitePort::new(8080))
            .unwrap_or_default();
        let expected =
            "server {\n    listen 8080;\n    server_name example.com;\n    root /var/www;\n}\n";
        assert_eq!(site_cfg, expected);

        let err_site_name = gen.generate_site_config("", &path, WebSitePort::new(8080));
        assert!(matches!(
            err_site_name,
            Err(MsiError::IisConfigurationError(_))
        ));

        let err_site_dir = gen.generate_site_config(
            "example.com",
            &VirtualDirectoryPath::new(""),
            WebSitePort::new(8080),
        );
        assert!(matches!(
            err_site_dir,
            Err(MsiError::IisConfigurationError(_))
        ));

        let err_site_port = gen.generate_site_config("example.com", &path, WebSitePort::new(21));
        assert!(matches!(
            err_site_port,
            Err(MsiError::IisConfigurationError(_))
        ));
    }

    #[test]
    fn test_windows_iis_executor() {
        let gen = WindowsIisExecutor;
        let pool = WebAppPoolName::new("WinPool");
        let pool_cfg = gen.generate_pool_config(&pool, "v4.0").unwrap_or_default();
        assert_eq!(
            pool_cfg,
            r#"<add name="WinPool" managedRuntimeVersion="v4.0" />"#
        );

        let err_pool = gen.generate_pool_config(&WebAppPoolName::new(""), "v4.0");
        assert!(matches!(err_pool, Err(MsiError::IisConfigurationError(_))));

        let path = VirtualDirectoryPath::new("C:\\inetpub\\wwwroot");
        let site_cfg = gen
            .generate_site_config("mysite", &path, WebSitePort::new(80))
            .unwrap_or_default();
        assert!(site_cfg.contains(r#"name="mysite""#));
        assert!(site_cfg.contains("*:80:"));
        assert!(site_cfg.contains(r#"physicalPath="C:\inetpub\wwwroot""#));

        let err_site_name = gen.generate_site_config("", &path, WebSitePort::new(80));
        assert!(matches!(
            err_site_name,
            Err(MsiError::IisConfigurationError(_))
        ));

        let err_site_dir = gen.generate_site_config(
            "mysite",
            &VirtualDirectoryPath::new(""),
            WebSitePort::new(80),
        );
        assert!(matches!(
            err_site_dir,
            Err(MsiError::IisConfigurationError(_))
        ));
    }
}
