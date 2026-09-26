//! End-to-End Terminal User Interface (TUI) Wizard Integration Tests.
//!
//! Validates full interactive terminal flows:
//! - Interactive Edit fields with text typing (`TuiKey::Char`), backspace, cursor navigation, password masking (`*`), and port number validation feedback.
//! - Scrollable License Viewer with keyboard scrolling (`Up`, `Down`, `PageUp`, `PageDown`, `Home`, `End`) and progress indicators.
//! - Interactive Radio Button Groups with focus indicators `(•)` / `( )` and arrow key / `Space` selection.
//! - Interactive Feature Selection Checklist with toggleable component items.
//! - Prominent navigation buttons (`[< Back]`, `[ Next >]`, `[ Cancel ]`, `[ Install ]`, `[ Finish ]`) with focus indicators and advancement on `Enter`.
//! - Real-time progress bar with animated blocks (`████░░░░`) and live action descriptions.
//! - Live diagnostics log view toggled via `F2`.
//! - Exit summary screen with active services and listening endpoints (`render_exit_summary`).

use msi::error::Result;
use msi::execution::EvaluationContext;
use msi::ui::{
    ControlCondition, ControlConditionAction, ControlDefinition, ControlEvent, ControlEventType,
    ControlType, DialogDefinition, DialogReturnCode, DluRect, TerminalEvent, TerminalWizard,
    TuiKey, UiEngine, DIALOG_ATTR_VISIBLE,
};

/// Builds a test installer wizard configured for interactive terminal testing.
#[allow(clippy::too_many_lines)]
fn create_test_tui_engine() -> Result<UiEngine> {
    let mut context = EvaluationContext::new();
    context.set_property("ProductName", "Open edX Stack");
    context.set_property("ACCEPT_EULA", "0");
    context.set_property("ADMIN_PASSWORD", "secret123");
    context.set_property("LMS_PORT", "8000");
    context.set_property("STACK_MODE", "Standard");

    let mut engine = UiEngine::new(context);

    // 1. WelcomeDlg
    engine.add_dialog(DialogDefinition {
        name: "WelcomeDlg".to_string(),
        h_centering: 50,
        v_centering: 50,
        width: 370,
        height: 270,
        attributes: DIALOG_ATTR_VISIBLE,
        title: Some("Welcome to [ProductName] Installer".to_string()),
        control_first: "NextBtn".to_string(),
        control_default: Some("NextBtn".to_string()),
        control_cancel: Some("CancelBtn".to_string()),
    });
    engine.add_control(
        ControlDefinition::new(
            "WelcomeDlg",
            "IntroText",
            ControlType::Text,
            DluRect::new(20, 20, 300, 20),
            3,
        )
        .text("Welcome to Open edX setup."),
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

    // 2. LicenseDlg
    engine.add_dialog(DialogDefinition {
        name: "LicenseDlg".to_string(),
        h_centering: 50,
        v_centering: 50,
        width: 370,
        height: 270,
        attributes: DIALOG_ATTR_VISIBLE,
        title: Some("License Agreement".to_string()),
        control_first: "LicenseScroll".to_string(),
        control_default: Some("NextBtn".to_string()),
        control_cancel: Some("CancelBtn".to_string()),
    });
    let license_body = (1..=30)
        .map(|i| format!("Line {i}: Agreement term and conditions clause {i}"))
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    engine.add_control(
        ControlDefinition::new(
            "LicenseDlg",
            "LicenseScroll",
            ControlType::ScrollableText,
            DluRect::new(20, 20, 330, 160),
            3,
        )
        .text(license_body),
    );
    engine.add_control(
        ControlDefinition::new(
            "LicenseDlg",
            "AgreeCheck",
            ControlType::CheckBox,
            DluRect::new(20, 190, 200, 15),
            3,
        )
        .property("ACCEPT_EULA")
        .text("I agree to all license terms"),
    );
    engine.add_control(
        ControlDefinition::new(
            "LicenseDlg",
            "NextBtn",
            ControlType::PushButton,
            DluRect::new(236, 243, 56, 17),
            1, // disabled initially
        )
        .text("Next >"),
    );
    engine.add_condition(ControlCondition {
        dialog: "LicenseDlg".to_string(),
        control: "NextBtn".to_string(),
        action: ControlConditionAction::Enable,
        condition: r#"ACCEPT_EULA = "1""#.to_string(),
    });
    engine.add_event(ControlEvent::new(
        "LicenseDlg",
        "NextBtn",
        ControlEventType::NewDialog("ConfigDlg".to_string()),
        None,
        1,
    ));

    // 3. ConfigDlg (Edit fields: port with validation and password with masking)
    engine.add_dialog(DialogDefinition {
        name: "ConfigDlg".to_string(),
        h_centering: 50,
        v_centering: 50,
        width: 370,
        height: 270,
        attributes: DIALOG_ATTR_VISIBLE,
        title: Some("Service Configuration".to_string()),
        control_first: "PortEdit".to_string(),
        control_default: Some("NextBtn".to_string()),
        control_cancel: Some("CancelBtn".to_string()),
    });
    engine.add_control(
        ControlDefinition::new(
            "ConfigDlg",
            "PortEdit",
            ControlType::Edit,
            DluRect::new(20, 30, 120, 15),
            3,
        )
        .property("LMS_PORT")
        .text("8000"),
    );
    engine.add_control(
        ControlDefinition::new(
            "ConfigDlg",
            "PwdEdit",
            ControlType::Edit,
            DluRect::new(20, 60, 120, 15),
            3 | 0x0200, // password mask flag
        )
        .property("ADMIN_PASSWORD")
        .text("secret123"),
    );
    engine.add_control(
        ControlDefinition::new(
            "ConfigDlg",
            "ModeRadioStd",
            ControlType::RadioButtonGroup,
            DluRect::new(20, 90, 120, 15),
            3,
        )
        .property("STACK_MODE")
        .text("Standard"),
    );
    engine.add_control(
        ControlDefinition::new(
            "ConfigDlg",
            "ModeRadioAdv",
            ControlType::RadioButtonGroup,
            DluRect::new(20, 110, 120, 15),
            3,
        )
        .property("STACK_MODE")
        .text("Advanced"),
    );
    engine.add_control(
        ControlDefinition::new(
            "ConfigDlg",
            "TreeComponents",
            ControlType::SelectionTree,
            DluRect::new(20, 130, 200, 40),
            3,
        )
        .text("Services"),
    );
    engine.add_control(
        ControlDefinition::new(
            "ConfigDlg",
            "NextBtn",
            ControlType::PushButton,
            DluRect::new(236, 243, 56, 17),
            3,
        )
        .text("Next >"),
    );
    engine.add_event(ControlEvent::new(
        "ConfigDlg",
        "NextBtn",
        ControlEventType::NewDialog("ProgressDlg".to_string()),
        None,
        1,
    ));

    // 4. ProgressDlg
    engine.add_dialog(DialogDefinition {
        name: "ProgressDlg".to_string(),
        h_centering: 50,
        v_centering: 50,
        width: 370,
        height: 270,
        attributes: DIALOG_ATTR_VISIBLE,
        title: Some("Deploying Components".to_string()),
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
        .text("Finish"),
    );
    engine.add_event(ControlEvent::new(
        "ProgressDlg",
        "NextBtn",
        ControlEventType::EndDialog(DialogReturnCode::Return),
        None,
        1,
    ));

    engine.set_active_dialog("WelcomeDlg")?;
    Ok(engine)
}

/// Tests the end-to-end interactive terminal wizard lifecycle.
#[test]
#[allow(clippy::too_many_lines)]
fn test_tui_wizard_e2e_full_flow() -> Result<()> {
    let engine = create_test_tui_engine()?;
    let mut wizard = TerminalWizard::new(engine);

    // Initial WelcomeDlg frame
    let frame1 = wizard.render_frame(80, 24).render_to_string();
    assert!(frame1.contains("Welcome to Open edX Stack Installer"));
    assert!(frame1.contains("[> Next > <]"));

    // Advance to LicenseDlg on Enter
    let res = wizard.handle_key(TuiKey::Enter)?;
    assert_eq!(res, None);
    assert_eq!(
        wizard.engine().active_dialog().map(|d| d.name.as_str()),
        Some("LicenseDlg")
    );

    // Test ScrollableText keyboard scrolling
    let initial_frame = wizard.render_frame(80, 24).render_to_string();
    assert!(initial_frame.contains("Line 1: Agreement term"));
    assert!(initial_frame.contains("[Lines 1-"));

    // Scroll Down and PageDown
    wizard.handle_key(TuiKey::Down)?;
    wizard.handle_key(TuiKey::PageDown)?;
    assert!(wizard.scroll_offset() > 0);

    // Scroll Up and Home
    wizard.handle_key(TuiKey::Up)?;
    wizard.handle_key(TuiKey::Home)?;
    assert_eq!(wizard.scroll_offset(), 0);

    // End scrolls to bottom
    wizard.handle_key(TuiKey::End)?;
    assert!(wizard.scroll_offset() > 0);
    wizard.handle_key(TuiKey::Home)?;

    // Tab to AgreeCheck (index 1)
    wizard.handle_key(TuiKey::Tab)?;
    // Space toggles checkbox
    wizard.handle_key(TuiKey::Space)?;
    assert_eq!(
        wizard.engine().context().get_property("ACCEPT_EULA"),
        Some("1")
    );

    // Tab to NextBtn (index 2) and press Enter
    wizard.handle_key(TuiKey::Tab)?;
    wizard.handle_key(TuiKey::Enter)?;
    assert_eq!(
        wizard.engine().active_dialog().map(|d| d.name.as_str()),
        Some("ConfigDlg")
    );

    // Focused on PortEdit (index 0). Test Char typing and Backspace
    // Current is "8000". Backspace to "800"
    wizard.handle_key(TuiKey::End)?;
    wizard.handle_key(TuiKey::Backspace)?;
    assert_eq!(
        wizard.engine().context().get_property("LMS_PORT"),
        Some("800")
    );
    // Type '1' -> "8001"
    wizard.handle_key(TuiKey::Char('1'))?;
    assert_eq!(
        wizard.engine().context().get_property("LMS_PORT"),
        Some("8001")
    );

    // Test port number validation feedback
    wizard.handle_key(TuiKey::Char('x'))?;
    let invalid_port_frame = wizard.render_frame(80, 24).render_to_string();
    assert!(invalid_port_frame.contains("[!] Invalid port"));
    wizard.handle_key(TuiKey::Backspace)?; // back to valid "8001"

    // Tab to PwdEdit (index 1). Test password masking
    wizard.handle_key(TuiKey::Tab)?;
    let pwd_frame = wizard.render_frame(80, 24).render_to_string();
    assert!(pwd_frame.contains("*********")); // "secret123" masked as 9 asterisks

    // Tab to ModeRadioAdv (index 3). Test Radio selection
    wizard.handle_key(TuiKey::Tab)?; // ModeRadioStd
    wizard.handle_key(TuiKey::Tab)?; // ModeRadioAdv
    wizard.handle_key(TuiKey::Space)?;
    assert_eq!(
        wizard.engine().context().get_property("STACK_MODE"),
        Some("Advanced")
    );
    let radio_frame = wizard.render_frame(80, 24).render_to_string();
    assert!(radio_frame.contains("(•) Advanced"));

    // Tab to TreeComponents (index 4)
    wizard.handle_key(TuiKey::Tab)?;
    let tree_frame = wizard.render_frame(80, 24).render_to_string();
    assert!(tree_frame.contains("Components Checklist:"));

    // Tab to NextBtn and advance to ProgressDlg
    wizard.handle_key(TuiKey::Tab)?;
    wizard.handle_key(TuiKey::Enter)?;
    assert_eq!(
        wizard.engine().active_dialog().map(|d| d.name.as_str()),
        Some("ProgressDlg")
    );

    // Live progress updating with action descriptions
    wizard.engine_mut().update_progress(60);
    wizard.set_action_text("Extracting courseware databases (60%)...");
    assert_eq!(
        wizard.action_text(),
        Some("Extracting courseware databases (60%)...")
    );

    let progress_frame = wizard.render_frame(80, 24).render_to_string();
    assert!(progress_frame.contains("60%"));
    assert!(progress_frame.contains("Extracting courseware databases"));

    // Toggle Diagnostics Log view via F2
    wizard.handle_key(TuiKey::F(2))?;
    assert!(wizard.is_diagnostics_log_open());
    let log_frame = wizard.render_frame(80, 24).render_to_string();
    assert!(log_frame.contains("Diagnostics Log [F2: Close]"));

    // Close Diagnostics Log via F2
    wizard.handle_key(TuiKey::F(2))?;
    assert!(!wizard.is_diagnostics_log_open());

    // Advance to completion on Enter
    let final_res = wizard.handle_key(TuiKey::Enter)?;
    assert_eq!(final_res, Some(DialogReturnCode::Return));

    // Render exit summary screen
    let ports = [
        ("LMS".to_string(), 8001),
        ("CMS / Studio".to_string(), 8002),
        ("MySQL".to_string(), 3306),
    ];
    let services = [
        "openedx-lms".to_string(),
        "openedx-cms".to_string(),
        "mysql".to_string(),
    ];
    let summary_buf = wizard.render_exit_summary(80, 24, "Open edX Stack", &ports, &services);
    let summary_str = summary_buf.render_to_string();
    assert!(summary_str.contains("Open edX Stack - Setup Complete"));
    assert!(summary_str.contains("• openedx-lms (running)"));
    assert!(summary_str.contains("• LMS: http://localhost:8001"));

    Ok(())
}

/// Tests terminal event stream dispatch and terminal resize handling.
#[test]
fn test_tui_wizard_stream_and_resize() -> Result<()> {
    let engine = create_test_tui_engine()?;
    let mut wizard = TerminalWizard::new(engine);

    // Handle resize event
    let (code, buf) = wizard.handle_event(
        TerminalEvent::Resize {
            cols: 100,
            rows: 30,
        },
        100,
        30,
    )?;
    assert_eq!(code, None);
    assert_eq!(buf.width, 100);
    assert_eq!(buf.height, 30);

    // Handle key event via event stream
    let (key_code, _key_buf) = wizard.handle_event(TerminalEvent::Key(TuiKey::BackTab), 100, 30)?;
    assert_eq!(key_code, None);

    // Escape cancellation
    let (esc_code, _esc_buf) = wizard.handle_event(TerminalEvent::Key(TuiKey::Escape), 100, 30)?;
    assert_eq!(esc_code, Some(DialogReturnCode::Exit));

    Ok(())
}
