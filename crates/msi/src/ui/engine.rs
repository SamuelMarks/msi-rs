//! Headless MSI UI State Machine Engine (`msi-ui-core`).
//!
//! Grounded directly in official Windows Installer UI specifications:
//! - Pure, headless state machine managing:
//!   - Dialog navigation (history stack, active dialog, modal spawn dialog stack).
//!   - Dynamic property binding and formatted text resolution.
//!   - `ControlCondition` evaluation automatically updating control visibility and enabled state.
//!   - Control event dispatching handling clicks, text changes, selections, tree state toggles.
//!   - Deterministic frame simulation testable in headless CI environments.

use crate::database::tables::record::FieldValue;
use crate::error::{Error, Result};
use crate::execution::custom_action::{CustomActionDefinition, CustomActionExecutor};
use crate::execution::properties::EvaluationContext;
use crate::ui::controls::{ControlDefinition, ControlRuntimeState, ControlType};
use crate::ui::events::{
    ControlCondition, ControlConditionAction, ControlEvent, ControlEventType, DialogReturnCode,
    EventMapping,
};
use crate::ui::layout::{DluRect, FontMetrics, PixelRect};
use crate::wix::linker::LinkedDatabase;
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
    /// Registered custom actions by action name.
    custom_actions: HashMap<String, CustomActionDefinition>,
    /// Coordinator for synchronous custom action execution during the UI phase.
    custom_action_executor: Option<CustomActionExecutor>,
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
            custom_actions: HashMap::new(),
            custom_action_executor: None,
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
                        if def.control_type() == ControlType::Edit {
                            state.current_text = val.to_string();
                        }
                    }
                    if def.control_type() != ControlType::Edit || def.property_name().is_none() {
                        if let Some(tmpl) = def.text_template() {
                            state.current_text = self.context.format_string(tmpl)?;
                        }
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
                    if let Some(ca_def) = self.custom_actions.get(action).cloned() {
                        if let Some(ref mut executor) = self.custom_action_executor {
                            executor.execute(&ca_def, &mut self.context)?;
                            self.evaluate_conditions_and_formatting()?;
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Registers a custom action definition for execution during control events (`DoAction`).
    ///
    /// # Arguments
    ///
    /// * `action` - Custom action definition.
    pub fn add_custom_action(&mut self, action: CustomActionDefinition) {
        self.custom_actions
            .insert(action.name().to_string(), action);
    }

    /// Configures the custom action executor for synchronous action execution.
    ///
    /// # Arguments
    ///
    /// * `executor` - Configured [`CustomActionExecutor`].
    pub fn set_custom_action_executor(&mut self, executor: CustomActionExecutor) {
        self.custom_action_executor = Some(executor);
    }

    /// Returns a reference to the active custom action executor, if configured.
    ///
    /// # Returns
    ///
    /// Optional reference to [`CustomActionExecutor`].
    #[must_use]
    pub const fn custom_action_executor(&self) -> Option<&CustomActionExecutor> {
        self.custom_action_executor.as_ref()
    }

    /// Returns a mutable reference to the active custom action executor, if configured.
    ///
    /// # Returns
    ///
    /// Optional mutable reference to [`CustomActionExecutor`].
    pub const fn custom_action_executor_mut(&mut self) -> Option<&mut CustomActionExecutor> {
        self.custom_action_executor.as_mut()
    }

    /// Updates a control's runtime value and synchronizes its bound MSI property.
    ///
    /// Immediately triggers dynamic [`ControlCondition`] re-evaluation.
    ///
    /// # Arguments
    ///
    /// * `dialog` - Enclosing dialog name.
    /// * `control` - Target control identifier name.
    /// * `value` - Updated text or property value.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if property condition evaluation fails.
    pub fn update_control_value(&mut self, dialog: &str, control: &str, value: &str) -> Result<()> {
        let key = (dialog.to_string(), control.to_string());
        if let Some(state) = self.control_states.get_mut(&key) {
            state.bound_value = Some(value.to_string());
            state.current_text = value.to_string();
        }

        let bound_prop = self
            .controls
            .get(dialog)
            .and_then(|defs| defs.iter().find(|d| d.control() == control))
            .and_then(ControlDefinition::property_name)
            .map(ToString::to_string);

        if let Some(prop) = bound_prop {
            self.context.set_property(&prop, value);
        }

        self.evaluate_conditions_and_formatting()?;
        Ok(())
    }

    /// Toggles a checkbox control between `"1"` and `"0"`, updating its bound property.
    ///
    /// # Arguments
    ///
    /// * `dialog` - Enclosing dialog name.
    /// * `control` - Checkbox control identifier name.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if condition evaluation fails.
    pub fn toggle_checkbox(&mut self, dialog: &str, control: &str) -> Result<()> {
        let key = (dialog.to_string(), control.to_string());
        let cur_val = self
            .control_states
            .get(&key)
            .and_then(|s| s.bound_value.as_deref())
            .unwrap_or("0");
        let next_val = if cur_val == "1" { "0" } else { "1" };
        self.update_control_value(dialog, control, next_val)
    }

    /// Selects an option within a radio button group, updating the group property identifier.
    ///
    /// # Arguments
    ///
    /// * `dialog` - Enclosing dialog name.
    /// * `control` - Radio button group control identifier.
    /// * `value` - Selected option value string.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if condition evaluation fails.
    pub fn select_radio_button(&mut self, dialog: &str, control: &str, value: &str) -> Result<()> {
        self.update_control_value(dialog, control, value)
    }

    /// Loads dialog definitions, controls, events, conditions, and actions directly from a [`LinkedDatabase`].
    ///
    /// # Arguments
    ///
    /// * `db` - Compiled or linked MSI database.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if loading or initial condition formatting fails.
    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::many_single_char_names,
        clippy::cast_sign_loss
    )]
    pub fn load_from_database(&mut self, db: &LinkedDatabase) -> Result<()> {
        // 1. Load Dialog table
        for rec in db.get_records("Dialog") {
            let Some(FieldValue::String(name)) = rec.get(0) else {
                continue;
            };
            let h_centering = match rec.get(1) {
                Some(FieldValue::Short(v)) => *v,
                Some(FieldValue::Long(v)) => *v as i16,
                _ => 50,
            };
            let v_centering = match rec.get(2) {
                Some(FieldValue::Short(v)) => *v,
                Some(FieldValue::Long(v)) => *v as i16,
                _ => 50,
            };
            let width = match rec.get(3) {
                Some(FieldValue::Short(v)) => *v,
                Some(FieldValue::Long(v)) => *v as i16,
                _ => 370,
            };
            let height = match rec.get(4) {
                Some(FieldValue::Short(v)) => *v,
                Some(FieldValue::Long(v)) => *v as i16,
                _ => 270,
            };
            let attributes = match rec.get(5) {
                Some(FieldValue::Long(v)) => *v as u32,
                Some(FieldValue::Short(v)) => *v as u32,
                _ => 3,
            };
            let title = match rec.get(6) {
                Some(FieldValue::String(s)) => Some(s.clone()),
                _ => None,
            };
            let control_first = match rec.get(7) {
                Some(FieldValue::String(s)) => s.clone(),
                _ => String::new(),
            };
            let control_default = match rec.get(8) {
                Some(FieldValue::String(s)) => Some(s.clone()),
                _ => None,
            };
            let control_cancel = match rec.get(9) {
                Some(FieldValue::String(s)) => Some(s.clone()),
                _ => None,
            };
            self.add_dialog(DialogDefinition {
                name: name.clone(),
                h_centering,
                v_centering,
                width,
                height,
                attributes,
                title,
                control_first,
                control_default,
                control_cancel,
            });
        }

        // 2. Load Control table
        for rec in db.get_records("Control") {
            let Some(FieldValue::String(dialog)) = rec.get(0) else {
                continue;
            };
            let Some(FieldValue::String(control)) = rec.get(1) else {
                continue;
            };
            let ctype_str = match rec.get(2) {
                Some(FieldValue::String(s)) => s.as_str(),
                _ => "PushButton",
            };
            let control_type = ControlType::from_name(ctype_str).unwrap_or(ControlType::PushButton);
            let x = match rec.get(3) {
                Some(FieldValue::Short(v)) => *v,
                Some(FieldValue::Long(v)) => *v as i16,
                _ => 0,
            };
            let y = match rec.get(4) {
                Some(FieldValue::Short(v)) => *v,
                Some(FieldValue::Long(v)) => *v as i16,
                _ => 0,
            };
            let w = match rec.get(5) {
                Some(FieldValue::Short(v)) => *v,
                Some(FieldValue::Long(v)) => *v as i16,
                _ => 56,
            };
            let h = match rec.get(6) {
                Some(FieldValue::Short(v)) => *v,
                Some(FieldValue::Long(v)) => *v as i16,
                _ => 17,
            };
            let attributes = match rec.get(7) {
                Some(FieldValue::Long(v)) => *v as u32,
                Some(FieldValue::Short(v)) => *v as u32,
                _ => 3,
            };
            let property = match rec.get(8) {
                Some(FieldValue::String(s)) => Some(s.clone()),
                _ => None,
            };
            let text = match rec.get(9) {
                Some(FieldValue::String(s)) => Some(s.clone()),
                _ => None,
            };
            let control_next = match rec.get(10) {
                Some(FieldValue::String(s)) => Some(s.clone()),
                _ => None,
            };
            let help = match rec.get(11) {
                Some(FieldValue::String(s)) => Some(s.clone()),
                _ => None,
            };

            let mut def = ControlDefinition::new(
                dialog,
                control,
                control_type,
                DluRect::new(x, y, w, h),
                attributes,
            );
            if let Some(p) = property {
                def = def.property(p);
            }
            if let Some(t) = text {
                def = def.text(t);
            }
            if let Some(n) = control_next {
                def = def.control_next(n);
            }
            if let Some(hp) = help {
                def = def.help(hp);
            }
            self.add_control(def);
        }

        // 3. Load ControlCondition table
        for rec in db.get_records("ControlCondition") {
            let (
                Some(FieldValue::String(dialog)),
                Some(FieldValue::String(control)),
                Some(FieldValue::String(action_str)),
                Some(FieldValue::String(condition)),
            ) = (rec.get(0), rec.get(1), rec.get(2), rec.get(3))
            else {
                continue;
            };
            if let Ok(action) = ControlConditionAction::from_action(action_str) {
                self.add_condition(ControlCondition {
                    dialog: dialog.clone(),
                    control: control.clone(),
                    action,
                    condition: condition.clone(),
                });
            }
        }

        // 4. Load ControlEvent table
        for rec in db.get_records("ControlEvent") {
            let (
                Some(FieldValue::String(dialog)),
                Some(FieldValue::String(control)),
                Some(FieldValue::String(event_name)),
                Some(FieldValue::String(arg)),
            ) = (rec.get(0), rec.get(1), rec.get(2), rec.get(3))
            else {
                continue;
            };
            let cond = match rec.get(4) {
                Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
                _ => None,
            };
            let order = match rec.get(5) {
                Some(FieldValue::Short(v)) => *v,
                Some(FieldValue::Long(v)) => *v as i16,
                _ => 1,
            };
            if let Ok(event_type) = ControlEventType::parse(event_name, arg) {
                self.add_event(ControlEvent::new(dialog, control, event_type, cond, order));
            }
        }

        // 5. Load EventMapping table
        for rec in db.get_records("EventMapping") {
            let (
                Some(FieldValue::String(dialog)),
                Some(FieldValue::String(control)),
                Some(FieldValue::String(event)),
                Some(FieldValue::String(attr)),
            ) = (rec.get(0), rec.get(1), rec.get(2), rec.get(3))
            else {
                continue;
            };
            self.add_event_mapping(EventMapping {
                dialog: dialog.clone(),
                control: control.clone(),
                event: event.clone(),
                attribute: attr.clone(),
            });
        }

        // 6. Load Binary table payloads and CustomAction table records
        let mut executor = self.custom_action_executor.take().unwrap_or_default();
        for b in db.get_records("Binary") {
            if let Some(FieldValue::String(b_name)) = b.get(0) {
                let b_data = match b.get(1) {
                    Some(FieldValue::String(s)) => s.as_bytes().to_vec(),
                    _ => Vec::new(),
                };
                executor.add_binary(b_name.clone(), b_data);
            }
        }
        for rec in db.get_records("CustomAction") {
            let (
                Some(FieldValue::String(name)),
                Some(FieldValue::String(source)),
                Some(FieldValue::String(target)),
            ) = (rec.get(0), rec.get(2), rec.get(3))
            else {
                continue;
            };
            let raw_type = match rec.get(1) {
                Some(FieldValue::Long(v)) => *v as u32,
                Some(FieldValue::Short(v)) => *v as u32,
                _ => 0,
            };
            if let Ok(ca_def) = CustomActionDefinition::parse(name, raw_type, source, target) {
                self.add_custom_action(ca_def);
            }
        }
        self.custom_action_executor = Some(executor);

        // 7. Determine initial active dialog from InstallUISequence or standard fallbacks
        let mut initial_dialog = None;
        let mut min_seq = i32::MAX;
        for rec in db.get_records("InstallUISequence") {
            if let (Some(FieldValue::String(action)), Some(FieldValue::Short(seq))) =
                (rec.get(0), rec.get(2))
            {
                if self.dialogs.contains_key(action) && i32::from(*seq) < min_seq {
                    min_seq = i32::from(*seq);
                    initial_dialog = Some(action.clone());
                }
            }
        }

        if initial_dialog.is_none() {
            if self.dialogs.contains_key("WelcomeDlg") {
                initial_dialog = Some("WelcomeDlg".to_string());
            } else if self.dialogs.contains_key("Welcome") {
                initial_dialog = Some("Welcome".to_string());
            } else {
                initial_dialog = self.dialogs.keys().next().cloned();
            }
        }

        if let Some(ref dlg_name) = initial_dialog {
            self.set_active_dialog(dlg_name)?;
        }

        Ok(())
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

    /// Appends an entry to the action execution log.
    ///
    /// # Arguments
    ///
    /// * `entry` - Description string to log.
    pub fn append_action_log(&mut self, entry: impl Into<String>) {
        self.action_log.push(entry.into());
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
    fn test_ui_engine_navigation_and_conditions() {
        let mut engine = setup_test_wizard();
        assert!(engine.set_active_dialog("WelcomeDlg").is_ok());
        assert_eq!(
            engine.active_dialog().map(|d| d.name.as_str()),
            Some("WelcomeDlg")
        );

        // Click CancelBtn -> Spawns CancelDlg modal
        let ret = engine.click_control("WelcomeDlg", "CancelBtn");
        assert_eq!(ret, Ok(None));
        assert_eq!(
            engine.active_dialog().map(|d| d.name.as_str()),
            Some("CancelDlg")
        );

        // Click NoBtn on CancelDlg -> Returns to WelcomeDlg parent
        let ret_no = engine.click_control("CancelDlg", "NoBtn");
        assert_eq!(ret_no, Ok(None));
        assert_eq!(
            engine.active_dialog().map(|d| d.name.as_str()),
            Some("WelcomeDlg")
        );

        // Click NextBtn on WelcomeDlg -> Navigates to LicenseDlg
        let ret_next = engine.click_control("WelcomeDlg", "NextBtn");
        assert_eq!(ret_next, Ok(None));
        assert_eq!(
            engine.active_dialog().map(|d| d.name.as_str()),
            Some("LicenseDlg")
        );

        // In LicenseDlg, NextBtn should initially be disabled because ACCEPT_EULA is "0"
        let next_state = engine.get_control_state("LicenseDlg", "NextBtn");
        assert!(next_state.is_some_and(|s| !s.is_enabled));

        // Simulate checking the box: setting ACCEPT_EULA = "1"
        engine.context_mut().set_property("ACCEPT_EULA", "1");
        assert!(engine.evaluate_conditions_and_formatting().is_ok());

        let next_state_enabled = engine.get_control_state("LicenseDlg", "NextBtn");
        assert!(next_state_enabled.is_some_and(|s| s.is_enabled));

        // Click NextBtn on LicenseDlg -> Ends dialog with Return
        let end_res = engine.click_control("LicenseDlg", "NextBtn");
        assert_eq!(end_res, Ok(Some(DialogReturnCode::Return)));

        // Test progress update
        engine.update_progress(75);
        let cur_state = engine.get_control_state("LicenseDlg", "NextBtn");
        assert_eq!(cur_state.map(|s| s.progress_percent), Some(75));
    }

    /// Tests `SetProperty`, `Reset`, and `DoAction` event handling.
    #[test]
    fn test_ui_engine_events_variety() {
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

        assert!(engine.set_active_dialog("Dlg").is_ok());
        assert_eq!(engine.context().get_property("PROP1"), Some("Original"));

        // Click SetBtn
        assert_eq!(engine.click_control("Dlg", "SetBtn"), Ok(None));
        assert_eq!(engine.context().get_property("PROP1"), Some("Modified"));
        assert_eq!(engine.action_log(), &["DoAction(CustomAction1)"]);

        // Click ResetBtn
        assert_eq!(engine.click_control("Dlg", "ResetBtn"), Ok(None));
        assert_eq!(engine.context().get_property("PROP1"), Some("Original"));

        engine.append_action_log("ManualLogEntry");
        assert_eq!(
            engine.action_log(),
            &["DoAction(CustomAction1)", "ManualLogEntry"]
        );
    }

    /// Tests edge cases: `is_modal`, unbound properties, `Default` and `Show` conditions, `SpawnWaitDialog`, progress clamping, and state mutations.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ui_engine_edge_cases() {
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
        assert!(engine.set_active_dialog("NonModal").is_ok());
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
        assert_eq!(engine.click_control("NonModal", "Btn"), Ok(None));
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
        assert!(engine.set_active_dialog("EmptyDlg").is_ok());

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
        assert!(engine.set_active_dialog("NonModal").is_ok());
        engine.clear_control_states();
        assert!(engine.get_control_state("NonModal", "Btn").is_none());
        assert!(engine.evaluate_conditions_and_formatting().is_ok());
    }

    /// Tests error branches for invalid property formatting, condition evaluation errors, and missing navigation targets.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ui_engine_error_paths() {
        // 1. evaluate_conditions_and_formatting format_string error
        let mut engine1 = UiEngine::new(EvaluationContext::new());
        let mut dlg1 = DialogDefinition {
            name: "ErrorDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 100,
            height: 100,
            attributes: DIALOG_ATTR_VISIBLE,
            title: None,
            control_first: String::new(),
            control_default: None,
            control_cancel: None,
        };
        let ctrl_bad_fmt = ControlDefinition::new(
            "ErrorDlg",
            "BadFmt",
            ControlType::Text,
            DluRect::new(0, 0, 10, 10),
            0,
        )
        .text("[Unclosed");
        dlg1.control_first = "BadFmt".to_string();
        engine1.add_dialog(dlg1);
        engine1.add_control(ctrl_bad_fmt);
        assert!(engine1.set_active_dialog("ErrorDlg").is_err());

        // 2. evaluate_conditions_and_formatting condition error
        let mut engine2 = UiEngine::new(EvaluationContext::new());
        let dlg2 = DialogDefinition {
            name: "CondErrDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 100,
            height: 100,
            attributes: DIALOG_ATTR_VISIBLE,
            title: None,
            control_first: String::new(),
            control_default: None,
            control_cancel: None,
        };
        engine2.add_dialog(dlg2);
        engine2.add_condition(ControlCondition {
            dialog: "CondErrDlg".to_string(),
            control: "Btn".to_string(),
            action: ControlConditionAction::Enable,
            condition: "A === B".to_string(),
        });
        assert!(engine2.set_active_dialog("CondErrDlg").is_err());

        // 3. click_control event.is_satisfied error
        let mut engine3 = UiEngine::new(EvaluationContext::new());
        let dlg3 = DialogDefinition {
            name: "SatisfyErrDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 100,
            height: 100,
            attributes: DIALOG_ATTR_VISIBLE,
            title: None,
            control_first: String::new(),
            control_default: None,
            control_cancel: None,
        };
        engine3.add_dialog(dlg3);
        assert!(engine3.set_active_dialog("SatisfyErrDlg").is_ok());
        engine3.add_event(ControlEvent::new(
            "SatisfyErrDlg",
            "Btn",
            ControlEventType::Reset,
            Some("A === B".to_string()),
            1,
        ));
        assert!(engine3.click_control("SatisfyErrDlg", "Btn").is_err());

        // 4. click_control NewDialog / SpawnDialog with nonexistent target
        let mut engine4 = UiEngine::new(EvaluationContext::new());
        let dlg4 = DialogDefinition {
            name: "NavErrDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 100,
            height: 100,
            attributes: DIALOG_ATTR_VISIBLE,
            title: None,
            control_first: String::new(),
            control_default: None,
            control_cancel: None,
        };
        engine4.add_dialog(dlg4);
        assert!(engine4.set_active_dialog("NavErrDlg").is_ok());
        engine4.add_event(ControlEvent::new(
            "NavErrDlg",
            "NewDlgBtn",
            ControlEventType::NewDialog("GhostDlg".to_string()),
            None,
            1,
        ));
        assert!(engine4.click_control("NavErrDlg", "NewDlgBtn").is_err());

        engine4.add_event(ControlEvent::new(
            "NavErrDlg",
            "SpawnDlgBtn",
            ControlEventType::SpawnDialog("GhostDlg".to_string()),
            None,
            1,
        ));
        assert!(engine4.click_control("NavErrDlg", "SpawnDlgBtn").is_err());

        // 5. click_control SetProperty format_string error
        engine4.add_event(ControlEvent::new(
            "NavErrDlg",
            "SetPropBtn",
            ControlEventType::SetProperty {
                property: "P".to_string(),
                value: "[Unclosed".to_string(),
            },
            None,
            1,
        ));
        assert!(engine4.click_control("NavErrDlg", "SetPropBtn").is_err());

        // 6. EndDialog return to modal parent that has condition error on re-evaluation
        let mut engine5 = UiEngine::new(EvaluationContext::new());
        let parent_dlg = DialogDefinition {
            name: "ParentDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 100,
            height: 100,
            attributes: DIALOG_ATTR_VISIBLE,
            title: None,
            control_first: String::new(),
            control_default: None,
            control_cancel: None,
        };
        let child_dlg = DialogDefinition {
            name: "ChildDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 100,
            height: 100,
            attributes: DIALOG_ATTR_MODAL,
            title: None,
            control_first: String::new(),
            control_default: None,
            control_cancel: None,
        };
        engine5.add_dialog(parent_dlg);
        engine5.add_dialog(child_dlg);
        assert!(engine5.set_active_dialog("ParentDlg").is_ok());
        engine5.add_event(ControlEvent::new(
            "ParentDlg",
            "OpenChildBtn",
            ControlEventType::SpawnDialog("ChildDlg".to_string()),
            None,
            1,
        ));
        assert!(engine5.click_control("ParentDlg", "OpenChildBtn").is_ok());

        engine5.add_condition(ControlCondition {
            dialog: "ParentDlg".to_string(),
            control: "C".to_string(),
            action: ControlConditionAction::Enable,
            condition: "A === B".to_string(),
        });
        engine5.add_event(ControlEvent::new(
            "ChildDlg",
            "ExitBtn",
            ControlEventType::EndDialog(DialogReturnCode::Return),
            None,
            1,
        ));
        assert!(engine5.click_control("ChildDlg", "ExitBtn").is_err());

        // 7. SetProperty and Reset evaluation error when condition fails
        let mut engine6 = UiEngine::new(EvaluationContext::new());
        let dlg6 = DialogDefinition {
            name: "SetPropErrDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 100,
            height: 100,
            attributes: DIALOG_ATTR_VISIBLE,
            title: None,
            control_first: String::new(),
            control_default: None,
            control_cancel: None,
        };
        engine6.add_dialog(dlg6);
        assert!(engine6.set_active_dialog("SetPropErrDlg").is_ok());
        engine6.add_condition(ControlCondition {
            dialog: "SetPropErrDlg".to_string(),
            control: "C".to_string(),
            action: ControlConditionAction::Enable,
            condition: "A === B".to_string(),
        });
        engine6.add_event(ControlEvent::new(
            "SetPropErrDlg",
            "SetBtn",
            ControlEventType::SetProperty {
                property: "P".to_string(),
                value: "valid".to_string(),
            },
            None,
            1,
        ));
        assert!(engine6.click_control("SetPropErrDlg", "SetBtn").is_err());

        engine6.add_event(ControlEvent::new(
            "SetPropErrDlg",
            "ResetBtn",
            ControlEventType::Reset,
            None,
            1,
        ));
        assert!(engine6.click_control("SetPropErrDlg", "ResetBtn").is_err());
    }

    /// Tests two-way data binding, checkbox toggling, radio selection, and custom actions execution.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ui_engine_two_way_binding_and_custom_actions() -> Result<()> {
        let mut context = EvaluationContext::new();
        context.set_property("EDIT_PROP", "InitialText");
        context.set_property("CHECK_PROP", "0");
        context.set_property("RADIO_PROP", "OptA");

        let mut engine = UiEngine::new(context);
        let dlg = DialogDefinition {
            name: "BindingDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 300,
            height: 200,
            attributes: DIALOG_ATTR_VISIBLE,
            title: Some("Binding Test".to_string()),
            control_first: "Edit1".to_string(),
            control_default: Some("OkBtn".to_string()),
            control_cancel: Some("CancelBtn".to_string()),
        };
        engine.add_dialog(dlg);

        let edit_ctrl = ControlDefinition::new(
            "BindingDlg",
            "Edit1",
            ControlType::Edit,
            DluRect::new(10, 10, 100, 15),
            3,
        )
        .property("EDIT_PROP")
        .text("InitialText");
        engine.add_control(edit_ctrl);

        let check_ctrl = ControlDefinition::new(
            "BindingDlg",
            "Check1",
            ControlType::CheckBox,
            DluRect::new(10, 30, 100, 15),
            3,
        )
        .property("CHECK_PROP")
        .text("Enable Feature");
        engine.add_control(check_ctrl);

        let radio_ctrl = ControlDefinition::new(
            "BindingDlg",
            "RadioGroup",
            ControlType::RadioButtonGroup,
            DluRect::new(10, 50, 100, 30),
            3,
        )
        .property("RADIO_PROP")
        .text("OptA");
        engine.add_control(radio_ctrl);

        engine.set_active_dialog("BindingDlg")?;

        // 1. Two-way binding for Edit control
        engine.update_control_value("BindingDlg", "Edit1", "UpdatedText")?;
        assert_eq!(
            engine.context().get_property("EDIT_PROP"),
            Some("UpdatedText")
        );
        assert_eq!(
            engine
                .get_control_state("BindingDlg", "Edit1")
                .map(|s| s.current_text.as_str()),
            Some("UpdatedText")
        );

        // 2. Checkbox toggling
        engine.toggle_checkbox("BindingDlg", "Check1")?;
        assert_eq!(engine.context().get_property("CHECK_PROP"), Some("1"));
        engine.toggle_checkbox("BindingDlg", "Check1")?;
        assert_eq!(engine.context().get_property("CHECK_PROP"), Some("0"));

        // 3. Radio button selection
        engine.select_radio_button("BindingDlg", "RadioGroup", "OptB")?;
        assert_eq!(engine.context().get_property("RADIO_PROP"), Some("OptB"));

        // 4. Custom action registration and synchronous execution on DoAction
        let mut executor = CustomActionExecutor::new();
        executor.set_mock_result("ValidateAction", 0);
        engine.set_custom_action_executor(executor);
        assert!(engine.custom_action_executor().is_some());
        assert!(engine.custom_action_executor_mut().is_some());

        let ca = CustomActionDefinition::parse("ValidateAction", 1, "BinarySrc", "ValidateFn")?;
        engine.add_custom_action(ca);

        let action_btn = ControlDefinition::new(
            "BindingDlg",
            "ActionBtn",
            ControlType::PushButton,
            DluRect::new(10, 90, 50, 15),
            3,
        );
        engine.add_control(action_btn);
        engine.add_event(ControlEvent::new(
            "BindingDlg",
            "ActionBtn",
            ControlEventType::DoAction("ValidateAction".to_string()),
            None,
            1,
        ));

        let res = engine.click_control("BindingDlg", "ActionBtn")?;
        assert_eq!(res, None);
        assert!(engine
            .action_log()
            .iter()
            .any(|a| a.contains("ValidateAction")));

        // 5. Failing Custom Action on DoAction
        let mut fail_executor = CustomActionExecutor::new();
        fail_executor.set_mock_result("FailAction", 1603);
        engine.set_custom_action_executor(fail_executor);

        let fail_ca = CustomActionDefinition::parse("FailAction", 1, "BinarySrc", "FailFn")?;
        engine.add_custom_action(fail_ca);
        engine.add_event(ControlEvent::new(
            "BindingDlg",
            "ActionBtn",
            ControlEventType::DoAction("FailAction".to_string()),
            None,
            2,
        ));
        assert!(engine.click_control("BindingDlg", "ActionBtn").is_err());

        // 6. SpawnDialog and modal stack popping via EndDialog
        let modal_dlg = DialogDefinition {
            name: "ModalDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 200,
            height: 150,
            attributes: 3,
            title: Some("Modal".to_string()),
            control_first: "CloseBtn".to_string(),
            control_default: None,
            control_cancel: None,
        };
        engine.add_dialog(modal_dlg);
        let close_btn = ControlDefinition::new(
            "ModalDlg",
            "CloseBtn",
            ControlType::PushButton,
            DluRect::new(10, 10, 50, 15),
            3,
        );
        engine.add_control(close_btn);
        engine.add_event(ControlEvent::new(
            "ModalDlg",
            "CloseBtn",
            ControlEventType::EndDialog(DialogReturnCode::Return),
            None,
            1,
        ));

        // Event on BindingDlg that spawns ModalDlg and SpawnWaitDialog
        engine.add_event(ControlEvent::new(
            "BindingDlg",
            "ActionBtn",
            ControlEventType::SpawnDialog("ModalDlg".to_string()),
            None,
            3,
        ));
        engine.add_event(ControlEvent::new(
            "BindingDlg",
            "ActionBtn",
            ControlEventType::SpawnWaitDialog("WaitDlg".to_string()),
            None,
            4,
        ));

        // Restore working executor so event chain proceeds
        let mut ok_executor = CustomActionExecutor::new();
        ok_executor.set_mock_result("ValidateAction", 0);
        ok_executor.set_mock_result("FailAction", 0);
        engine.set_custom_action_executor(ok_executor);

        let _ = engine.click_control("BindingDlg", "ActionBtn")?;
        assert_eq!(
            engine.active_dialog().map(|d| d.name.as_str()),
            Some("ModalDlg")
        );
        assert_eq!(engine.wait_dialog.as_deref(), Some("WaitDlg"));

        // Close modal dialog, popping back to BindingDlg
        let modal_ret = engine.click_control("ModalDlg", "CloseBtn")?;
        assert_eq!(modal_ret, None);
        assert_eq!(
            engine.active_dialog().map(|d| d.name.as_str()),
            Some("BindingDlg")
        );

        // 7. DoAction when custom_action_executor is None
        let mut no_exec_engine = UiEngine::new(EvaluationContext::new());
        no_exec_engine.add_dialog(DialogDefinition {
            name: "NoExecDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 200,
            height: 150,
            attributes: 3,
            title: None,
            control_first: "Btn".to_string(),
            control_default: None,
            control_cancel: None,
        });
        no_exec_engine.add_control(ControlDefinition::new(
            "NoExecDlg",
            "Btn",
            ControlType::PushButton,
            DluRect::new(10, 10, 50, 15),
            3,
        ));
        let no_exec_ca = CustomActionDefinition::parse("ActionNoExec", 1, "BinarySrc", "Fn")?;
        no_exec_engine.add_custom_action(no_exec_ca);
        no_exec_engine.add_event(ControlEvent::new(
            "NoExecDlg",
            "Btn",
            ControlEventType::DoAction("ActionNoExec".to_string()),
            None,
            1,
        ));
        assert!(no_exec_engine.set_active_dialog("NoExecDlg").is_ok());
        let _ = no_exec_engine.click_control("NoExecDlg", "Btn")?;

        // 8. DoAction when evaluate_conditions_and_formatting fails
        engine.add_condition(ControlCondition {
            dialog: "BindingDlg".to_string(),
            control: "ActionBtn".to_string(),
            action: ControlConditionAction::Enable,
            condition: "INVALID ===".to_string(),
        });
        assert!(engine.click_control("BindingDlg", "ActionBtn").is_err());

        Ok(())
    }

    /// Tests `load_from_database` populating dialogs, controls, conditions, events, and actions.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ui_engine_load_from_database() -> Result<()> {
        use crate::database::tables::record::Record;

        let mut db = LinkedDatabase::new()?;

        // 1. Dialog table
        db.add_record(
            "Dialog",
            Record::with_fields(vec![
                FieldValue::String("WelcomeDlg".to_string()),
                FieldValue::Short(50),
                FieldValue::Short(50),
                FieldValue::Short(370),
                FieldValue::Short(270),
                FieldValue::Long(3),
                FieldValue::String("Welcome to [ProductName]".to_string()),
                FieldValue::String("NextBtn".to_string()),
                FieldValue::String("NextBtn".to_string()),
                FieldValue::String("CancelBtn".to_string()),
            ]),
        );

        // 2. Control table
        db.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("WelcomeDlg".to_string()),
                FieldValue::String("NextBtn".to_string()),
                FieldValue::String("PushButton".to_string()),
                FieldValue::Short(236),
                FieldValue::Short(243),
                FieldValue::Short(56),
                FieldValue::Short(17),
                FieldValue::Long(3),
                FieldValue::Null,
                FieldValue::String("Next".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("WelcomeDlg".to_string()),
                FieldValue::String("CancelBtn".to_string()),
                FieldValue::String("PushButton".to_string()),
                FieldValue::Short(304),
                FieldValue::Short(243),
                FieldValue::Short(56),
                FieldValue::Short(17),
                FieldValue::Long(3),
                FieldValue::Null,
                FieldValue::String("Cancel".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        // 3. ControlEvent table
        db.add_record(
            "ControlEvent",
            Record::with_fields(vec![
                FieldValue::String("WelcomeDlg".to_string()),
                FieldValue::String("NextBtn".to_string()),
                FieldValue::String("EndDialog".to_string()),
                FieldValue::String("Return".to_string()),
                FieldValue::String("1".to_string()),
                FieldValue::Short(1),
            ]),
        );

        // 4. ControlCondition table
        db.add_record(
            "ControlCondition",
            Record::with_fields(vec![
                FieldValue::String("WelcomeDlg".to_string()),
                FieldValue::String("NextBtn".to_string()),
                FieldValue::String("Enable".to_string()),
                FieldValue::String("1".to_string()),
            ]),
        );

        // 5. EventMapping table
        db.add_record(
            "EventMapping",
            Record::with_fields(vec![
                FieldValue::String("WelcomeDlg".to_string()),
                FieldValue::String("NextBtn".to_string()),
                FieldValue::String("SetProgress".to_string()),
                FieldValue::String("Progress".to_string()),
            ]),
        );

        // 6. CustomAction & Binary tables
        db.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("TestBin".to_string()),
                FieldValue::String("data".to_string()),
            ]),
        );

        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("TestAction".to_string()),
                FieldValue::Long(1),
                FieldValue::String("TestBin".to_string()),
                FieldValue::String("Entry".to_string()),
            ]),
        );

        // 7. InstallUISequence table
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("WelcomeDlg".to_string()),
                FieldValue::String("1".to_string()),
                FieldValue::Short(100),
            ]),
        );

        let mut context = EvaluationContext::new();
        context.set_property("ProductName", "LoadedApp");
        let mut engine = UiEngine::new(context);
        engine.load_from_database(&db)?;

        assert_eq!(
            engine.active_dialog().map(|d| d.name.as_str()),
            Some("WelcomeDlg")
        );
        assert_eq!(engine.get_dialog_controls("WelcomeDlg").len(), 2);
        let ret = engine.click_control("WelcomeDlg", "NextBtn")?;
        assert_eq!(ret, Some(DialogReturnCode::Return));

        Ok(())
    }

    /// Tests `load_from_database` covering all `FieldValue` variants (Short, Long, Null),
    /// fallback defaults, invalid rows, and sequence ordering.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ui_engine_load_from_database_coverage_matrix() -> Result<()> {
        use crate::database::tables::record::Record;

        let mut db = LinkedDatabase::new()?;

        // 1. Dialog with Long centering/dimensions and Short attributes
        db.add_record(
            "Dialog",
            Record::with_fields(vec![
                FieldValue::String("DlgLong".to_string()),
                FieldValue::Long(60),
                FieldValue::Long(70),
                FieldValue::Long(400),
                FieldValue::Long(300),
                FieldValue::Short(7),
                FieldValue::String("Title".to_string()),
                FieldValue::String("FirstCtrl".to_string()),
                FieldValue::String("DefCtrl".to_string()),
                FieldValue::String("CancelCtrl".to_string()),
            ]),
        );

        // Dialog with defaults (Null fields for everything except name)
        db.add_record(
            "Dialog",
            Record::with_fields(vec![
                FieldValue::String("DlgDefaults".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        // Dialog skipped (first field is not String)
        db.add_record("Dialog", Record::with_fields(vec![FieldValue::Short(123)]));

        // 2. Control table: Long coordinates/dimensions and Short attributes, plus all optional fields
        db.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("DlgLong".to_string()),
                FieldValue::String("CtrlLong".to_string()),
                FieldValue::String("Edit".to_string()),
                FieldValue::Long(10),
                FieldValue::Long(20),
                FieldValue::Long(100),
                FieldValue::Long(20),
                FieldValue::Short(3),
                FieldValue::String("MY_PROP".to_string()),
                FieldValue::String("Initial".to_string()),
                FieldValue::String("NextCtrl".to_string()),
                FieldValue::String("HelpText".to_string()),
            ]),
        );

        // Control with defaults (Null coordinates and dimensions)
        db.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("DlgLong".to_string()),
                FieldValue::String("CtrlDef".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        // Control records skipped (missing dialog or control name)
        db.add_record("Control", Record::with_fields(vec![FieldValue::Short(1)]));
        db.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("DlgLong".to_string()),
                FieldValue::Short(2),
            ]),
        );

        // 3. ControlCondition table
        db.add_record(
            "ControlCondition",
            Record::with_fields(vec![
                FieldValue::String("DlgLong".to_string()),
                FieldValue::String("CtrlLong".to_string()),
                FieldValue::String("Disable".to_string()),
                FieldValue::String("PROP = 1".to_string()),
            ]),
        );
        // Skipped: invalid action string
        db.add_record(
            "ControlCondition",
            Record::with_fields(vec![
                FieldValue::String("DlgLong".to_string()),
                FieldValue::String("CtrlLong".to_string()),
                FieldValue::String("InvalidActionName".to_string()),
                FieldValue::String("1".to_string()),
            ]),
        );
        // Skipped: fewer fields than required
        db.add_record(
            "ControlCondition",
            Record::with_fields(vec![FieldValue::String("DlgLong".to_string())]),
        );

        // 4. ControlEvent table
        // With Long order and empty condition string
        db.add_record(
            "ControlEvent",
            Record::with_fields(vec![
                FieldValue::String("DlgLong".to_string()),
                FieldValue::String("CtrlLong".to_string()),
                FieldValue::String("EndDialog".to_string()),
                FieldValue::String("Return".to_string()),
                FieldValue::String(String::new()),
                FieldValue::Long(5),
            ]),
        );
        // With Null order and Null condition
        db.add_record(
            "ControlEvent",
            Record::with_fields(vec![
                FieldValue::String("DlgLong".to_string()),
                FieldValue::String("CtrlLong".to_string()),
                FieldValue::String("EndDialog".to_string()),
                FieldValue::String("Exit".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Skipped: invalid event name
        db.add_record(
            "ControlEvent",
            Record::with_fields(vec![
                FieldValue::String("DlgLong".to_string()),
                FieldValue::String("CtrlLong".to_string()),
                FieldValue::String("NonexistentEvent".to_string()),
                FieldValue::String("Arg".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Skipped: incomplete record
        db.add_record(
            "ControlEvent",
            Record::with_fields(vec![FieldValue::String("DlgLong".to_string())]),
        );

        // 5. EventMapping table: incomplete record
        db.add_record(
            "EventMapping",
            Record::with_fields(vec![FieldValue::String("DlgLong".to_string())]),
        );

        // 6. Binary table with non-string data
        db.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("BinNonString".to_string()),
                FieldValue::Short(99),
            ]),
        );
        // Skipped Binary record (non-string name)
        db.add_record("Binary", Record::with_fields(vec![FieldValue::Short(1)]));

        // 7. CustomAction table with Short type and Null type
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("CAShort".to_string()),
                FieldValue::Short(1),
                FieldValue::String("BinNonString".to_string()),
                FieldValue::String("Entry".to_string()),
            ]),
        );
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("CANullType".to_string()),
                FieldValue::Null,
                FieldValue::String("BinNonString".to_string()),
                FieldValue::String("Entry".to_string()),
            ]),
        );
        // Skipped CustomAction record (incomplete)
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![FieldValue::String("CAIncomplete".to_string())]),
        );

        // 8. InstallUISequence with sequence comparison (second record seq is higher, and action not in dialogs)
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("DlgLong".to_string()),
                FieldValue::Null,
                FieldValue::Short(50),
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("DlgLong".to_string()),
                FieldValue::Null,
                FieldValue::Short(150),
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("NotInDialogs".to_string()),
                FieldValue::Null,
                FieldValue::Short(10),
            ]),
        );
        // Skipped InstallUISequence record
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![FieldValue::String("DlgLong".to_string())]),
        );

        let mut engine = UiEngine::new(EvaluationContext::new());
        engine.load_from_database(&db)?;
        assert_eq!(
            engine.active_dialog().map(|d| d.name.as_str()),
            Some("DlgLong")
        );

        // 9. Initial dialog fallbacks:
        // A. Database with "Welcome" dialog
        let mut db_welcome = LinkedDatabase::new()?;
        db_welcome.add_record(
            "Dialog",
            Record::with_fields(vec![
                FieldValue::String("Welcome".to_string()),
                FieldValue::Short(50),
                FieldValue::Short(50),
                FieldValue::Short(300),
                FieldValue::Short(200),
                FieldValue::Short(3),
                FieldValue::String("Welcome".to_string()),
                FieldValue::String("Btn".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        let mut engine_welcome = UiEngine::new(EvaluationContext::new());
        engine_welcome.load_from_database(&db_welcome)?;
        assert_eq!(
            engine_welcome.active_dialog().map(|d| d.name.as_str()),
            Some("Welcome")
        );

        // B. Database with arbitrary dialog (neither WelcomeDlg nor Welcome)
        let mut db_arbitrary = LinkedDatabase::new()?;
        db_arbitrary.add_record(
            "Dialog",
            Record::with_fields(vec![
                FieldValue::String("CustomDialog".to_string()),
                FieldValue::Short(50),
                FieldValue::Short(50),
                FieldValue::Short(300),
                FieldValue::Short(200),
                FieldValue::Short(3),
                FieldValue::String("Custom".to_string()),
                FieldValue::String("Btn".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        let mut engine_arbitrary = UiEngine::new(EvaluationContext::new());
        engine_arbitrary.load_from_database(&db_arbitrary)?;
        assert_eq!(
            engine_arbitrary.active_dialog().map(|d| d.name.as_str()),
            Some("CustomDialog")
        );

        // C. Database with no dialogs at all
        let db_empty = LinkedDatabase::new()?;
        let mut engine_empty = UiEngine::new(EvaluationContext::new());
        engine_empty.load_from_database(&db_empty)?;
        assert!(engine_empty.active_dialog().is_none());

        // D. Database with "WelcomeDlg" (and no sequence)
        let mut db_welcomedlg = LinkedDatabase::new()?;
        db_welcomedlg.add_record(
            "Dialog",
            Record::with_fields(vec![
                FieldValue::String("WelcomeDlg".to_string()),
                FieldValue::Short(50),
                FieldValue::Short(50),
                FieldValue::Short(300),
                FieldValue::Short(200),
                FieldValue::Short(3),
                FieldValue::String("Welcome".to_string()),
                FieldValue::String("Btn".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        let mut engine_welcomedlg = UiEngine::new(EvaluationContext::new());
        engine_welcomedlg.load_from_database(&db_welcomedlg)?;
        assert_eq!(
            engine_welcomedlg.active_dialog().map(|d| d.name.as_str()),
            Some("WelcomeDlg")
        );

        // E. Database with invalid condition making set_active_dialog fail
        let mut db_err_cond = LinkedDatabase::new()?;
        db_err_cond.add_record(
            "Dialog",
            Record::with_fields(vec![
                FieldValue::String("ErrDlg".to_string()),
                FieldValue::Short(50),
                FieldValue::Short(50),
                FieldValue::Short(300),
                FieldValue::Short(200),
                FieldValue::Short(3),
                FieldValue::String("Err".to_string()),
                FieldValue::String("Btn".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_err_cond.add_record(
            "ControlCondition",
            Record::with_fields(vec![
                FieldValue::String("ErrDlg".to_string()),
                FieldValue::String("Btn".to_string()),
                FieldValue::String("Enable".to_string()),
                FieldValue::String("INVALID ===".to_string()),
            ]),
        );
        let mut engine_err_cond = UiEngine::new(EvaluationContext::new());
        assert!(engine_err_cond.load_from_database(&db_err_cond).is_err());

        Ok(())
    }
}
