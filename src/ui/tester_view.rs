use std::cell::RefCell;
use std::rc::Rc;

use gtk4::cairo::Context;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, DrawingArea, Label, Orientation, ProgressBar};
use libadwaita::prelude::*;
use libadwaita::{ActionRow, Clamp, PreferencesGroup};

use crate::ui::get_accent_rgb;

pub struct TesterViewState {
    pub left_stick_x: f32,  // -1.0..1.0
    pub left_stick_y: f32,  // -1.0..1.0
    pub right_stick_x: f32, // -1.0..1.0
    pub right_stick_y: f32, // -1.0..1.0
    pub left_trigger: f32,  // 0.0..1.0
    pub right_trigger: f32, // 0.0..1.0
    pub btn_a: bool,
    pub btn_b: bool,
    pub btn_x: bool,
    pub btn_y: bool,
    pub btn_lb: bool,
    pub btn_rb: bool,
    pub btn_l3: bool,
    pub btn_r3: bool,
    pub poll_hz: f64,
    pub latency_us: u64,
}

pub struct TesterView {
    pub container: GtkBox,
    pub state: Rc<RefCell<TesterViewState>>,
    pub scope_area: DrawingArea,
    pub button_area: DrawingArea,
    pub lt_bar: ProgressBar,
    pub rt_bar: ProgressBar,
    pub hz_label: Label,
}

impl TesterView {
    pub fn new() -> Self {
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

        let state = Rc::new(RefCell::new(TesterViewState {
            left_stick_x: 0.0,
            left_stick_y: 0.0,
            right_stick_x: 0.0,
            right_stick_y: 0.0,
            left_trigger: 0.0,
            right_trigger: 0.0,
            btn_a: false,
            btn_b: false,
            btn_x: false,
            btn_y: false,
            btn_lb: false,
            btn_rb: false,
            btn_l3: false,
            btn_r3: false,
            poll_hz: 1000.0,
            latency_us: 850,
        }));

        // left column: stick scopes
        let left_box = GtkBox::new(Orientation::Vertical, 12);
        left_box.set_hexpand(true);

        let left_group = PreferencesGroup::new();

        let scope_area = DrawingArea::new();
        scope_area.set_content_width(320);
        scope_area.set_content_height(170);
        scope_area.set_hexpand(true);
        scope_area.set_vexpand(false);

        let st_draw = state.clone();
        scope_area.set_draw_func(move |area, cr, width, height| {
            let st = st_draw.borrow();
            let accent = get_accent_rgb(area);
            draw_stick_scopes(cr, width as f64, height as f64, &st, accent);
        });

        let scope_row = ActionRow::new();
        scope_row.set_title("Analog Joysticks");
        scope_row.set_subtitle("Left Stick (LS) & Right Stick (RS)");
        scope_row.add_suffix(&scope_area);
        left_group.add(&scope_row);

        left_box.append(&left_group);

        // right column: triggers, buttons n telemetry
        let right_box = GtkBox::new(Orientation::Vertical, 12);
        right_box.set_hexpand(true);

        let right_group = PreferencesGroup::new();

        let lt_bar = ProgressBar::new();
        lt_bar.set_fraction(0.0);
        lt_bar.set_show_text(true);
        lt_bar.set_text(Some("0.0%"));
        lt_bar.set_hexpand(true);

        let lt_row = ActionRow::new();
        lt_row.set_title("Left Trigger (LT)");
        lt_row.add_suffix(&lt_bar);
        right_group.add(&lt_row);

        let rt_bar = ProgressBar::new();
        rt_bar.set_fraction(0.0);
        rt_bar.set_show_text(true);
        rt_bar.set_text(Some("0.0%"));
        rt_bar.set_hexpand(true);

        let rt_row = ActionRow::new();
        rt_row.set_title("Right Trigger (RT)");
        rt_row.add_suffix(&rt_bar);
        right_group.add(&rt_row);

        // button grid canvas
        let button_area = DrawingArea::new();
        button_area.set_content_width(280);
        button_area.set_content_height(40);
        button_area.set_hexpand(true);
        let st_btn = state.clone();
        button_area.set_draw_func(move |_area, cr, width, height| {
            let st = st_btn.borrow();
            draw_button_states(cr, width as f64, height as f64, &st);
        });

        let btn_row = ActionRow::new();
        btn_row.set_title("Button Actuations");
        btn_row.set_subtitle("Real-time press highlights");
        btn_row.add_suffix(&button_area);
        right_group.add(&btn_row);

        let hz_label = Label::new(Some("1000 Hz (~0.85 ms)"));
        hz_label.add_css_class("numeric");
        hz_label.add_css_class("heading");

        let stats_row = ActionRow::new();
        stats_row.set_title("USB Polling Rate");
        stats_row.set_subtitle("Direct hidraw synchronous query round-trip");
        stats_row.add_suffix(&hz_label);
        right_group.add(&stats_row);

        right_box.append(&right_group);

        container.append(&left_box);
        container.append(&right_box);

        Self {
            container: outer_container,
            state,
            scope_area,
            button_area,
            lt_bar,
            rt_bar,
            hz_label,
        }
    }

    pub fn update(&self) {
        let st = self.state.borrow();
        self.lt_bar.set_fraction(st.left_trigger as f64);
        self.lt_bar
            .set_text(Some(&format!("{:.1}%", st.left_trigger * 100.0)));

        self.rt_bar.set_fraction(st.right_trigger as f64);
        self.rt_bar
            .set_text(Some(&format!("{:.1}%", st.right_trigger * 100.0)));

        self.hz_label.set_text(&format!(
            "{:.0} Hz ({:.2} ms)",
            st.poll_hz,
            st.latency_us as f64 / 1000.0
        ));

        self.scope_area.queue_draw();
        self.button_area.queue_draw();
    }
}

fn draw_stick_scopes(
    cr: &Context,
    width: f64,
    height: f64,
    state: &TesterViewState,
    accent: (f64, f64, f64),
) {
    let half_w = width / 2.0;
    let radius = (half_w.min(height) / 2.0) - 14.0;

    // draw left stick scope
    let ls_info = StickDrawInfo {
        center: (half_w / 2.0, height / 2.0),
        radius,
        val: (state.left_stick_x as f64, state.left_stick_y as f64),
        label: "LS",
        accent,
    };
    draw_single_stick(cr, &ls_info);

    // draw right stick scope
    let rs_info = StickDrawInfo {
        center: (half_w + half_w / 2.0, height / 2.0),
        radius,
        val: (state.right_stick_x as f64, state.right_stick_y as f64),
        label: "RS",
        accent,
    };
    draw_single_stick(cr, &rs_info);
}

struct StickDrawInfo<'a> {
    center: (f64, f64),
    radius: f64,
    val: (f64, f64),
    label: &'a str,
    accent: (f64, f64, f64),
}

fn draw_single_stick(cr: &Context, info: &StickDrawInfo) {
    let (cx, cy) = info.center;
    let r = info.radius;
    let (val_x, val_y) = info.val;
    let accent = info.accent;

    // circular boundary
    cr.arc(cx, cy, r, 0.0, std::f64::consts::TAU);
    cr.set_source_rgb(0.11, 0.12, 0.14);
    cr.fill().unwrap();

    cr.arc(cx, cy, r, 0.0, std::f64::consts::TAU);
    cr.set_source_rgba(0.35, 0.36, 0.40, 0.7);
    cr.set_line_width(1.2);
    cr.stroke().unwrap();

    // crosshairs
    cr.set_source_rgba(0.25, 0.26, 0.30, 0.8);
    cr.set_line_width(0.8);
    cr.move_to(cx - r, cy);
    cr.line_to(cx + r, cy);
    cr.move_to(cx, cy - r);
    cr.line_to(cx, cy + r);
    cr.stroke().unwrap();

    // reticle dot
    let dot_x = cx + val_x * (r - 6.0);
    let dot_y = cy + val_y * (r - 6.0);

    // trace vector line
    cr.move_to(cx, cy);
    cr.line_to(dot_x, dot_y);
    cr.set_source_rgba(accent.0, accent.1, accent.2, 0.6);
    cr.set_line_width(1.5);
    cr.stroke().unwrap();

    // dot circle
    cr.arc(dot_x, dot_y, 5.0, 0.0, std::f64::consts::TAU);
    cr.set_source_rgb(accent.0, accent.1, accent.2);
    cr.fill().unwrap();

    // stick label
    cr.set_source_rgba(0.8, 0.8, 0.85, 0.7);
    cr.set_font_size(9.0);
    cr.move_to(cx - 6.0, cy - r - 4.0);
    cr.show_text(info.label).unwrap();
}

fn draw_button_states(cr: &Context, width: f64, height: f64, state: &TesterViewState) {
    let buttons = [
        ("A", state.btn_a, (0.18, 0.80, 0.44)),
        ("B", state.btn_b, (0.91, 0.30, 0.24)),
        ("X", state.btn_x, (0.20, 0.60, 0.86)),
        ("Y", state.btn_y, (0.95, 0.77, 0.06)),
        ("LB", state.btn_lb, (0.50, 0.55, 0.55)),
        ("RB", state.btn_rb, (0.50, 0.55, 0.55)),
        ("L3", state.btn_l3, (0.90, 0.49, 0.13)),
        ("R3", state.btn_r3, (0.61, 0.35, 0.71)),
    ];

    let count = buttons.len() as f64;
    let slot_w = width / count;
    let btn_size = (24.0f64).min(height - 4.0).min(slot_w - 4.0);

    for (i, (label, is_pressed, (r, g, b))) in buttons.iter().enumerate() {
        let cx = (i as f64) * slot_w + slot_w / 2.0;
        let cy = height / 2.0;

        cr.arc(cx, cy, btn_size / 2.0, 0.0, std::f64::consts::TAU);
        if *is_pressed {
            cr.set_source_rgb(*r, *g, *b);
            cr.fill().unwrap();
            cr.set_source_rgb(0.08, 0.08, 0.08);
        } else {
            cr.set_source_rgb(0.18, 0.19, 0.22);
            cr.fill().unwrap();
            cr.arc(cx, cy, btn_size / 2.0, 0.0, std::f64::consts::TAU);
            cr.set_source_rgba(0.35, 0.36, 0.40, 0.8);
            cr.set_line_width(1.0);
            cr.stroke().unwrap();
            cr.set_source_rgba(0.85, 0.85, 0.90, 0.75);
        }

        cr.set_font_size(8.5);
        let ext = cr.text_extents(label).unwrap();
        cr.move_to(cx - ext.width() / 2.0, cy + ext.height() / 2.0);
        cr.show_text(label).unwrap();
    }
}
