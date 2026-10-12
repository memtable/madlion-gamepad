use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gtk4::DrawingArea;
use gtk4::cairo::{Context, ImageSurface};
use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::glib;
use gtk4::prelude::*;

use crate::keyboard::{KeyId, find_key_matrix_index};
use crate::ui::get_accent_rgb;

#[derive(Clone, Debug)]
pub struct KeyCapLayout {
    pub key: KeyId,
    pub label: &'static str,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub matrix_index: usize,
}

pub struct KeyboardViewState {
    pub key_depths: Arc<Mutex<[f32; 70]>>,
    pub key_bindings: Arc<Mutex<HashMap<KeyId, String>>>,
    pub hovered_key: Option<KeyId>,
    pub drag_hover_key: Option<KeyId>,
    pub selected_group: HashSet<KeyId>,
    pub board_image: Option<ImageSurface>,
    pub icon_cache: HashMap<String, Pixbuf>,
}

#[allow(dead_code)]
pub struct KeyboardView {
    pub drawing_area: DrawingArea,
    pub state: Rc<RefCell<KeyboardViewState>>,
}

impl KeyboardView {
    pub fn set_selected_group(&self, group: HashSet<KeyId>) {
        self.state.borrow_mut().selected_group = group;
        self.drawing_area.queue_draw();
    }
}

const CHASSIS_ASPECT_RATIO: f64 = 1000.0 / 384.0; // ~2.604167
const MAX_KEYBOARD_WIDTH: f64 = 820.0;
const MAX_KEYBOARD_HEIGHT: f64 = MAX_KEYBOARD_WIDTH / CHASSIS_ASPECT_RATIO; // ~314.88

impl KeyboardView {
    pub fn new(
        key_depths: Arc<Mutex<[f32; 70]>>,
        key_bindings: Arc<Mutex<HashMap<KeyId, String>>>,
        on_binding_changed: Option<Rc<dyn Fn()>>,
        on_key_clicked: Option<Rc<dyn Fn(KeyId)>>,
    ) -> Self {
        let drawing_area = DrawingArea::new();
        drawing_area.set_content_width(820);
        drawing_area.set_content_height(315);
        drawing_area.set_hexpand(true);
        drawing_area.set_vexpand(false);

        // load board image n icon cache
        let board_image = load_board_image();
        let accent_hex = crate::ui::get_accent_hex(&drawing_area);
        let icon_cache = load_icon_cache(&accent_hex);

        let state = Rc::new(RefCell::new(KeyboardViewState {
            key_depths,
            key_bindings: key_bindings.clone(),
            hovered_key: None,
            drag_hover_key: None,
            selected_group: HashSet::new(),
            board_image,
            icon_cache,
        }));

        let state_clone = state.clone();
        drawing_area.set_draw_func(move |area, cr, width, height| {
            let st = state_clone.borrow();
            let accent = get_accent_rgb(area);
            draw_keyboard(cr, width as f64, height as f64, &st, accent);
        });

        // drop target for controller badges
        let drop_target = gtk4::DropTarget::new(glib::Type::STRING, gtk4::gdk::DragAction::COPY);

        let state_drop_motion = state.clone();
        let area_drop_motion = drawing_area.clone();
        drop_target.connect_motion(move |_target, x, y| {
            let width = area_drop_motion.width() as f64;
            let height = area_drop_motion.height() as f64;
            let (offset_x, offset_y, fit_w, fit_h) = compute_keyboard_viewport(width, height);
            let key_layouts = compute_key_layouts(offset_x, offset_y, fit_w, fit_h);
            let mut target_key = None;
            for kl in key_layouts {
                if x >= kl.x && x <= kl.x + kl.w && y >= kl.y && y <= kl.y + kl.h {
                    target_key = Some(kl.key);
                    break;
                }
            }
            if state_drop_motion.borrow().drag_hover_key != target_key {
                state_drop_motion.borrow_mut().drag_hover_key = target_key;
                area_drop_motion.queue_draw();
            }
            gtk4::gdk::DragAction::COPY
        });

        let state_drop_leave = state.clone();
        let area_drop_leave = drawing_area.clone();
        drop_target.connect_leave(move |_target| {
            state_drop_leave.borrow_mut().drag_hover_key = None;
            area_drop_leave.queue_draw();
        });

        let state_drop = state.clone();
        let bindings_drop = key_bindings.clone();
        let area_drop = drawing_area.clone();
        let on_change_drop = on_binding_changed.clone();
        drop_target.connect_drop(move |_target, value, x, y| {
            state_drop.borrow_mut().drag_hover_key = None;
            if let Ok(code_str) = value.get::<String>() {
                let width = area_drop.width() as f64;
                let height = area_drop.height() as f64;
                let (offset_x, offset_y, fit_w, fit_h) = compute_keyboard_viewport(width, height);
                let key_layouts = compute_key_layouts(offset_x, offset_y, fit_w, fit_h);
                for kl in key_layouts {
                    if x >= kl.x && x <= kl.x + kl.w && y >= kl.y && y <= kl.y + kl.h {
                        let mut b = bindings_drop.lock().unwrap();
                        if code_str == "Clear" {
                            b.remove(&kl.key);
                        } else {
                            b.insert(kl.key, code_str.clone());
                        }
                        drop(b);
                        area_drop.queue_draw();
                        if let Some(ref cb) = on_change_drop {
                            cb();
                        }
                        return true;
                    }
                }
            }
            false
        });
        drawing_area.add_controller(drop_target);

        // right click clears key binding
        let gesture_right = gtk4::GestureClick::new();
        gesture_right.set_button(3);
        let bindings_right = key_bindings;
        let area_right = drawing_area.clone();
        let on_change_right = on_binding_changed;
        gesture_right.connect_pressed(move |_g, _n, x, y| {
            let width = area_right.width() as f64;
            let height = area_right.height() as f64;
            let (offset_x, offset_y, fit_w, fit_h) = compute_keyboard_viewport(width, height);
            let key_layouts = compute_key_layouts(offset_x, offset_y, fit_w, fit_h);
            for kl in key_layouts {
                if x >= kl.x && x <= kl.x + kl.w && y >= kl.y && y <= kl.y + kl.h {
                    let mut b = bindings_right.lock().unwrap();
                    b.remove(&kl.key);
                    drop(b);
                    area_right.queue_draw();
                    if let Some(ref cb) = on_change_right {
                        cb();
                    }
                    break;
                }
            }
        });
        drawing_area.add_controller(gesture_right);

        // hover tracking
        let motion = gtk4::EventControllerMotion::new();
        let state_motion = state.clone();
        let area_motion = drawing_area.clone();
        motion.connect_motion(move |_c, x, y| {
            let width = area_motion.width() as f64;
            let height = area_motion.height() as f64;
            let (offset_x, offset_y, fit_w, fit_h) = compute_keyboard_viewport(width, height);
            let key_layouts = compute_key_layouts(offset_x, offset_y, fit_w, fit_h);
            let mut hovered = None;
            for kl in key_layouts {
                if x >= kl.x && x <= kl.x + kl.w && y >= kl.y && y <= kl.y + kl.h {
                    hovered = Some(kl.key);
                    break;
                }
            }
            if state_motion.borrow().hovered_key != hovered {
                state_motion.borrow_mut().hovered_key = hovered;
                area_motion.queue_draw();
            }
        });
        drawing_area.add_controller(motion);

        // left click selects key for curve tuning
        let gesture_left = gtk4::GestureClick::new();
        gesture_left.set_button(1);
        let area_left = drawing_area.clone();
        let on_click = on_key_clicked;
        gesture_left.connect_pressed(move |_g, _n, x, y| {
            let width = area_left.width() as f64;
            let height = area_left.height() as f64;
            let (offset_x, offset_y, fit_w, fit_h) = compute_keyboard_viewport(width, height);
            let key_layouts = compute_key_layouts(offset_x, offset_y, fit_w, fit_h);
            for kl in key_layouts {
                if x >= kl.x && x <= kl.x + kl.w && y >= kl.y && y <= kl.y + kl.h {
                    if let Some(ref cb) = on_click {
                        cb(kl.key);
                    }
                    break;
                }
            }
        });
        drawing_area.add_controller(gesture_left);

        Self {
            drawing_area,
            state,
        }
    }
}

fn load_board_image() -> Option<ImageSurface> {
    if let Some(surface) = crate::ui::get_board_image() {
        return Some(surface);
    }
    let path = "resources/dark_board.png";
    if std::path::Path::new(path).exists()
        && let Ok(mut file) = std::fs::File::open(path)
        && let Ok(surface) = ImageSurface::create_from_png(&mut file)
    {
        return Some(surface);
    }
    None
}

fn load_icon_cache(accent_hex: &str) -> HashMap<String, Pixbuf> {
    let mut map = HashMap::new();
    let codes = [
        "A", "B", "X", "Y", "Start", "Back", "Guide", "D-U", "D-D", "D-L", "D-R", "LS-U", "LS-D",
        "LS-L", "LS-R", "L3", "RS-U", "RS-D", "RS-L", "RS-R", "R3", "LB", "LT", "RB", "RT",
    ];

    for code in codes {
        if let Some(pixbuf) = crate::ui::load_accented_pixbuf_for_code(code, accent_hex, 32) {
            map.insert(code.to_string(), pixbuf);
        }
    }

    map
}

// fit keyboard chassis maintaining aspect ratio
fn compute_keyboard_viewport(area_w: f64, area_h: f64) -> (f64, f64, f64, f64) {
    let mut fit_w = area_w.min(MAX_KEYBOARD_WIDTH);
    let mut fit_h = fit_w / CHASSIS_ASPECT_RATIO;

    if fit_h > area_h.min(MAX_KEYBOARD_HEIGHT) {
        fit_h = area_h.min(MAX_KEYBOARD_HEIGHT);
        fit_w = fit_h * CHASSIS_ASPECT_RATIO;
    }

    let offset_x = (area_w - fit_w) / 2.0;
    let offset_y = (area_h - fit_h) / 2.0;

    (offset_x, offset_y, fit_w, fit_h)
}

fn get_matrix_rows() -> Vec<Vec<(KeyId, &'static str, f64)>> {
    vec![
        // row 0
        vec![
            (KeyId::Esc, "Esc", 1.0),
            (KeyId::Key1, "1", 1.0),
            (KeyId::Key2, "2", 1.0),
            (KeyId::Key3, "3", 1.0),
            (KeyId::Key4, "4", 1.0),
            (KeyId::Key5, "5", 1.0),
            (KeyId::Key6, "6", 1.0),
            (KeyId::Key7, "7", 1.0),
            (KeyId::Key8, "8", 1.0),
            (KeyId::Key9, "9", 1.0),
            (KeyId::Key0, "0", 1.0),
            (KeyId::Minus, "-", 1.0),
            (KeyId::Equal, "=", 1.0),
            (KeyId::Backspace, "Backspace", 2.0),
        ],
        // row 1
        vec![
            (KeyId::Tab, "Tab", 1.5),
            (KeyId::Q, "Q", 1.0),
            (KeyId::W, "W", 1.0),
            (KeyId::E, "E", 1.0),
            (KeyId::R, "R", 1.0),
            (KeyId::T, "T", 1.0),
            (KeyId::Y, "Y", 1.0),
            (KeyId::U, "U", 1.0),
            (KeyId::I, "I", 1.0),
            (KeyId::O, "O", 1.0),
            (KeyId::P, "P", 1.0),
            (KeyId::LeftBracket, "[", 1.0),
            (KeyId::RightBracket, "]", 1.0),
            (KeyId::Backslash, "\\", 1.5),
        ],
        // row 2
        vec![
            (KeyId::CapsLock, "Caps", 1.75),
            (KeyId::A, "A", 1.0),
            (KeyId::S, "S", 1.0),
            (KeyId::D, "D", 1.0),
            (KeyId::F, "F", 1.0),
            (KeyId::G, "G", 1.0),
            (KeyId::H, "H", 1.0),
            (KeyId::J, "J", 1.0),
            (KeyId::K, "K", 1.0),
            (KeyId::L, "L", 1.0),
            (KeyId::Semicolon, ";", 1.0),
            (KeyId::Quote, "'", 1.0),
            (KeyId::Enter, "Enter", 2.25),
        ],
        // row 3
        vec![
            (KeyId::LeftShift, "L-Shift", 2.25),
            (KeyId::Z, "Z", 1.0),
            (KeyId::X, "X", 1.0),
            (KeyId::C, "C", 1.0),
            (KeyId::V, "V", 1.0),
            (KeyId::B, "B", 1.0),
            (KeyId::N, "N", 1.0),
            (KeyId::M, "M", 1.0),
            (KeyId::Comma, ",", 1.0),
            (KeyId::Dot, ".", 1.0),
            (KeyId::Slash, "/", 1.0),
            (KeyId::RightShift, "R-Shift", 2.75),
        ],
        // row 4
        vec![
            (KeyId::LeftControl, "L-Ctrl", 1.25),
            (KeyId::LeftMeta, "L-Super", 1.25),
            (KeyId::LeftAlt, "L-Alt", 1.25),
            (KeyId::Space, "Spacebar", 6.25),
            (KeyId::RightAlt, "R-Alt", 1.25),
            (KeyId::Menu, "Menu", 1.25),
            (KeyId::RightControl, "R-Ctrl", 1.25),
            (KeyId::Fn, "Fn 1", 1.25),
        ],
    ]
}

fn compute_key_layouts(offset_x: f64, offset_y: f64, fit_w: f64, fit_h: f64) -> Vec<KeyCapLayout> {
    let mut layouts = Vec::new();
    let rows = get_matrix_rows();

    let margin_x = offset_x + fit_w * 0.040;
    let margin_top = offset_y + fit_h * 0.062;
    let available_w = fit_w * 0.938;
    let available_h = fit_h * 0.795;

    let row_count = rows.len() as f64;
    let gap = 3.5 * (fit_w / MAX_KEYBOARD_WIDTH);
    let total_units_x = 15.0;
    let unit_w = (available_w - (total_units_x - 1.0) * gap) / total_units_x;
    let unit_h = (available_h - (row_count - 1.0) * gap) / row_count;

    let mut current_y = margin_top;

    for row in rows.iter() {
        let mut current_x = margin_x;
        for (key, label, u_w) in row {
            let key_w = u_w * unit_w + (u_w - 1.0) * gap;
            let key_h = unit_h;
            let matrix_index = find_key_matrix_index(*key)
                .map(|idx| idx as usize)
                .unwrap_or(0);

            layouts.push(KeyCapLayout {
                key: *key,
                label,
                x: current_x,
                y: current_y,
                w: key_w,
                h: key_h,
                matrix_index,
            });

            current_x += key_w + gap;
        }
        current_y += unit_h + gap;
    }

    layouts
}

fn draw_keyboard(
    cr: &Context,
    width: f64,
    height: f64,
    state: &KeyboardViewState,
    accent: (f64, f64, f64),
) {
    let (offset_x, offset_y, fit_w, fit_h) = compute_keyboard_viewport(width, height);

    // draw chassis frame
    if let Some(ref img) = state.board_image {
        let img_w = img.width() as f64;
        let img_h = img.height() as f64;
        cr.save().unwrap();
        cr.translate(offset_x, offset_y);
        cr.scale(fit_w / img_w, fit_h / img_h);
        cr.set_source_surface(img, 0.0, 0.0).unwrap();
        cr.paint().unwrap();
        cr.restore().unwrap();
    } else {
        draw_rounded_rect(cr, offset_x, offset_y, fit_w, fit_h, 10.0);
        cr.set_source_rgb(0.11, 0.12, 0.13);
        cr.fill().unwrap();
        draw_rounded_rect(
            cr,
            offset_x + 1.0,
            offset_y + 1.0,
            fit_w - 2.0,
            fit_h - 2.0,
            10.0,
        );
        cr.set_source_rgb(0.22, 0.23, 0.25);
        cr.set_line_width(1.5);
        cr.stroke().unwrap();
    }

    // compute keycap boxes
    let key_layouts = compute_key_layouts(offset_x, offset_y, fit_w, fit_h);
    let depths = state.key_depths.lock().unwrap();
    let bindings = state.key_bindings.lock().unwrap();

    for kl in key_layouts {
        let depth = if kl.matrix_index < depths.len() {
            depths[kl.matrix_index]
        } else {
            0.0
        };

        let is_hovered = state.hovered_key == Some(kl.key);
        let is_drag_target = state.drag_hover_key == Some(kl.key);
        let is_group_selected = state.selected_group.contains(&kl.key);
        let binding = bindings.get(&kl.key);
        let icon = binding.and_then(|b| state.icon_cache.get(b));

        let draw_info = KeycapDrawInfo {
            depth,
            is_hovered,
            is_drag_target,
            is_group_selected,
            binding,
            icon,
            accent,
        };

        draw_keycap(cr, &kl, &draw_info);
    }
}

struct KeycapDrawInfo<'a> {
    depth: f32,
    is_hovered: bool,
    is_drag_target: bool,
    is_group_selected: bool,
    binding: Option<&'a String>,
    icon: Option<&'a Pixbuf>,
    accent: (f64, f64, f64),
}

fn draw_keycap(cr: &Context, kl: &KeyCapLayout, info: &KeycapDrawInfo) {
    let radius = 4.0;

    // keycap shadow
    draw_rounded_rect(cr, kl.x, kl.y + 1.5, kl.w, kl.h, radius);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.5);
    cr.fill().unwrap();

    // keycap base
    draw_rounded_rect(cr, kl.x, kl.y, kl.w, kl.h, radius);
    if info.is_drag_target {
        cr.set_source_rgb(0.25, 0.45, 0.65); // highlighted drop target
    } else if info.is_group_selected {
        cr.set_source_rgb(
            0.18 + info.accent.0 * 0.14,
            0.19 + info.accent.1 * 0.14,
            0.22 + info.accent.2 * 0.14,
        );
    } else if info.is_hovered {
        cr.set_source_rgb(0.23, 0.24, 0.27);
    } else {
        cr.set_source_rgb(0.16, 0.17, 0.19);
    }
    cr.fill().unwrap();

    // live travel depth fill
    if info.depth > 0.01 {
        let fill_h = (kl.h * (info.depth as f64)).min(kl.h);
        let fill_y = kl.y + kl.h - fill_h;

        cr.save().unwrap();
        draw_rounded_rect(cr, kl.x, kl.y, kl.w, kl.h, radius);
        cr.clip();

        let (r, g, b) = if info.depth > 0.75 {
            (0.95, 0.30, 0.25)
        } else if info.depth > 0.35 {
            (0.95, 0.75, 0.20)
        } else {
            (0.20, 0.80, 0.50)
        };

        draw_rounded_rect(cr, kl.x, fill_y, kl.w, fill_h, 0.0);
        cr.set_source_rgba(r, g, b, 0.45 + (info.depth as f64) * 0.45);
        cr.fill().unwrap();
        cr.restore().unwrap();
    }

    // keycap border
    draw_rounded_rect(cr, kl.x, kl.y, kl.w, kl.h, radius);
    if info.is_drag_target {
        cr.set_source_rgb(0.40, 0.75, 1.0);
        cr.set_line_width(2.2);
    } else if info.is_group_selected {
        cr.set_source_rgb(info.accent.0, info.accent.1, info.accent.2);
        cr.set_line_width(2.0);
    } else if info.is_hovered {
        cr.set_source_rgb(0.45, 0.48, 0.52);
        cr.set_line_width(1.2);
    } else {
        cr.set_source_rgba(0.28, 0.29, 0.33, 0.7);
        cr.set_line_width(0.9);
    }
    cr.stroke().unwrap();

    // selection glow halo
    if info.is_group_selected && !info.is_drag_target {
        draw_rounded_rect(
            cr,
            kl.x - 1.5,
            kl.y - 1.5,
            kl.w + 3.0,
            kl.h + 3.0,
            radius + 1.2,
        );
        cr.set_source_rgba(info.accent.0, info.accent.1, info.accent.2, 0.45);
        cr.set_line_width(1.5);
        cr.stroke().unwrap();
    }

    // draw badge icon if mapped
    if let Some(pixbuf) = info.icon {
        let icon_w = pixbuf.width() as f64;
        let icon_h = pixbuf.height() as f64;
        let icon_x = kl.x + (kl.w - icon_w) / 2.0;
        let icon_y = kl.y + (kl.h - icon_h) / 2.0;

        cr.set_source_pixbuf(pixbuf, icon_x, icon_y);
        cr.paint().unwrap();
    } else if let Some(b_text) = info.binding {
        // text fallback if icon missing
        let badge_w = (kl.w - 6.0).clamp(14.0, 32.0);
        let badge_h = 14.0;
        let badge_x = kl.x + (kl.w - badge_w) / 2.0;
        let badge_y = kl.y + (kl.h - badge_h) / 2.0;

        draw_rounded_rect(cr, badge_x, badge_y, badge_w, badge_h, 3.5);
        cr.set_source_rgb(info.accent.0, info.accent.1, info.accent.2);
        cr.fill().unwrap();

        cr.set_source_rgb(0.08, 0.08, 0.08);
        cr.set_font_size(8.5);
        let b_ext = cr.text_extents(b_text).unwrap();
        cr.move_to(
            badge_x + (badge_w - b_ext.width()) / 2.0,
            badge_y + (badge_h + b_ext.height()) / 2.0 - 0.5,
        );
        cr.show_text(b_text).unwrap();
    } else {
        // draw normal key label if unmapped
        cr.set_source_rgba(0.90, 0.90, 0.93, 0.88);
        cr.set_font_size(9.5);
        let extents = cr.text_extents(kl.label).unwrap();
        let text_x = kl.x + (kl.w - extents.width()) / 2.0;
        let text_y = kl.y + (kl.h + extents.height()) / 2.0 - 1.0;
        cr.move_to(text_x, text_y);
        cr.show_text(kl.label).unwrap();
    }
}

fn draw_rounded_rect(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    cr.arc(
        x + r,
        y + h - r,
        r,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    cr.arc(
        x + r,
        y + r,
        r,
        std::f64::consts::PI,
        3.0 * std::f64::consts::FRAC_PI_2,
    );
    cr.close_path();
}
