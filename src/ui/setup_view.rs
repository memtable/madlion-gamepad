use std::cell::Cell;
use std::rc::Rc;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, DragSource, FlowBox, Image, Orientation, SelectionMode,
    StringList,
};
use libadwaita::prelude::*;
use libadwaita::{Clamp, ComboRow, PreferencesGroup, SwitchRow};

pub type SetupActionCallback = Rc<dyn Fn(SetupAction)>;

#[derive(Debug, Clone)]
pub enum SetupAction {
    GamepadEnabled(bool),
    PollingRate(u64),
    ControllerType(String),
    EnableTyping(bool),
    MappingOverride(bool),
    PollOnlyMappedKeys(bool),
}

pub struct SetupView {
    pub container: GtkBox,
    pub enable_gamepad_switch: SwitchRow,
    pub polling_rate_combo: ComboRow,
    pub controller_type_combo: ComboRow,
    pub enable_typing_switch: SwitchRow,
    pub mapping_override_switch: SwitchRow,
    pub poll_only_mapped_switch: SwitchRow,
    updating: Rc<Cell<bool>>,
}

impl SetupView {
    pub fn new(
        gamepad_enabled: bool,
        polling_rate_hz: u64,
        controller_type: &str,
        enable_typing: bool,
        mapping_override: bool,
        poll_only_mapped_keys: bool,
        on_action: Option<SetupActionCallback>,
    ) -> Self {
        let updating = Rc::new(Cell::new(false));

        let container = GtkBox::new(Orientation::Vertical, 16);
        container.set_margin_start(16);
        container.set_margin_end(16);
        container.set_margin_top(16);
        container.set_margin_bottom(24);
        container.set_hexpand(true);

        // settings n palette columns
        let columns_box = GtkBox::new(Orientation::Horizontal, 16);
        columns_box.set_homogeneous(true);
        columns_box.set_hexpand(true);

        // left column: gamepad settings
        let left_box = GtkBox::new(Orientation::Vertical, 12);
        left_box.set_hexpand(true);

        let left_group = PreferencesGroup::new();
        left_group.set_title("Gamepad Output Settings");
        left_group.set_description(Some(
            "Configure virtual controller device emulation and input behaviors.",
        ));

        let enable_gamepad_switch = SwitchRow::new();
        enable_gamepad_switch.set_title("Enable Gamepad");
        enable_gamepad_switch.set_subtitle("Broadcast analog joystick and button events to games");
        enable_gamepad_switch.set_active(gamepad_enabled);

        let action_gamepad = on_action.clone();
        let updating_gamepad = updating.clone();
        enable_gamepad_switch.connect_active_notify(move |sw| {
            if updating_gamepad.get() {
                return;
            }
            if let Some(ref cb) = action_gamepad {
                cb(SetupAction::GamepadEnabled(sw.is_active()));
            }
        });
        left_group.add(&enable_gamepad_switch);

        let controller_type_combo = ComboRow::new();
        controller_type_combo.set_title("Controller Type");
        controller_type_combo.set_subtitle("Select emulated virtual gamepad model");
        let controller_types = StringList::new(&["Xbox Controller", "Classic Controller"]);
        controller_type_combo.set_model(Some(&controller_types));
        let type_idx = match controller_type {
            "Classic Controller" => 1,
            _ => 0,
        };
        controller_type_combo.set_selected(type_idx);

        let action_type = on_action.clone();
        let updating_type = updating.clone();
        controller_type_combo.connect_selected_notify(move |combo| {
            if updating_type.get() {
                return;
            }
            let val = match combo.selected() {
                1 => "Classic Controller".to_string(),
                _ => "Xbox Controller".to_string(),
            };
            if let Some(ref cb) = action_type {
                cb(SetupAction::ControllerType(val));
            }
        });
        left_group.add(&controller_type_combo);

        let polling_rate_combo = ComboRow::new();
        polling_rate_combo.set_title("Polling Rate");
        polling_rate_combo.set_subtitle("Hardware USB query frequency");
        let rates = StringList::new(&["1000 Hz", "500 Hz", "250 Hz", "125 Hz"]);
        polling_rate_combo.set_model(Some(&rates));
        let rate_idx = match polling_rate_hz {
            500 => 1,
            250 => 2,
            125 => 3,
            _ => 0,
        };
        polling_rate_combo.set_selected(rate_idx);

        let action_rate = on_action.clone();
        let updating_rate = updating.clone();
        polling_rate_combo.connect_selected_notify(move |combo| {
            if updating_rate.get() {
                return;
            }
            let hz = match combo.selected() {
                1 => 500,
                2 => 250,
                3 => 125,
                _ => 1000,
            };
            if let Some(ref cb) = action_rate {
                cb(SetupAction::PollingRate(hz));
            }
        });
        left_group.add(&polling_rate_combo);

        let enable_typing_switch = SwitchRow::new();
        enable_typing_switch.set_title("Keyboard Key Output");
        enable_typing_switch
            .set_subtitle("Output standard digital keyboard keys alongside gamepad events");
        enable_typing_switch.set_active(enable_typing);

        let action_typing = on_action.clone();
        let updating_typing = updating.clone();
        enable_typing_switch.connect_active_notify(move |sw| {
            if updating_typing.get() {
                return;
            }
            if let Some(ref cb) = action_typing {
                cb(SetupAction::EnableTyping(sw.is_active()));
            }
        });
        left_group.add(&enable_typing_switch);

        let mapping_override_switch = SwitchRow::new();
        mapping_override_switch.set_title("Mapping Override");
        mapping_override_switch
            .set_subtitle("Suppress digital keyboard input on keys assigned to gamepad actions");
        mapping_override_switch.set_active(mapping_override);

        let action_override = on_action.clone();
        let updating_override = updating.clone();
        mapping_override_switch.connect_active_notify(move |sw| {
            if updating_override.get() {
                return;
            }
            if let Some(ref cb) = action_override {
                cb(SetupAction::MappingOverride(sw.is_active()));
            }
        });
        left_group.add(&mapping_override_switch);

        let poll_only_mapped_switch = SwitchRow::new();
        poll_only_mapped_switch.set_title("Fast Polling (Bound Keys Only)");
        poll_only_mapped_switch.set_subtitle("Query only mapped gamepad keys to achieve max 1000+ Hz rate (disables unmapped key visualizer)");
        poll_only_mapped_switch.set_active(poll_only_mapped_keys);

        let action_poll_only = on_action;
        let updating_poll_only = updating.clone();
        poll_only_mapped_switch.connect_active_notify(move |sw| {
            if updating_poll_only.get() {
                return;
            }
            if let Some(ref cb) = action_poll_only {
                cb(SetupAction::PollOnlyMappedKeys(sw.is_active()));
            }
        });
        left_group.add(&poll_only_mapped_switch);

        left_box.append(&left_group);
        columns_box.append(&left_box);

        // right column: mapping palette
        let right_box = GtkBox::new(Orientation::Vertical, 12);
        right_box.set_hexpand(true);

        let right_group = PreferencesGroup::new();
        right_group.set_title("Controller Mapping");
        right_group.set_description(Some(
            "Drag and drop badges onto keyboard keys to assign gamepad functions.",
        ));
        let flow_box = FlowBox::new();
        flow_box.set_valign(Align::Start);
        flow_box.set_max_children_per_line(7);
        flow_box.set_column_spacing(6);
        flow_box.set_row_spacing(6);
        flow_box.set_selection_mode(SelectionMode::None);
        flow_box.set_homogeneous(true);
        flow_box.set_margin_top(8);
        flow_box.set_margin_bottom(8);
        flow_box.set_margin_start(4);
        flow_box.set_margin_end(4);

        // controller icon badges
        let badges = [
            ("A", "Button A (Green)"),
            ("B", "Button B (Red)"),
            ("X", "Button X (Blue)"),
            ("Y", "Button Y (Yellow)"),
            ("Start", "Start / Menu"),
            ("Back", "Back / Select"),
            ("Guide", "Xbox / Guide Button"),
            ("D-U", "D-Pad Up"),
            ("D-D", "D-Pad Down"),
            ("D-L", "D-Pad Left"),
            ("D-R", "D-Pad Right"),
            ("LS-U", "Left Stick Up"),
            ("LS-D", "Left Stick Down"),
            ("LS-L", "Left Stick Left"),
            ("LS-R", "Left Stick Right"),
            ("L3", "Left Stick Press (L3)"),
            ("RS-U", "Right Stick Up"),
            ("RS-D", "Right Stick Down"),
            ("RS-L", "Right Stick Left"),
            ("RS-R", "Right Stick Right"),
            ("R3", "Right Stick Press (R3)"),
            ("LB", "Left Bumper (LB)"),
            ("LT", "Left Trigger (LT)"),
            ("RB", "Right Bumper (RB)"),
            ("RT", "Right Trigger (RT)"),
        ];

        let accent_hex = crate::ui::get_accent_hex(&container);

        for &(code, tooltip_str) in &badges {
            let btn = Button::new();
            btn.set_tooltip_text(Some(tooltip_str));
            btn.add_css_class("controller-badge-btn");
            btn.set_size_request(38, 38);

            let maybe_pixbuf = crate::ui::load_accented_pixbuf_for_code(code, &accent_hex, 32);

            if let Some(ref pixbuf) = maybe_pixbuf {
                let tex = gtk4::gdk::Texture::for_pixbuf(pixbuf);
                let img = Image::from_paintable(Some(&tex));
                img.set_pixel_size(30);
                btn.set_child(Some(&img));
            } else {
                btn.set_label(code);
                btn.add_css_class("badge-btn-clear");
            }

            // drag source for badges
            let drag_source = DragSource::new();
            drag_source.set_actions(gtk4::gdk::DragAction::COPY);
            let code_for_drag = code.to_string();
            drag_source.connect_prepare(move |_src, _x, _y| {
                let val = glib::Value::from(&code_for_drag);
                let content = gtk4::gdk::ContentProvider::for_value(&val);
                Some(content)
            });

            if let Some(ref pixbuf) = maybe_pixbuf {
                let tex = gtk4::gdk::Texture::for_pixbuf(pixbuf);
                drag_source.set_icon(Some(&tex), 16, 16);
            }

            btn.add_controller(drag_source);

            flow_box.append(&btn);
        }

        right_group.add(&flow_box);
        right_box.append(&right_group);
        columns_box.append(&right_box);

        let cols_clamp = Clamp::new();
        cols_clamp.set_maximum_size(1000);
        cols_clamp.set_child(Some(&columns_box));
        container.append(&cols_clamp);

        Self {
            container,
            enable_gamepad_switch,
            polling_rate_combo,
            controller_type_combo,
            enable_typing_switch,
            mapping_override_switch,
            poll_only_mapped_switch,
            updating,
        }
    }

    pub fn set_state(
        &self,
        gamepad_enabled: bool,
        polling_rate_hz: u64,
        controller_type: &str,
        enable_typing: bool,
        mapping_override: bool,
        poll_only_mapped_keys: bool,
    ) {
        self.updating.set(true);
        self.enable_gamepad_switch.set_active(gamepad_enabled);

        let type_idx = match controller_type {
            "Classic Controller" => 1,
            _ => 0,
        };
        self.controller_type_combo.set_selected(type_idx);

        let rate_idx = match polling_rate_hz {
            500 => 1,
            250 => 2,
            125 => 3,
            _ => 0,
        };
        self.polling_rate_combo.set_selected(rate_idx);

        self.enable_typing_switch.set_active(enable_typing);
        self.mapping_override_switch.set_active(mapping_override);
        self.poll_only_mapped_switch
            .set_active(poll_only_mapped_keys);
        self.updating.set(false);
    }
}
