pub mod app;
pub mod config_view;
pub mod curve_canvas;
pub mod keyboard_view;
pub mod profile_view;
pub mod setup_view;
pub mod tester_view;

use gtk4::cairo::ImageSurface;
use gtk4::gdk_pixbuf::{Pixbuf, PixbufLoader};
use gtk4::prelude::*;

pub use app::run_gui;

pub fn get_icon_bytes(code: &str) -> Option<&'static [u8]> {
    match code {
        "A" => Some(include_bytes!("../../resources/icons/btn_a.svg")),
        "B" => Some(include_bytes!("../../resources/icons/btn_b.svg")),
        "X" => Some(include_bytes!("../../resources/icons/btn_x.svg")),
        "Y" => Some(include_bytes!("../../resources/icons/btn_y.svg")),
        "Start" => Some(include_bytes!("../../resources/icons/btn_start.svg")),
        "Back" => Some(include_bytes!("../../resources/icons/btn_back.svg")),
        "Guide" => Some(include_bytes!("../../resources/icons/btn_guide.svg")),
        "D-U" => Some(include_bytes!("../../resources/icons/dpad_up.svg")),
        "D-D" => Some(include_bytes!("../../resources/icons/dpad_down.svg")),
        "D-L" => Some(include_bytes!("../../resources/icons/dpad_left.svg")),
        "D-R" => Some(include_bytes!("../../resources/icons/dpad_right.svg")),
        "LS-U" => Some(include_bytes!("../../resources/icons/ls_up.svg")),
        "LS-D" => Some(include_bytes!("../../resources/icons/ls_down.svg")),
        "LS-L" => Some(include_bytes!("../../resources/icons/ls_left.svg")),
        "LS-R" => Some(include_bytes!("../../resources/icons/ls_right.svg")),
        "L3" => Some(include_bytes!("../../resources/icons/ls_press.svg")),
        "RS-U" => Some(include_bytes!("../../resources/icons/rs_up.svg")),
        "RS-D" => Some(include_bytes!("../../resources/icons/rs_down.svg")),
        "RS-L" => Some(include_bytes!("../../resources/icons/rs_left.svg")),
        "RS-R" => Some(include_bytes!("../../resources/icons/rs_right.svg")),
        "R3" => Some(include_bytes!("../../resources/icons/rs_press.svg")),
        "LB" => Some(include_bytes!("../../resources/icons/bumper_left.svg")),
        "LT" => Some(include_bytes!("../../resources/icons/trigger_left.svg")),
        "RB" => Some(include_bytes!("../../resources/icons/bumper_right.svg")),
        "RT" => Some(include_bytes!("../../resources/icons/trigger_right.svg")),
        _ => None,
    }
}

pub fn get_board_image() -> Option<ImageSurface> {
    const BOARD_PNG: &[u8] = include_bytes!("../../resources/dark_board.png");
    let mut reader = std::io::Cursor::new(BOARD_PNG);
    ImageSurface::create_from_png(&mut reader).ok()
}

pub fn get_accent_rgb(widget: &impl IsA<gtk4::Widget>) -> (f64, f64, f64) {
    #[allow(deprecated)]
    if let Some(c) = widget.style_context().lookup_color("accent_bg_color") {
        (c.red() as f64, c.green() as f64, c.blue() as f64)
    } else if let Some(c) = widget.style_context().lookup_color("accent_color") {
        (c.red() as f64, c.green() as f64, c.blue() as f64)
    } else {
        (0.22, 0.53, 0.96)
    }
}

pub fn get_accent_hex(widget: &impl IsA<gtk4::Widget>) -> String {
    let (r, g, b) = get_accent_rgb(widget);
    format!(
        "#{:02x}{:02x}{:02x}",
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8
    )
}

pub fn load_accented_pixbuf_for_code(code: &str, accent_hex: &str, size: i32) -> Option<Pixbuf> {
    if let Some(bytes) = get_icon_bytes(code)
        && let Ok(svg_str) = std::str::from_utf8(bytes)
    {
        let is_accentable = code.starts_with("LS")
            || code.starts_with("RS")
            || code == "L3"
            || code == "R3"
            || code == "Guide";
        let replaced = if is_accentable {
            svg_str
                .replace("#EAB308", accent_hex)
                .replace("#eab308", accent_hex)
        } else {
            svg_str.to_string()
        };
        let loader = PixbufLoader::new();
        if size > 0 {
            loader.set_size(size, size);
        }
        if loader.write(replaced.as_bytes()).is_ok() && loader.close().is_ok() {
            return loader.pixbuf();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AnalogCategory;
    use crate::gamepad::ResponseCurve;
    use crate::keyboard::KeyId;
    use crate::ui::config_view::{ConfigView, CurvePreset};
    use crate::ui::keyboard_view::KeyboardView;
    use libadwaita::AlertDialog;
    use libadwaita::prelude::*;
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_gtk_ui_components() {
        if gtk4::init().is_err() {
            return;
        }

        // test configview category state
        let cv = ConfigView::new(
            "Linear",
            ResponseCurve::Linear
                .to_points()
                .map(|(x, y)| (x as f64, y as f64)),
            false,
            45.0,
            false,
            true,
            None,
        );

        // initially inactive on tab open
        assert_eq!(cv.state.borrow().active_category, None);
        for (_, b) in cv.preset_btn_list.borrow().iter() {
            assert!(!b.is_sensitive());
        }

        // set active category to left stick
        cv.set_active_category(
            Some(AnalogCategory::LeftStick),
            "Aggro",
            ResponseCurve::Aggro
                .to_points()
                .map(|(x, y)| (x as f64, y as f64)),
        );

        assert_eq!(
            cv.state.borrow().active_category,
            Some(AnalogCategory::LeftStick)
        );
        assert_eq!(cv.state.borrow().current_preset, CurvePreset::Aggro);
        for (_, b) in cv.preset_btn_list.borrow().iter() {
            assert!(b.is_sensitive());
        }
        assert!(cv.title_label.text().contains("Left Stick"));

        // reset to inactive
        cv.set_active_category(
            None,
            "Linear",
            ResponseCurve::Linear
                .to_points()
                .map(|(x, y)| (x as f64, y as f64)),
        );
        assert_eq!(cv.state.borrow().active_category, None);
        for (_, b) in cv.preset_btn_list.borrow().iter() {
            assert!(!b.is_sensitive());
        }

        // test keyboardview selected group
        let depths = Arc::new(Mutex::new([0.0f32; 70]));
        let bindings = Arc::new(Mutex::new(HashMap::new()));
        let kv = KeyboardView::new(depths, bindings, None, None);

        assert!(kv.state.borrow().selected_group.is_empty());

        let mut group = HashSet::new();
        group.insert(KeyId::W);
        group.insert(KeyId::A);
        group.insert(KeyId::S);
        group.insert(KeyId::D);

        kv.set_selected_group(group.clone());
        assert_eq!(kv.state.borrow().selected_group, group);

        // test alertdialog entry focus n enter key
        let window = gtk4::Window::new();
        window.present();

        let dialog = AlertDialog::new(Some("Title"), None);
        let entry = gtk4::Entry::new();
        entry.set_text("New Profile");
        dialog.set_extra_child(Some(&entry));
        dialog.set_focus(Some(&entry));
        entry.set_activates_default(true);
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("create", "Create");
        dialog.set_default_response(Some("create"));
        dialog.set_close_response("cancel");

        assert_eq!(dialog.default_response().as_deref(), Some("create"));
        assert!(dialog.focus().is_some());
        assert!(dialog.default_widget().is_some());

        let choose_res = std::rc::Rc::new(std::cell::RefCell::new(None));
        let choose_clone = choose_res.clone();

        let activated_called = std::rc::Rc::new(std::cell::Cell::new(false));
        let act_called_clone = activated_called.clone();
        let d_clone = dialog.clone();
        entry.connect_activate(move |_| {
            act_called_clone.set(true);
            if let Some(default_widget) = d_clone.default_widget()
                && let Ok(btn) = default_widget.downcast::<gtk4::Button>()
            {
                btn.emit_clicked();
            } else {
                d_clone.emit_by_name::<()>("response", &[&"create"]);
            }
        });

        dialog.choose(&window, None::<&gtk4::gio::Cancellable>, move |r| {
            *choose_clone.borrow_mut() = Some(r.to_string());
        });

        while glib::MainContext::default().iteration(false) {}

        entry.emit_activate();

        while glib::MainContext::default().iteration(false) {}

        assert!(activated_called.get());
        assert_eq!(choose_res.borrow().as_deref(), Some("create"));
    }
}
