//! Comprehensive Headless GUI Wizard Integration Tests.
//!
//! Validates end-to-end user journeys through interactive installation wizards:
//! - Multi-dialog sequential progression (`Welcome` -> `License` -> `SetupType` -> `Features` -> `Verify` -> `Progress` -> `Exit`).
//! - Conditional branching (`SETUP_MODE="Custom"` routing to `FeaturesDlg` vs `"Typical"` routing to `VerifyReadyDlg`).
//! - Two-way data binding for text input, checkboxes, and radio buttons.
//! - Dynamic `ControlCondition` enabling/disabling controls in real time.
//! - Synchronous custom action execution (`DoAction`) in the UI phase.
//! - Non-resizable modal dialog window constraint enforcement.
//! - Full keyboard navigation (`Tab`, `Shift+Tab`, `Enter`, `Escape`).
//! - Visual focus rings and `AccessKit` screen reader accessibility tree generation.

use msi::error::Result;
use msi::execution::custom_action::{CustomActionDefinition, CustomActionExecutor};
use msi::execution::EvaluationContext;
use msi::ui::{
    AccessibleRole, ControlCondition, ControlConditionAction, ControlDefinition, ControlEvent,
    ControlEventType, ControlType, DialogDefinition, DialogReturnCode, DluRect, GuiDesktopRuntime,
    GuiHardwareBackend, GuiInputEvent, UiWidget, WizardTheme, DIALOG_ATTR_MODAL,
    DIALOG_ATTR_VISIBLE,
};

/// Builds a fully configured multi-step installer wizard engine.
#[allow(clippy::too_many_lines)]
fn create_test_wizard_engine() -> Result<msi::ui::UiEngine> {
    let mut context = EvaluationContext::new();
    context.set_property("ProductName", "Open edX Deployment Suite");
    context.set_property("ACCEPT_EULA", "0");
    context.set_property("SETUP_MODE", "Typical");
    context.set_property("INSTALLDIR", r"C:\Program Files\Open edX");
    context.set_property("PORT_NUMBER", "8000");

    let mut engine = msi::ui::UiEngine::new(context);

    // =========================================================================
    // 1. WelcomeDlg
    // =========================================================================
    engine.add_dialog(DialogDefinition {
        name: "WelcomeDlg".to_string(),
        h_centering: 50,
        v_centering: 50,
        width: 370,
        height: 270,
        attributes: DIALOG_ATTR_VISIBLE | DIALOG_ATTR_MODAL,
        title: Some("Welcome to [ProductName] Setup".to_string()),
        control_first: "NextBtn".to_string(),
        control_default: Some("NextBtn".to_string()),
        control_cancel: Some("CancelBtn".to_string()),
    });
    engine.add_control(
        ControlDefinition::new(
            "WelcomeDlg",
            "TitleText",
            ControlType::Text,
            DluRect::new(20, 20, 330, 30),
            3,
        )
        .text("Welcome to the setup wizard for [ProductName]"),
    );
    engine.add_control(
        ControlDefinition::new(
            "WelcomeDlg",
            "NextBtn",
            ControlType::PushButton,
            DluRect::new(236, 243, 56, 17),
            3,
        )
        .text("Next >"),
    );
    engine.add_control(
        ControlDefinition::new(
            "WelcomeDlg",
            "CancelBtn",
            ControlType::PushButton,
            DluRect::new(304, 243, 56, 17),
            3,
        )
        .text("Cancel"),
    );
    engine.add_event(ControlEvent::new(
        "WelcomeDlg",
        "NextBtn",
        ControlEventType::NewDialog("LicenseDlg".to_string()),
        None,
        1,
    ));
    engine.add_event(ControlEvent::new(
        "WelcomeDlg",
        "CancelBtn",
        ControlEventType::EndDialog(DialogReturnCode::Exit),
        None,
        1,
    ));

    // =========================================================================
    // 2. LicenseDlg
    // =========================================================================
    engine.add_dialog(DialogDefinition {
        name: "LicenseDlg".to_string(),
        h_centering: 50,
        v_centering: 50,
        width: 370,
        height: 270,
        attributes: DIALOG_ATTR_VISIBLE | DIALOG_ATTR_MODAL,
        title: Some("[ProductName] License Agreement".to_string()),
        control_first: "AcceptCheck".to_string(),
        control_default: Some("NextBtn".to_string()),
        control_cancel: Some("CancelBtn".to_string()),
    });
    engine.add_control(
        ControlDefinition::new(
            "LicenseDlg",
            "LicenseText",
            ControlType::ScrollableText,
            DluRect::new(20, 20, 330, 160),
            3,
        )
        .text(
            "END USER LICENSE AGREEMENT:
1. Terms of Use
2. Open Source Licenses",
        ),
    );
    engine.add_control(
        ControlDefinition::new(
            "LicenseDlg",
            "AcceptCheck",
            ControlType::CheckBox,
            DluRect::new(20, 190, 200, 15),
            3,
        )
        .property("ACCEPT_EULA")
        .text("I accept the terms in the License Agreement"),
    );
    engine.add_control(
        ControlDefinition::new(
            "LicenseDlg",
            "BackBtn",
            ControlType::PushButton,
            DluRect::new(180, 243, 56, 17),
            3,
        )
        .text("< Back"),
    );
    engine.add_control(
        ControlDefinition::new(
            "LicenseDlg",
            "NextBtn",
            ControlType::PushButton,
            DluRect::new(236, 243, 56, 17),
            1, // Initially disabled
        )
        .text("Next >"),
    );
    engine.add_control(
        ControlDefinition::new(
            "LicenseDlg",
            "CancelBtn",
            ControlType::PushButton,
            DluRect::new(304, 243, 56, 17),
            3,
        )
        .text("Cancel"),
    );
    // Dynamic conditions on NextBtn
    engine.add_condition(ControlCondition {
        dialog: "LicenseDlg".to_string(),
        control: "NextBtn".to_string(),
        action: ControlConditionAction::Enable,
        condition: r#"ACCEPT_EULA = "1""#.to_string(),
    });
    engine.add_condition(ControlCondition {
        dialog: "LicenseDlg".to_string(),
        control: "NextBtn".to_string(),
        action: ControlConditionAction::Disable,
        condition: r#"NOT (ACCEPT_EULA = "1")"#.to_string(),
    });
    engine.add_event(ControlEvent::new(
        "LicenseDlg",
        "BackBtn",
        ControlEventType::NewDialog("WelcomeDlg".to_string()),
        None,
        1,
    ));
    engine.add_event(ControlEvent::new(
        "LicenseDlg",
        "NextBtn",
        ControlEventType::NewDialog("SetupTypeDlg".to_string()),
        None,
        1,
    ));

    // =========================================================================
    // 3. SetupTypeDlg
    // =========================================================================
    engine.add_dialog(DialogDefinition {
        name: "SetupTypeDlg".to_string(),
        h_centering: 50,
        v_centering: 50,
        width: 370,
        height: 270,
        attributes: DIALOG_ATTR_VISIBLE | DIALOG_ATTR_MODAL,
        title: Some("Choose Setup Type".to_string()),
        control_first: "ModeTypical".to_string(),
        control_default: Some("NextBtn".to_string()),
        control_cancel: Some("CancelBtn".to_string()),
    });
    engine.add_control(
        ControlDefinition::new(
            "SetupTypeDlg",
            "ModeTypical",
            ControlType::RadioButtonGroup,
            DluRect::new(20, 40, 200, 20),
            3,
        )
        .property("SETUP_MODE")
        .text("Typical"),
    );
    engine.add_control(
        ControlDefinition::new(
            "SetupTypeDlg",
            "ModeCustom",
            ControlType::RadioButtonGroup,
            DluRect::new(20, 70, 200, 20),
            3,
        )
        .property("SETUP_MODE")
        .text("Custom"),
    );
    engine.add_control(
        ControlDefinition::new(
            "SetupTypeDlg",
            "BackBtn",
            ControlType::PushButton,
            DluRect::new(180, 243, 56, 17),
            3,
        )
        .text("< Back"),
    );
    engine.add_control(
        ControlDefinition::new(
            "SetupTypeDlg",
            "NextBtn",
            ControlType::PushButton,
            DluRect::new(236, 243, 56, 17),
            3,
        )
        .text("Next >"),
    );
    // Conditional routing on NextBtn
    engine.add_event(ControlEvent::new(
        "SetupTypeDlg",
        "NextBtn",
        ControlEventType::NewDialog("FeaturesDlg".to_string()),
        Some(r#"SETUP_MODE = "Custom""#.to_string()),
        1,
    ));
    engine.add_event(ControlEvent::new(
        "SetupTypeDlg",
        "NextBtn",
        ControlEventType::NewDialog("VerifyReadyDlg".to_string()),
        Some(r#"SETUP_MODE = "Typical""#.to_string()),
        2,
    ));

    // =========================================================================
    // 4. FeaturesDlg
    // =========================================================================
    engine.add_dialog(DialogDefinition {
        name: "FeaturesDlg".to_string(),
        h_centering: 50,
        v_centering: 50,
        width: 370,
        height: 270,
        attributes: DIALOG_ATTR_VISIBLE | DIALOG_ATTR_MODAL,
        title: Some("Custom Feature Selection".to_string()),
        control_first: "InstallDir".to_string(),
        control_default: Some("NextBtn".to_string()),
        control_cancel: Some("CancelBtn".to_string()),
    });
    engine.add_control(
        ControlDefinition::new(
            "FeaturesDlg",
            "FeatureTree",
            ControlType::SelectionTree,
            DluRect::new(20, 20, 330, 120),
            3,
        )
        .text("Features"),
    );
    engine.add_control(
        ControlDefinition::new(
            "FeaturesDlg",
            "InstallDir",
            ControlType::Edit,
            DluRect::new(20, 160, 240, 15),
            3,
        )
        .property("INSTALLDIR")
        .text(r"C:\Program Files\Open edX"),
    );
    engine.add_control(
        ControlDefinition::new(
            "FeaturesDlg",
            "NextBtn",
            ControlType::PushButton,
            DluRect::new(236, 243, 56, 17),
            3,
        )
        .text("Next >"),
    );
    engine.add_event(ControlEvent::new(
        "FeaturesDlg",
        "NextBtn",
        ControlEventType::NewDialog("VerifyReadyDlg".to_string()),
        None,
        1,
    ));

    // =========================================================================
    // 5. VerifyReadyDlg
    // =========================================================================
    engine.add_dialog(DialogDefinition {
        name: "VerifyReadyDlg".to_string(),
        h_centering: 50,
        v_centering: 50,
        width: 370,
        height: 270,
        attributes: DIALOG_ATTR_VISIBLE | DIALOG_ATTR_MODAL,
        title: Some("Ready to Install [ProductName]".to_string()),
        control_first: "InstallBtn".to_string(),
        control_default: Some("InstallBtn".to_string()),
        control_cancel: Some("CancelBtn".to_string()),
    });
    engine.add_control(
        ControlDefinition::new(
            "VerifyReadyDlg",
            "InstallBtn",
            ControlType::PushButton,
            DluRect::new(236, 243, 56, 17),
            3,
        )
        .text("Install"),
    );
    engine.add_event(ControlEvent::new(
        "VerifyReadyDlg",
        "InstallBtn",
        ControlEventType::DoAction("CA_ValidateInstall".to_string()),
        None,
        1,
    ));
    engine.add_event(ControlEvent::new(
        "VerifyReadyDlg",
        "InstallBtn",
        ControlEventType::NewDialog("ProgressDlg".to_string()),
        None,
        2,
    ));

    // =========================================================================
    // 6. ProgressDlg
    // =========================================================================
    engine.add_dialog(DialogDefinition {
        name: "ProgressDlg".to_string(),
        h_centering: 50,
        v_centering: 50,
        width: 370,
        height: 270,
        attributes: DIALOG_ATTR_VISIBLE | DIALOG_ATTR_MODAL,
        title: Some("Installing [ProductName]".to_string()),
        control_first: "ProgressBar".to_string(),
        control_default: Some("NextBtn".to_string()),
        control_cancel: None,
    });
    engine.add_control(
        ControlDefinition::new(
            "ProgressDlg",
            "ProgressBar",
            ControlType::ProgressBar,
            DluRect::new(20, 100, 330, 15),
            3,
        )
        .text("Progress"),
    );
    engine.add_control(
        ControlDefinition::new(
            "ProgressDlg",
            "NextBtn",
            ControlType::PushButton,
            DluRect::new(236, 243, 56, 17),
            3,
        )
        .text("Next >"),
    );
    engine.add_event(ControlEvent::new(
        "ProgressDlg",
        "NextBtn",
        ControlEventType::NewDialog("ExitDlg".to_string()),
        None,
        1,
    ));

    // =========================================================================
    // 7. ExitDlg
    // =========================================================================
    engine.add_dialog(DialogDefinition {
        name: "ExitDlg".to_string(),
        h_centering: 50,
        v_centering: 50,
        width: 370,
        height: 270,
        attributes: DIALOG_ATTR_VISIBLE | DIALOG_ATTR_MODAL,
        title: Some("Completed [ProductName] Setup".to_string()),
        control_first: "FinishBtn".to_string(),
        control_default: Some("FinishBtn".to_string()),
        control_cancel: None,
    });
    engine.add_control(
        ControlDefinition::new(
            "ExitDlg",
            "LaunchCheck",
            ControlType::CheckBox,
            DluRect::new(20, 180, 200, 15),
            3,
        )
        .property("LAUNCH_APP")
        .text("Launch application now"),
    );
    engine.add_control(
        ControlDefinition::new(
            "ExitDlg",
            "FinishBtn",
            ControlType::PushButton,
            DluRect::new(236, 243, 56, 17),
            3,
        )
        .text("Finish"),
    );
    engine.add_event(ControlEvent::new(
        "ExitDlg",
        "FinishBtn",
        ControlEventType::EndDialog(DialogReturnCode::Return),
        None,
        1,
    ));

    // Configure synchronous custom action
    let mut ca_exec = CustomActionExecutor::new();
    ca_exec.set_mock_result("CA_ValidateInstall", 0);
    engine.set_custom_action_executor(ca_exec);

    let ca_def = CustomActionDefinition::parse("CA_ValidateInstall", 1, "BinValidate", "Validate")?;
    engine.add_custom_action(ca_def);

    engine.set_active_dialog("WelcomeDlg")?;
    Ok(engine)
}

/// Tests a full interactive user installation wizard journey with custom setup branching.
#[test]
#[allow(clippy::too_many_lines)]
fn test_headless_gui_wizard_full_journey() -> Result<()> {
    let engine = create_test_wizard_engine()?;
    let mut runtime = GuiDesktopRuntime::new(
        engine,
        WizardTheme::mondo(),
        GuiHardwareBackend::SoftbufferHeadlessRasterizer,
    );

    // 1. Initial State: WelcomeDlg
    let (_bounds, widgets, _draws) = runtime.render_frame();
    assert_eq!(
        runtime.window_config().title,
        "Welcome to Open edX Deployment Suite Setup"
    );
    assert_eq!(
        runtime.engine().active_dialog().map(|d| d.name.as_str()),
        Some("WelcomeDlg")
    );
    assert_eq!(widgets.len(), 3);

    // Verify AccessKit nodes
    let nodes = runtime.access_bridge().nodes();
    assert!(nodes.iter().any(|n| n.role == AccessibleRole::Window));
    assert!(nodes.iter().any(|n| n.label == "Next >"));

    // Navigate WelcomeDlg -> LicenseDlg via Enter (SubmitDefault clicks NextBtn)
    let ret = runtime.process_event(GuiInputEvent::SubmitDefault)?;
    assert_eq!(ret, None);
    assert_eq!(
        runtime.engine().active_dialog().map(|d| d.name.as_str()),
        Some("LicenseDlg")
    );

    // 2. LicenseDlg: Check NextBtn is disabled initially
    runtime.render_frame();
    let next_btn_state = runtime.engine().get_control_state("LicenseDlg", "NextBtn");
    assert!(next_btn_state.is_some_and(|s| !s.is_enabled));

    // Toggle AcceptCheck checkbox
    runtime.process_event(GuiInputEvent::ToggleCheckBox {
        control: "AcceptCheck".to_string(),
    })?;
    assert_eq!(
        runtime.engine().context().get_property("ACCEPT_EULA"),
        Some("1")
    );

    // Verify NextBtn is now enabled by dynamic ControlCondition evaluation
    runtime.render_frame();
    let next_btn_enabled = runtime.engine().get_control_state("LicenseDlg", "NextBtn");
    assert!(next_btn_enabled.is_some_and(|s| s.is_enabled));

    // Advance to SetupTypeDlg
    runtime.process_event(GuiInputEvent::SubmitDefault)?;
    assert_eq!(
        runtime.engine().active_dialog().map(|d| d.name.as_str()),
        Some("SetupTypeDlg")
    );

    // 3. SetupTypeDlg: Select "Custom" setup mode via radio button
    runtime.process_event(GuiInputEvent::SelectRadio {
        group: "ModeCustom".to_string(),
        value: "Custom".to_string(),
    })?;
    assert_eq!(
        runtime.engine().context().get_property("SETUP_MODE"),
        Some("Custom")
    );

    // Click NextBtn -> Because SETUP_MODE="Custom", branches to FeaturesDlg
    runtime.process_event(GuiInputEvent::SubmitDefault)?;
    assert_eq!(
        runtime.engine().active_dialog().map(|d| d.name.as_str()),
        Some("FeaturesDlg")
    );

    // 4. FeaturesDlg: Update target install path via ValueChange
    runtime.process_event(GuiInputEvent::ValueChange {
        control: "InstallDir".to_string(),
        value: "/opt/openedx".to_string(),
    })?;
    assert_eq!(
        runtime.engine().context().get_property("INSTALLDIR"),
        Some("/opt/openedx")
    );

    // Verify rich widgets rendered (SelectionTree and PathEdit)
    let (_f_bounds, f_widgets, _f_draws) = runtime.render_frame();
    assert!(f_widgets
        .iter()
        .any(|(_, w)| matches!(w, UiWidget::SelectionTree { .. })));
    assert!(f_widgets
        .iter()
        .any(|(_, w)| matches!(w, UiWidget::PathEdit { .. })));

    // Advance to VerifyReadyDlg
    runtime.process_event(GuiInputEvent::SubmitDefault)?;
    assert_eq!(
        runtime.engine().active_dialog().map(|d| d.name.as_str()),
        Some("VerifyReadyDlg")
    );

    // 5. VerifyReadyDlg: Click InstallBtn -> executes synchronous CA_ValidateInstall action
    runtime.process_event(GuiInputEvent::SubmitDefault)?;
    assert!(runtime
        .engine()
        .action_log()
        .iter()
        .any(|a| a.contains("CA_ValidateInstall")));
    assert_eq!(
        runtime.engine().active_dialog().map(|d| d.name.as_str()),
        Some("ProgressDlg")
    );

    // 6. ProgressDlg: Simulate progress updating to 50% and 100%
    runtime.engine_mut().update_progress(50);
    let (_p_bounds, p_widgets, _p_draws) = runtime.render_frame();
    assert!(p_widgets.iter().any(
        |(_, w)| matches!(w, UiWidget::ProgressBar { fraction } if (*fraction - 0.5).abs() < 0.01)
    ));

    runtime.engine_mut().update_progress(100);
    runtime.process_event(GuiInputEvent::SubmitDefault)?;
    assert_eq!(
        runtime.engine().active_dialog().map(|d| d.name.as_str()),
        Some("ExitDlg")
    );

    // 7. ExitDlg: Toggle completion checkbox and click FinishBtn
    runtime.process_event(GuiInputEvent::ToggleCheckBox {
        control: "LaunchCheck".to_string(),
    })?;
    assert_eq!(
        runtime.engine().context().get_property("LAUNCH_APP"),
        Some("1")
    );

    let finish_res = runtime.process_event(GuiInputEvent::SubmitDefault)?;
    assert_eq!(finish_res, Some(DialogReturnCode::Return));

    Ok(())
}

/// Tests keyboard tab ordering, Escape cancellation, and focus rings.
#[test]
fn test_headless_gui_wizard_keyboard_navigation_and_cancel() -> Result<()> {
    let engine = create_test_wizard_engine()?;
    let mut runtime = GuiDesktopRuntime::new(
        engine,
        WizardTheme::mondo(),
        GuiHardwareBackend::SoftbufferHeadlessRasterizer,
    );

    // Render frame to populate focus and tab stops
    let (_bounds, _widgets, draws) = runtime.render_frame();

    // Verify focus ring draw command generated
    assert!(draws.iter().any(|cmd| matches!(
        cmd,
        msi::ui::DrawCommand::StrokeRect {
            color: msi::ui::Color32::FOCUS_RING,
            ..
        }
    )));

    // Cycle tab forward
    runtime.process_event(GuiInputEvent::TabNext)?;
    // Cycle tab reverse
    runtime.process_event(GuiInputEvent::TabPrev)?;

    // Press Escape key -> Triggers cancel control and returns DialogReturnCode::Exit
    let cancel_res = runtime.process_event(GuiInputEvent::CancelEscape)?;
    assert_eq!(cancel_res, Some(DialogReturnCode::Exit));

    Ok(())
}
