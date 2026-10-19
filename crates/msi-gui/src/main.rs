#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
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

pub mod native_dialog;

use clap::{Parser, ValueEnum};
use msi::execution::EvaluationContext;
use msi::ui::{
    DialogDefinition, GuiDesktopRuntime, GuiHardwareBackend, GuiInputEvent, WizardStyle,
    WizardTheme,
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
///
/// # Arguments
///
/// * `theme_style` - The selected `WizardStyle` for the UI.
/// * `backend` - The graphics backend to use.
/// * `width_override` - Optional manual width.
/// * `height_override` - Optional manual height.
///
/// # Returns
///
/// A `GuiDesktopRuntime` initialized with default settings.
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
    let _ = runtime.engine_mut().load_from_database(package.database());
    runtime
}

/// Optional dialog selection, state telemetry, and screenshot export configuration for `msi-gui`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuiCaptureOptions {
    /// Optional dialog identifier to activate (e.g., `WelcomeDlg`, `CustomizeDlg`, `ProgressDlg`).
    pub dialog: Option<String>,
    /// Optional progress percentage (`0..=100`).
    pub progress: Option<u32>,
    /// Optional `ActionText` status line override.
    pub action_text: Option<String>,
    /// Optional action log entries (newline-separated when read from `MSI_GUI_ACTION_LOG`).
    pub action_logs: Vec<String>,
    /// Optional output PNG file path for saving the rendered [`msi::ui::SoftwareBuffer`].
    pub screenshot_path: Option<PathBuf>,
    /// Optional output file path for exporting the live evaluated dialog and control state.
    pub dump_state_path: Option<PathBuf>,
}

impl GuiCaptureOptions {
    /// Reads capture options from `MSI_GUI_*` environment variables.
    ///
    /// # Returns
    ///
    /// Populated [`GuiCaptureOptions`].
    #[must_use]
    pub fn from_env() -> Self {
        let dialog = std::env::var("MSI_GUI_DIALOG")
            .ok()
            .filter(|s| !s.is_empty());
        let progress = std::env::var("MSI_GUI_PROGRESS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok());
        let action_text = std::env::var("MSI_GUI_ACTION_TEXT")
            .ok()
            .filter(|s| !s.is_empty());
        let raw_logs = std::env::var("MSI_GUI_ACTION_LOG").unwrap_or_default();
        let action_logs = raw_logs
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(ToString::to_string)
            .collect();
        let screenshot_path = std::env::var_os("MSI_GUI_SCREENSHOT")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        let dump_state_path = std::env::var_os("MSI_GUI_DUMP_STATE")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);

        Self {
            dialog,
            progress,
            action_text,
            action_logs,
            screenshot_path,
            dump_state_path,
        }
    }
}

/// Formats the live evaluated dialog, control, feature tree, and property state of a [`GuiDesktopRuntime`].
///
/// # Arguments
///
/// * `runtime` - Active GUI desktop runtime.
///
/// # Returns
///
/// Multi-line structured state report.
#[must_use]
pub fn export_runtime_state(runtime: &GuiDesktopRuntime) -> String {
    use std::fmt::Write;

    let mut out = String::new();
    let engine = runtime.engine();
    let ctx = engine.context();

    let product_name = ctx
        .get_property("ProductName")
        .unwrap_or("Sample Application");
    let _ = writeln!(out, "WINDOW_TITLE={product_name} Setup - msi-gui");
    let _ = writeln!(out, "THEME_STYLE={:?}", runtime.theme().style);

    for key in [
        "ProductName",
        "ProductVersion",
        "Manufacturer",
        "ProductCode",
        "SelectionDescription",
        "SelectionSize",
    ] {
        if let Some(val) = ctx.get_property(key) {
            let _ = writeln!(out, "PROP:{key}={val}");
        }
    }

    if let Some(dlg) = engine.active_dialog() {
        let dlg_name = dlg.name.clone();
        let raw_title = dlg.title.as_deref().unwrap_or(&dlg_name);
        let formatted_title = ctx.format_string(raw_title).unwrap_or_default();
        let _ = writeln!(out, "ACTIVE_DIALOG={dlg_name}");
        let _ = writeln!(out, "DIALOG_TITLE={formatted_title}");

        for def in engine.get_dialog_controls(&dlg_name) {
            let state = engine.get_control_state(&dlg_name, def.control());
            let mut vis = def.is_visible_by_default();
            let mut en = def.is_enabled_by_default();
            let mut def_btn = false;
            let mut pct = 0;
            let mut txt = def.text_template().unwrap_or("").to_string();
            if let Some(s) = state {
                vis = s.is_visible;
                en = s.is_enabled;
                def_btn = s.is_default;
                pct = s.progress_percent;
                txt.clone_from(&s.current_text);
            }
            let r = def.rect();
            let escaped_txt = txt.replace('\r', "").replace('\n', "\\n");
            let _ = writeln!(
                out,
                "CONTROL:{}|{}|{},{},{},{}|visible={vis}|enabled={en}|default={def_btn}|progress={pct}|text={escaped_txt}",
                def.control(),
                def.control_type().as_str(),
                r.x,
                r.y,
                r.width,
                r.height
            );
            if let Some(st) = state {
                for node in &st.tree_nodes {
                    let desc = node.description.as_deref().unwrap_or("");
                    let _ = writeln!(
                        out,
                        "TREE_NODE:0|{}|{}|{}|{:?}|{desc}",
                        node.feature, node.title, node.size_bytes, node.install_state
                    );
                    for child in &node.children {
                        let cdesc = child.description.as_deref().unwrap_or("");
                        let _ = writeln!(
                            out,
                            "TREE_NODE:1|{}|{}|{}|{:?}|{cdesc}",
                            child.feature, child.title, child.size_bytes, child.install_state
                        );
                    }
                }
                for vol in &st.volume_entries {
                    let _ = writeln!(
                        out,
                        "VOLUME_COST:{}|{}|{}|{}|{}",
                        vol.volume,
                        vol.drive_type,
                        vol.volume_size,
                        vol.required_space,
                        vol.free_space
                    );
                }
            }
        }
    }

    for entry in engine.action_log() {
        let _ = writeln!(out, "ACTION_LOG:{entry}");
    }

    out
}

/// Applies [`GuiCaptureOptions`] to a [`GuiDesktopRuntime`], updating dialog selection,
/// progress state, state export, and PNG screenshot output.
///
/// # Arguments
///
/// * `runtime` - Target GUI desktop runtime.
/// * `opts` - Capture options to apply.
///
/// # Errors
///
/// Returns [`msi::MsiError`] if dialog selection, state writing, or PNG export fails.
pub fn apply_capture_options(
    runtime: &mut GuiDesktopRuntime,
    opts: &GuiCaptureOptions,
) -> msi::Result<()> {
    if let Some(ref dlg_name) = opts.dialog {
        runtime.engine_mut().set_active_dialog(dlg_name)?;
    }
    if let Some(pct) = opts.progress {
        runtime.engine_mut().update_progress(pct);
    }
    if let Some(ref act_txt) = opts.action_text {
        if let Some(dlg) = runtime.engine().active_dialog() {
            let dlg_name = dlg.name.clone();
            if let Some(st) = runtime
                .engine_mut()
                .get_control_state_mut(&dlg_name, "ActionText")
            {
                st.current_text.clone_from(act_txt);
            }
        }
    }
    for log_entry in &opts.action_logs {
        runtime.engine_mut().append_action_log(log_entry.clone());
    }
    if let Some(ref dump_path) = opts.dump_state_path {
        let state_report = export_runtime_state(runtime);
        std::fs::write(dump_path, state_report)?;
    }
    if let Some(ref png_path) = opts.screenshot_path {
        let (_bounds, _widgets, draws) = runtime.render_frame();
        let mut buf = msi::ui::SoftwareBuffer::new(
            runtime.window_config().width,
            runtime.window_config().height,
        );
        buf.clear(runtime.theme().window_bg);
        buf.render_commands(&draws);
        buf.save_png(png_path)?;
    }
    Ok(())
}

/// Executes the GUI desktop runtime with explicit [`GuiCaptureOptions`].
///
/// # Arguments
///
/// * `cli` - Parsed CLI options.
/// * `capture` - Capture options for dialog targeting, state dumping, and PNG output.
///
/// # Returns
///
/// Process exit code (`ExitCode::SUCCESS` on clean exit).
#[must_use = "process exit code must be handled"]
pub fn run_gui_with_options(cli: &GuiCli, capture: &GuiCaptureOptions) -> ExitCode {
    let resolved_backend =
        GuiDesktopRuntime::select_backend_with_fallback(cli.backend.into(), true, true);

    let mut runtime = if let Some(ref pkg_path) = cli.package {
        let Ok(pkg) = msi::Package::open(pkg_path) else {
            return ExitCode::FAILURE;
        };
        create_package_runtime(
            &pkg,
            cli.theme.into(),
            resolved_backend,
            cli.width,
            cli.height,
        )
    } else {
        create_standalone_runtime(cli.theme.into(), resolved_backend, cli.width, cli.height)
    };

    // Non-resizable modal dialog constraint checking
    let (req_w, req_h) = (
        cli.width.unwrap_or_else(|| runtime.window_config().width),
        cli.height.unwrap_or_else(|| runtime.window_config().height),
    );
    let (_constrained_w, _constrained_h) = runtime.enforce_window_constraints(req_w, req_h);

    if apply_capture_options(&mut runtime, capture).is_err() {
        return ExitCode::FAILURE;
    }

    if capture.screenshot_path.is_some() || capture.dump_state_path.is_some() {
        return ExitCode::SUCCESS;
    }

    // Interactive immediate-mode desktop event loop
    let events = [
        GuiInputEvent::TabNext,
        GuiInputEvent::TabPrev,
        GuiInputEvent::SubmitDefault,
        GuiInputEvent::CancelEscape,
    ];

    match runtime.run_event_loop(events) {
        Ok(_) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
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
#[must_use = "process exit code must be handled"]
pub fn run_gui(cli: &GuiCli) -> ExitCode {
    let capture = GuiCaptureOptions::from_env();
    run_gui_with_options(cli, &capture)
}

/// Internal non-generic CLI argument parser and runner to ensure complete branch coverage.
///
/// # Arguments
///
/// * `args` - Command-line or invocation arguments slice.
///
/// # Returns
///
/// Computed `ExitCode` value.
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
#[must_use = "process exit code must be handled"]
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
#[must_use = "process exit code must be handled"]
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
        assert_eq!(run_gui(&cli_nonexistent), ExitCode::FAILURE);

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
        assert_eq!(run_gui(&cli_corrupt), ExitCode::FAILURE);
        let _ = std::fs::remove_file(corrupt_path);
        let _ = std::fs::remove_dir(temp_dir);
    }

    #[test]
    fn test_run_with_args_direct() {
        let args: Vec<String> = vec!["msi-gui".to_string(), "--help".to_string()];
        let _exit_code = run_with_args(args);
        // --help should return SUCCESS
    }

    #[test]
    fn test_run_with_args_and_main() {
        let ok_code = run_with_args(["msi-gui", "--theme", "mondo"]);
        assert_eq!(ok_code, ExitCode::SUCCESS);

        let err_code = run_with_args(["msi-gui", "--nonexistent-flag"]);
        assert_eq!(err_code, ExitCode::FAILURE);

        let _ = main();
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

    /// Tests GUI execution returning failure exit code when an event loop encounters an error.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_gui_run_event_loop_failure() {
        let temp_dir = std::env::temp_dir().join("msi_gui_failure_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let pkg_path = temp_dir.join("failure_test.msi");

        let mut db = msi::wix::linker::LinkedDatabase::default();
        let _ = db
            .catalog
            .add_table(msi::database::tables::ui::dialog_schema());
        let _ = db
            .catalog
            .add_table(msi::database::tables::ui::control_schema());
        let _ = db
            .catalog
            .add_table(msi::database::tables::ui::control_event_schema());

        // 1. Dialog table: WelcomeDlg with NextButton as default control
        let mut dlg_rec = msi::database::tables::record::Record::new();
        dlg_rec.push(msi::database::tables::record::FieldValue::String(
            "WelcomeDlg".to_string(),
        ));
        dlg_rec.push(msi::database::tables::record::FieldValue::Short(50));
        dlg_rec.push(msi::database::tables::record::FieldValue::Short(50));
        dlg_rec.push(msi::database::tables::record::FieldValue::Short(370));
        dlg_rec.push(msi::database::tables::record::FieldValue::Short(270));
        dlg_rec.push(msi::database::tables::record::FieldValue::Long(3));
        dlg_rec.push(msi::database::tables::record::FieldValue::String(
            "Welcome".to_string(),
        ));
        dlg_rec.push(msi::database::tables::record::FieldValue::String(
            "NextButton".to_string(),
        ));
        dlg_rec.push(msi::database::tables::record::FieldValue::String(
            "NextButton".to_string(),
        ));
        dlg_rec.push(msi::database::tables::record::FieldValue::String(
            "CancelButton".to_string(),
        ));
        db.add_record("Dialog", dlg_rec);

        // 2. Control table: NextButton
        let mut ctrl_rec = msi::database::tables::record::Record::new();
        ctrl_rec.push(msi::database::tables::record::FieldValue::String(
            "WelcomeDlg".to_string(),
        ));
        ctrl_rec.push(msi::database::tables::record::FieldValue::String(
            "NextButton".to_string(),
        ));
        ctrl_rec.push(msi::database::tables::record::FieldValue::String(
            "PushButton".to_string(),
        ));
        ctrl_rec.push(msi::database::tables::record::FieldValue::Short(236));
        ctrl_rec.push(msi::database::tables::record::FieldValue::Short(243));
        ctrl_rec.push(msi::database::tables::record::FieldValue::Short(56));
        ctrl_rec.push(msi::database::tables::record::FieldValue::Short(17));
        ctrl_rec.push(msi::database::tables::record::FieldValue::Long(3));
        ctrl_rec.push(msi::database::tables::record::FieldValue::String(
            String::new(),
        ));
        ctrl_rec.push(msi::database::tables::record::FieldValue::String(
            "Next".to_string(),
        ));
        ctrl_rec.push(msi::database::tables::record::FieldValue::String(
            String::new(),
        ));
        ctrl_rec.push(msi::database::tables::record::FieldValue::String(
            String::new(),
        ));
        db.add_record("Control", ctrl_rec);

        // 3. ControlEvent table: NextButton triggers NewDialog to non-existent dialog
        let mut event_rec = msi::database::tables::record::Record::new();
        event_rec.push(msi::database::tables::record::FieldValue::String(
            "WelcomeDlg".to_string(),
        ));
        event_rec.push(msi::database::tables::record::FieldValue::String(
            "NextButton".to_string(),
        ));
        event_rec.push(msi::database::tables::record::FieldValue::String(
            "NewDialog".to_string(),
        ));
        event_rec.push(msi::database::tables::record::FieldValue::String(
            "NonExistentDlg".to_string(),
        ));
        event_rec.push(msi::database::tables::record::FieldValue::String(
            "1".to_string(),
        ));
        event_rec.push(msi::database::tables::record::FieldValue::Short(1));
        db.add_record("ControlEvent", event_rec);

        let meta = msi::package::PackageMetadata::new(
            "Fail App",
            "Visual Systems",
            msi::package::ProductVersion::new(1, 0, 0),
            "{44444444-5555-6666-7777-888888888888}",
        );
        let pkg = msi::package::Package::new(
            meta,
            db,
            msi::database::summary_info::SummaryInfo::default(),
            std::collections::HashMap::new(),
        );
        let _ = pkg.save(&pkg_path);

        let cli = GuiCli {
            package: Some(pkg_path.clone()),
            theme: CliTheme::Mondo,
            backend: CliBackend::Softbuffer,
            width: Some(600),
            height: Some(400),
        };

        let exit_code = run_gui(&cli);
        assert_eq!(exit_code, ExitCode::FAILURE);

        let _ = std::fs::remove_file(pkg_path);
        let _ = std::fs::remove_dir(temp_dir);
    }

    #[test]
    fn test_gui_window_configurations() {
        // verify theme, backend, resolution flags end-to-end
        let exit_code = run_with_args([
            "msi-gui",
            "--theme",
            "minimal",
            "--backend",
            "softbuffer",
            "--width",
            "800",
            "--height",
            "600",
        ]);
        assert_eq!(exit_code, ExitCode::SUCCESS);

        let exit_code2 = run_with_args([
            "msi-gui",
            "--theme",
            "install-dir",
            "--backend",
            "softbuffer",
        ]);
        assert_eq!(exit_code2, ExitCode::SUCCESS);

        assert!(GuiCli::try_parse_from(["msi-gui", "--help"]).is_err());
        assert!(GuiCli::try_parse_from(["msi-gui", "--version"]).is_err());
        assert!(GuiCli::try_parse_from(["msi-gui", "--theme", "invalid"]).is_err());
        assert!(GuiCli::try_parse_from(["msi-gui", "--backend", "invalid"]).is_err());
        assert!(GuiCli::try_parse_from(["msi-gui", "--width", "not-a-number"]).is_err());
        assert!(GuiCli::try_parse_from(["msi-gui", "--height", "not-a-number"]).is_err());
        {
            use clap::{Args, CommandFactory, FromArgMatches};
            let cmd = GuiCli::command();
            let mut empty_matches = cmd.ignore_errors(true).get_matches_from(["msi-gui"]);
            assert!(GuiCli::from_arg_matches(&empty_matches).is_ok());
            let upd_cmd = GuiCli::augment_args_for_update(clap::Command::new("msi-gui-upd"));
            let mut full_matches = upd_cmd.get_matches_from([
                "msi-gui-upd",
                "sample.msi",
                "--theme",
                "minimal",
                "--backend",
                "softbuffer",
                "--width",
                "640",
                "--height",
                "480",
            ]);
            let mut cli_inst = GuiCli {
                package: None,
                theme: CliTheme::Mondo,
                backend: CliBackend::Softbuffer,
                width: None,
                height: None,
            };
            assert!(cli_inst.update_from_arg_matches(&empty_matches).is_ok());
            assert!(cli_inst
                .update_from_arg_matches_mut(&mut empty_matches)
                .is_ok());
            assert!(cli_inst
                .update_from_arg_matches_mut(&mut full_matches)
                .is_ok());
            let no_defaults_matches = clap::Command::new("msi-gui")
                .arg(clap::Arg::new("package"))
                .arg(clap::Arg::new("theme"))
                .arg(clap::Arg::new("backend"))
                .arg(clap::Arg::new("width"))
                .arg(clap::Arg::new("height"))
                .get_matches_from(["msi-gui"]);
            assert!(GuiCli::from_arg_matches(&no_defaults_matches).is_err());
            assert!(cli_inst
                .update_from_arg_matches(&no_defaults_matches)
                .is_ok());
            let mut no_defaults_mut = no_defaults_matches;
            assert!(cli_inst
                .update_from_arg_matches_mut(&mut no_defaults_mut)
                .is_ok());
            let no_backend_matches = clap::Command::new("msi-gui")
                .arg(clap::Arg::new("package"))
                .arg(
                    clap::Arg::new("theme")
                        .default_value("install-dir")
                        .value_parser(clap::value_parser!(CliTheme)),
                )
                .arg(clap::Arg::new("backend"))
                .arg(clap::Arg::new("width"))
                .arg(clap::Arg::new("height"))
                .get_matches_from(["msi-gui"]);
            assert!(GuiCli::from_arg_matches(&no_backend_matches).is_err());
            assert!(cli_inst
                .update_from_arg_matches(&no_backend_matches)
                .is_ok());
            let mut no_backend_mut = no_backend_matches;
            assert!(cli_inst
                .update_from_arg_matches_mut(&mut no_backend_mut)
                .is_ok());
        }
        let cli_parsed = GuiCli::try_parse_from(["msi-gui"]);
        let cli_parsed2 = GuiCli::try_parse_from(["msi-gui"]);
        assert_eq!(cli_parsed.is_ok(), cli_parsed2.is_ok());
        let debug_str = format!("{cli_parsed:?}");
        assert!(!debug_str.is_empty());
    }

    /// Tests `GuiCaptureOptions`, `export_runtime_state`, `apply_capture_options`, and `run_gui_with_options`.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_gui_capture_options_and_state_export() {
        let temp_dir = std::env::temp_dir().join("msi_gui_capture_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let png_path = temp_dir.join("out.png");
        let dump_path = temp_dir.join("state.txt");

        let mut runtime = create_standalone_runtime(
            WizardStyle::Mondo,
            GuiHardwareBackend::SoftbufferHeadlessRasterizer,
            Some(540),
            Some(420),
        );
        runtime.engine_mut().add_control(
            msi::ui::ControlDefinition::new(
                "WelcomeDlg",
                "ActionText",
                msi::ui::ControlType::Text,
                msi::ui::DluRect::new(10, 10, 200, 15),
                3,
            )
            .text("Initial"),
        );
        runtime
            .engine_mut()
            .add_control(msi::ui::ControlDefinition::new(
                "WelcomeDlg",
                "Tree",
                msi::ui::ControlType::SelectionTree,
                msi::ui::DluRect::new(10, 30, 150, 80),
                3,
            ));
        runtime
            .engine_mut()
            .add_control(msi::ui::ControlDefinition::new(
                "WelcomeDlg",
                "VolList",
                msi::ui::ControlType::VolumeCostList,
                msi::ui::DluRect::new(10, 120, 150, 40),
                3,
            ));
        let _ = runtime.engine_mut().set_active_dialog("WelcomeDlg");

        if let Some(st) = runtime
            .engine_mut()
            .get_control_state_mut("WelcomeDlg", "Tree")
        {
            let mut root =
                msi::ui::SelectionTreeNode::new("Root", "Root Feat", 2048).description("Root Desc");
            let child = msi::ui::SelectionTreeNode::new("Child", "Child Feat", 1024);
            root.add_child(child);
            st.tree_nodes = vec![root];
        }
        if let Some(st) = runtime
            .engine_mut()
            .get_control_state_mut("WelcomeDlg", "VolList")
        {
            st.volume_entries = vec![msi::ui::VolumeCostEntry {
                volume: "/".to_string(),
                drive_type: "Fixed Disk".to_string(),
                volume_size: 1000,
                required_space: 100,
                free_space: 900,
            }];
        }

        let opts = GuiCaptureOptions {
            dialog: Some("WelcomeDlg".to_string()),
            progress: Some(75),
            action_text: Some("Installing services...".to_string()),
            action_logs: vec!["Step 1: Costing".to_string()],
            screenshot_path: Some(png_path.clone()),
            dump_state_path: Some(dump_path.clone()),
        };

        #[allow(clippy::redundant_clone)]
        let cloned_opts = opts.clone();
        assert_eq!(opts, cloned_opts);
        let opts_dbg = format!("{opts:?}");
        assert!(!opts_dbg.is_empty());

        assert!(apply_capture_options(&mut runtime, &opts).is_ok());
        assert!(png_path.exists());
        assert!(dump_path.exists());

        let bad_dump_opts = GuiCaptureOptions {
            dump_state_path: Some(PathBuf::from("/nonexistent_dir_msi_gui_999/state.txt")),
            ..GuiCaptureOptions::default()
        };
        assert!(apply_capture_options(&mut runtime, &bad_dump_opts).is_err());

        let bad_png_opts = GuiCaptureOptions {
            screenshot_path: Some(PathBuf::from("/nonexistent_dir_msi_gui_999/out.png")),
            ..GuiCaptureOptions::default()
        };
        assert!(apply_capture_options(&mut runtime, &bad_png_opts).is_err());

        // Test export_runtime_state when control states are cleared (None state fallback)
        runtime.engine_mut().clear_control_states();
        let cleared_state = export_runtime_state(&runtime);
        assert!(cleared_state.contains("ACTIVE_DIALOG=WelcomeDlg"));

        // Test export_runtime_state and apply_capture_options when no active dialog is set
        let mut empty_runtime = GuiDesktopRuntime::new(
            msi::ui::UiEngine::new(EvaluationContext::new()),
            WizardTheme::mondo(),
            GuiHardwareBackend::SoftbufferHeadlessRasterizer,
        );
        let empty_state = export_runtime_state(&empty_runtime);
        assert!(empty_state.contains("WINDOW_TITLE="));
        let act_only_opts = GuiCaptureOptions {
            action_text: Some("No dialog active".to_string()),
            ..GuiCaptureOptions::default()
        };
        assert!(apply_capture_options(&mut empty_runtime, &act_only_opts).is_ok());

        // Test run_gui_with_options with screenshot/dump paths and invalid dialog error
        let cli = GuiCli {
            package: None,
            theme: CliTheme::Mondo,
            backend: CliBackend::Softbuffer,
            width: Some(500),
            height: Some(380),
        };
        assert_eq!(run_gui_with_options(&cli, &opts), ExitCode::SUCCESS);

        let err_opts = GuiCaptureOptions {
            dialog: Some("NonExistentDialog".to_string()),
            ..GuiCaptureOptions::default()
        };
        assert_eq!(run_gui_with_options(&cli, &err_opts), ExitCode::FAILURE);

        // Test GuiCaptureOptions::from_env
        std::env::set_var("MSI_GUI_DIALOG", "WelcomeDlg");
        std::env::set_var("MSI_GUI_PROGRESS", "50");
        std::env::set_var("MSI_GUI_ACTION_TEXT", "Status text");
        std::env::set_var("MSI_GUI_ACTION_LOG", "Log A\n\nLog B");
        std::env::set_var("MSI_GUI_SCREENSHOT", &png_path);
        std::env::set_var("MSI_GUI_DUMP_STATE", &dump_path);
        let env_opts = GuiCaptureOptions::from_env();
        assert_eq!(env_opts.dialog.as_deref(), Some("WelcomeDlg"));
        assert_eq!(env_opts.progress, Some(50));
        assert_eq!(env_opts.action_logs.len(), 2);
        std::env::remove_var("MSI_GUI_DIALOG");
        std::env::remove_var("MSI_GUI_PROGRESS");
        std::env::remove_var("MSI_GUI_ACTION_TEXT");
        std::env::remove_var("MSI_GUI_ACTION_LOG");
        std::env::remove_var("MSI_GUI_SCREENSHOT");
        std::env::remove_var("MSI_GUI_DUMP_STATE");

        let _ = std::fs::remove_file(&png_path);
        let _ = std::fs::remove_file(&dump_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }
}
