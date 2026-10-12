use gtk4::cairo::{Context, LineCap, LineJoin};

use super::config_view::{ConfigViewState, CurvePreset};

pub const MAX_TRAVEL_MM: f64 = 3.50;
pub const HANDLE_RADIUS: f64 = 6.0;
pub const HANDLE_HIT_RADIUS: f64 = 14.0;

pub fn get_paddings() -> (f64, f64, f64, f64) {
    // left, right, top, bottom
    (42.0, 16.0, 16.0, 28.0)
}

// draw preset icon curves
pub fn draw_curve_preset_icon(
    cr: &Context,
    width: f64,
    height: f64,
    preset: CurvePreset,
    accent: (f64, f64, f64),
) {
    let pad_x = 4.0;
    let pad_y = 4.0;
    let pw = width - pad_x * 2.0;
    let ph = height - pad_y * 2.0;

    let map_coord = |(sx, sy): (f64, f64)| -> (f64, f64) {
        (pad_x + (sx / 500.0) * pw, pad_y + (sy / 400.0) * ph)
    };

    cr.set_line_cap(LineCap::Round);
    cr.set_line_join(LineJoin::Round);

    let draw_handles = |cr: &Context, pts: &[(f64, f64)]| {
        for pt in pts {
            cr.arc(pt.0, pt.1, 2.8, 0.0, std::f64::consts::TAU);
            cr.set_source_rgb(accent.0, accent.1, accent.2);
            cr.fill().unwrap();
            cr.arc(pt.0, pt.1, 1.2, 0.0, std::f64::consts::TAU);
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.fill().unwrap();
        }
    };

    let draw_polyline = |line_pts: &[(f64, f64)], handle_pts: &[(f64, f64)]| {
        if let Some(&first) = line_pts.first() {
            let p0 = map_coord(first);
            cr.move_to(p0.0, p0.1);
            for &pt in &line_pts[1..] {
                let p = map_coord(pt);
                cr.line_to(p.0, p.1);
            }
            cr.set_source_rgba(0.91, 0.92, 0.93, 0.85);
            cr.set_line_width(2.0);
            cr.stroke().unwrap();
        }
        let mapped: Vec<(f64, f64)> = handle_pts.iter().map(|&p| map_coord(p)).collect();
        draw_handles(cr, &mapped);
    };

    match preset {
        CurvePreset::Linear => {
            let pts = [(21.0, 383.0), (169.0, 264.0), (317.0, 146.0), (465.0, 28.0)];
            draw_polyline(&[(21.0, 383.0), (465.0, 28.0)], &pts);
        }
        CurvePreset::Aggro => {
            let pts = [(21.0, 383.0), (89.0, 214.0), (257.0, 56.0), (465.0, 28.0)];
            draw_polyline(&pts, &pts);
        }
        CurvePreset::Slow => {
            let pts = [(21.0, 383.0), (279.0, 344.0), (423.0, 266.0), (465.0, 28.0)];
            draw_polyline(&pts, &pts);
        }
        CurvePreset::Smooth => {
            let pts = [(21.0, 383.0), (172.0, 295.0), (347.0, 176.0), (465.0, 28.0)];
            draw_polyline(&pts, &pts);
        }
        CurvePreset::Steep => {
            let pts = [(21.0, 383.0), (69.0, 214.0), (397.0, 216.0), (465.0, 28.0)];
            draw_polyline(&pts, &pts);
        }
        CurvePreset::Instant => {
            let pts = [(21.0, 381.0), (46.0, 26.0), (386.0, 25.0), (465.0, 25.0)];
            draw_polyline(&[(21.0, 381.0), (46.0, 26.0), (465.0, 25.0)], &pts);
        }
        CurvePreset::Turbo => {
            let mut first = true;
            let mut x = 14.0;
            while x <= 220.0 {
                let y = 372.0 + 9.0 * ((x - 14.0) * (2.0 * std::f64::consts::PI / 51.5)).sin();
                let pt = map_coord((x, y));
                if first {
                    cr.move_to(pt.0, pt.1);
                    first = false;
                } else {
                    cr.line_to(pt.0, pt.1);
                }
                x += 2.0;
            }

            let step_bottom = map_coord((221.0, 372.0));
            let step_top = map_coord((222.0, 26.0));
            let plateau_end = map_coord((328.0, 26.0));

            cr.line_to(step_bottom.0, step_bottom.1);
            cr.line_to(step_top.0, step_top.1);
            cr.line_to(plateau_end.0, plateau_end.1);

            let mut x_top = 328.0;
            while x_top <= 492.0 {
                let y = 26.0 + 9.0 * ((x_top - 328.0) * (2.0 * std::f64::consts::PI / 41.0)).sin();
                let pt = map_coord((x_top, y));
                cr.line_to(pt.0, pt.1);
                x_top += 2.0;
            }

            cr.set_source_rgba(0.91, 0.92, 0.93, 0.85);
            cr.set_line_width(2.0);
            cr.stroke().unwrap();

            let h0 = map_coord((221.0, 366.0));
            let h1 = map_coord((222.0, 26.0));
            let h2 = map_coord((276.0, 25.0));
            let h3 = map_coord((325.0, 25.0));

            draw_handles(cr, &[h0, h1, h2, h3]);
        }
        CurvePreset::Custom => {}
    }
}

pub fn draw_curve_graph(
    cr: &Context,
    width: f64,
    height: f64,
    state: &ConfigViewState,
    accent: (f64, f64, f64),
) {
    let (padding_left, padding_right, padding_top, padding_bottom) = get_paddings();
    let plot_w = (width - padding_left - padding_right).max(10.0);
    let plot_h = (height - padding_top - padding_bottom).max(10.0);

    // canvas background
    cr.rectangle(padding_left, padding_top, plot_w, plot_h);
    cr.set_source_rgb(0.06, 0.07, 0.09);
    cr.fill().unwrap();

    // grid lines
    cr.set_line_width(1.0);
    cr.set_source_rgba(0.20, 0.22, 0.28, 0.65);

    for i in 1..4 {
        let x = padding_left + (plot_w / 4.0) * (i as f64);
        cr.move_to(x, padding_top);
        cr.line_to(x, padding_top + plot_h);
    }
    let mid_y = padding_top + plot_h / 2.0;
    cr.move_to(padding_left, mid_y);
    cr.line_to(padding_left + plot_w, mid_y);
    cr.stroke().unwrap();

    let map_pt = |(px, py): (f64, f64)| -> (f64, f64) {
        let cx = padding_left + (px / 255.0) * plot_w;
        let cy = padding_top + plot_h - (py / 255.0) * plot_h;
        (cx, cy)
    };

    let p0 = map_pt(state.points[0]);
    let p1 = map_pt(state.points[1]);
    let p2 = map_pt(state.points[2]);
    let p3 = map_pt(state.points[3]);

    let start_dz_x = p0.0;
    let end_dz_x = p3.0;
    let min_y_val = state.points[0]
        .1
        .min(state.points[1].1)
        .min(state.points[2].1)
        .min(state.points[3].1);
    let anti_dz_y = padding_top + plot_h - (min_y_val / 255.0) * plot_h;

    let is_active = state.active_category.is_some();
    let effective_accent = if is_active {
        accent
    } else {
        (0.42, 0.44, 0.48)
    };

    // fill area under curve
    cr.save().unwrap();
    cr.rectangle(padding_left, padding_top, plot_w, plot_h);
    cr.clip();

    let plot_right = padding_left + plot_w;
    cr.move_to(start_dz_x, padding_top + plot_h);
    cr.line_to(p0.0, p0.1);
    cr.line_to(p1.0, p1.1);
    cr.line_to(p2.0, p2.1);
    cr.line_to(p3.0, p3.1);
    cr.line_to(end_dz_x, padding_top + plot_h);
    cr.close_path();

    if is_active {
        cr.set_source_rgba(accent.0, accent.1, accent.2, 0.22);
    } else {
        cr.set_source_rgba(0.40, 0.42, 0.46, 0.10);
    }
    cr.fill().unwrap();

    // curve top stroke
    cr.set_line_cap(LineCap::Round);
    cr.set_line_join(LineJoin::Round);

    cr.move_to(p0.0, p0.1);
    cr.line_to(p1.0, p1.1);
    cr.line_to(p2.0, p2.1);
    cr.line_to(p3.0, p3.1);

    cr.set_source_rgb(effective_accent.0, effective_accent.1, effective_accent.2);
    cr.set_line_width(if is_active { 3.0 } else { 2.2 });
    cr.stroke().unwrap();
    cr.restore().unwrap();

    // deadzone hatched overlays
    // start deadzone bar
    if start_dz_x > padding_left + 1.0 {
        let dz_w = start_dz_x - padding_left;
        draw_deadzone_overlay(
            cr,
            padding_left,
            padding_top,
            dz_w,
            plot_h,
            "Key Start Deadzone",
            true,
        );
    }

    // end deadzone bar
    if end_dz_x < plot_right - 1.0 {
        let edz_w = plot_right - end_dz_x;
        draw_deadzone_overlay(
            cr,
            end_dz_x,
            padding_top,
            edz_w,
            plot_h,
            "Key End Deadzone",
            true,
        );
    }

    // anti deadzone bar at bottom
    if anti_dz_y < padding_top + plot_h - 1.0 {
        let adz_h = padding_top + plot_h - anti_dz_y;
        let adz_w = (end_dz_x - start_dz_x).max(0.0);
        if adz_w > 2.0 {
            draw_deadzone_overlay(
                cr,
                start_dz_x,
                anti_dz_y,
                adz_w,
                adz_h,
                "Anti-Deadzone",
                false,
            );
        }
    }

    // live switch depth beam
    if is_active && state.live_depth_mm > 0.02 {
        let cur_norm = (state.live_depth_mm / MAX_TRAVEL_MM).clamp(0.0, 1.0);
        let cur_x = padding_left + cur_norm * plot_w;

        let cur_val_255 = cur_norm * 255.0;
        let pts_f32 = state.points.map(|(x, y)| (x as f32, y as f32));
        let cur_y_val =
            crate::gamepad::engine::interpolate_piecewise(&pts_f32, cur_val_255 as f32) as f64;
        let cur_y = padding_top + plot_h - (cur_y_val / 255.0) * plot_h;

        cr.move_to(cur_x, padding_top);
        cr.line_to(cur_x, padding_top + plot_h);
        cr.set_source_rgba(0.13, 0.83, 0.93, 0.85); // cyan
        cr.set_line_width(1.8);
        cr.stroke().unwrap();

        cr.arc(cur_x, cur_y, 4.5, 0.0, std::f64::consts::TAU);
        cr.set_source_rgb(0.13, 0.83, 0.93);
        cr.fill().unwrap();
    }

    // outer border
    cr.rectangle(padding_left, padding_top, plot_w, plot_h);
    cr.set_source_rgba(0.25, 0.27, 0.32, 0.85);
    cr.set_line_width(1.0);
    cr.stroke().unwrap();

    // draggable handles
    let handle_points = [p0, p1, p2, p3];
    for (i, &(hx, hy)) in handle_points.iter().enumerate() {
        let is_hovered = state.hovered_handle == Some(i);
        let is_dragging = state.dragging_handle == Some(i);

        let radius = if is_dragging {
            7.5
        } else if is_hovered {
            6.8
        } else {
            HANDLE_RADIUS
        };

        if is_active && (is_hovered || is_dragging) {
            cr.arc(hx, hy, radius + 4.5, 0.0, std::f64::consts::TAU);
            cr.set_source_rgba(accent.0, accent.1, accent.2, 0.35);
            cr.fill().unwrap();
        }

        // handle body
        cr.arc(hx, hy, radius, 0.0, std::f64::consts::TAU);
        cr.set_source_rgb(effective_accent.0, effective_accent.1, effective_accent.2);
        cr.fill().unwrap();

        // inner circle
        cr.arc(hx, hy, radius * 0.42, 0.0, std::f64::consts::TAU);
        if is_active {
            cr.set_source_rgb(1.0, 1.0, 1.0);
        } else {
            cr.set_source_rgb(0.60, 0.62, 0.66);
        }
        cr.fill().unwrap();

        // dark outline
        cr.arc(hx, hy, radius, 0.0, std::f64::consts::TAU);
        cr.set_source_rgba(0.1, 0.1, 0.1, 0.8);
        cr.set_line_width(1.0);
        cr.stroke().unwrap();
    }

    // axis labels
    cr.set_source_rgb(0.58, 0.63, 0.69); // label color
    cr.set_font_size(11.0);

    // 255 top left
    cr.move_to(padding_left - 28.0, padding_top + 9.0);
    cr.show_text("255").unwrap();

    // analog value y axis label
    cr.save().unwrap();
    cr.translate(padding_left - 24.0, padding_top + plot_h / 2.0);
    cr.rotate(-std::f64::consts::FRAC_PI_2);
    let ext_val = cr.text_extents("Analog Value").unwrap();
    cr.move_to(-ext_val.width() / 2.0, 0.0);
    cr.show_text("Analog Value").unwrap();
    cr.restore().unwrap();

    // 0 bottom left
    cr.move_to(padding_left - 16.0, padding_top + plot_h);
    cr.show_text("0").unwrap();

    // 0.10mm bottom left
    cr.move_to(padding_left, padding_top + plot_h + 18.0);
    cr.show_text("0.10mm").unwrap();

    // depth axis label
    let depth_text = "How Deep a Key is Pressed";
    let ext_d = cr.text_extents(depth_text).unwrap();
    cr.move_to(
        padding_left + (plot_w - ext_d.width()) / 2.0,
        padding_top + plot_h + 18.0,
    );
    cr.show_text(depth_text).unwrap();

    // 3.50mm bottom right
    let max_mm_text = format!("{:.2}mm", MAX_TRAVEL_MM);
    let ext_m = cr.text_extents(&max_mm_text).unwrap();
    cr.move_to(
        padding_left + plot_w - ext_m.width(),
        padding_top + plot_h + 18.0,
    );
    cr.show_text(&max_mm_text).unwrap();
}

// hatched diagonal deadzone bar
fn draw_deadzone_overlay(
    cr: &Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    label: &str,
    is_vertical: bool,
) {
    if w <= 1.0 || h <= 1.0 {
        return;
    }

    cr.save().unwrap();
    cr.rectangle(x, y, w, h);
    cr.clip();

    // white base background
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.rectangle(x, y, w, h);
    cr.fill().unwrap();

    // 45 deg diagonal gray stripes
    cr.set_source_rgb(0.867, 0.867, 0.867);
    cr.set_line_width(5.5);

    let step = 14.0;
    let total_span = w + h;
    let mut offset = -h;
    while offset < total_span {
        cr.move_to(x + offset, y + h);
        cr.line_to(x + offset + h, y);
        offset += step;
    }
    cr.stroke().unwrap();

    // boundary border
    cr.rectangle(x, y, w, h);
    cr.set_source_rgba(0.70, 0.74, 0.80, 0.90);
    cr.set_line_width(1.0);
    cr.stroke().unwrap();

    // label text
    cr.set_source_rgb(0.58, 0.63, 0.69);

    if is_vertical {
        // vertical label for start n end deadzones
        if w >= 18.0 && h >= 36.0 {
            let font_size = if w >= 65.0 {
                12.5
            } else if w >= 45.0 {
                11.0
            } else if w >= 30.0 {
                9.5
            } else {
                8.5
            };
            cr.set_font_size(font_size);

            let is_start = label.contains("Start");
            let lines: Vec<&str> = if w >= 60.0 {
                if is_start {
                    vec!["Key Start", "Deadzone"]
                } else {
                    vec!["Key End", "Deadzone"]
                }
            } else if w >= 36.0 {
                if is_start {
                    vec!["Key", "Start", "Deadzone"]
                } else {
                    vec!["Key", "End", "Deadzone"]
                }
            } else {
                if is_start {
                    vec!["Key", "Start", "DZ"]
                } else {
                    vec!["Key", "End", "DZ"]
                }
            };

            let line_height = font_size * 1.25;
            let start_y = y + 16.0;

            for (idx, line) in lines.iter().enumerate() {
                let ext = cr.text_extents(line).unwrap();
                let tx = x + (w - ext.width()) / 2.0;
                let ty = start_y + (idx as f64) * line_height;
                if ty + font_size <= y + h {
                    cr.move_to(tx, ty);
                    cr.show_text(line).unwrap();
                }
            }
        }
    } else {
        // horizontal centered label for anti deadzone
        if w >= 36.0 && h >= 8.0 {
            let font_size = if h < 18.0 { 10.0 } else { 12.0 };
            cr.set_font_size(font_size);
            let ext = cr.text_extents(label).unwrap();
            if ext.width() + 4.0 <= w {
                let tx = x + (w - ext.width()) / 2.0;
                let ty = y + (h + ext.height()) / 2.0;
                cr.move_to(tx, ty);
                cr.show_text(label).unwrap();
            }
        }
    }

    cr.restore().unwrap();
}

#[allow(dead_code)]
pub fn compute_deadzones(points: &[(f64, f64); 4]) -> (f64, f64, f64) {
    let start_dz_mm = (points[0].0 / 255.0) * MAX_TRAVEL_MM;
    let anti_dz_val = points[0]
        .1
        .min(points[1].1)
        .min(points[2].1)
        .min(points[3].1);
    let end_dz_mm = ((255.0 - points[3].0) / 255.0) * MAX_TRAVEL_MM;
    (start_dz_mm, anti_dz_val, end_dz_mm)
}

pub fn clamp_handle_movement(
    points: &mut [(f64, f64); 4],
    handle_idx: usize,
    target_x: f64,
    target_y: f64,
) {
    let target_x = target_x.clamp(0.0, 255.0);
    let target_y = target_y.clamp(0.0, 255.0);

    match handle_idx {
        0 => {
            let max_x = points[1].0;
            points[0].0 = target_x.min(max_x).max(0.0);
            points[0].1 = target_y;
        }
        1 => {
            let min_x = points[0].0;
            let max_x = points[2].0;
            points[1].0 = target_x.clamp(min_x, max_x);
            points[1].1 = target_y;
        }
        2 => {
            let min_x = points[1].0;
            let max_x = points[3].0;
            points[2].0 = target_x.clamp(min_x, max_x);
            points[2].1 = target_y;
        }
        3 => {
            let min_x = points[2].0;
            points[3].0 = target_x.clamp(min_x, 255.0);
            points[3].1 = target_y;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamepad::ResponseCurve;

    #[test]
    fn test_compute_deadzones_linear() {
        let linear_pts = ResponseCurve::Linear
            .to_points()
            .map(|(x, y)| (x as f64, y as f64));
        let (start_dz, anti_dz, end_dz) = compute_deadzones(&linear_pts);
        assert_eq!(start_dz, 0.0);
        assert_eq!(anti_dz, 20.0);
        assert!((end_dz - (2.0 / 255.0 * MAX_TRAVEL_MM)).abs() < 1e-4);
    }

    #[test]
    fn test_compute_deadzones_pushed_down_by_intermediate_points() {
        let pts = [(10.0, 50.0), (80.0, 15.0), (180.0, 100.0), (250.0, 255.0)];
        let (start_dz, anti_dz, end_dz) = compute_deadzones(&pts);
        assert!((start_dz - (10.0 / 255.0 * MAX_TRAVEL_MM)).abs() < 1e-4);
        assert_eq!(anti_dz, 15.0);
        assert!((end_dz - (5.0 / 255.0 * MAX_TRAVEL_MM)).abs() < 1e-4);
    }

    #[test]
    fn test_clamp_handle_movement_ordering() {
        let mut pts = [(0.0, 20.0), (60.0, 60.0), (194.0, 195.0), (253.0, 255.0)];

        clamp_handle_movement(&mut pts, 0, 100.0, 50.0);
        assert_eq!(pts[0].0, 60.0);
        assert_eq!(pts[0].1, 50.0);

        clamp_handle_movement(&mut pts, 1, 20.0, 80.0);
        assert_eq!(pts[1].0, 60.0);
        assert_eq!(pts[1].1, 80.0);

        clamp_handle_movement(&mut pts, 3, 100.0, 200.0);
        assert_eq!(pts[3].0, 194.0);
        assert_eq!(pts[3].1, 200.0);
    }
}
