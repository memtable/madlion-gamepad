use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gtk4::gdk::Display;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, CssProvider, Orientation, ScrolledWindow};
use libadwaita::prelude::*;
use libadwaita::{
    Application, ApplicationWindow, Banner, HeaderBar, ToolbarView, ViewStack, ViewSwitcher,
    ViewSwitcherPolicy,
};

use crate::config::AppConfig;
use crate::gamepad::{GamepadButton, spawn_worker};
use crate::ipc::IpcServer;
use crate::keyboard::KeyId;
use crate::ui::config_view::{ConfigView, ConfigViewState};
use crate::ui::keyboard_view::KeyboardView;
use crate::ui::profile_view::{ProfileAction, ProfileView};
use crate::ui::setup_view::{SetupAction, SetupView};
use crate::ui::tester_view::TesterView;

pub fn run_gui() {
    let app = Application::builder()
        .application_id("io.madlion.Gamepad")
        .build();

    app.connect_startup(|app| {
        load_custom_css();

        if let Some(display) = Display::default() {
            let icon_theme = gtk4::IconTheme::for_display(&display);
            icon_theme.add_search_path("resources/icons");
            icon_theme.add_search_path("resources");
        }

        let app_sigint = app.clone();
        glib::unix_signal_add_local(libc::SIGINT, move || {
            app_sigint.quit();
            glib::ControlFlow::Break
        });

        let app_sigterm = app.clone();
        glib::unix_signal_add_local(libc::SIGTERM, move || {
            app_sigterm.quit();
            glib::ControlFlow::Break
        });
    });

    app.connect_activate(build_ui);
    app.run();
}

fn load_custom_css() {
    let provider = CssProvider::new();
    provider.load_from_string(
        "
        .keyboard-frame-container {
            background-color: var(--card-bg-color);
            border-bottom: 1px solid var(--border-color);
            padding: 10px 16px 14px 16px;
        }

        .controller-badge-btn {
            min-width: 38px;
            min-height: 38px;
            max-width: 38px;
            max-height: 38px;
            border-radius: 6px;
            padding: 0;
            transition: all 120ms ease;
        }
        .controller-badge-btn image {
            margin: 0;
            padding: 0;
        }

        .badge-btn-clear {
            color: var(--destructive-color);
            font-weight: 700;
            font-size: 10px;
        }

        .badge-active-selected {
            outline: 2px solid var(--accent-bg-color);
            outline-offset: -2px;
        }

        .preset-pill-btn {
            font-weight: 600;
            font-size: 11px;
            min-height: 26px;
            padding: 2px 8px;
            border-radius: 12px;
            transition: all 120ms ease;
        }
        .preset-pill-btn.active {
            background-color: var(--accent-bg-color);
            color: var(--accent-fg-color);
            font-weight: 700;
        }

        .gamepad-card {
            border-radius: 8px;
            padding: 20px 22px;
        }
        .card-header-title {
            font-size: 16px;
            font-weight: 600;
        }
        .card-section-title {
            font-size: 13px;
            font-weight: 500;
            margin-top: 14px;
            margin-bottom: 8px;
        }
        .preset-tile-btn {
            min-width: 72px;
            min-height: 68px;
            border-radius: 6px;
            padding: 4px 2px;
            transition: all 120ms ease;
        }
        .preset-tile-btn.active {
            outline: 2px solid var(--accent-bg-color);
            outline-offset: -2px;
            background-color: color-mix(in srgb, var(--accent-bg-color) 20%, transparent);
        }
        .preset-tile-label {
            font-size: 11px;
            font-weight: 500;
            margin-top: 2px;
        }
        ",
    );

    if let Some(display) = Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("MADLIONS Gamepad")
        .default_width(960)
        .default_height(740)
        .icon_name("io.madlion.Gamepad")
        .build();

    // load saved config from xdg
    let config = Rc::new(RefCell::new(AppConfig::load()));
    let cfg = config.borrow().clone();

    // spawn background worker thread
    let initial_profile = cfg.to_gamepad_profile();
    let worker = Rc::new(spawn_worker(
        initial_profile,
        cfg.gamepad_enabled,
        cfg.polling_rate_hz,
        cfg.mapping_override,
        cfg.enable_typing,
        cfg.toggle_override_key,
    ));
    worker.set_adaptive_query_mode(!cfg.poll_only_mapped_keys);

    // shared state for key rendering
    let key_depths = Arc::new(Mutex::new([0.0f32; 70]));
    let key_bindings = Arc::new(Mutex::new(cfg.key_bindings.clone()));

    // profile sync helper
    let sync_config_to_worker = {
        let config = config.clone();
        let worker = worker.clone();
        Rc::new(move || {
            let (
                prof,
                gamepad_enabled,
                polling_rate_hz,
                mapping_override,
                enable_typing,
                poll_only,
            ) = {
                let cfg = config.borrow();
                let active = cfg.active();
                (
                    cfg.to_gamepad_profile(),
                    active.gamepad_enabled,
                    active.polling_rate_hz,
                    active.mapping_override,
                    active.enable_typing,
                    active.poll_only_mapped_keys,
                )
            };
            worker.set_gamepad_enabled(gamepad_enabled);
            worker.set_polling_rate_hz(polling_rate_hz);
            worker.set_mapping_override(mapping_override);
            worker.set_enable_typing(enable_typing);
            worker.set_adaptive_query_mode(!poll_only);
            worker.update_profile(prof);
            let _ = config.borrow().save();
        })
    };

    // header bar n tab switcher
    let header = HeaderBar::new();

    // view stack
    let view_stack = ViewStack::new();
    view_stack.set_vexpand(true);

    let view_switcher = ViewSwitcher::builder()
        .stack(&view_stack)
        .policy(ViewSwitcherPolicy::Wide)
        .build();
    header.set_title_widget(Some(&view_switcher));

    // banner for disconnected hardware status
    let banner = Banner::new("No Madlion device detected (Scanning...)");
    banner.set_revealed(true);

    // root toolbar view
    let toolbar_view = ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.add_top_bar(&banner);

    // top keyboard canvas
    let kb_container = GtkBox::new(Orientation::Vertical, 0);
    kb_container.add_css_class("keyboard-frame-container");

    let on_binding_changed = {
        let config = config.clone();
        let bindings_ref = key_bindings.clone();
        let sync = sync_config_to_worker.clone();
        Rc::new(move || {
            let b = bindings_ref.lock().unwrap().clone();
            config.borrow_mut().active_mut().key_bindings = b;
            sync();
        })
    };

    let config_view_slot = Rc::new(RefCell::new(None::<Rc<ConfigView>>));
    let view_stack_slot = Rc::new(RefCell::new(None::<ViewStack>));
    let keyboard_view_slot = Rc::new(RefCell::new(None::<Rc<KeyboardView>>));

    let on_key_clicked = {
        let config = config.clone();
        let keyboard_view_slot = keyboard_view_slot.clone();
        let config_view_slot = config_view_slot.clone();
        let view_stack_slot = view_stack_slot.clone();
        Rc::new(move |key: KeyId| {
            if let Some(vs) = view_stack_slot.borrow().as_ref()
                && vs.visible_child_name().as_deref() != Some("config")
            {
                return;
            }

            let c = config.borrow();
            let prof = c.active();
            if let Some(category) = prof.category_for_key(key) {
                let curve = prof.get_curve(category);
                let keys = prof.keys_for_category(category);
                drop(c);

                if let Some(kv) = keyboard_view_slot.borrow().as_ref() {
                    kv.set_selected_group(keys.into_iter().collect());
                }

                if let Some(cv) = config_view_slot.borrow().as_ref() {
                    cv.set_active_category(Some(category), &curve.preset, curve.points);
                }
            }
        })
    };

    let keyboard_view = Rc::new(KeyboardView::new(
        key_depths.clone(),
        key_bindings.clone(),
        Some(on_binding_changed),
        Some(on_key_clicked),
    ));
    *keyboard_view_slot.borrow_mut() = Some(keyboard_view.clone());
    kb_container.append(&keyboard_view.drawing_area);

    // setup n remap tab
    let on_setup_action = {
        let config = config.clone();
        let worker = worker.clone();
        Rc::new(move |action: SetupAction| {
            match action {
                SetupAction::GamepadEnabled(enabled) => {
                    config.borrow_mut().active_mut().gamepad_enabled = enabled;
                    worker.set_gamepad_enabled(enabled);
                }
                SetupAction::PollingRate(hz) => {
                    config.borrow_mut().active_mut().polling_rate_hz = hz;
                    worker.set_polling_rate_hz(hz);
                }
                SetupAction::ControllerType(t) => {
                    config.borrow_mut().active_mut().controller_type = t;
                }
                SetupAction::EnableTyping(en) => {
                    config.borrow_mut().active_mut().enable_typing = en;
                    worker.set_enable_typing(en);
                }
                SetupAction::MappingOverride(ov) => {
                    config.borrow_mut().active_mut().mapping_override = ov;
                    worker.set_mapping_override(ov);
                }
                SetupAction::PollOnlyMappedKeys(poll_only) => {
                    config.borrow_mut().active_mut().poll_only_mapped_keys = poll_only;
                    worker.set_adaptive_query_mode(!poll_only);
                }
            }
            let prof = config.borrow().to_gamepad_profile();
            worker.update_profile(prof);
            let _ = config.borrow().save();
        })
    };

    let setup_view = Rc::new(SetupView::new(
        cfg.gamepad_enabled,
        cfg.polling_rate_hz,
        &cfg.controller_type,
        cfg.enable_typing,
        cfg.mapping_override,
        cfg.poll_only_mapped_keys,
        Some(on_setup_action),
    ));

    let (ipc_req_tx, ipc_req_rx) =
        std::sync::mpsc::channel::<(String, std::sync::mpsc::Sender<String>)>();

    let ipc_server = IpcServer::start(move |cmd| {
        let (resp_tx, resp_rx) = std::sync::mpsc::channel();
        if ipc_req_tx.send((cmd.to_string(), resp_tx)).is_ok() {
            resp_rx
                .recv_timeout(Duration::from_millis(500))
                .unwrap_or_else(|_| "error: timeout waiting for GUI response".to_string())
        } else {
            "error: GUI main thread disconnected".to_string()
        }
    })
    .ok();

    // curves config tab
    let on_config_change = {
        let config = config.clone();
        let sync = sync_config_to_worker.clone();
        Rc::new(move |st: &ConfigViewState| {
            {
                let mut c = config.borrow_mut();
                let prof = c.active_mut();
                if let Some(cat) = st.active_category {
                    prof.set_curve(
                        cat,
                        crate::config::CurveConfig::new(st.current_preset.to_str_name(), st.points),
                    );
                }
                prof.enable_angle_adj = st.enable_angle_adj;
                prof.angle_adjustment = st.angle_adjustment;
                prof.square_joystick = st.square_joystick;
                prof.snappy_joystick = st.snappy_joystick;
            }
            sync();
        })
    };

    let config_view = Rc::new(ConfigView::new(
        &cfg.curve_preset,
        cfg.curve_points,
        cfg.enable_angle_adj,
        cfg.angle_adjustment,
        cfg.square_joystick,
        cfg.snappy_joystick,
        Some(on_config_change),
    ));
    *config_view_slot.borrow_mut() = Some(config_view.clone());

    // profile switch n ui sync logic
    let profile_view_slot = Rc::new(RefCell::new(None::<Rc<ProfileView>>));

    let apply_profile_switch = {
        let config = config.clone();
        let key_bindings = key_bindings.clone();
        let keyboard_view = keyboard_view.clone();
        let setup_view = setup_view.clone();
        let config_view = config_view.clone();
        let worker = worker.clone();
        let profile_view_slot = profile_view_slot.clone();
        Rc::new(move |profile_name: &str| {
            let switched = config.borrow_mut().switch_profile(profile_name);
            if !switched {
                return;
            }
            let active_cfg = config.borrow().active().clone();

            // update bindings n redraw board
            {
                let mut b = key_bindings.lock().unwrap();
                *b = active_cfg.key_bindings.clone();
            }
            keyboard_view.drawing_area.queue_draw();

            // update setup tab widgets
            setup_view.set_state(
                active_cfg.gamepad_enabled,
                active_cfg.polling_rate_hz,
                &active_cfg.controller_type,
                active_cfg.enable_typing,
                active_cfg.mapping_override,
                active_cfg.poll_only_mapped_keys,
            );

            // update curves tab widgets
            config_view.set_state(
                &active_cfg.curve_preset,
                active_cfg.curve_points,
                active_cfg.enable_angle_adj,
                active_cfg.angle_adjustment,
                active_cfg.square_joystick,
                active_cfg.snappy_joystick,
            );
            if let Some(cat) = config_view.state.borrow().active_category {
                let curve = active_cfg.get_curve(cat);
                config_view.set_active_category(Some(cat), &curve.preset, curve.points);
            }

            // push profile to worker
            worker.set_gamepad_enabled(active_cfg.gamepad_enabled);
            worker.set_polling_rate_hz(active_cfg.polling_rate_hz);
            worker.set_mapping_override(active_cfg.mapping_override);
            worker.set_enable_typing(active_cfg.enable_typing);
            worker.set_adaptive_query_mode(!active_cfg.poll_only_mapped_keys);
            let prof = config.borrow().to_gamepad_profile();
            worker.update_profile(prof);

            // write config to disk
            let _ = config.borrow().save();

            // refresh profile list
            if let Some(pv) = profile_view_slot.borrow().as_ref() {
                pv.refresh(&config.borrow());
            }
        })
    };

    // profiles tab
    let on_profile_action = {
        let config = config.clone();
        let apply_switch = apply_profile_switch.clone();
        let profile_view_slot = profile_view_slot.clone();
        Rc::new(move |action: ProfileAction| match action {
            ProfileAction::Activate(name) => {
                apply_switch(&name);
            }
            ProfileAction::Create { name } => {
                let mut base_profile = config.borrow().active().clone();
                let base_name = if name.trim().is_empty() {
                    "New Profile".to_string()
                } else {
                    name.trim().to_string()
                };
                let mut unique_name = base_name.clone();
                let mut counter = 2;
                while config
                    .borrow()
                    .profiles
                    .iter()
                    .any(|p| p.name == unique_name)
                {
                    unique_name = format!("{} {}", base_name, counter);
                    counter += 1;
                }
                base_profile.name = unique_name.clone();
                config.borrow_mut().add_profile(base_profile);
                config.borrow_mut().switch_profile(&unique_name);
                if let Some(pv) = profile_view_slot.borrow().as_ref() {
                    pv.refresh(&config.borrow());
                }
                apply_switch(&unique_name);
            }
            ProfileAction::Duplicate(name) => {
                let base_name = format!("{} (Copy)", name);
                let mut unique_name = base_name.clone();
                let mut counter = 2;
                while config
                    .borrow()
                    .profiles
                    .iter()
                    .any(|p| p.name == unique_name)
                {
                    unique_name = format!("{} {}", base_name, counter);
                    counter += 1;
                }
                config
                    .borrow_mut()
                    .duplicate_profile(&name, unique_name.clone());
                config.borrow_mut().switch_profile(&unique_name);
                if let Some(pv) = profile_view_slot.borrow().as_ref() {
                    pv.refresh(&config.borrow());
                }
                apply_switch(&unique_name);
            }
            ProfileAction::Rename { old_name, new_name } => {
                let trimmed = new_name.trim().to_string();
                if !trimmed.is_empty() && trimmed != old_name {
                    let ok = config.borrow_mut().rename_profile(&old_name, trimmed);
                    if ok {
                        if let Some(pv) = profile_view_slot.borrow().as_ref() {
                            pv.refresh(&config.borrow());
                        }
                        let _ = config.borrow().save();
                    }
                }
            }
            ProfileAction::Delete(name) => {
                let (success, next_active) = {
                    let mut cfg = config.borrow_mut();
                    if cfg.profiles.len() <= 1 {
                        (false, String::new())
                    } else {
                        let deleted = cfg.delete_profile(&name);
                        (deleted, cfg.active_profile.clone())
                    }
                };
                if success {
                    if let Some(pv) = profile_view_slot.borrow().as_ref() {
                        pv.refresh(&config.borrow());
                    }
                    apply_switch(&next_active);
                }
            }
        })
    };

    let profile_view = Rc::new(ProfileView::new(&cfg, Some(on_profile_action)));
    *profile_view_slot.borrow_mut() = Some(profile_view.clone());

    // tester tab
    let tester_view = Rc::new(TesterView::new());

    let setup_scroll = ScrolledWindow::new();
    setup_scroll.set_child(Some(&setup_view.container));

    let config_scroll = ScrolledWindow::new();
    config_scroll.set_child(Some(&config_view.container));

    let profile_scroll = ScrolledWindow::new();
    profile_scroll.set_child(Some(&profile_view.container));

    let tester_scroll = ScrolledWindow::new();
    tester_scroll.set_child(Some(&tester_view.container));

    let p1 = view_stack.add_titled(&setup_scroll, Some("setup"), "Setup & Remap");
    p1.set_icon_name(Some("input-keyboard-symbolic"));

    let p2 = view_stack.add_titled(&config_scroll, Some("config"), "Configuration");
    p2.set_icon_name(Some("preferences-other-symbolic"));

    let p3 = view_stack.add_titled(&profile_scroll, Some("profiles"), "Profiles");
    p3.set_icon_name(Some("document-properties-symbolic"));

    let p4 = view_stack.add_titled(&tester_scroll, Some("tester"), "Gamepad Tester");
    p4.set_icon_name(Some("input-gaming-symbolic"));

    *view_stack_slot.borrow_mut() = Some(view_stack.clone());

    let kb_tab_switch = keyboard_view.clone();
    let cf_tab_switch = config_view.clone();
    view_stack.connect_visible_child_name_notify(move |vs| {
        if vs.visible_child_name().as_deref() == Some("config") {
            // keep curve grayed out until an analog key is selected
            cf_tab_switch.set_active_category(
                None,
                "Linear",
                crate::gamepad::ResponseCurve::Linear
                    .to_points()
                    .map(|(x, y)| (x as f64, y as f64)),
            );
            kb_tab_switch.set_selected_group(std::collections::HashSet::new());
        } else {
            kb_tab_switch.set_selected_group(std::collections::HashSet::new());
        }
    });

    let main_content = GtkBox::new(Orientation::Vertical, 0);
    main_content.append(&kb_container);
    main_content.append(&view_stack);

    toolbar_view.set_content(Some(&main_content));
    window.set_content(Some(&toolbar_view));

    let worker_for_close = worker.clone();
    let ipc_server_slot = Rc::new(RefCell::new(ipc_server));
    let ipc_for_close = ipc_server_slot.clone();
    window.connect_close_request(move |_| {
        if let Some(ref mut srv) = *ipc_for_close.borrow_mut() {
            srv.stop();
        }
        worker_for_close.shutdown();
        glib::Propagation::Proceed
    });

    window.present();

    // 60 fps timer syncing telemetry to ui
    let kb_v = keyboard_view;
    let ts_v = tester_view;
    let cf_v = config_view;
    let depths_lock = key_depths;
    let worker_ref = worker;
    let banner_ref = banner;
    let config_timer = config;
    let setup_view_timer = setup_view;

    glib::timeout_add_local(Duration::from_millis(16), move || {
        // handle incoming ipc requests on main thread
        while let Ok((cmd, resp_tx)) = ipc_req_rx.try_recv() {
            match cmd.as_str() {
                "toggle-override" => {
                    let new_val = {
                        let mut c = config_timer.borrow_mut();
                        let prof = c.active_mut();
                        prof.mapping_override = !prof.mapping_override;
                        prof.mapping_override
                    };
                    worker_ref.set_mapping_override(new_val);
                    let _ = config_timer.borrow().save();
                    setup_view_timer.mapping_override_switch.set_active(new_val);
                    let _ = resp_tx.send(format!("ok: mapping_override = {}", new_val));
                }
                "toggle-gamepad" => {
                    let new_val = {
                        let mut c = config_timer.borrow_mut();
                        let prof = c.active_mut();
                        prof.gamepad_enabled = !prof.gamepad_enabled;
                        prof.gamepad_enabled
                    };
                    worker_ref.set_gamepad_enabled(new_val);
                    let _ = config_timer.borrow().save();
                    setup_view_timer.enable_gamepad_switch.set_active(new_val);
                    let _ = resp_tx.send(format!("ok: gamepad_enabled = {}", new_val));
                }
                "status" => {
                    let c = config_timer.borrow();
                    let active = c.active();
                    let _ = resp_tx.send(format!(
                        "profile: {}, gamepad_enabled: {}, mapping_override: {}, typing_enabled: {}",
                        active.name, active.gamepad_enabled, active.mapping_override, active.enable_typing
                    ));
                }
                _ => {
                    let _ = resp_tx.send(format!("error: unknown command '{}'", cmd));
                }
            }
        }

        if let Ok(tel) = worker_ref.telemetry.try_lock() {
            // update key depths on board
            {
                let mut d = depths_lock.lock().unwrap();
                *d = tel.key_depths;
            }
            kb_v.drawing_area.queue_draw();

            // update curve cursor depth
            let max_depth = tel.key_depths.iter().copied().fold(0.0f32, f32::max) as f64 * 3.50;
            cf_v.state.borrow_mut().live_depth_mm = max_depth;
            cf_v.graph_area.queue_draw();

            // update tester scopes n meters
            {
                let mut tv = ts_v.state.borrow_mut();
                tv.left_stick_x = tel.gamepad_state.left_stick_x as f32 / 32767.0;
                tv.left_stick_y = tel.gamepad_state.left_stick_y as f32 / 32767.0;
                tv.right_stick_x = tel.gamepad_state.right_stick_x as f32 / 32767.0;
                tv.right_stick_y = tel.gamepad_state.right_stick_y as f32 / 32767.0;
                tv.left_trigger = (tel.gamepad_state.left_trigger as f32 / 255.0).clamp(0.0, 1.0);
                tv.right_trigger = (tel.gamepad_state.right_trigger as f32 / 255.0).clamp(0.0, 1.0);

                tv.btn_a = tel
                    .gamepad_state
                    .buttons
                    .get(&GamepadButton::A)
                    .copied()
                    .unwrap_or(false);
                tv.btn_b = tel
                    .gamepad_state
                    .buttons
                    .get(&GamepadButton::B)
                    .copied()
                    .unwrap_or(false);
                tv.btn_x = tel
                    .gamepad_state
                    .buttons
                    .get(&GamepadButton::X)
                    .copied()
                    .unwrap_or(false);
                tv.btn_y = tel
                    .gamepad_state
                    .buttons
                    .get(&GamepadButton::Y)
                    .copied()
                    .unwrap_or(false);
                tv.btn_lb = tel
                    .gamepad_state
                    .buttons
                    .get(&GamepadButton::LeftBumper)
                    .copied()
                    .unwrap_or(false);
                tv.btn_rb = tel
                    .gamepad_state
                    .buttons
                    .get(&GamepadButton::RightBumper)
                    .copied()
                    .unwrap_or(false);
                tv.btn_l3 = tel
                    .gamepad_state
                    .buttons
                    .get(&GamepadButton::LeftThumb)
                    .copied()
                    .unwrap_or(false);
                tv.btn_r3 = tel
                    .gamepad_state
                    .buttons
                    .get(&GamepadButton::RightThumb)
                    .copied()
                    .unwrap_or(false);

                tv.poll_hz = tel.poll_hz;
                tv.latency_us = tel.latency_us;
            }
            ts_v.update();

            // update hardware banner
            if tel.device_connected {
                banner_ref.set_revealed(false);
            } else {
                banner_ref.set_title("No Madlion keyboard detected (Scanning...)");
                banner_ref.set_revealed(true);
            }
        }

        glib::ControlFlow::Continue
    });
}
