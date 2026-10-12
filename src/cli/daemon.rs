use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::{RunArgs, open_keyboard, parse_key_list};
use crate::gamepad::{GamepadEngine, ResponseCurve, StickBinding, TriggerBinding, UinputGamepad};
use crate::keyboard::{
    ColorRGB, KeyId, KeyboardInterceptor, find_all_keyboard_event_devices, plan_queries,
};

pub fn cmd_run(args: RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    let mut keyboard = open_keyboard(args.device.as_ref())?;
    println!("Opened Madlion keyboard at: {}", keyboard.path().display());

    let mut gamepad = UinputGamepad::new()?;
    println!("Created Linux virtual gamepad device (/dev/uinput)");

    let config = crate::config::AppConfig::load();
    let profile_config = if let Some(ref name) = args.profile {
        config
            .get_profile(name)
            .ok_or_else(|| format!("Profile '{}' not found in configuration", name))?
    } else {
        config.active()
    };

    println!("Using profile: {}", profile_config.name);
    let mut profile = profile_config.to_gamepad_profile();

    if let Some(ref ls_str) = args.left_stick {
        let ls_keys = parse_key_list(ls_str)?;
        if ls_keys.len() != 4 {
            return Err("Left stick requires exactly 4 keys: Up,Down,Left,Right".into());
        }
        if let Some(ref mut ls) = profile.left_stick {
            ls.up = Some(ls_keys[0]);
            ls.down = Some(ls_keys[1]);
            ls.left = Some(ls_keys[2]);
            ls.right = Some(ls_keys[3]);
        } else {
            profile.left_stick = Some(StickBinding {
                up: Some(ls_keys[0]),
                down: Some(ls_keys[1]),
                left: Some(ls_keys[2]),
                right: Some(ls_keys[3]),
                deadzone: 0.05,
                curve: ResponseCurve::Linear,
                square_joystick: false,
                snappy_joystick: true,
                angle_adjustment_deg: None,
            });
        }
    }

    if let Some(ref lt_str) = args.left_trigger {
        let lt_key = lt_str.parse::<KeyId>()?;
        if let Some(ref mut lt) = profile.left_trigger {
            lt.key = lt_key;
        } else {
            profile.left_trigger = Some(TriggerBinding {
                key: lt_key,
                deadzone: 0.05,
                curve: ResponseCurve::Linear,
                max_value: 255,
            });
        }
    }

    if let Some(ref rt_str) = args.right_trigger {
        let rt_key = rt_str.parse::<KeyId>()?;
        if let Some(ref mut rt) = profile.right_trigger {
            rt.key = rt_key;
        } else {
            profile.right_trigger = Some(TriggerBinding {
                key: rt_key,
                deadzone: 0.05,
                curve: ResponseCurve::Linear,
                max_value: 255,
            });
        }
    }

    let poll_hz = args.poll_hz.unwrap_or(profile_config.polling_rate_hz);
    let mut engine = GamepadEngine::new(profile);
    let required_keys = engine.required_keys();
    let queries = plan_queries(&required_keys);

    println!(
        "Engine configured with {} monitored keys ({} chunk queries per cycle)",
        required_keys.len(),
        queries.len()
    );
    println!("Target Polling Rate: {} Hz", poll_hz);

    let override_flag = Arc::new(AtomicBool::new(profile_config.mapping_override));
    let ov_ipc = override_flag.clone();
    let ov_toggle_cb = override_flag.clone();

    let on_interceptor_toggle: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        let cur = ov_toggle_cb.load(Ordering::SeqCst);
        ov_toggle_cb.store(!cur, Ordering::SeqCst);
    });

    let _ipc_srv = crate::ipc::IpcServer::start(move |cmd| match cmd {
        "toggle-override" => {
            let cur = ov_ipc.load(Ordering::SeqCst);
            let next = !cur;
            ov_ipc.store(next, Ordering::SeqCst);
            format!("ok: mapping_override = {}", next)
        }
        "status" => format!(
            "ok: daemon running, mapping_override = {}",
            ov_ipc.load(Ordering::SeqCst)
        ),
        _ => format!("error: unknown command '{}'", cmd),
    })
    .ok();

    // optional key suppression grabber
    let interceptor = if profile_config.gamepad_enabled {
        let event_devs = find_all_keyboard_event_devices();
        if !event_devs.is_empty() {
            for dev in &event_devs {
                println!("Grabbing keyboard event node: {}", dev.display());
            }
            KeyboardInterceptor::spawn_all(
                &event_devs,
                &required_keys,
                profile_config.mapping_override,
                profile_config.enable_typing,
                profile_config.toggle_override_key,
                Some(on_interceptor_toggle),
            )
            .ok()
        } else {
            None
        }
    } else {
        None
    };

    // set hardware rgb highlights if in custom mode
    if profile_config.gamepad_enabled
        && profile_config.mapping_override
        && let Ok(true) = keyboard.apply_mapped_rgb(&required_keys, ColorRGB::RED)
    {
        println!("Applied static RGB lighting to mapped keys (Customization mode active)");
    }

    println!("Running gamepad engine loop. Press Ctrl+C to stop.");

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // hook ctrl-c
    let _ = ctrlc_handler(move || {
        r.store(false, Ordering::SeqCst);
    });

    let poll_interval = if poll_hz > 0 {
        Some(Duration::from_secs_f64(1.0 / (poll_hz as f64)))
    } else {
        None
    };

    let mut last_override = profile_config.mapping_override;

    while running.load(Ordering::SeqCst) {
        let t0 = Instant::now();

        let cur_override = override_flag.load(Ordering::Relaxed);
        if cur_override != last_override {
            last_override = cur_override;
            if let Some(ref inter) = interceptor {
                inter.update_settings(
                    &required_keys,
                    cur_override,
                    profile_config.enable_typing,
                    profile_config.toggle_override_key,
                );
            }
            if cur_override {
                let _ = keyboard.apply_mapped_rgb(&required_keys, ColorRGB::RED);
            } else {
                let _ = keyboard.restore_original_rgb();
            }
        }

        let readings = keyboard.read_keys(&queries)?;
        let state = engine.update(&readings);
        engine.dispatch(&state, &mut gamepad)?;

        if let Some(target) = poll_interval {
            let elapsed = t0.elapsed();
            std::thread::sleep(target.saturating_sub(elapsed));
        }
    }

    let _ = keyboard.restore_original_rgb();
    println!("\nShutting down gamepad mapper.");
    Ok(())
}

fn ctrlc_handler<F>(f: F) -> io::Result<()>
where
    F: Fn() + Send + Sync + 'static,
{
    // signal handler using libc
    unsafe {
        static mut HANDLER: Option<Box<dyn Fn() + Send + Sync>> = None;
        HANDLER = Some(Box::new(f));

        extern "C" fn sigint_handler(_: libc::c_int) {
            unsafe {
                if let Some(ref h) = HANDLER {
                    h();
                }
            }
        }

        libc::signal(
            libc::SIGINT,
            sigint_handler as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            sigint_handler as *const () as libc::sighandler_t,
        );
    }
    Ok(())
}
