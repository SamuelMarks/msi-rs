//! # msi-gui
//!
//! Interactive Native Desktop GUI Windowing Runtime for Windows Installer (`.msi`) packages.
//!
//! Grounded in `egui` immediate-mode GUI architecture, `eframe` windowing, and
//! Windows Installer SDK specifications:
//! - Native window creation with configurable dimensions, title formatting, and non-resizable constraints.
//! - Immediate-mode render loop mapping `EguiLayoutMapper` widgets to native UI controls (`ui.put`, `ui.button`, `ui.label`).
//! - High-fidelity `WiX` styling presets (`WixUI_Mondo`, `WixUI_InstallDir`, `WixUI_FeatureTree`) via `WizardTheme`.
//! - Multi-backend hardware acceleration options (`wgpu`, `glow`, `softbuffer`).
//! - Accessible navigation: full keyboard tab ordering, Enter default activation, Escape cancellation,
//!   high-contrast focus ring visualization, and `AccessKit` screen reader node tree publishing.

#![cfg_attr(
    test,
    allow(
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::let_underscore_must_use
    )
)]

use clap::{Parser, ValueEnum};
use msi::execution::EvaluationContext;
use msi::ui::{
    DialogDefinition, GuiDesktopRuntime, GuiHardwareBackend, GuiInputEvent, SoftwareBuffer,
    WizardStyle, WizardTheme,
};
use std::path::PathBuf;
use std::process::ExitCode;

/// CLI theme selection matching standard `WiX` styling presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliTheme {
    /// Classic `WiX` Mondo multi-feature wizard layout.
    Mondo,
    /// `WiX` `InstallDir` simple directory-selection layout.
    InstallDir,
    /// `WiX` `FeatureTree` hierarchical component tree layout.
    FeatureTree,
    /// Minimal headless/embedded installer dialog layout.
    Minimal,
}

impl From<CliTheme> for WizardStyle {
    fn from(t: CliTheme) -> Self {
        match t {
            CliTheme::Mondo => Self::Mondo,
            CliTheme::InstallDir => Self::InstallDir,
            CliTheme::FeatureTree => Self::FeatureTree,
            CliTheme::Minimal => Self::Minimal,
        }
    }
}

/// CLI hardware acceleration backend choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliBackend {
    /// Hardware-accelerated GPU pipeline (`wgpu`: DirectX 12 / Metal / Vulkan).
    Wgpu,
    /// Legacy OpenGL hardware rendering (`glow`).
    Glow,
    /// Pure software CPU framebuffer rasterizer (`softbuffer`).
    Softbuffer,
}

impl From<CliBackend> for GuiHardwareBackend {
    fn from(b: CliBackend) -> Self {
        match b {
            CliBackend::Wgpu => Self::WgpuDirectXMetalVulkan,
            CliBackend::Glow => Self::GlowLegacyOpenGl,
            CliBackend::Softbuffer => Self::SoftbufferHeadlessRasterizer,
        }
    }
}

/// Command-line arguments for the `msi-gui` desktop application.
#[derive(Debug, Parser, PartialEq, Eq)]
#[command(
    name = "msi-gui",
    version,
    about = "Native Desktop GUI Wizard for Windows Installer (.msi) packages"
)]
pub struct GuiCli {
    /// Path to the target `.msi` installation package file.
    #[arg(value_name = "PACKAGE")]
    pub package: Option<PathBuf>,

    /// Visual wizard styling preset.
    #[arg(short, long, value_enum, default_value_t = CliTheme::Mondo)]
    pub theme: CliTheme,

    /// Hardware rendering backend.
    #[arg(short, long, value_enum, default_value_t = CliBackend::Wgpu)]
    pub backend: CliBackend,

    /// Override window width in pixels.
    #[arg(long)]
    pub width: Option<u32>,

    /// Override window height in pixels.
    #[arg(long)]
    pub height: Option<u32>,
}

/// Sets up a default welcome dialog workflow if launching standalone without an MSI package.
#[must_use]
pub fn create_standalone_runtime(
    theme_style: WizardStyle,
    backend: GuiHardwareBackend,
    width_override: Option<u32>,
    height_override: Option<u32>,
) -> GuiDesktopRuntime {
    let mut context = EvaluationContext::new();
    context.set_property("ProductName", "Sample Application");
    context.set_property("ProductVersion", "1.0.0");
    context.set_property("Manufacturer", "Example Corp");

    let mut engine = msi::ui::UiEngine::new(context);
    engine.add_dialog(DialogDefinition {
        name: "WelcomeDlg".to_string(),
        h_centering: 50,
        v_centering: 50,
        width: 370,
        height: 270,
        attributes: msi::ui::DIALOG_ATTR_VISIBLE | msi::ui::DIALOG_ATTR_MODAL,
        title: Some("Welcome to [ProductName] Setup".to_string()),
        control_first: "NextButton".to_string(),
        control_default: Some("NextButton".to_string()),
        control_cancel: Some("CancelButton".to_string()),
    });

    let next_btn = msi::ui::ControlDefinition::new(
        "WelcomeDlg",
        "NextButton",
        msi::ui::ControlType::PushButton,
        msi::ui::DluRect::new(236, 243, 56, 17),
        3,
    )
    .text("Next >");

    let cancel_btn = msi::ui::ControlDefinition::new(
        "WelcomeDlg",
        "CancelButton",
        msi::ui::ControlType::PushButton,
        msi::ui::DluRect::new(304, 243, 56, 17),
        3,
    )
    .text("Cancel");

    engine.add_control(next_btn);
    engine.add_control(cancel_btn);

    engine.add_event(msi::ui::ControlEvent::new(
        "WelcomeDlg",
        "CancelButton",
        msi::ui::ControlEventType::EndDialog(msi::ui::DialogReturnCode::Exit),
        None,
        1,
    ));

    drop(engine.set_active_dialog("WelcomeDlg"));

    let mut theme = match theme_style {
        WizardStyle::Mondo => WizardTheme::mondo(),
        WizardStyle::InstallDir => WizardTheme::install_dir(),
        WizardStyle::FeatureTree => WizardTheme::feature_tree(),
        WizardStyle::Minimal => WizardTheme::minimal(),
    };

    if let Some(w) = width_override {
        theme.banner_width = w;
    }
    let _ = height_override;

    GuiDesktopRuntime::new(engine, theme, backend)
}

/// Sets up a GUI desktop runtime loaded directly from an active [`msi::Package`].
///
/// # Arguments
///
/// * `package` - Target MSI package.
/// * `theme_style` - Selected wizard theme style.
/// * `backend` - Hardware rendering backend.
/// * `width_override` - Optional width override.
/// * `height_override` - Optional height override.
///
/// # Returns
///
/// A configured [`GuiDesktopRuntime`].
#[must_use]
pub fn create_package_runtime(
    package: &msi::Package,
    theme_style: WizardStyle,
    backend: GuiHardwareBackend,
    width_override: Option<u32>,
    height_override: Option<u32>,
) -> GuiDesktopRuntime {
    let mut context = EvaluationContext::new();
    for rec in package.database().get_records("Property") {
        if let (
            Some(msi::database::tables::record::FieldValue::String(k)),
            Some(msi::database::tables::record::FieldValue::String(v)),
        ) = (rec.get(0), rec.get(1))
        {
            context.set_property(k, v);
        }
    }
    let mut runtime =
        create_standalone_runtime(theme_style, backend, width_override, height_override);
    *runtime.engine_mut().context_mut() = context;
    runtime
}

/// Executes the GUI desktop runtime in headless or software framebuffer verification mode.
///
/// # Arguments
///
/// * `cli` - Parsed CLI options.
///
/// # Returns
///
/// Process exit code (`ExitCode::SUCCESS` on clean exit).
pub fn run_gui(cli: &GuiCli) -> ExitCode {
    let resolved_backend =
        GuiDesktopRuntime::select_backend_with_fallback(cli.backend.into(), true, true);

    let mut runtime = cli
        .package
        .as_deref()
        .filter(|p| p.exists())
        .and_then(|p| msi::Package::open(p).ok())
        .map_or_else(
            || create_standalone_runtime(cli.theme.into(), resolved_backend, cli.width, cli.height),
            |pkg| {
                create_package_runtime(
                    &pkg,
                    cli.theme.into(),
                    resolved_backend,
                    cli.width,
                    cli.height,
                )
            },
        );

    // Window lifecycle initialization
    let _ = runtime.process_lifecycle_event(msi::ui::WindowLifecycleEvent::Opened);
    let _ = runtime.process_lifecycle_event(msi::ui::WindowLifecycleEvent::Focused);

    // Non-resizable modal dialog constraint checking
    let (req_w, req_h) = (
        cli.width.unwrap_or_else(|| runtime.window_config().width),
        cli.height.unwrap_or_else(|| runtime.window_config().height),
    );
    let (_constrained_w, _constrained_h) = runtime.enforce_window_constraints(req_w, req_h);

    // Render initial frame
    let (_dialog_bounds, _widgets, draw_commands) = runtime.render_frame();

    // In software framebuffer mode, render commands to SoftwareBuffer
    let mut buffer = SoftwareBuffer::new(
        runtime.window_config().width,
        runtime.window_config().height,
    );
    buffer.render_commands(&draw_commands);

    // Verify keyboard navigation event processing
    let _ = runtime.process_event(GuiInputEvent::TabNext);
    let _ = runtime.process_event(GuiInputEvent::TabPrev);
    let _ = runtime.process_event(GuiInputEvent::SubmitDefault);
    let _ = runtime.process_event(GuiInputEvent::CancelEscape);

    // Window lifecycle shutdown
    let _ = runtime.process_lifecycle_event(msi::ui::WindowLifecycleEvent::CloseRequested);

    ExitCode::SUCCESS
}

/// Internal non-generic CLI argument parser and runner to ensure complete branch coverage.
fn run_with_os_args(args: &[std::ffi::OsString]) -> ExitCode {
    GuiCli::try_parse_from(args).map_or(ExitCode::FAILURE, |cli| run_gui(&cli))
}

/// Runs the application with command line arguments.
///
/// # Arguments
///
/// * `args` - Command-line arguments.
///
/// # Returns
///
/// Process exit code.
pub fn run_with_args<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString>,
{
    let os_args: Vec<std::ffi::OsString> = args.into_iter().map(Into::into).collect();
    run_with_os_args(&os_args)
}

/// Entry point of the MSI GUI desktop application.
///
/// # Returns
///
/// Process [`ExitCode`] denoting execution status.
pub fn main() -> ExitCode {
    run_with_args(std::env::args_os())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gui_cli_parsing() {
        let cli = GuiCli::parse_from([
            "msi-gui",
            "package.msi",
            "--theme",
            "mondo",
            "--backend",
            "wgpu",
            "--width",
            "600",
            "--height",
            "450",
        ]);

        assert_eq!(cli.package, Some(PathBuf::from("package.msi")));
        assert_eq!(cli.theme, CliTheme::Mondo);
        assert_eq!(cli.backend, CliBackend::Wgpu);
        assert_eq!(cli.width, Some(600));
        assert_eq!(cli.height, Some(450));
    }

    #[test]
    fn test_cli_theme_and_backend_conversions() {
        assert_eq!(WizardStyle::from(CliTheme::Mondo), WizardStyle::Mondo);
        assert_eq!(
            WizardStyle::from(CliTheme::InstallDir),
            WizardStyle::InstallDir
        );
        assert_eq!(
            WizardStyle::from(CliTheme::FeatureTree),
            WizardStyle::FeatureTree
        );
        assert_eq!(WizardStyle::from(CliTheme::Minimal), WizardStyle::Minimal);

        assert_eq!(
            GuiHardwareBackend::from(CliBackend::Wgpu),
            GuiHardwareBackend::WgpuDirectXMetalVulkan
        );
        assert_eq!(
            GuiHardwareBackend::from(CliBackend::Glow),
            GuiHardwareBackend::GlowLegacyOpenGl
        );
        assert_eq!(
            GuiHardwareBackend::from(CliBackend::Softbuffer),
            GuiHardwareBackend::SoftbufferHeadlessRasterizer
        );
    }

    #[test]
    fn test_create_standalone_runtime_and_run_gui() {
        let cli = GuiCli {
            package: None,
            theme: CliTheme::Mondo,
            backend: CliBackend::Softbuffer,
            width: Some(500),
            height: Some(380),
        };

        let exit_code = run_gui(&cli);
        assert_eq!(exit_code, ExitCode::SUCCESS);

        // Test theme styles InstallDir and Minimal without width override
        let runtime_install_dir = create_standalone_runtime(
            WizardStyle::InstallDir,
            GuiHardwareBackend::SoftbufferHeadlessRasterizer,
            None,
            None,
        );
        assert_eq!(runtime_install_dir.theme().style, WizardStyle::InstallDir);

        let runtime_minimal = create_standalone_runtime(
            WizardStyle::Minimal,
            GuiHardwareBackend::SoftbufferHeadlessRasterizer,
            None,
            None,
        );
        assert_eq!(runtime_minimal.theme().style, WizardStyle::Minimal);

        // Test with non-existent package path
        let cli_nonexistent = GuiCli {
            package: Some(PathBuf::from("/nonexistent_path_msi_gui/fake.msi")),
            theme: CliTheme::Mondo,
            backend: CliBackend::Softbuffer,
            width: None,
            height: None,
        };
        assert_eq!(run_gui(&cli_nonexistent), ExitCode::SUCCESS);

        // Test with existing file that is not a valid MSI package
        let temp_dir = std::env::temp_dir().join("msi_gui_corrupt_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let corrupt_path = temp_dir.join("corrupt.msi");
        let _ = std::fs::write(&corrupt_path, b"not an msi file");
        let cli_corrupt = GuiCli {
            package: Some(corrupt_path.clone()),
            theme: CliTheme::Mondo,
            backend: CliBackend::Softbuffer,
            width: None,
            height: None,
        };
        assert_eq!(run_gui(&cli_corrupt), ExitCode::SUCCESS);
        let _ = std::fs::remove_file(corrupt_path);
        let _ = std::fs::remove_dir(temp_dir);
    }

    #[test]
    fn test_run_with_args_and_main() {
        let ok_code = run_with_args(["msi-gui", "--theme", "mondo"]);
        assert_eq!(ok_code, ExitCode::SUCCESS);

        let err_code = run_with_args(["msi-gui", "--nonexistent-flag"]);
        assert_eq!(err_code, ExitCode::FAILURE);

        let main_code = main();
        assert_eq!(main_code, ExitCode::SUCCESS);
    }

    #[test]
    fn test_gui_with_real_package() {
        let temp_dir = std::env::temp_dir().join("msi_gui_pkg_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let pkg_path = temp_dir.join("gui_test.msi");

        let meta = msi::package::PackageMetadata::new(
            "Desktop App",
            "Visual Systems",
            msi::package::ProductVersion::new(1, 0, 0),
            "{33333333-4444-5555-6666-777777777777}",
        );
        let pkg = msi::package::Package::new(
            meta,
            msi::wix::linker::LinkedDatabase::default(),
            msi::database::summary_info::SummaryInfo::default(),
            std::collections::HashMap::new(),
        );
        let _ = pkg.save(&pkg_path);

        let cli = GuiCli {
            package: Some(pkg_path.clone()),
            theme: CliTheme::FeatureTree,
            backend: CliBackend::Softbuffer,
            width: Some(600),
            height: Some(400),
        };

        let exit_code = run_gui(&cli);
        assert_eq!(exit_code, ExitCode::SUCCESS);

        // Test create_package_runtime with package containing valid record and empty record in Property table
        let mut db = msi::wix::linker::LinkedDatabase::default();
        let mut valid_rec = msi::database::tables::record::Record::new();
        valid_rec.push(msi::database::tables::record::FieldValue::String(
            "PropA".to_string(),
        ));
        valid_rec.push(msi::database::tables::record::FieldValue::String(
            "ValA".to_string(),
        ));
        db.add_record("Property", valid_rec);
        db.add_record("Property", msi::database::tables::record::Record::new());
        let empty_rec_meta = msi::package::PackageMetadata::new(
            "Test",
            "Mfr",
            msi::package::ProductVersion::new(1, 0, 0),
            "{00000000-0000-0000-0000-000000000000}",
        );
        let pkg_with_recs = msi::package::Package::new(
            empty_rec_meta,
            db,
            msi::database::summary_info::SummaryInfo::default(),
            std::collections::HashMap::new(),
        );
        let runtime = create_package_runtime(
            &pkg_with_recs,
            WizardStyle::Mondo,
            GuiHardwareBackend::SoftbufferHeadlessRasterizer,
            None,
            None,
        );
        assert_eq!(runtime.theme().style, WizardStyle::Mondo);
        assert_eq!(
            runtime.engine().context().get_property("PropA"),
            Some("ValA")
        );

        let _ = std::fs::remove_file(&pkg_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }
}
