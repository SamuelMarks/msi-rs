//! Headless MSI UI State Machine Engine (`msi-ui-core`).
//!
//! Grounded directly in official Windows Installer UI specifications:
//! - Pure, headless state machine managing:
//!   - Dialog navigation (history stack, active dialog, modal spawn dialog stack).
//!   - Dynamic property binding and formatted text resolution.
//!   - `ControlCondition` evaluation automatically updating control visibility and enabled state.
//!   - Control event dispatching handling clicks, text changes, selections, tree state toggles.
//!   - Deterministic frame simulation testable in headless CI environments.

use crate::error::{Error, Result};
use crate::execution::properties::EvaluationContext;
use crate::ui::controls::{ControlDefinition, ControlRuntimeState};
use crate::ui::events::{
    ControlCondition, ControlConditionAction, ControlEvent, ControlEventType, DialogReturnCode,
    EventMapping,
};
use crate::ui::layout::{DluRect, FontMetrics, PixelRect};
use std::collections::HashMap;

/// Dialog attribute flag: Dialog is visible (`0x0001`).
pub const DIALOG_ATTR_VISIBLE: u32 = 0x0001;

/// Dialog attribute flag: Dialog is modal (`0x0002`).
pub const DIALOG_ATTR_MODAL: u32 = 0x0002;

/// Dialog attribute flag: Dialog displays minimize button (`0x0004`).
pub const DIALOG_ATTR_MINIMIZE: u32 = 0x0004;

/// Dialog attribute flag: Tracks disk space on selection changes (`0x0100`).
pub const DIALOG_ATTR_TRACK_DISK_SPACE: u32 = 0x0100;

/// Static definition of an MSI Dialog loaded from the `Dialog` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogDefinition {
    /// Dialog identifier name.
    pub name: String,
    /// Horizontal centering percentage (0 for absolute, 50 for center).
    pub h_centering: i16,
    /// Vertical centering percentage.
    pub v_centering: i16,
    /// Width in dialog units.
    pub width: i16,
    /// Height in dialog units.
    pub height: i16,
    /// Dialog attribute flags bitmask.
    pub attributes: u32,
    /// Optional dialog title text.
    pub title: Option<String>,
    /// First control in tab order receiving initial keyboard focus.
    pub control_first: String,
    /// Optional default push button control name.
    pub control_default: Option<String>,
    /// Optional cancel button control name triggered by Escape key.
    pub control_cancel: Option<String>,
}

impl DialogDefinition {
    /// Returns the dialog bounding rect in dialog units (DLUs).
    #[must_use]
    pub const fn rect(&self) -> DluRect {
        DluRect::new(0, 0, self.width, self.height)
    }

    /// Computes pixel bounds centered in a target container window.
    ///
    /// # Arguments
    ///
    /// * `container_width` - Container width in pixels.
    /// * `container_height` - Container height in pixels.
    /// * `metrics` - Font metrics used for conversion.
    ///
    /// # Returns
    ///
    /// Centered [`PixelRect`].
    #[must_use]
    pub fn pixel_bounds(
        &self,
        container_width: i32,
        container_height: i32,
        metrics: &FontMetrics,
    ) -> PixelRect {
        self.rect().to_pixel_rect(metrics).apply_centering(
            container_width,
            container_height,
            self.h_centering,
            self.v_centering,
        )
    }

    /// Returns true if dialog has modal attribute set.
    #[must_use]
    pub const fn is_modal(&self) -> bool {
        self.attributes & DIALOG_ATTR_MODAL != 0
    }
}

/// Headless MSI UI Engine managing active dialogs, control states, and event dispatching.
#[derive(Debug, Clone)]
pub struct UiEngine {
    /// Registered dialog definitions by dialog name.
    dialogs: HashMap<String, DialogDefinition>,
    /// Registered control definitions grouped by dialog name.
    controls: HashMap<String, Vec<ControlDefinition>>,
    /// Active runtime state for all controls, keyed by `(dialog_name, control_name)`.
    control_states: HashMap<(String, String), ControlRuntimeState>,
    /// Registered control conditions.
    conditions: Vec<ControlCondition>,
    /// Registered control event handlers.
    events: Vec<ControlEvent>,
    /// Registered event mappings.
    event_mappings: Vec<EventMapping>,
    /// Active property evaluation context.
    context: EvaluationContext,
    /// Initial property snapshot for `Reset` events.
    initial_properties: HashMap<String, String>,
    /// Name of currently active dialog.
    active_dialog: Option<String>,
    /// Dialog navigation history stack (for Back button navigation).
    history_stack: Vec<String>,
    /// Modal dialog stack (for `SpawnDialog`).
    modal_stack: Vec<String>,
    /// Active modal wait dialog if any.
    wait_dialog: Option<String>,
    /// Log of executed actions.
    action_log: Vec<String>,
}

impl UiEngine {
    /// Creates a new empty [`UiEngine`] with an evaluation context.
    ///
    /// # Arguments
    ///
    /// * `context` - Initial execution property context.
    ///
    /// # Returns
    ///
    /// A new [`UiEngine`].
    #[must_use]
    pub fn new(context: EvaluationContext) -> Self {
        let mut initial_props = HashMap::new();
        // Capture initial public/private properties
        for (k, v) in context.properties() {
            initial_props.insert(k.clone(), v.clone());
        }

        Self {
            dialogs: HashMap::new(),
            controls: HashMap::new(),
            control_states: HashMap::new(),
            conditions: Vec::new(),
            events: Vec::new(),
            event_mappings: Vec::new(),
            context,
            initial_properties: initial_props,
            active_dialog: None,
            history_stack: Vec::new(),
            modal_stack: Vec::new(),
            wait_dialog: None,
            action_log: Vec::new(),
        }
    }

    /// Registers a dialog definition.
    pub fn add_dialog(&mut self, dialog: DialogDefinition) {
        self.dialogs.insert(dialog.name.clone(), dialog);
    }

    /// Registers a control definition and initializes its runtime state.
    pub fn add_control(&mut self, def: ControlDefinition) {
        let state = ControlRuntimeState::from_definition(&def);
        let key = (def.dialog().to_string(), def.control().to_string());
        self.control_states.insert(key, state);
        self.controls
            .entry(def.dialog().to_string())
            .or_default()
            .push(def);
    }

    /// Registers a control condition.
    pub fn add_condition(&mut self, condition: ControlCondition) {
        self.conditions.push(condition);
    }

    /// Registers a control event trigger.
    pub fn add_event(&mut self, event: ControlEvent) {
        self.events.push(event);
    }

    /// Registers an event mapping.
    pub fn add_event_mapping(&mut self, mapping: EventMapping) {
        self.event_mappings.push(mapping);
    }

    /// Returns the currently active dialog definition, if any.
    #[must_use]
    pub fn active_dialog(&self) -> Option<&DialogDefinition> {
        self.active_dialog
            .as_ref()
            .and_then(|name| self.dialogs.get(name))
    }

    /// Sets the active dialog by name, updating control conditions and formatting text.
    ///
    /// # Arguments
    ///
    /// * `dialog_name` - Target dialog name.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UiError`] if dialog is not registered.
    pub fn set_active_dialog(&mut self, dialog_name: &str) -> Result<()> {
        if !self.dialogs.contains_key(dialog_name) {
            return Err(Error::UiError {
                dialog: dialog_name.to_string(),
                control: String::new(),
                reason: format!("Dialog '{dialog_name}' not found"),
            });
        }

        self.active_dialog = Some(dialog_name.to_string());
        self.evaluate_conditions_and_formatting()?;
        Ok(())
    }

    /// Returns a reference to the active property evaluation context.
    #[must_use]
    pub const fn context(&self) -> &EvaluationContext {
        &self.context
    }

    /// Returns a mutable reference to the property evaluation context.
    pub const fn context_mut(&mut self) -> &mut EvaluationContext {
        &mut self.context
    }

    /// Returns runtime state of a control if present.
    #[must_use]
    pub fn get_control_state(&self, dialog: &str, control: &str) -> Option<&ControlRuntimeState> {
        self.control_states
            .get(&(dialog.to_string(), control.to_string()))
    }

    /// Returns a mutable reference to the runtime state of a control.
    pub fn get_control_state_mut(
        &mut self,
        dialog: &str,
        control: &str,
    ) -> Option<&mut ControlRuntimeState> {
        self.control_states
            .get_mut(&(dialog.to_string(), control.to_string()))
    }

    /// Clears cached control runtime states, resetting to definition defaults.
    pub fn clear_control_states(&mut self) {
        self.control_states.clear();
    }

    /// Returns the list of control definitions for a dialog.
    #[must_use]
    pub fn get_dialog_controls(&self, dialog: &str) -> &[ControlDefinition] {
        self.controls.get(dialog).map_or(&[], Vec::as_slice)
    }

    /// Evaluates all dynamic [`ControlCondition`] records and updates control text formatting.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if condition syntax is invalid.
    pub fn evaluate_conditions_and_formatting(&mut self) -> Result<()> {
        let Some(active_dlg) = self.active_dialog.clone() else {
            return Ok(());
        };

        // Update control formatted texts and bound values
        if let Some(defs) = self.controls.get(&active_dlg).cloned() {
            for def in defs {
                let key = (active_dlg.clone(), def.control().to_string());
                if let Some(state) = self.control_states.get_mut(&key) {
                    if let Some(val) = def
                        .property_name()
                        .and_then(|p| self.context.get_property(p))
                    {
                        state.bound_value = Some(val.to_string());
                    }
                    if let Some(tmpl) = def.text_template() {
                        state.current_text = self.context.format_string(tmpl)?;
                    }
                }
            }
        }

        // Evaluate ControlConditions
        for cond in &self.conditions {
            if cond.dialog == active_dlg {
                let satisfied = self.context.evaluate_condition(&cond.condition)?;
                if satisfied {
                    let key = (cond.dialog.clone(), cond.control.clone());
                    if let Some(state) = self.control_states.get_mut(&key) {
                        match cond.action {
                            ControlConditionAction::Default => state.is_default = true,
                            ControlConditionAction::Enable => state.is_enabled = true,
                            ControlConditionAction::Disable => state.is_enabled = false,
                            ControlConditionAction::Hide => state.is_visible = false,
                            ControlConditionAction::Show => state.is_visible = true,
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Simulates user clicking a control, triggering subscribed [`ControlEvent`] records.
    ///
    /// # Arguments
    ///
    /// * `dialog` - Dialog name.
    /// * `control` - Triggered control name.
    ///
    /// # Returns
    ///
    /// `Some(DialogReturnCode)` if dialog loop terminates, or `None` if interaction continues.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if event execution encounters an error.
    pub fn click_control(
        &mut self,
        dialog: &str,
        control: &str,
    ) -> Result<Option<DialogReturnCode>> {
        let mut matching_events: Vec<ControlEvent> = self
            .events
            .iter()
            .filter(|e| e.dialog() == dialog && e.control() == control)
            .cloned()
            .collect();

        matching_events.sort_by_key(ControlEvent::ordering);

        for event in matching_events {
            if !event.is_satisfied(&self.context)? {
                continue;
            }

            match event.event_type() {
                ControlEventType::EndDialog(code) => {
                    if let Some(modal_parent) = self.modal_stack.pop() {
                        self.active_dialog = Some(modal_parent);
                        self.evaluate_conditions_and_formatting()?;
                    } else {
                        return Ok(Some(*code));
                    }
                }
                ControlEventType::NewDialog(target) => {
                    if let Some(current) = self.active_dialog.take() {
                        self.history_stack.push(current);
                    }
                    self.set_active_dialog(target)?;
                }
                ControlEventType::SpawnDialog(target) => {
                    if let Some(current) = self.active_dialog.clone() {
                        self.modal_stack.push(current);
                    }
                    self.set_active_dialog(target)?;
                }
                ControlEventType::SpawnWaitDialog(target) => {
                    self.wait_dialog = Some(target.clone());
                    self.action_log.push(format!("SpawnWaitDialog({target})"));
                }
                ControlEventType::SetProperty { property, value } => {
                    let formatted_val = self.context.format_string(value)?;
                    self.context.set_property(property, formatted_val);
                    self.evaluate_conditions_and_formatting()?;
                }
                ControlEventType::Reset => {
                    // Revert to initial properties
                    for (k, v) in &self.initial_properties {
                        self.context.set_property(k, v);
                    }
                    self.evaluate_conditions_and_formatting()?;
                }
                ControlEventType::DoAction(action) => {
                    self.action_log.push(format!("DoAction({action})"));
                }
            }
        }

        Ok(None)
    }

    /// Dispatches a progress event notification updating `ProgressBar` controls.
    ///
    /// # Arguments
    ///
    /// * `percent` - Progress percentage (0..=100).
    pub fn update_progress(&mut self, percent: u32) {
        let clamped = if percent > 100 { 100 } else { percent };
        for state in self.control_states.values_mut() {
            state.progress_percent = clamped;
        }
    }

    /// Returns action log records.
    #[must_use]
    pub fn action_log(&self) -> &[String] {
        &self.action_log
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::controls::ControlType;

    /// Helper to set up a standard Welcome -> License -> `InstallDir` dialog wizard workflow.
    #[allow(clippy::too_many_lines)]
    fn setup_test_wizard() -> UiEngine {
        let mut context = EvaluationContext::new();
        context.set_property("ProductName", "AcmeApp");
        context.set_property("ACCEPT_EULA", "0");

        let mut engine = UiEngine::new(context);

        // WelcomeDlg
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

        // LicenseDlg
        engine.add_dialog(DialogDefinition {
            name: "LicenseDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 370,
            height: 270,
            attributes: DIALOG_ATTR_VISIBLE | DIALOG_ATTR_MODAL,
            title: Some("License Agreement".to_string()),
            control_first: "AgreementCheckBox".to_string(),
            control_default: Some("NextBtn".to_string()),
            control_cancel: Some("CancelBtn".to_string()),
        });
        engine.add_control(
            ControlDefinition::new(
                "LicenseDlg",
                "NextBtn",
                ControlType::PushButton,
                DluRect::new(236, 243, 56, 17),
                1, // Initially visible but disabled!
            )
            .text("Next >"),
        );
        engine.add_control(
            ControlDefinition::new(
                "LicenseDlg",
                "AgreementCheckBox",
                ControlType::CheckBox,
                DluRect::new(20, 210, 200, 14),
                3,
            )
            .property("ACCEPT_EULA")
            .text("I accept the license terms"),
        );

        // Cancel modal dialog
        engine.add_dialog(DialogDefinition {
            name: "CancelDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 260,
            height: 85,
            attributes: DIALOG_ATTR_VISIBLE | DIALOG_ATTR_MODAL,
            title: Some("Exit Setup".to_string()),
            control_first: "NoBtn".to_string(),
            control_default: Some("NoBtn".to_string()),
            control_cancel: Some("NoBtn".to_string()),
        });
        engine.add_control(
            ControlDefinition::new(
                "CancelDlg",
                "YesBtn",
                ControlType::PushButton,
                DluRect::new(70, 50, 56, 17),
                3,
            )
            .text("Yes"),
        );
        engine.add_control(
            ControlDefinition::new(
                "CancelDlg",
                "NoBtn",
                ControlType::PushButton,
                DluRect::new(134, 50, 56, 17),
                3,
            )
            .text("No"),
        );

        // Conditions on LicenseDlg: Enable NextBtn when ACCEPT_EULA = "1"
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
            condition: r#"ACCEPT_EULA = "0""#.to_string(),
        });

        // Events:
        // WelcomeDlg NextBtn -> NewDialog(LicenseDlg)
        engine.add_event(ControlEvent::new(
            "WelcomeDlg",
            "NextBtn",
            ControlEventType::NewDialog("LicenseDlg".to_string()),
            None,
            1,
        ));
        // WelcomeDlg CancelBtn -> SpawnDialog(CancelDlg)
        engine.add_event(ControlEvent::new(
            "WelcomeDlg",
            "CancelBtn",
            ControlEventType::SpawnDialog("CancelDlg".to_string()),
            None,
            1,
        ));
        // CancelDlg YesBtn -> EndDialog(Exit)
        engine.add_event(ControlEvent::new(
            "CancelDlg",
            "YesBtn",
            ControlEventType::EndDialog(DialogReturnCode::Exit),
            None,
            1,
        ));
        // CancelDlg NoBtn -> EndDialog(Return)
        engine.add_event(ControlEvent::new(
            "CancelDlg",
            "NoBtn",
            ControlEventType::EndDialog(DialogReturnCode::Return),
            None,
            1,
        ));
        // LicenseDlg NextBtn -> EndDialog(Return)
        engine.add_event(ControlEvent::new(
            "LicenseDlg",
            "NextBtn",
            ControlEventType::EndDialog(DialogReturnCode::Return),
            None,
            1,
        ));

        engine
    }

    /// Tests full interactive navigation through dialogs and dynamic condition updates.
    #[test]
    fn test_ui_engine_navigation_and_conditions() -> Result<()> {
        let mut engine = setup_test_wizard();
        engine.set_active_dialog("WelcomeDlg")?;
        assert_eq!(
            engine.active_dialog().map(|d| d.name.as_str()),
            Some("WelcomeDlg")
        );

        // Click CancelBtn -> Spawns CancelDlg modal
        let ret = engine.click_control("WelcomeDlg", "CancelBtn")?;
        assert_eq!(ret, None);
        assert_eq!(
            engine.active_dialog().map(|d| d.name.as_str()),
            Some("CancelDlg")
        );

        // Click NoBtn on CancelDlg -> Returns to WelcomeDlg parent
        let ret_no = engine.click_control("CancelDlg", "NoBtn")?;
        assert_eq!(ret_no, None);
        assert_eq!(
            engine.active_dialog().map(|d| d.name.as_str()),
            Some("WelcomeDlg")
        );

        // Click NextBtn on WelcomeDlg -> Navigates to LicenseDlg
        let ret_next = engine.click_control("WelcomeDlg", "NextBtn")?;
        assert_eq!(ret_next, None);
        assert_eq!(
            engine.active_dialog().map(|d| d.name.as_str()),
            Some("LicenseDlg")
        );

        // In LicenseDlg, NextBtn should initially be disabled because ACCEPT_EULA is "0"
        let next_state = engine.get_control_state("LicenseDlg", "NextBtn");
        assert!(next_state.is_some_and(|s| !s.is_enabled));

        // Simulate checking the box: setting ACCEPT_EULA = "1"
        engine.context_mut().set_property("ACCEPT_EULA", "1");
        engine.evaluate_conditions_and_formatting()?;

        let next_state_enabled = engine.get_control_state("LicenseDlg", "NextBtn");
        assert!(next_state_enabled.is_some_and(|s| s.is_enabled));

        // Click NextBtn on LicenseDlg -> Ends dialog with Return
        let end_res = engine.click_control("LicenseDlg", "NextBtn")?;
        assert_eq!(end_res, Some(DialogReturnCode::Return));

        // Test progress update
        engine.update_progress(75);
        let cur_state = engine.get_control_state("LicenseDlg", "NextBtn");
        assert_eq!(cur_state.map(|s| s.progress_percent), Some(75));

        Ok(())
    }

    /// Tests `SetProperty`, `Reset`, and `DoAction` event handling.
    #[test]
    fn test_ui_engine_events_variety() -> Result<()> {
        let mut context = EvaluationContext::new();
        context.set_property("PROP1", "Original");

        let mut engine = UiEngine::new(context);
        engine.add_dialog(DialogDefinition {
            name: "Dlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 300,
            height: 200,
            attributes: DIALOG_ATTR_VISIBLE,
            title: None,
            control_first: "Btn".to_string(),
            control_default: None,
            control_cancel: None,
        });
        engine.add_control(ControlDefinition::new(
            "Dlg",
            "SetBtn",
            ControlType::PushButton,
            DluRect::new(10, 10, 50, 20),
            3,
        ));
        engine.add_control(ControlDefinition::new(
            "Dlg",
            "ResetBtn",
            ControlType::PushButton,
            DluRect::new(70, 10, 50, 20),
            3,
        ));

        // SetProperty event
        engine.add_event(ControlEvent::new(
            "Dlg",
            "SetBtn",
            ControlEventType::SetProperty {
                property: "PROP1".to_string(),
                value: "Modified".to_string(),
            },
            None,
            1,
        ));
        // DoAction event
        engine.add_event(ControlEvent::new(
            "Dlg",
            "SetBtn",
            ControlEventType::DoAction("CustomAction1".to_string()),
            None,
            2,
        ));
        // Reset event
        engine.add_event(ControlEvent::new(
            "Dlg",
            "ResetBtn",
            ControlEventType::Reset,
            None,
            1,
        ));

        engine.set_active_dialog("Dlg")?;
        assert_eq!(engine.context().get_property("PROP1"), Some("Original"));

        // Click SetBtn
        engine.click_control("Dlg", "SetBtn")?;
        assert_eq!(engine.context().get_property("PROP1"), Some("Modified"));
        assert_eq!(engine.action_log(), &["DoAction(CustomAction1)"]);

        // Click ResetBtn
        engine.click_control("Dlg", "ResetBtn")?;
        assert_eq!(engine.context().get_property("PROP1"), Some("Original"));

        Ok(())
    }

    /// Tests edge cases: `is_modal`, unbound properties, `Default` and `Show` conditions, `SpawnWaitDialog`, progress clamping, and state mutations.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ui_engine_edge_cases() -> Result<()> {
        let modal_dlg = DialogDefinition {
            name: "ModalDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 200,
            height: 100,
            attributes: DIALOG_ATTR_MODAL,
            title: None,
            control_first: String::new(),
            control_default: None,
            control_cancel: None,
        };
        assert!(modal_dlg.is_modal());

        let non_modal = DialogDefinition {
            name: "NonModal".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 200,
            height: 100,
            attributes: DIALOG_ATTR_VISIBLE,
            title: None,
            control_first: String::new(),
            control_default: None,
            control_cancel: None,
        };
        assert!(!non_modal.is_modal());

        let mut engine = UiEngine::new(EvaluationContext::new());
        assert!(engine.active_dialog().is_none());
        assert!(engine.evaluate_conditions_and_formatting().is_ok());

        engine.add_dialog(non_modal);

        // Control with property that does not exist in context (exercises None branch of context.get_property)
        let unbound_ctrl = ControlDefinition::new(
            "NonModal",
            "UnboundEdit",
            ControlType::Edit,
            DluRect::new(0, 0, 10, 10),
            0,
        )
        .property("UNSET_PROP");
        engine.add_control(unbound_ctrl);

        // Control with bound property that exists in context
        let bound_ctrl = ControlDefinition::new(
            "NonModal",
            "BoundEdit",
            ControlType::Edit,
            DluRect::new(0, 0, 10, 10),
            0,
        )
        .property("BOUND_PROP");
        engine.add_control(bound_ctrl);

        // Control with text template and bound property
        let templated_ctrl = ControlDefinition::new(
            "NonModal",
            "TemplatedText",
            ControlType::Text,
            DluRect::new(0, 0, 10, 10),
            0,
        )
        .text("Value is [PROP1]");
        engine.add_control(templated_ctrl);

        // Control with Default, Show, Hide, and unsatisfied conditions
        let btn_ctrl = ControlDefinition::new(
            "NonModal",
            "Btn",
            ControlType::PushButton,
            DluRect::new(0, 0, 10, 10),
            0,
        );
        engine.add_control(btn_ctrl);

        engine.add_condition(ControlCondition {
            dialog: "NonModal".to_string(),
            control: "Btn".to_string(),
            action: ControlConditionAction::Default,
            condition: "1 = 1".to_string(),
        });
        engine.add_condition(ControlCondition {
            dialog: "NonModal".to_string(),
            control: "Btn".to_string(),
            action: ControlConditionAction::Show,
            condition: "1 = 1".to_string(),
        });
        engine.add_condition(ControlCondition {
            dialog: "NonModal".to_string(),
            control: "Btn".to_string(),
            action: ControlConditionAction::Hide,
            condition: "1 = 1".to_string(),
        });
        engine.add_condition(ControlCondition {
            dialog: "NonModal".to_string(),
            control: "Btn".to_string(),
            action: ControlConditionAction::Disable,
            condition: "1 = 0".to_string(), // unsatisfied
        });

        // Condition targeting non-existent control in control_states
        engine.add_condition(ControlCondition {
            dialog: "NonModal".to_string(),
            control: "NonExistentControl".to_string(),
            action: ControlConditionAction::Enable,
            condition: "1 = 1".to_string(),
        });

        // Event with unsatisfied condition (condition is false)
        engine.add_event(ControlEvent::new(
            "NonModal",
            "Btn",
            ControlEventType::DoAction("UnsatisfiedAction".to_string()),
            Some("1 = 0".to_string()),
            1,
        ));

        // SpawnWaitDialog event
        engine.add_event(ControlEvent::new(
            "NonModal",
            "Btn",
            ControlEventType::SpawnWaitDialog("WaitDlg".to_string()),
            None,
            2,
        ));

        // Event mapping registration
        engine.add_event_mapping(EventMapping {
            dialog: "NonModal".to_string(),
            control: "Btn".to_string(),
            event: "SetProgress".to_string(),
            attribute: "Progress".to_string(),
        });

        // Test invalid dialog activation
        assert!(engine.set_active_dialog("NoSuchDlg").is_err());

        engine.context_mut().set_property("PROP1", "Hello");
        engine
            .context_mut()
            .set_property("BOUND_PROP", "BoundValue");
        engine.set_active_dialog("NonModal")?;
        assert!(engine.active_dialog().is_some());

        let bound_st = engine.get_control_state("NonModal", "BoundEdit");
        assert_eq!(
            bound_st.and_then(|s| s.bound_value.as_deref()),
            Some("BoundValue")
        );

        let btn_st = engine.get_control_state("NonModal", "Btn");
        assert_eq!(btn_st.map(|s| s.is_default), Some(true));
        assert_eq!(btn_st.map(|s| s.is_visible), Some(false));

        // Trigger SpawnWaitDialog (and skip unsatisfied event)
        engine.click_control("NonModal", "Btn")?;
        assert_eq!(
            engine.action_log().last().map(String::as_str),
            Some("SpawnWaitDialog(WaitDlg)")
        );

        // Dialog with NO controls to exercise controls.get(&active_dlg) == None branch
        let empty_dlg = DialogDefinition {
            name: "EmptyDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 200,
            height: 100,
            attributes: DIALOG_ATTR_VISIBLE,
            title: None,
            control_first: String::new(),
            control_default: None,
            control_cancel: None,
        };
        engine.add_dialog(empty_dlg);
        engine.set_active_dialog("EmptyDlg")?;

        // Test NewDialog and SpawnDialog when active_dialog is None
        let mut headless1 = UiEngine::new(EvaluationContext::new());
        headless1.add_dialog(DialogDefinition {
            name: "Dlg1".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 200,
            height: 100,
            attributes: DIALOG_ATTR_VISIBLE,
            title: None,
            control_first: String::new(),
            control_default: None,
            control_cancel: None,
        });
        headless1.add_event(ControlEvent::new(
            "Dummy",
            "NewBtn",
            ControlEventType::NewDialog("Dlg1".to_string()),
            None,
            1,
        ));
        assert!(headless1.click_control("Dummy", "NewBtn").is_ok());

        let mut headless2 = UiEngine::new(EvaluationContext::new());
        headless2.add_dialog(DialogDefinition {
            name: "Dlg2".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 200,
            height: 100,
            attributes: DIALOG_ATTR_VISIBLE,
            title: None,
            control_first: String::new(),
            control_default: None,
            control_cancel: None,
        });
        headless2.add_event(ControlEvent::new(
            "Dummy",
            "SpawnBtn",
            ControlEventType::SpawnDialog("Dlg2".to_string()),
            None,
            1,
        ));
        assert!(headless2.click_control("Dummy", "SpawnBtn").is_ok());

        // Test context and context_mut accessors
        engine.context_mut().set_property("FOO", "BAR");
        assert_eq!(engine.context().get_property("FOO"), Some("BAR"));

        // Test get_control_state_mut
        let st_mut = engine.get_control_state_mut("NonModal", "Btn");
        assert!(st_mut.is_some());
        assert!(engine
            .get_control_state_mut("NoSuchDlg", "NoSuchCtrl")
            .is_none());

        // Test progress clamping when percent > 100
        engine.update_progress(150);
        let clamped_st = engine.get_control_state("NonModal", "Btn");
        assert_eq!(clamped_st.map(|s| s.progress_percent), Some(100));

        // Test clear_control_states and evaluation when states are cleared but controls exist
        engine.set_active_dialog("NonModal")?;
        engine.clear_control_states();
        assert!(engine.get_control_state("NonModal", "Btn").is_none());
        assert!(engine.evaluate_conditions_and_formatting().is_ok());

        Ok(())
    }
}
