//! MSI Control Specifications and Runtime Models.
//!
//! Grounded directly in official Windows Installer SDK specifications:
//! - Complete support for all standard control types:
//!   - `PushButton`, `RadioButtonGroup`, `CheckBox`, `Edit`, `Text`, `ComboBox`, `ListBox`,
//!     `ListView`, `ProgressBar`, `Bitmap`, `Line`, `ScrollableText`, `VolumeCostList`, `SelectionTree`.
//! - Control attributes bitmasks (Visible, Enabled, `PasswordInput`, Integer, etc.).
//! - Rich models for selection trees, volume cost entries, list items, and runtime control state.

use crate::error::{Error, Result};
use crate::execution::properties::InstallState;
use crate::ui::layout::DluRect;

/// Control attribute flag: Control is initially visible (`0x0001`).
pub const CONTROL_ATTR_VISIBLE: u32 = 0x0001;

/// Control attribute flag: Control is initially enabled (`0x0002`).
pub const CONTROL_ATTR_ENABLED: u32 = 0x0002;

/// Control attribute flag: Control has sunken / 3D border outline (`0x0004`).
pub const CONTROL_ATTR_SUNKEN: u32 = 0x0004;

/// Control attribute flag: Control binds property value indirectly (`0x0008`).
pub const CONTROL_ATTR_INDIRECT: u32 = 0x0008;

/// Control attribute flag: Control accepts only integer digits (`0x0010`).
pub const CONTROL_ATTR_INTEGER: u32 = 0x0010;

/// Control attribute flag: Control displays text with right alignment (`0x0020`).
pub const CONTROL_ATTR_RIGHT_ALIGNED: u32 = 0x0020;

/// Control attribute flag: Control renders right-to-left layout (`0x0040`).
pub const CONTROL_ATTR_RIGHT_TO_LEFT: u32 = 0x0040;

/// Control attribute flag: Control supports multiline text wrapping (`0x0080`).
pub const CONTROL_ATTR_MULTILINE: u32 = 0x0080;

/// Control attribute flag: Password edit control masking characters with `*` (`0x0200`).
pub const CONTROL_ATTR_PASSWORD_INPUT: u32 = 0x0200;

/// Control attribute flag: Renders an image resource stream (`0x00010000`).
pub const CONTROL_ATTR_IMAGE_RESOURCE: u32 = 0x0001_0000;

/// Control attribute flag: Renders a Bitmap image (`0x00020000`).
pub const CONTROL_ATTR_BITMAP: u32 = 0x0002_0000;

/// Control attribute flag: Renders an Icon image (`0x00040000`).
pub const CONTROL_ATTR_ICON: u32 = 0x0004_0000;

/// Control attribute flag: Fixed size image without stretching (`0x00080000`).
pub const CONTROL_ATTR_FIXED_SIZE: u32 = 0x0008_0000;

/// Standard MSI control types specified in the `Control` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlType {
    /// Standard push button executing events on click.
    PushButton,
    /// Container for grouped radio buttons bound to a shared property.
    RadioButtonGroup,
    /// Toggle checkbox setting or unsetting a bound property.
    CheckBox,
    /// Single-line or multiline text input box.
    Edit,
    /// Static or formatted text label with font styling and hyperlinks.
    Text,
    /// Drop-down combo box list populated from table records.
    ComboBox,
    /// Fixed list box populated from table records.
    ListBox,
    /// Multi-column list display with icon support.
    ListView,
    /// Progress bar bound to action progress updates.
    ProgressBar,
    /// Static image display rendering BMP, PNG, or JPEG from `Binary` table.
    Bitmap,
    /// Horizontal or vertical separator line.
    Line,
    /// Rich text (RTF) or plain-text scrollable viewer for license agreements.
    ScrollableText,
    /// Disk space breakdown table per volume.
    VolumeCostList,
    /// Hierarchical feature tree control for feature selection.
    SelectionTree,
}

impl ControlType {
    /// Parses a control type string from the `Control` table.
    ///
    /// # Arguments
    ///
    /// * `s` - Raw type name string.
    ///
    /// # Returns
    ///
    /// Parsed [`ControlType`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if the type name is unrecognized.
    pub fn from_name(s: &str) -> Result<Self> {
        match s {
            "PushButton" => Ok(Self::PushButton),
            "RadioButtonGroup" => Ok(Self::RadioButtonGroup),
            "CheckBox" => Ok(Self::CheckBox),
            "Edit" => Ok(Self::Edit),
            "Text" => Ok(Self::Text),
            "ComboBox" => Ok(Self::ComboBox),
            "ListBox" => Ok(Self::ListBox),
            "ListView" => Ok(Self::ListView),
            "ProgressBar" => Ok(Self::ProgressBar),
            "Bitmap" => Ok(Self::Bitmap),
            "Line" => Ok(Self::Line),
            "ScrollableText" => Ok(Self::ScrollableText),
            "VolumeCostList" => Ok(Self::VolumeCostList),
            "SelectionTree" => Ok(Self::SelectionTree),
            other => Err(Error::InvalidArgument {
                argument: "Control.Type".to_string(),
                reason: format!("Unknown MSI control type '{other}'"),
            }),
        }
    }

    /// Returns the standard MSI table type string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PushButton => "PushButton",
            Self::RadioButtonGroup => "RadioButtonGroup",
            Self::CheckBox => "CheckBox",
            Self::Edit => "Edit",
            Self::Text => "Text",
            Self::ComboBox => "ComboBox",
            Self::ListBox => "ListBox",
            Self::ListView => "ListView",
            Self::ProgressBar => "ProgressBar",
            Self::Bitmap => "Bitmap",
            Self::Line => "Line",
            Self::ScrollableText => "ScrollableText",
            Self::VolumeCostList => "VolumeCostList",
            Self::SelectionTree => "SelectionTree",
        }
    }
}

/// Generic item entry in a combo box, list box, or list view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    /// Bound property value or unique item value.
    pub value: String,
    /// Localized display label text.
    pub text: String,
    /// Sorting order index.
    pub order: i16,
    /// Optional binary icon identifier.
    pub icon_key: Option<String>,
}

/// Disk volume cost entry for `VolumeCostList` controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeCostEntry {
    /// Volume identifier or mount point (e.g. `C:`).
    pub volume: String,
    /// Drive type description (e.g. "Fixed Disk", "Network Drive").
    pub drive_type: String,
    /// Total volume size in bytes.
    pub volume_size: u64,
    /// Required installation disk space in bytes.
    pub required_space: u64,
    /// Remaining available free space in bytes.
    pub free_space: u64,
}

/// Feature node in a hierarchical `SelectionTree` control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionTreeNode {
    /// Unique feature identifier name.
    pub feature: String,
    /// Feature title text.
    pub title: String,
    /// Optional feature description text.
    pub description: Option<String>,
    /// Required disk space for this feature in bytes.
    pub size_bytes: u64,
    /// Currently configured installation target state.
    pub install_state: InstallState,
    /// Child feature nodes.
    pub children: Vec<Self>,
}

impl SelectionTreeNode {
    /// Creates a new [`SelectionTreeNode`].
    ///
    /// # Arguments
    ///
    /// * `feature` - Feature identifier.
    /// * `title` - Display title.
    /// * `size_bytes` - Required space in bytes.
    ///
    /// # Returns
    ///
    /// A new [`SelectionTreeNode`].
    #[must_use]
    pub fn new(feature: impl Into<String>, title: impl Into<String>, size_bytes: u64) -> Self {
        Self {
            feature: feature.into(),
            title: title.into(),
            description: None,
            size_bytes,
            install_state: InstallState::Local,
            children: Vec::new(),
        }
    }

    /// Sets the description text.
    #[must_use]
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Adds a child feature node.
    pub fn add_child(&mut self, child: Self) {
        self.children.push(child);
    }
}

/// Static definition of a control loaded from the `Control` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlDefinition {
    /// Enclosing dialog identifier.
    dialog: String,
    /// Control identifier name.
    control: String,
    /// Control specification type.
    control_type: ControlType,
    /// Geometry rectangle in dialog units.
    rect: DluRect,
    /// Attributes bitmask.
    attributes: u32,
    /// Optional bound property name.
    property: Option<String>,
    /// Optional text or formatted template string.
    text: Option<String>,
    /// Tab order next control identifier.
    control_next: Option<String>,
    /// Optional tooltip or help text.
    help: Option<String>,
}

impl ControlDefinition {
    /// Creates a new [`ControlDefinition`].
    ///
    /// # Arguments
    ///
    /// * `dialog` - Enclosing dialog name.
    /// * `control` - Control name.
    /// * `control_type` - Control type.
    /// * `rect` - Position and size in DLUs.
    /// * `attributes` - Raw attributes bitmask.
    ///
    /// # Returns
    ///
    /// A new [`ControlDefinition`].
    #[must_use]
    pub fn new(
        dialog: impl Into<String>,
        control: impl Into<String>,
        control_type: ControlType,
        rect: DluRect,
        attributes: u32,
    ) -> Self {
        Self {
            dialog: dialog.into(),
            control: control.into(),
            control_type,
            rect,
            attributes,
            property: None,
            text: None,
            control_next: None,
            help: None,
        }
    }

    /// Sets the bound property name.
    #[must_use]
    pub fn property(mut self, prop: impl Into<String>) -> Self {
        self.property = Some(prop.into());
        self
    }

    /// Sets the text or formatted template.
    #[must_use]
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Sets the next control identifier in tab order.
    #[must_use]
    pub fn control_next(mut self, next: impl Into<String>) -> Self {
        self.control_next = Some(next.into());
        self
    }

    /// Sets the help/tooltip text.
    #[must_use]
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Returns the enclosing dialog name.
    #[must_use]
    pub fn dialog(&self) -> &str {
        &self.dialog
    }

    /// Returns the control name.
    #[must_use]
    pub fn control(&self) -> &str {
        &self.control
    }

    /// Returns the control type.
    #[must_use]
    pub const fn control_type(&self) -> ControlType {
        self.control_type
    }

    /// Returns the geometry rectangle in DLUs.
    #[must_use]
    pub const fn rect(&self) -> DluRect {
        self.rect
    }

    /// Returns the raw attributes bitmask.
    #[must_use]
    pub const fn attributes(&self) -> u32 {
        self.attributes
    }

    /// Returns the bound property name if any.
    #[must_use]
    pub fn property_name(&self) -> Option<&str> {
        self.property.as_deref()
    }

    /// Returns the template text if any.
    #[must_use]
    pub fn text_template(&self) -> Option<&str> {
        self.text.as_deref()
    }

    /// Returns the next control in tab order if any.
    #[must_use]
    pub fn next_control(&self) -> Option<&str> {
        self.control_next.as_deref()
    }

    /// Returns the help text if any.
    #[must_use]
    pub fn help_text(&self) -> Option<&str> {
        self.help.as_deref()
    }

    /// Returns true if initially visible.
    #[must_use]
    pub const fn is_visible_by_default(&self) -> bool {
        self.attributes & CONTROL_ATTR_VISIBLE != 0
    }

    /// Returns true if initially enabled.
    #[must_use]
    pub const fn is_enabled_by_default(&self) -> bool {
        self.attributes & CONTROL_ATTR_ENABLED != 0
    }

    /// Returns true if password input masking is configured.
    #[must_use]
    pub const fn is_password_input(&self) -> bool {
        self.attributes & CONTROL_ATTR_PASSWORD_INPUT != 0
    }

    /// Returns true if integer-only digits restriction is enabled.
    #[must_use]
    pub const fn is_integer_only(&self) -> bool {
        self.attributes & CONTROL_ATTR_INTEGER != 0
    }
}

/// Dynamic runtime state of a control within an active UI session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlRuntimeState {
    /// Rendered formatted text label.
    pub current_text: String,
    /// Bound property value if applicable.
    pub bound_value: Option<String>,
    /// Whether control is currently visible.
    pub is_visible: bool,
    /// Whether control is currently enabled / interactive.
    pub is_enabled: bool,
    /// Whether this control is the default push button for the dialog.
    pub is_default: bool,
    /// Progress completion percentage (0..=100) for `ProgressBar`.
    pub progress_percent: u32,
    /// Available list items for `ComboBox`, `ListBox`, or `ListView`.
    pub list_items: Vec<ListItem>,
    /// Hierarchical tree nodes for `SelectionTree`.
    pub tree_nodes: Vec<SelectionTreeNode>,
    /// Volume entries for `VolumeCostList`.
    pub volume_entries: Vec<VolumeCostEntry>,
}

impl ControlRuntimeState {
    /// Creates a new [`ControlRuntimeState`] initialized from a [`ControlDefinition`].
    ///
    /// # Arguments
    ///
    /// * `def` - Base [`ControlDefinition`].
    ///
    /// # Returns
    ///
    /// An initialized [`ControlRuntimeState`].
    #[must_use]
    pub fn from_definition(def: &ControlDefinition) -> Self {
        Self {
            current_text: def.text_template().unwrap_or("").to_string(),
            bound_value: None,
            is_visible: def.is_visible_by_default(),
            is_enabled: def.is_enabled_by_default(),
            is_default: false,
            progress_percent: 0,
            list_items: Vec::new(),
            tree_nodes: Vec::new(),
            volume_entries: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests parsing all control type names.
    #[test]
    fn test_control_type_parsing() {
        let types = [
            ("PushButton", ControlType::PushButton),
            ("RadioButtonGroup", ControlType::RadioButtonGroup),
            ("CheckBox", ControlType::CheckBox),
            ("Edit", ControlType::Edit),
            ("Text", ControlType::Text),
            ("ComboBox", ControlType::ComboBox),
            ("ListBox", ControlType::ListBox),
            ("ListView", ControlType::ListView),
            ("ProgressBar", ControlType::ProgressBar),
            ("Bitmap", ControlType::Bitmap),
            ("Line", ControlType::Line),
            ("ScrollableText", ControlType::ScrollableText),
            ("VolumeCostList", ControlType::VolumeCostList),
            ("SelectionTree", ControlType::SelectionTree),
        ];

        for (name, expected) in types {
            let parsed = ControlType::from_name(name);
            assert_eq!(parsed, Ok(expected));
            assert_eq!(expected.as_str(), name);
        }

        assert!(ControlType::from_name("InvalidControl").is_err());
    }

    /// Tests control definition attributes and runtime state creation.
    #[test]
    fn test_control_definition_and_runtime_state() {
        let rect = DluRect::new(10, 20, 100, 30);
        let def = ControlDefinition::new(
            "InstallDlg",
            "NextBtn",
            ControlType::PushButton,
            rect,
            CONTROL_ATTR_VISIBLE
                | CONTROL_ATTR_ENABLED
                | CONTROL_ATTR_PASSWORD_INPUT
                | CONTROL_ATTR_INTEGER,
        )
        .property("MY_PROP")
        .text("Next >")
        .control_next("CancelBtn")
        .help("Click to proceed");

        assert_eq!(def.dialog(), "InstallDlg");
        assert_eq!(def.control(), "NextBtn");
        assert_eq!(def.control_type(), ControlType::PushButton);
        assert_eq!(def.rect(), rect);
        assert_eq!(def.property_name(), Some("MY_PROP"));
        assert_eq!(def.text_template(), Some("Next >"));
        assert_eq!(def.next_control(), Some("CancelBtn"));
        assert_eq!(def.help_text(), Some("Click to proceed"));
        assert_eq!(
            def.attributes(),
            CONTROL_ATTR_VISIBLE
                | CONTROL_ATTR_ENABLED
                | CONTROL_ATTR_PASSWORD_INPUT
                | CONTROL_ATTR_INTEGER
        );

        assert!(def.is_visible_by_default());
        assert!(def.is_enabled_by_default());
        assert!(def.is_password_input());
        assert!(def.is_integer_only());

        let state = ControlRuntimeState::from_definition(&def);
        assert_eq!(state.current_text, "Next >");
        assert!(state.is_visible);
        assert!(state.is_enabled);
        assert!(!state.is_default);
        assert_eq!(state.progress_percent, 0);
    }

    /// Tests `SelectionTreeNode` creation and hierarchy.
    #[test]
    fn test_selection_tree_node() {
        let mut root = SelectionTreeNode::new("MainFeature", "Main Application", 50_000_000)
            .description("Core program binaries");
        assert_eq!(root.feature, "MainFeature");
        assert_eq!(root.title, "Main Application");
        assert_eq!(root.size_bytes, 50_000_000);
        assert_eq!(root.description.as_deref(), Some("Core program binaries"));
        assert_eq!(root.install_state, InstallState::Local);

        let child = SelectionTreeNode::new("Docs", "Documentation", 5_000_000);
        root.add_child(child);
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].feature, "Docs");
    }

    /// Tests `VolumeCostEntry` and `ListItem`.
    #[test]
    fn test_volume_cost_and_list_items() {
        let item = ListItem {
            value: "1".to_string(),
            text: "Option One".to_string(),
            order: 1,
            icon_key: Some("Icon1".to_string()),
        };
        assert_eq!(item.value, "1");
        assert_eq!(item.order, 1);

        let vol = VolumeCostEntry {
            volume: r"C:\".to_string(),
            drive_type: "Fixed Disk".to_string(),
            volume_size: 500_000_000_000,
            required_space: 100_000_000,
            free_space: 200_000_000_000,
        };
        assert_eq!(vol.volume, r"C:\");
        assert_eq!(vol.drive_type, "Fixed Disk");
        assert_eq!(vol.required_space, 100_000_000);
    }
}
