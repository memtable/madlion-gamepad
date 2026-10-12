use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, DrawingArea, EventControllerMotion, FlowBox, GestureDrag, Label,
    Orientation, SelectionMode,
};
use libadwaita::prelude::*;
use libadwaita::{Clamp, PreferencesGroup, SpinRow, SwitchRow};

use crate::config::AnalogCategory;
use crate::gamepad::ResponseCurve;
use crate::ui::curve_canvas::{
    HANDLE_HIT_RADIUS, clamp_handle_movement, draw_curve_graph, draw_curve_preset_icon,
    get_paddings,
};
use crate::ui::get_accent_rgb;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CurvePreset {
    Linear,
    Aggro,
    Slow,
    Smooth,
    Steep,
    Instant,
    Turbo,
    Custom,
}

impl CurvePreset {
    pub fn from_str_name(name: &str) -> Self {
        match name {
            "Linear" => CurvePreset::Linear,
            "Aggro" => CurvePreset::Aggro,
            "Slow" => CurvePreset::Slow,
            "Smooth" => CurvePreset::Smooth,
            "Steep" => CurvePreset::Steep,
            "Instant" => CurvePreset::Instant,
            "Turbo" => CurvePreset::Turbo,
            _ => CurvePreset::Custom,
        }
    }

    pub fn to_str_name(self) -> &'static str {
        match self {
            CurvePreset::Linear => "Linear",
            CurvePreset::Aggro => "Aggro",
            CurvePreset::Slow => "Slow",
            CurvePreset::Smooth => "Smooth",
            CurvePreset::Steep => "Steep",
            CurvePreset::Instant => "Instant",
            CurvePreset::Turbo => "Turbo",
            CurvePreset::Custom => "Custom",
        }
    }

    pub fn to_points(self) -> [(f64, f64); 4] {
        let pts = match self {
            CurvePreset::Linear => ResponseCurve::Linear.to_points(),
            CurvePreset::Aggro => ResponseCurve::Aggro.to_points(),
            CurvePreset::Slow => ResponseCurve::Slow.to_points(),
            CurvePreset::Smooth => ResponseCurve::Smooth.to_points(),
            CurvePreset::Steep => ResponseCurve::Steep.to_points(),
            CurvePreset::Instant => ResponseCurve::Instant.to_points(),
            CurvePreset::Turbo => ResponseCurve::Turbo.to_points(),
            CurvePreset::Custom => ResponseCurve::Linear.to_points(),
        };
        pts.map(|(x, y)| (x as f64, y as f64))
    }
}

pub struct ConfigViewState {
    pub active_category: Option<AnalogCategory>,
    pub current_preset: CurvePreset,
    // 4 control points in 0..255 space (x = travel, y = output)
    pub points: [(f64, f64); 4],
    pub dragging_handle: Option<usize>,
    pub drag_start_point: Option<(f64, f64)>,
    pub hovered_handle: Option<usize>,
    pub angle_adjustment: f64,
    pub enable_angle_adj: bool,
    pub square_joystick: bool,
    pub snappy_joystick: bool,
    pub live_depth_mm: f64,
}

pub struct ConfigView {
    pub container: GtkBox,
    pub state: Rc<RefCell<ConfigViewState>>,
    pub graph_area: DrawingArea,
    pub title_label: Label,
    pub preset_btn_list: Rc<RefCell<Vec<(CurvePreset, Button)>>>,
    pub angle_switch: SwitchRow,
    pub angle_spin: SpinRow,
    pub square_switch: SwitchRow,
    pub snappy_switch: SwitchRow,
    updating: Rc<Cell<bool>>,
}

pub type ConfigChangeCallback = Rc<dyn Fn(&ConfigViewState)>;

impl ConfigView {
    pub fn new(
        preset_name: &str,
        initial_points: [(f64, f64); 4],
        enable_angle_adj: bool,
        angle_adjustment: f64,
        square_joystick: bool,
        snappy_joystick: bool,
        on_change: Option<ConfigChangeCallback>,
    ) -> Self {
        let outer_container = GtkBox::new(Orientation::Vertical, 16);
        outer_container.set_margin_start(16);
        outer_container.set_margin_end(16);
        outer_container.set_margin_top(16);
        outer_container.set_margin_bottom(24);
        outer_container.set_hexpand(true);

        let container = GtkBox::new(Orientation::Horizontal, 16);
        container.set_homogeneous(true);
        container.set_hexpand(true);

        let clamp = Clamp::new();
        clamp.set_maximum_size(1000);
        clamp.set_child(Some(&container));
        outer_container.append(&clamp);

        let current_preset = CurvePreset::from_str_name(preset_name);
        let updating = Rc::new(Cell::new(false));

        let state = Rc::new(RefCell::new(ConfigViewState {
            active_category: None,
            current_preset,
            points: initial_points,
            dragging_handle: None,
            drag_start_point: None,
            hovered_handle: None,
            angle_adjustment,
            enable_angle_adj,
            square_joystick,
            snappy_joystick,
            live_depth_mm: 0.0,
        }));

        // left column: curve editor
        let left_group = PreferencesGroup::new();
        left_group.set_title("Analog Response Curve");

        let left_card = GtkBox::new(Orientation::Vertical, 10);
        left_card.add_css_class("card");
        left_card.set_margin_top(4);
        left_card.set_margin_bottom(4);
        left_card.set_hexpand(true);

        // curve header
        let title_label = Label::new(Some("Analog Response Curve"));
        title_label.add_css_class("heading");
        title_label.set_halign(Align::Start);
        title_label.set_margin_start(16);
        title_label.set_margin_top(14);
        left_card.append(&title_label);

        // graph canvas
        let graph_area = DrawingArea::new();
        graph_area.set_content_width(360);
        graph_area.set_content_height(210);
        graph_area.set_hexpand(true);
        graph_area.set_vexpand(false);
        graph_area.set_margin_start(12);
        graph_area.set_margin_end(12);
        graph_area.set_margin_top(4);
        graph_area.set_margin_bottom(6);

        let st_draw = state.clone();
        graph_area.set_draw_func(move |area, cr, width, height| {
            let st = st_draw.borrow();
            let accent = get_accent_rgb(area);
            draw_curve_graph(cr, width as f64, height as f64, &st, accent);
        });
        left_card.append(&graph_area);

        // preset selector header
        let preset_section_label = Label::new(Some("Curve Presets"));
        preset_section_label.add_css_class("heading");
        preset_section_label.set_halign(Align::Start);
        preset_section_label.set_margin_start(16);
        preset_section_label.set_margin_top(8);
        left_card.append(&preset_section_label);

        // preset tile grid
        let preset_flow = FlowBox::new();
        preset_flow.set_max_children_per_line(4);
        preset_flow.set_min_children_per_line(4);
        preset_flow.set_selection_mode(SelectionMode::None);
        preset_flow.set_row_spacing(8);
        preset_flow.set_column_spacing(8);
        preset_flow.set_margin_start(16);
        preset_flow.set_margin_end(16);
        preset_flow.set_margin_bottom(16);

        let presets = [
            (CurvePreset::Linear, "Linear"),
            (CurvePreset::Aggro, "Aggro"),
            (CurvePreset::Slow, "Slow"),
            (CurvePreset::Smooth, "Smooth"),
            (CurvePreset::Steep, "Steep"),
            (CurvePreset::Instant, "Instant"),
            (CurvePreset::Turbo, "Turbo"),
        ];

        let preset_btn_list: Rc<RefCell<Vec<(CurvePreset, Button)>>> =
            Rc::new(RefCell::new(Vec::new()));

        for (preset_enum, label_str) in presets {
            let btn = Button::new();
            btn.add_css_class("preset-tile-btn");
            btn.set_sensitive(false);
            if preset_enum == current_preset {
                btn.add_css_class("active");
            }

            let tile_box = GtkBox::new(Orientation::Vertical, 0);
            tile_box.set_halign(gtk4::Align::Center);
            tile_box.set_valign(gtk4::Align::Center);

            // mini curve preview
            let mini_preview = DrawingArea::new();
            mini_preview.set_content_width(50);
            mini_preview.set_content_height(28);
            mini_preview.set_draw_func(move |area, cr, w, h| {
                let accent = get_accent_rgb(area);
                draw_curve_preset_icon(cr, w as f64, h as f64, preset_enum, accent);
            });
            tile_box.append(&mini_preview);

            let lbl = Label::new(Some(label_str));
            lbl.add_css_class("preset-tile-label");
            tile_box.append(&lbl);

            btn.set_child(Some(&tile_box));

            let st_click = state.clone();
            let graph_click = graph_area.clone();
            let btn_list_click = preset_btn_list.clone();
            let on_change_preset = on_change.clone();
            btn.connect_clicked(move |_| {
                if st_click.borrow().active_category.is_none() {
                    return;
                }
                let pts = preset_enum.to_points();
                let mut st = st_click.borrow_mut();
                st.current_preset = preset_enum;
                st.points = pts;
                let snapshot = st.clone_snapshot();
                drop(st);

                for (p, b) in btn_list_click.borrow().iter() {
                    if *p == preset_enum {
                        b.add_css_class("active");
                    } else {
                        b.remove_css_class("active");
                    }
                }

                graph_click.queue_draw();

                if let Some(ref cb) = on_change_preset {
                    cb(&snapshot);
                }
            });

            preset_btn_list
                .borrow_mut()
                .push((preset_enum, btn.clone()));
            preset_flow.append(&btn);
        }

        left_card.append(&preset_flow);
        left_group.add(&left_card);
        container.append(&left_group);

        // drag gesture for curve handles
        let gesture_drag = GestureDrag::new();
        let btn_list_drag = preset_btn_list.clone();
        let st_drag_begin = state.clone();
        let area_drag_begin = graph_area.clone();
        gesture_drag.connect_drag_begin(move |_gesture, start_x, start_y| {
            let width = area_drag_begin.width() as f64;
            let height = area_drag_begin.height() as f64;
            let (padding_left, padding_right, padding_top, padding_bottom) = get_paddings();
            let plot_w = width - padding_left - padding_right;
            let plot_h = height - padding_top - padding_bottom;

            let mut st = st_drag_begin.borrow_mut();
            if st.active_category.is_none() {
                return;
            }
            let hit_radius_sq = HANDLE_HIT_RADIUS * HANDLE_HIT_RADIUS;
            let mut hit_handle = None;

            for (i, &(px, py)) in st.points.iter().enumerate() {
                let hx = padding_left + (px / 255.0) * plot_w;
                let hy = padding_top + plot_h - (py / 255.0) * plot_h;
                let dx = start_x - hx;
                let dy = start_y - hy;
                if dx * dx + dy * dy <= hit_radius_sq {
                    hit_handle = Some(i);
                    break;
                }
            }

            if let Some(h) = hit_handle {
                st.dragging_handle = Some(h);
                st.drag_start_point = Some(st.points[h]);
                st.current_preset = CurvePreset::Custom;
                for (_, b) in btn_list_drag.borrow().iter() {
                    b.remove_css_class("active");
                }
                area_drag_begin.queue_draw();
            }
        });

        let st_drag_update = state.clone();
        let area_drag_update = graph_area.clone();
        gesture_drag.connect_drag_update(move |_gesture, offset_x, offset_y| {
            let width = area_drag_update.width() as f64;
            let height = area_drag_update.height() as f64;
            let (padding_left, padding_right, padding_top, padding_bottom) = get_paddings();
            let plot_w = width - padding_left - padding_right;
            let plot_h = height - padding_top - padding_bottom;

            let mut st = st_drag_update.borrow_mut();
            if let (Some(h_idx), Some((orig_x, orig_y))) = (st.dragging_handle, st.drag_start_point)
            {
                let dx_val = (offset_x / plot_w) * 255.0;
                let dy_val = (-offset_y / plot_h) * 255.0;

                clamp_handle_movement(&mut st.points, h_idx, orig_x + dx_val, orig_y + dy_val);
                area_drag_update.queue_draw();
            }
        });

        let st_drag_end = state.clone();
        let area_drag_end = graph_area.clone();
        let on_change_drag = on_change.clone();
        gesture_drag.connect_drag_end(move |_gesture, _offset_x, _offset_y| {
            let mut st = st_drag_end.borrow_mut();
            st.dragging_handle = None;
            st.drag_start_point = None;
            let snapshot = st.clone_snapshot();
            drop(st);
            area_drag_end.queue_draw();

            if let Some(ref cb) = on_change_drag {
                cb(&snapshot);
            }
        });

        graph_area.add_controller(gesture_drag);

        // hover controller
        let motion = EventControllerMotion::new();
        let st_motion = state.clone();
        let area_motion = graph_area.clone();
        motion.connect_motion(move |_ctrl, mouse_x, mouse_y| {
            let width = area_motion.width() as f64;
            let height = area_motion.height() as f64;
            let (padding_left, padding_right, padding_top, padding_bottom) = get_paddings();
            let plot_w = width - padding_left - padding_right;
            let plot_h = height - padding_top - padding_bottom;

            let mut st = st_motion.borrow_mut();
            if st.active_category.is_none() {
                if st.hovered_handle.is_some() {
                    st.hovered_handle = None;
                    area_motion.queue_draw();
                }
                return;
            }
            let hit_radius_sq = HANDLE_HIT_RADIUS * HANDLE_HIT_RADIUS;
            let mut hovered = None;

            for (i, &(px, py)) in st.points.iter().enumerate() {
                let hx = padding_left + (px / 255.0) * plot_w;
                let hy = padding_top + plot_h - (py / 255.0) * plot_h;
                let dx = mouse_x - hx;
                let dy = mouse_y - hy;
                if dx * dx + dy * dy <= hit_radius_sq {
                    hovered = Some(i);
                    break;
                }
            }

            if st.hovered_handle != hovered {
                st.hovered_handle = hovered;
                area_motion.queue_draw();
            }
        });

        let st_leave = state.clone();
        let area_leave = graph_area.clone();
        motion.connect_leave(move |_ctrl| {
            st_leave.borrow_mut().hovered_handle = None;
            area_leave.queue_draw();
        });

        graph_area.add_controller(motion);

        // right column: stick dynamics
        let right_group = PreferencesGroup::new();
        right_group.set_title("Joystick Dynamics");

        let angle_switch = SwitchRow::new();
        angle_switch.set_title("Enable Diagonal Angle Adjustment");
        angle_switch.set_subtitle("Shift the center point of diagonal movement directions");
        angle_switch.set_active(enable_angle_adj);

        let angle_adj = gtk4::Adjustment::new(angle_adjustment, 0.0, 90.0, 1.0, 5.0, 0.0);
        let angle_spin = SpinRow::new(Some(&angle_adj), 1.0, 0);
        angle_spin.set_title("Diagonal Angle");
        angle_spin.set_subtitle(
            "Center point of the diagonal direction in degrees (45° = standard diagonal)",
        );
        angle_spin.set_visible(enable_angle_adj);

        let st_ang = state.clone();
        let on_change_ang = on_change.clone();
        let updating_ang = updating.clone();
        angle_spin.connect_changed(move |sr| {
            if updating_ang.get() {
                return;
            }
            let mut st = st_ang.borrow_mut();
            st.angle_adjustment = sr.value();
            let snapshot = st.clone_snapshot();
            drop(st);
            if let Some(ref cb) = on_change_ang {
                cb(&snapshot);
            }
        });

        let st_a = state.clone();
        let angle_spin_toggle = angle_spin.clone();
        let on_change_sw = on_change.clone();
        let updating_sw = updating.clone();
        angle_switch.connect_active_notify(move |sw| {
            let active = sw.is_active();
            if updating_sw.get() {
                angle_spin_toggle.set_visible(active);
                return;
            }
            let mut st = st_a.borrow_mut();
            st.enable_angle_adj = active;
            let snapshot = st.clone_snapshot();
            drop(st);
            angle_spin_toggle.set_visible(active);
            if let Some(ref cb) = on_change_sw {
                cb(&snapshot);
            }
        });

        right_group.add(&angle_switch);
        right_group.add(&angle_spin);

        let square_switch = SwitchRow::new();
        square_switch.set_title("Square Joystick Output");
        square_switch.set_subtitle(
            "Restrict the joystick movement vector to a square to maximize diagonal axis output",
        );
        square_switch.set_active(square_joystick);

        let st_sq = state.clone();
        let on_change_sq = on_change.clone();
        let updating_sq = updating.clone();
        square_switch.connect_active_notify(move |sw| {
            if updating_sq.get() {
                return;
            }
            let mut st = st_sq.borrow_mut();
            st.square_joystick = sw.is_active();
            let snapshot = st.clone_snapshot();
            drop(st);
            if let Some(ref cb) = on_change_sq {
                cb(&snapshot);
            }
        });
        right_group.add(&square_switch);

        let snappy_switch = SwitchRow::new();
        snappy_switch.set_title("Snappy Joystick");
        snappy_switch.set_subtitle(
            "Prioritize maximum displacement instead of combining opposite axis directions",
        );
        snappy_switch.set_active(snappy_joystick);

        let st_sn = state.clone();
        let on_change_sn = on_change;
        let updating_sn = updating.clone();
        snappy_switch.connect_active_notify(move |sw| {
            if updating_sn.get() {
                return;
            }
            let mut st = st_sn.borrow_mut();
            st.snappy_joystick = sw.is_active();
            let snapshot = st.clone_snapshot();
            drop(st);
            if let Some(ref cb) = on_change_sn {
                cb(&snapshot);
            }
        });
        right_group.add(&snappy_switch);

        container.append(&right_group);

        Self {
            container: outer_container,
            state,
            graph_area,
            title_label,
            preset_btn_list,
            angle_switch,
            angle_spin,
            square_switch,
            snappy_switch,
            updating,
        }
    }

    pub fn set_active_category(
        &self,
        category: Option<AnalogCategory>,
        preset: &str,
        points: [(f64, f64); 4],
    ) {
        self.updating.set(true);
        let current_preset = CurvePreset::from_str_name(preset);
        {
            let mut st = self.state.borrow_mut();
            st.active_category = category;
            st.current_preset = current_preset;
            st.points = points;
            st.dragging_handle = None;
            st.drag_start_point = None;
            st.hovered_handle = None;
        }

        let is_active = category.is_some();
        for (p, b) in self.preset_btn_list.borrow().iter() {
            b.set_sensitive(is_active);
            if is_active && *p == current_preset {
                b.add_css_class("active");
            } else {
                b.remove_css_class("active");
            }
        }

        if let Some(cat) = category {
            self.title_label
                .set_text(&format!("Response Curve ({})", cat.display_name()));
        } else {
            self.title_label.set_text("Analog Response Curve");
        }

        self.graph_area.queue_draw();
        self.updating.set(false);
    }

    pub fn set_state(
        &self,
        preset_name: &str,
        points: [(f64, f64); 4],
        enable_angle_adj: bool,
        angle_adjustment: f64,
        square_joystick: bool,
        snappy_joystick: bool,
    ) {
        self.updating.set(true);
        let current_preset = CurvePreset::from_str_name(preset_name);
        {
            let mut st = self.state.borrow_mut();
            st.current_preset = current_preset;
            st.points = points;
            st.enable_angle_adj = enable_angle_adj;
            st.angle_adjustment = angle_adjustment;
            st.square_joystick = square_joystick;
            st.snappy_joystick = snappy_joystick;
            st.dragging_handle = None;
            st.drag_start_point = None;
            st.hovered_handle = None;
        }

        let is_active = self.state.borrow().active_category.is_some();
        for (p, b) in self.preset_btn_list.borrow().iter() {
            b.set_sensitive(is_active);
            if is_active && *p == current_preset {
                b.add_css_class("active");
            } else {
                b.remove_css_class("active");
            }
        }

        self.angle_switch.set_active(enable_angle_adj);
        self.angle_spin.set_value(angle_adjustment);
        self.angle_spin.set_visible(enable_angle_adj);
        self.square_switch.set_active(square_joystick);
        self.snappy_switch.set_active(snappy_joystick);

        self.graph_area.queue_draw();
        self.updating.set(false);
    }
}

impl ConfigViewState {
    pub fn clone_snapshot(&self) -> Self {
        Self {
            active_category: self.active_category,
            current_preset: self.current_preset,
            points: self.points,
            dragging_handle: self.dragging_handle,
            drag_start_point: self.drag_start_point,
            hovered_handle: self.hovered_handle,
            angle_adjustment: self.angle_adjustment,
            enable_angle_adj: self.enable_angle_adj,
            square_joystick: self.square_joystick,
            snappy_joystick: self.snappy_joystick,
            live_depth_mm: self.live_depth_mm,
        }
    }
}
