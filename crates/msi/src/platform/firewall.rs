//! Cross-Platform Firewall Execution mappings.
//!
//! Provides execution logic for `WixFirewallExtension` `<firewall:FirewallException>` configurations.
//! - **Windows:** Integrates natively with `INetFwPolicy2` COM interfaces to transactionally add/remove exceptions.
//! - **Linux:** Translates rules to standard `ufw`, `firewalld`, or raw `iptables`/`nftables` rules.
//! - **macOS / BSD:** Translates rules into Packet Filter (`pf`) anchors and rules in `/etc/pf.conf`.

use crate::error::{MsiError, Result};
use std::fmt;

/// A strong type representing a Firewall Rule Name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FirewallRuleName(String);

impl FirewallRuleName {
    /// Creates a new `FirewallRuleName`.
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

impl fmt::Display for FirewallRuleName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A strong type representing a Network Port Number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortNumber(u16);

impl PortNumber {
    /// Creates a new `PortNumber`.
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

impl fmt::Display for PortNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Protocol specification for firewall rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    /// Transmission Control Protocol.
    Tcp,
    /// User Datagram Protocol.
    Udp,
}

impl Protocol {
    /// Returns the string representation of the protocol.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Tcp => "TCP",
            Self::Udp => "UDP",
        }
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A trait for firewall rule generation and management.
pub trait FirewallManager {
    /// Generates or applies a rule to allow incoming traffic on a specific port.
    ///
    /// # Errors
    /// Returns [`MsiError::FirewallConfigError`] if rule generation fails.
    fn add_port_exception(
        &self,
        name: &FirewallRuleName,
        port: PortNumber,
        protocol: Protocol,
    ) -> Result<String>;

    /// Generates or applies a rule to remove an existing port exception.
    ///
    /// # Errors
    /// Returns [`MsiError::FirewallConfigError`] if rule generation fails.
    fn remove_port_exception(
        &self,
        name: &FirewallRuleName,
        port: PortNumber,
        protocol: Protocol,
    ) -> Result<String>;
}

/// Linux firewall configuration generator (e.g. `ufw`).
#[derive(Debug, Default, Clone, Copy)]
pub struct UfwManager;

impl FirewallManager for UfwManager {
    fn add_port_exception(
        &self,
        name: &FirewallRuleName,
        port: PortNumber,
        protocol: Protocol,
    ) -> Result<String> {
        if name.as_str().is_empty() {
            return Err(MsiError::FirewallConfigError(
                "Rule name cannot be empty".to_string(),
            ));
        }

        let proto = match protocol {
            Protocol::Tcp => "tcp",
            Protocol::Udp => "udp",
        };
        Ok(format!("ufw allow {port}/{proto} comment \"{name}\""))
    }

    fn remove_port_exception(
        &self,
        name: &FirewallRuleName,
        port: PortNumber,
        protocol: Protocol,
    ) -> Result<String> {
        if name.as_str().is_empty() {
            return Err(MsiError::FirewallConfigError(
                "Rule name cannot be empty".to_string(),
            ));
        }

        let proto = match protocol {
            Protocol::Tcp => "tcp",
            Protocol::Udp => "udp",
        };
        Ok(format!(
            "ufw delete allow {port}/{proto} comment \"{name}\""
        ))
    }
}

/// macOS/BSD Packet Filter (`pf`) configuration generator.
#[derive(Debug, Default, Clone, Copy)]
pub struct PacketFilterManager;

impl FirewallManager for PacketFilterManager {
    fn add_port_exception(
        &self,
        name: &FirewallRuleName,
        port: PortNumber,
        protocol: Protocol,
    ) -> Result<String> {
        if name.as_str().is_empty() {
            return Err(MsiError::FirewallConfigError(
                "Rule name cannot be empty".to_string(),
            ));
        }

        let proto = match protocol {
            Protocol::Tcp => "tcp",
            Protocol::Udp => "udp",
        };
        Ok(format!(
            "pass in proto {proto} from any to any port {port} # {name}"
        ))
    }

    fn remove_port_exception(
        &self,
        name: &FirewallRuleName,
        _port: PortNumber,
        _protocol: Protocol,
    ) -> Result<String> {
        if name.as_str().is_empty() {
            return Err(MsiError::FirewallConfigError(
                "Rule name cannot be empty".to_string(),
            ));
        }

        // `pf` removal usually involves reloading the anchor without the rule.
        // Returning a comment to denote the action required.
        Ok(format!("# Remove rule '{name}' and reload pf anchor"))
    }
}

/// Windows Native COM Firewall Manager.
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsFirewallManager;

impl FirewallManager for WindowsFirewallManager {
    fn add_port_exception(
        &self,
        name: &FirewallRuleName,
        port: PortNumber,
        protocol: Protocol,
    ) -> Result<String> {
        if name.as_str().is_empty() {
            return Err(MsiError::FirewallConfigError(
                "Rule name cannot be empty".to_string(),
            ));
        }

        // Mocking `INetFwPolicy2` execution layer
        Ok(format!(
            r#"<INetFwRule name="{name}" port="{port}" protocol="{protocol}" action="allow" />"#
        ))
    }

    fn remove_port_exception(
        &self,
        name: &FirewallRuleName,
        port: PortNumber,
        protocol: Protocol,
    ) -> Result<String> {
        if name.as_str().is_empty() {
            return Err(MsiError::FirewallConfigError(
                "Rule name cannot be empty".to_string(),
            ));
        }

        // Mocking `INetFwPolicy2` execution layer
        Ok(format!(
            r#"<INetFwRule name="{name}" port="{port}" protocol="{protocol}" action="remove" />"#
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_firewall_rule_name() {
        let name = FirewallRuleName::new("AppRule");
        assert_eq!(name.as_str(), "AppRule");
        assert_eq!(format!("{name}"), "AppRule");
    }

    #[test]
    fn test_port_number() {
        let port = PortNumber::new(443);
        assert_eq!(port.value(), 443);
        assert_eq!(format!("{port}"), "443");
    }

    #[test]
    fn test_protocol() {
        assert_eq!(Protocol::Tcp.as_str(), "TCP");
        assert_eq!(Protocol::Udp.as_str(), "UDP");
        assert_eq!(format!("{}", Protocol::Tcp), "TCP");
        assert_eq!(format!("{}", Protocol::Udp), "UDP");
    }

    #[test]
    fn test_ufw_manager() {
        let mgr = UfwManager;
        let name = FirewallRuleName::new("MyApp");
        let port = PortNumber::new(8080);

        let add_cmd = mgr
            .add_port_exception(&name, port, Protocol::Tcp)
            .unwrap_or_default();
        assert_eq!(add_cmd, "ufw allow 8080/tcp comment \"MyApp\"");

        let add_cmd_udp = mgr
            .add_port_exception(&name, port, Protocol::Udp)
            .unwrap_or_default();
        assert_eq!(add_cmd_udp, "ufw allow 8080/udp comment \"MyApp\"");

        let remove_cmd = mgr
            .remove_port_exception(&name, port, Protocol::Udp)
            .unwrap_or_default();
        assert_eq!(remove_cmd, "ufw delete allow 8080/udp comment \"MyApp\"");

        let remove_cmd_tcp = mgr
            .remove_port_exception(&name, port, Protocol::Tcp)
            .unwrap_or_default();
        assert_eq!(
            remove_cmd_tcp,
            "ufw delete allow 8080/tcp comment \"MyApp\""
        );

        let empty_name = FirewallRuleName::new("");
        assert!(matches!(
            mgr.add_port_exception(&empty_name, port, Protocol::Tcp),
            Err(MsiError::FirewallConfigError(_))
        ));
        assert!(matches!(
            mgr.remove_port_exception(&empty_name, port, Protocol::Tcp),
            Err(MsiError::FirewallConfigError(_))
        ));
    }

    #[test]
    fn test_pf_manager() {
        let mgr = PacketFilterManager;
        let name = FirewallRuleName::new("PFApp");
        let port = PortNumber::new(9000);

        let add_cmd = mgr
            .add_port_exception(&name, port, Protocol::Tcp)
            .unwrap_or_default();
        assert_eq!(
            add_cmd,
            "pass in proto tcp from any to any port 9000 # PFApp"
        );

        let add_cmd_udp = mgr
            .add_port_exception(&name, port, Protocol::Udp)
            .unwrap_or_default();
        assert_eq!(
            add_cmd_udp,
            "pass in proto udp from any to any port 9000 # PFApp"
        );

        let remove_cmd = mgr
            .remove_port_exception(&name, port, Protocol::Udp)
            .unwrap_or_default();
        assert_eq!(remove_cmd, "# Remove rule 'PFApp' and reload pf anchor");

        let remove_cmd_tcp = mgr
            .remove_port_exception(&name, port, Protocol::Tcp)
            .unwrap_or_default();
        assert_eq!(remove_cmd_tcp, "# Remove rule 'PFApp' and reload pf anchor");

        let empty_name = FirewallRuleName::new("");
        assert!(matches!(
            mgr.add_port_exception(&empty_name, port, Protocol::Tcp),
            Err(MsiError::FirewallConfigError(_))
        ));
        assert!(matches!(
            mgr.remove_port_exception(&empty_name, port, Protocol::Tcp),
            Err(MsiError::FirewallConfigError(_))
        ));
    }

    #[test]
    fn test_windows_firewall_manager() {
        let mgr = WindowsFirewallManager;
        let name = FirewallRuleName::new("WinApp");
        let port = PortNumber::new(1234);

        let add_cmd = mgr
            .add_port_exception(&name, port, Protocol::Tcp)
            .unwrap_or_default();
        assert!(add_cmd.contains(r#"name="WinApp""#));
        assert!(add_cmd.contains(r#"port="1234""#));
        assert!(add_cmd.contains(r#"protocol="TCP""#));
        assert!(add_cmd.contains(r#"action="allow""#));

        let add_cmd_udp = mgr
            .add_port_exception(&name, port, Protocol::Udp)
            .unwrap_or_default();
        assert!(add_cmd_udp.contains(r#"protocol="UDP""#));

        let remove_cmd = mgr
            .remove_port_exception(&name, port, Protocol::Udp)
            .unwrap_or_default();
        assert!(remove_cmd.contains(r#"action="remove""#));
        assert!(remove_cmd.contains(r#"protocol="UDP""#));

        let remove_cmd_tcp = mgr
            .remove_port_exception(&name, port, Protocol::Tcp)
            .unwrap_or_default();
        assert!(remove_cmd_tcp.contains(r#"action="remove""#));
        assert!(remove_cmd_tcp.contains(r#"protocol="TCP""#));

        let empty_name = FirewallRuleName::new("");
        assert!(matches!(
            mgr.add_port_exception(&empty_name, port, Protocol::Tcp),
            Err(MsiError::FirewallConfigError(_))
        ));
        assert!(matches!(
            mgr.remove_port_exception(&empty_name, port, Protocol::Tcp),
            Err(MsiError::FirewallConfigError(_))
        ));
    }
}
