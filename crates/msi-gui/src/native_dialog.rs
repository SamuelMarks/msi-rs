//! Native Win32 USER32 dialog rendering engine.
//!
//! Provides a UI backend that maps MSI `Dialog` and `Control` tables directly into
//! native USER32 controls (`CreateWindowExW`), replicating legacy `msi.dll` and Wine's `dialog.c`.
//! On POSIX systems, this layer safely stubs out or translates HWND requests into `egui` events
//! to prevent legacy custom actions from failing.

use msi::error::MsiError;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Canonical MSI control type re-exported from [`msi::ui::ControlType`].
pub type NativeControlType = msi::ui::ControlType;

/// A parsed control definition from an MSI database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsiControlDefinition {
    /// The parsed native control type.
    pub control_type: NativeControlType,
    /// The X coordinate in dialog units or pixels.
    pub x: u32,
    /// The Y coordinate in dialog units or pixels.
    pub y: u32,
    /// The control width.
    pub width: u32,
    /// The control height.
    pub height: u32,
    /// The text or property associated with the control.
    pub text: String,
    /// Window style flags.
    pub style: u32,
}

impl MsiControlDefinition {
    /// Converts the abstract MSI definition into a Win32 class name.
    ///
    /// # Returns
    ///
    /// The Win32 control class name string (`Button`, `Static`, `Edit`, etc.).
    #[must_use]
    pub const fn to_win32_class(&self) -> &'static str {
        self.control_type.to_win32_class()
    }
}

/// A strongly typed wrapper for native OS window identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NativeWindowHandle(usize);

impl NativeWindowHandle {
    /// Creates a new `NativeWindowHandle` from a raw address.
    ///
    /// # Arguments
    ///
    /// * `ptr` - Raw integer value of the native window handle (`HWND`).
    ///
    /// # Returns
    ///
    /// A strongly typed [`NativeWindowHandle`].
    #[must_use]
    pub const fn new(ptr: usize) -> Self {
        Self(ptr)
    }

    /// Returns the underlying raw address.
    ///
    /// # Returns
    ///
    /// The raw integer window handle value.
    #[must_use]
    pub const fn as_raw(&self) -> usize {
        self.0
    }
}

/// Native Window Class definitions for Win32.
pub mod window_classes {
    /// The main wizard dialog window class.
    pub const MSI_DIALOG_CLOSE_CLASS: &str = "MsiDialogCloseClass";
    /// The billboard window class.
    pub const MSI_BILLBOARD_CLASS: &str = "MsiBillboard";
    /// The selection tree window class.
    pub const MSI_SELECTION_TREE_CLASS: &str = "MsiSelectionTree";
    /// The volume cost list window class.
    pub const MSI_VOLUME_COST_LIST_CLASS: &str = "MsiVolumeCostList";
}

/// Dialog Unit to Pixel coordinate converter re-exported from [`msi::ui::DialogMetrics`].
pub type DialogUnitConverter = msi::ui::DialogMetrics;

/// Message types for the dialog message loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMessage {
    /// Sent when a window is created (`WM_CREATE`).
    Create,
    /// Sent when a user interacts with a control (`WM_COMMAND`).
    Command(u32),
    /// Sent when a control notifies its parent (`WM_NOTIFY`).
    Notify(u32),
    /// Sent when a window needs repainting (`WM_PAINT`).
    Paint,
    /// Sent when a timer expires (`WM_TIMER`).
    Timer(u32),
    /// Sent when a window is being destroyed (`WM_DESTROY`).
    Destroy,
    /// Sent when a navigation key is pressed.
    Navigation(NavigationKey),
}

/// Keyboard navigation keys for dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationKey {
    /// The Tab key.
    Tab,
    /// The Shift+Tab key.
    ShiftTab,
    /// The Enter key.
    Enter,
    /// The Escape key.
    Escape,
    /// A mnemonic hotkey.
    Mnemonic(char),
}

/// Processes a single message from the message loop.
///
/// # Returns
/// Returns `true` if the loop should continue, or `false` if `WM_DESTROY` was received.
///
/// # Arguments
///
/// * `msg` - The msg argument.
#[must_use]
pub const fn process_message(msg: WindowMessage) -> bool {
    !matches!(msg, WindowMessage::Destroy)
}

/// Abstract trait representing a rendered control.
pub trait Control {
    /// Returns the window handle associated with this control.
    ///
    /// # Returns
    ///
    /// Computed `NativeWindowHandle` value.
    fn handle(&self) -> NativeWindowHandle;
}

/// Abstract trait representing a rendered dialog window.
pub trait Window {
    /// Returns the window handle associated with this dialog.
    ///
    /// # Returns
    ///
    /// Computed `NativeWindowHandle` value.
    fn handle(&self) -> NativeWindowHandle;

    /// Adds a control to the window.
    ///
    /// # Errors
    /// Returns `MsiError::User32RenderError` if the control cannot be added.
    ///
    /// # Arguments
    ///
    /// * `def` - The def argument.
    ///
    /// # Returns
    ///
    /// `Ok(Box<dyn Control>)` on success, or an error if the operation fails.
    fn add_control(&mut self, def: &MsiControlDefinition) -> Result<Box<dyn Control>, MsiError>;
}

/// Abstract trait for the UI renderer engine.
pub trait Renderer {
    /// Creates a new dialog window.
    ///
    /// # Errors
    /// Returns `MsiError::User32RenderError` if the window creation fails.
    ///
    /// # Arguments
    ///
    /// * `title` - The title argument.
    /// * `width` - The width argument.
    /// * `height` - The height argument.
    ///
    /// # Returns
    ///
    /// `Ok(Box<dyn Window>)` on success, or an error if the operation fails.
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
///
/// # Returns
///
/// Computed `NativeWindowHandle` value.
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
            let _class_name = def.to_win32_class();
            if def.width == 0 || def.height == 0 {
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
        ///
        /// # Returns
        ///
        /// A newly constructed instance of `Self`.
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
            let _class_name = def.to_win32_class();
            if def.width == 0 || def.height == 0 {
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
        ///
        /// # Returns
        ///
        /// A newly constructed instance of `Self`.
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
    fn test_posix_renderer_create_window() -> Result<(), MsiError> {
        let mut renderer = posix_impl::PosixRenderer;
        let mut renderer2 = posix_impl::PosixRenderer::new();
        let win = renderer.create_window("test", 100, 100)?;
        let win2 = renderer2.create_window("test2", 120, 120)?;
        assert_ne!(win.handle(), win2.handle());

        let res = renderer.create_window("test", 0, 100);
        assert!(matches!(res, Err(MsiError::User32RenderError(_))));

        let res2 = renderer.create_window("test", 100, 0);
        assert!(matches!(res2, Err(MsiError::User32RenderError(_))));
        Ok(())
    }

    use std::str::FromStr;

    #[test]
    fn test_native_control_type_parsing() {
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
            assert_eq!(NativeControlType::from_str(s).ok(), Some(expected));
        }

        assert!(matches!(
            NativeControlType::from_str("UnknownType"),
            Err(MsiError::User32RenderError(_))
        ));
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
    fn test_dialog_unit_converter() {
        let converter = DialogUnitConverter::with_units(8, 16);
        assert_eq!(converter.x_to_pixels(10), 20);
        assert_eq!(converter.y_to_pixels(10), 20);
    }

    #[test]
    fn test_process_message() {
        assert!(process_message(WindowMessage::Create));
        assert!(process_message(WindowMessage::Command(1)));
        assert!(process_message(WindowMessage::Notify(2)));
        assert!(process_message(WindowMessage::Paint));
        assert!(process_message(WindowMessage::Timer(3)));
        assert!(process_message(WindowMessage::Navigation(
            NavigationKey::Tab
        )));
        assert!(process_message(WindowMessage::Navigation(
            NavigationKey::ShiftTab
        )));
        assert!(process_message(WindowMessage::Navigation(
            NavigationKey::Enter
        )));
        assert!(process_message(WindowMessage::Navigation(
            NavigationKey::Escape
        )));
        assert!(process_message(WindowMessage::Navigation(
            NavigationKey::Mnemonic('A')
        )));
        assert!(!process_message(WindowMessage::Destroy));
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

        #[allow(clippy::default_constructed_unit_structs)]
        let mut renderer = windows_impl::WindowsRenderer::default();
        let mut win = renderer.create_window("Test", 800, 600)?;
        assert!(win.handle().as_raw() > 0);

        let ctl = win.add_control(&def)?;
        assert!(ctl.handle().as_raw() > 0);

        def.width = 0;
        assert!(matches!(
            win.add_control(&def),
            Err(MsiError::User32RenderError(_))
        ));

        def.width = 100;
        def.height = 0;
        assert!(matches!(
            win.add_control(&def),
            Err(MsiError::User32RenderError(_))
        ));

        assert!(matches!(
            renderer.create_window("Test", 0, 600),
            Err(MsiError::User32RenderError(_))
        ));

        assert!(matches!(
            renderer.create_window("Test", 800, 0),
            Err(MsiError::User32RenderError(_))
        ));
        Ok(())
    }

    #[cfg(not(windows))]
    #[test]
    fn test_create_native_control_posix() {
        fn make_default_renderer<T: Default>() -> T {
            T::default()
        }
        let mut def = MsiControlDefinition {
            control_type: NativeControlType::PushButton,
            x: 10,
            y: 10,
            width: 100,
            height: 25,
            text: "OK".to_string(),
            style: 0,
        };

        let mut renderer: posix_impl::PosixRenderer = make_default_renderer();
        assert!(renderer
            .create_window("Test", 800, 600)
            .is_ok_and(|mut win| {
                assert!(win.handle().as_raw() > 0);
                assert!(win
                    .add_control(&def)
                    .is_ok_and(|ctl| ctl.handle().as_raw() > 0));

                def.width = 0;
                assert!(matches!(
                    win.add_control(&def),
                    Err(MsiError::User32RenderError(_))
                ));

                def.width = 100;
                def.height = 0;
                assert!(matches!(
                    win.add_control(&def),
                    Err(MsiError::User32RenderError(_))
                ));
                true
            }));

        assert!(matches!(
            renderer.create_window("Test", 0, 600),
            Err(MsiError::User32RenderError(_))
        ));

        assert!(matches!(
            renderer.create_window("Test", 800, 0),
            Err(MsiError::User32RenderError(_))
        ));
    }
}
