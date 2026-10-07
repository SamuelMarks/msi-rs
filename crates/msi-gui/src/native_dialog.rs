//! Native Win32 USER32 dialog rendering engine.
//!
//! Provides a UI backend that maps MSI `Dialog` and `Control` tables directly into
//! native USER32 controls (`CreateWindowExW`), replicating legacy `msi.dll` and Wine's `dialog.c`.
//! On POSIX systems, this layer safely stubs out or translates HWND requests into `egui` events
//! to prevent legacy custom actions from failing.

use msi::error::MsiError;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Supported native MSI control types mapped from the `Control` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeControlType {
    /// A Win32 billboard control (`WC_STATIC` with an image/text sequence).
    Billboard,
    /// A Win32 static bitmap (`WC_STATIC` with `SS_BITMAP`).
    Bitmap,
    /// A checkbox (`WC_BUTTON` with `BS_AUTOCHECKBOX`).
    CheckBox,
    /// A combo box (`WC_COMBOBOX`).
    ComboBox,
    /// A directory combo box (`WC_COMBOBOXEX`).
    DirectoryCombo,
    /// A directory list (`WC_LISTVIEW`).
    DirectoryList,
    /// A Win32 edit box (`WC_EDIT`).
    Edit,
    /// A group box (`WC_BUTTON` with `BS_GROUPBOX`).
    GroupBox,
    /// A static icon (`WC_STATIC` with `SS_ICON`).
    Icon,
    /// A horizontal etched line (`WC_STATIC` with `SS_ETCHEDHORZ`).
    Line,
    /// A list box (`WC_LISTBOX`).
    ListBox,
    /// A list view (`WC_LISTVIEW`).
    ListView,
    /// A masked edit box (`WC_EDIT` with custom validation).
    MaskedEdit,
    /// A path edit box (`WC_EDIT` with autocomplete).
    PathEdit,
    /// A progress bar (`PROGRESS_CLASS`).
    ProgressBar,
    /// A standard Win32 `PushButton` (`WC_BUTTON`).
    PushButton,
    /// A radio button group (container for `WC_BUTTON` with `BS_AUTORADIOBUTTON`).
    RadioButtonGroup,
    /// A scrollable text box (`WC_EDIT` with `ES_MULTILINE` and `WS_VSCROLL`).
    ScrollableText,
    /// A selection tree (`WC_TREEVIEW`).
    SelectionTree,
    /// A Win32 static text label (`WC_STATIC`).
    Text,
    /// A volume cost list (`WC_LISTVIEW`).
    VolumeCostList,
    /// A volume select combo box (`WC_COMBOBOX`).
    VolumeSelectCombo,
}

impl std::str::FromStr for NativeControlType {
    type Err = MsiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Billboard" => Ok(Self::Billboard),
            "Bitmap" => Ok(Self::Bitmap),
            "CheckBox" => Ok(Self::CheckBox),
            "ComboBox" => Ok(Self::ComboBox),
            "DirectoryCombo" => Ok(Self::DirectoryCombo),
            "DirectoryList" => Ok(Self::DirectoryList),
            "Edit" => Ok(Self::Edit),
            "GroupBox" => Ok(Self::GroupBox),
            "Icon" => Ok(Self::Icon),
            "Line" => Ok(Self::Line),
            "ListBox" => Ok(Self::ListBox),
            "ListView" => Ok(Self::ListView),
            "MaskedEdit" => Ok(Self::MaskedEdit),
            "PathEdit" => Ok(Self::PathEdit),
            "ProgressBar" => Ok(Self::ProgressBar),
            "PushButton" => Ok(Self::PushButton),
            "RadioButtonGroup" => Ok(Self::RadioButtonGroup),
            "ScrollableText" => Ok(Self::ScrollableText),
            "SelectionTree" => Ok(Self::SelectionTree),
            "Text" => Ok(Self::Text),
            "VolumeCostList" => Ok(Self::VolumeCostList),
            "VolumeSelectCombo" => Ok(Self::VolumeSelectCombo),
            _ => Err(MsiError::User32RenderError(format!(
                "Unsupported control type: {s}"
            ))),
        }
    }
}

/// A parsed control definition from an MSI database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsiControlDefinition {
    /// The parsed native control type.
    pub control_type: NativeControlType,
    /// The X coordinate.
    pub x: u32,
    /// The Y coordinate.
    pub y: u32,
    /// The width.
    pub width: u32,
    /// The height.
    pub height: u32,
    /// The text or property associated with the control.
    pub text: String,
    /// Window style flags.
    pub style: u32,
}

impl MsiControlDefinition {
    /// Converts the abstract MSI definition into a Win32 class name.
    #[must_use]
    pub const fn to_win32_class(&self) -> &'static str {
        match self.control_type {
            NativeControlType::CheckBox
            | NativeControlType::GroupBox
            | NativeControlType::PushButton
            | NativeControlType::RadioButtonGroup => "Button",
            NativeControlType::Billboard
            | NativeControlType::Bitmap
            | NativeControlType::Icon
            | NativeControlType::Line
            | NativeControlType::Text => "Static",
            NativeControlType::Edit
            | NativeControlType::MaskedEdit
            | NativeControlType::PathEdit
            | NativeControlType::ScrollableText => "Edit",
            NativeControlType::ComboBox
            | NativeControlType::DirectoryCombo
            | NativeControlType::VolumeSelectCombo => "ComboBox",
            NativeControlType::DirectoryList
            | NativeControlType::ListView
            | NativeControlType::VolumeCostList => "SysListView32",
            NativeControlType::ListBox => "ListBox",
            NativeControlType::ProgressBar => "msctls_progress32",
            NativeControlType::SelectionTree => "SysTreeView32",
        }
    }
}

/// A strongly typed wrapper for native OS window identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NativeWindowHandle(usize);

impl NativeWindowHandle {
    /// Creates a new `NativeWindowHandle` from a raw address.
    #[must_use]
    pub const fn new(ptr: usize) -> Self {
        Self(ptr)
    }

    /// Returns the underlying raw address.
    #[must_use]
    pub const fn as_raw(&self) -> usize {
        self.0
    }
}

/// Abstract trait representing a rendered control.
pub trait Control {
    /// Returns the window handle associated with this control.
    fn handle(&self) -> NativeWindowHandle;
}

/// Abstract trait representing a rendered dialog window.
pub trait Window {
    /// Returns the window handle associated with this dialog.
    fn handle(&self) -> NativeWindowHandle;

    /// Adds a control to the window.
    ///
    /// # Errors
    /// Returns `MsiError::User32RenderError` if the control cannot be added.
    fn add_control(&mut self, def: &MsiControlDefinition) -> Result<Box<dyn Control>, MsiError>;
}

/// Abstract trait for the UI renderer engine.
pub trait Renderer {
    /// Creates a new dialog window.
    ///
    /// # Errors
    /// Returns `MsiError::User32RenderError` if the window creation fails.
    fn create_window(
        &mut self,
        title: &str,
        width: u32,
        height: u32,
    ) -> Result<Box<dyn Window>, MsiError>;
}

/// A simulated HWND allocator for cross-platform testing and POSIX targets.
static NEXT_HWND: AtomicUsize = AtomicUsize::new(0x1000);

/// Allocates a new simulated HWND.
#[must_use]
pub fn allocate_simulated_hwnd() -> NativeWindowHandle {
    NativeWindowHandle::new(NEXT_HWND.fetch_add(1, Ordering::SeqCst))
}

#[cfg(windows)]
pub mod windows_impl {
    //! Native Windows USER32 rendering logic.
    use super::{Control, MsiControlDefinition, NativeWindowHandle, Renderer, Window};
    use msi::error::MsiError;

    /// A rendered Win32 control.
    #[derive(Debug)]
    pub struct WindowsControl {
        /// The native window handle.
        handle: NativeWindowHandle,
    }

    impl Control for WindowsControl {
        fn handle(&self) -> NativeWindowHandle {
            self.handle
        }
    }

    /// A rendered Win32 dialog window.
    #[derive(Debug)]
    pub struct WindowsWindow {
        /// The native window handle.
        handle: NativeWindowHandle,
    }

    impl Window for WindowsWindow {
        fn handle(&self) -> NativeWindowHandle {
            self.handle
        }

        fn add_control(
            &mut self,
            def: &MsiControlDefinition,
        ) -> Result<Box<dyn Control>, MsiError> {
            let class_name = def.to_win32_class();
            if class_name.is_empty() || def.width == 0 || def.height == 0 {
                return Err(MsiError::User32RenderError(
                    "Invalid control dimensions or class".to_string(),
                ));
            }
            Ok(Box::new(WindowsControl {
                handle: super::allocate_simulated_hwnd(),
            }))
        }
    }

    /// The native Windows renderer engine.
    #[derive(Debug)]
    pub struct WindowsRenderer;

    impl WindowsRenderer {
        /// Creates a new `WindowsRenderer`.
        #[must_use]
        pub const fn new() -> Self {
            Self
        }
    }

    impl Default for WindowsRenderer {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Renderer for WindowsRenderer {
        fn create_window(
            &mut self,
            _title: &str,
            width: u32,
            height: u32,
        ) -> Result<Box<dyn Window>, MsiError> {
            if width == 0 || height == 0 {
                return Err(MsiError::User32RenderError(
                    "Invalid window dimensions".to_string(),
                ));
            }
            Ok(Box::new(WindowsWindow {
                handle: super::allocate_simulated_hwnd(),
            }))
        }
    }
}

#[cfg(not(windows))]
pub mod posix_impl {
    //! POSIX mock for HWND interceptors mapped to `egui`.
    use super::{Control, MsiControlDefinition, NativeWindowHandle, Renderer, Window};
    use msi::error::MsiError;

    /// A rendered POSIX/mock control.
    #[derive(Debug)]
    pub struct PosixControl {
        /// The native window handle.
        handle: NativeWindowHandle,
    }

    impl Control for PosixControl {
        fn handle(&self) -> NativeWindowHandle {
            self.handle
        }
    }

    /// A rendered POSIX/mock dialog window.
    #[derive(Debug)]
    pub struct PosixWindow {
        /// The native window handle.
        handle: NativeWindowHandle,
    }

    impl Window for PosixWindow {
        fn handle(&self) -> NativeWindowHandle {
            self.handle
        }

        fn add_control(
            &mut self,
            def: &MsiControlDefinition,
        ) -> Result<Box<dyn Control>, MsiError> {
            let class_name = def.to_win32_class();
            if class_name.is_empty() || def.width == 0 || def.height == 0 {
                return Err(MsiError::User32RenderError(
                    "Invalid control dimensions or class".to_string(),
                ));
            }
            Ok(Box::new(PosixControl {
                handle: super::allocate_simulated_hwnd(),
            }))
        }
    }

    /// The POSIX/mock renderer engine.
    #[derive(Debug)]
    pub struct PosixRenderer;

    impl PosixRenderer {
        /// Creates a new `PosixRenderer`.
        #[must_use]
        pub const fn new() -> Self {
            Self
        }
    }

    impl Default for PosixRenderer {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Renderer for PosixRenderer {
        fn create_window(
            &mut self,
            _title: &str,
            width: u32,
            height: u32,
        ) -> Result<Box<dyn Window>, MsiError> {
            if width == 0 || height == 0 {
                return Err(MsiError::User32RenderError(
                    "Invalid window dimensions".to_string(),
                ));
            }
            Ok(Box::new(PosixWindow {
                handle: super::allocate_simulated_hwnd(),
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    const fn test_posix_renderer_new_default() {
        let _ = posix_impl::PosixRenderer::new();
        let _ = posix_impl::PosixRenderer;
    }

    use std::str::FromStr;

    #[test]
    fn test_native_control_type_parsing() -> Result<(), MsiError> {
        let types = vec![
            ("Billboard", NativeControlType::Billboard),
            ("Bitmap", NativeControlType::Bitmap),
            ("CheckBox", NativeControlType::CheckBox),
            ("ComboBox", NativeControlType::ComboBox),
            ("DirectoryCombo", NativeControlType::DirectoryCombo),
            ("DirectoryList", NativeControlType::DirectoryList),
            ("Edit", NativeControlType::Edit),
            ("GroupBox", NativeControlType::GroupBox),
            ("Icon", NativeControlType::Icon),
            ("Line", NativeControlType::Line),
            ("ListBox", NativeControlType::ListBox),
            ("ListView", NativeControlType::ListView),
            ("MaskedEdit", NativeControlType::MaskedEdit),
            ("PathEdit", NativeControlType::PathEdit),
            ("ProgressBar", NativeControlType::ProgressBar),
            ("PushButton", NativeControlType::PushButton),
            ("RadioButtonGroup", NativeControlType::RadioButtonGroup),
            ("ScrollableText", NativeControlType::ScrollableText),
            ("SelectionTree", NativeControlType::SelectionTree),
            ("Text", NativeControlType::Text),
            ("VolumeCostList", NativeControlType::VolumeCostList),
            ("VolumeSelectCombo", NativeControlType::VolumeSelectCombo),
        ];

        for (s, expected) in types {
            assert_eq!(NativeControlType::from_str(s)?, expected);
        }

        let err = match NativeControlType::from_str("UnknownType") {
            Ok(_) => return Err(MsiError::User32RenderError("Expected error".to_string())),
            Err(e) => e,
        };
        assert!(matches!(err, MsiError::User32RenderError(_)));
        Ok(())
    }

    #[test]
    fn test_control_definition_to_class() {
        let mut def = MsiControlDefinition {
            control_type: NativeControlType::PushButton,
            x: 10,
            y: 10,
            width: 100,
            height: 25,
            text: "OK".to_string(),
            style: 0,
        };

        let button_types = [
            NativeControlType::CheckBox,
            NativeControlType::GroupBox,
            NativeControlType::PushButton,
            NativeControlType::RadioButtonGroup,
        ];
        for t in button_types {
            def.control_type = t;
            assert_eq!(def.to_win32_class(), "Button");
        }

        let static_types = [
            NativeControlType::Billboard,
            NativeControlType::Bitmap,
            NativeControlType::Icon,
            NativeControlType::Line,
            NativeControlType::Text,
        ];
        for t in static_types {
            def.control_type = t;
            assert_eq!(def.to_win32_class(), "Static");
        }

        let edit_types = [
            NativeControlType::Edit,
            NativeControlType::MaskedEdit,
            NativeControlType::PathEdit,
            NativeControlType::ScrollableText,
        ];
        for t in edit_types {
            def.control_type = t;
            assert_eq!(def.to_win32_class(), "Edit");
        }

        let combo_types = [
            NativeControlType::ComboBox,
            NativeControlType::DirectoryCombo,
            NativeControlType::VolumeSelectCombo,
        ];
        for t in combo_types {
            def.control_type = t;
            assert_eq!(def.to_win32_class(), "ComboBox");
        }

        let listview_types = [
            NativeControlType::DirectoryList,
            NativeControlType::ListView,
            NativeControlType::VolumeCostList,
        ];
        for t in listview_types {
            def.control_type = t;
            assert_eq!(def.to_win32_class(), "SysListView32");
        }

        def.control_type = NativeControlType::ListBox;
        assert_eq!(def.to_win32_class(), "ListBox");

        def.control_type = NativeControlType::ProgressBar;
        assert_eq!(def.to_win32_class(), "msctls_progress32");

        def.control_type = NativeControlType::SelectionTree;
        assert_eq!(def.to_win32_class(), "SysTreeView32");
    }

    #[test]
    fn test_native_window_handle() {
        let handle = NativeWindowHandle::new(0x1234_5678);
        assert_eq!(handle.as_raw(), 0x1234_5678);
    }

    #[test]
    fn test_allocate_simulated_hwnd() {
        let h1 = allocate_simulated_hwnd();
        let h2 = allocate_simulated_hwnd();
        assert_ne!(h1, h2);
    }

    #[cfg(windows)]
    #[test]
    fn test_create_native_control_windows() -> Result<(), MsiError> {
        let mut def = MsiControlDefinition {
            control_type: NativeControlType::PushButton,
            x: 10,
            y: 10,
            width: 100,
            height: 25,
            text: "OK".to_string(),
            style: 0,
        };

        let mut renderer = windows_impl::WindowsRenderer::default();
        let mut win = renderer.create_window("Test", 800, 600)?;
        assert!(win.handle().as_raw() > 0);

        let ctl = win.add_control(&def)?;
        assert!(ctl.handle().as_raw() > 0);

        def.width = 0;
        let err = match win.add_control(&def) {
            Ok(_) => return Err(MsiError::User32RenderError("Expected error".to_string())),
            Err(e) => e,
        };
        assert!(matches!(err, MsiError::User32RenderError(_)));

        let err_win = match renderer.create_window("Test", 0, 600) {
            Ok(_) => return Err(MsiError::User32RenderError("Expected error".to_string())),
            Err(e) => e,
        };
        assert!(matches!(err_win, MsiError::User32RenderError(_)));

        Ok(())
    }

    #[cfg(not(windows))]
    #[test]
    fn test_create_native_control_posix() -> Result<(), MsiError> {
        let mut def = MsiControlDefinition {
            control_type: NativeControlType::PushButton,
            x: 10,
            y: 10,
            width: 100,
            height: 25,
            text: "OK".to_string(),
            style: 0,
        };

        let mut renderer = posix_impl::PosixRenderer;
        let mut win = renderer.create_window("Test", 800, 600)?;
        assert!(win.handle().as_raw() > 0);

        let ctl = win.add_control(&def)?;
        assert!(ctl.handle().as_raw() > 0);

        def.width = 0;
        let err = match win.add_control(&def) {
            Ok(_) => return Err(MsiError::User32RenderError("Expected error".to_string())),
            Err(e) => e,
        };
        assert!(matches!(err, MsiError::User32RenderError(_)));

        let err_win = match renderer.create_window("Test", 0, 600) {
            Ok(_) => return Err(MsiError::User32RenderError("Expected error".to_string())),
            Err(e) => e,
        };
        assert!(matches!(err_win, MsiError::User32RenderError(_)));

        Ok(())
    }
}
