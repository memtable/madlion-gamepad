use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::gamepad::{GamepadEngine, GamepadProfile, GamepadState, UinputGamepad};
use crate::keyboard::layout::MAD60_LAYOUT;
use crate::keyboard::{
    ColorRGB, KeyId, KeyboardInterceptor, MadlionKeyboard, find_all_keyboard_event_devices,
    plan_queries,
};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum WorkerCommand {
    UpdateProfile(Box<GamepadProfile>),
    SetGamepadEnabled(bool),
    SetPollingRateHz(u64),
    SetAdaptiveQueryMode(bool),
    SetMappingOverride(bool),
    SetEnableTyping(bool),
    ToggleMappingOverride,
    ToggleGamepad,
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct EngineTelemetry {
    pub key_depths: [f32; 70],
    pub gamepad_state: GamepadState,
    pub poll_hz: f64,
    pub latency_us: u64,
    pub device_connected: bool,
    pub device_name: String,
    pub device_path: String,
    pub virtual_gamepad_active: bool,
}

impl Default for EngineTelemetry {
    fn default() -> Self {
        Self {
            key_depths: [0.0; 70],
            gamepad_state: GamepadState::default(),
            poll_hz: 0.0,
            latency_us: 0,
            device_connected: false,
            device_name: "Scanning for device...".to_string(),
            device_path: String::new(),
            virtual_gamepad_active: false,
        }
    }
}

pub struct WorkerHandle {
    pub tx: Sender<WorkerCommand>,
    pub telemetry: Arc<Mutex<EngineTelemetry>>,
    is_running: Arc<AtomicBool>,
    thread_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

#[allow(dead_code)]
impl WorkerHandle {
    pub fn update_profile(&self, profile: GamepadProfile) {
        let _ = self
            .tx
            .send(WorkerCommand::UpdateProfile(Box::new(profile)));
    }

    pub fn set_gamepad_enabled(&self, enabled: bool) {
        let _ = self.tx.send(WorkerCommand::SetGamepadEnabled(enabled));
    }

    pub fn set_polling_rate_hz(&self, hz: u64) {
        let _ = self.tx.send(WorkerCommand::SetPollingRateHz(hz));
    }

    pub fn set_adaptive_query_mode(&self, adaptive: bool) {
        let _ = self.tx.send(WorkerCommand::SetAdaptiveQueryMode(adaptive));
    }

    pub fn set_mapping_override(&self, override_enabled: bool) {
        let _ = self
            .tx
            .send(WorkerCommand::SetMappingOverride(override_enabled));
    }

    pub fn set_enable_typing(&self, enable: bool) {
        let _ = self.tx.send(WorkerCommand::SetEnableTyping(enable));
    }

    pub fn toggle_mapping_override(&self) {
        let _ = self.tx.send(WorkerCommand::ToggleMappingOverride);
    }

    pub fn toggle_gamepad(&self) {
        let _ = self.tx.send(WorkerCommand::ToggleGamepad);
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(WorkerCommand::Shutdown);
        self.is_running.store(false, Ordering::SeqCst);
        if let Ok(mut lock) = self.thread_handle.lock()
            && let Some(h) = lock.take()
        {
            let _ = h.join();
        }
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Debug, Clone)]
pub struct WorkerInitialConfig {
    pub profile: GamepadProfile,
    pub gamepad_enabled: bool,
    pub polling_rate_hz: u64,
    pub mapping_override: bool,
    pub enable_typing: bool,
    pub toggle_key: Option<KeyId>,
}

pub fn spawn_worker(
    initial_profile: GamepadProfile,
    gamepad_enabled: bool,
    polling_rate_hz: u64,
    mapping_override: bool,
    enable_typing: bool,
    toggle_key: Option<KeyId>,
) -> WorkerHandle {
    let (tx, rx) = channel::<WorkerCommand>();
    let telemetry = Arc::new(Mutex::new(EngineTelemetry::default()));
    let is_running = Arc::new(AtomicBool::new(true));

    let telemetry_worker = telemetry.clone();
    let is_running_worker = is_running.clone();
    let tx_worker = tx.clone();

    let initial_config = WorkerInitialConfig {
        profile: initial_profile,
        gamepad_enabled,
        polling_rate_hz,
        mapping_override,
        enable_typing,
        toggle_key,
    };

    let join_handle = std::thread::spawn(move || {
        worker_loop(
            rx,
            tx_worker,
            telemetry_worker,
            is_running_worker,
            initial_config,
        );
    });

    WorkerHandle {
        tx,
        telemetry,
        is_running,
        thread_handle: Arc::new(Mutex::new(Some(join_handle))),
    }
}

fn worker_loop(
    rx: Receiver<WorkerCommand>,
    tx_self: Sender<WorkerCommand>,
    telemetry: Arc<Mutex<EngineTelemetry>>,
    is_running: Arc<AtomicBool>,
    config: WorkerInitialConfig,
) {
    let mut profile = config.profile;
    let mut gamepad_enabled = config.gamepad_enabled;
    let mut polling_rate_hz = config.polling_rate_hz;
    let mut mapping_override = config.mapping_override;
    let mut enable_typing = config.enable_typing;
    let toggle_key = config.toggle_key;

    let mut engine = GamepadEngine::new(profile.clone());
    let mut keyboard_opt = MadlionKeyboard::auto_discover().ok();
    let mut uinput_opt: Option<UinputGamepad> = if gamepad_enabled {
        UinputGamepad::new().ok()
    } else {
        None
    };

    let mut interceptor_opt: Option<KeyboardInterceptor> = None;
    let mut rgb_synced = !(gamepad_enabled && mapping_override);

    let all_keys: Vec<KeyId> = MAD60_LAYOUT.iter().flatten().copied().collect();
    let full_matrix_queries = plan_queries(&all_keys);
    let mut mapped_queries = plan_queries(&engine.required_keys());
    let mut adaptive_query_mode = true; // query full matrix for gui keycap visualization

    let mut last_reconnect_check = Instant::now();
    let mut poll_count = 0u64;
    let mut hz_timer = Instant::now();
    let mut current_hz = 0.0f64;
    let mut last_latency_us = 0u64;

    let tx_cb = tx_self.clone();
    let on_toggle_action: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        let _ = tx_cb.send(WorkerCommand::ToggleMappingOverride);
    });

    // initial grab n rgb setup
    let sync_interceptor = |interceptor_slot: &mut Option<KeyboardInterceptor>,
                            gamepad_on: bool,
                            override_on: bool,
                            typing_on: bool,
                            t_key: Option<KeyId>,
                            keys: &[KeyId],
                            on_toggle: &Arc<dyn Fn() + Send + Sync>| {
        let should_intercept = gamepad_on && (override_on || !typing_on || t_key.is_some());
        if should_intercept {
            if let Some(inter) = interceptor_slot.as_ref() {
                inter.update_settings(keys, override_on, typing_on, t_key);
            } else {
                let event_devs = find_all_keyboard_event_devices();
                if !event_devs.is_empty() {
                    *interceptor_slot = KeyboardInterceptor::spawn_all(
                        &event_devs,
                        keys,
                        override_on,
                        typing_on,
                        t_key,
                        Some(on_toggle.clone()),
                    )
                    .ok();
                }
            }
        } else {
            *interceptor_slot = None;
        }
    };

    sync_interceptor(
        &mut interceptor_opt,
        gamepad_enabled,
        mapping_override,
        enable_typing,
        toggle_key,
        &engine.required_keys(),
        &on_toggle_action,
    );

    while is_running.load(Ordering::Relaxed) {
        let loop_start = Instant::now();

        // drain command channel
        let mut settings_changed = false;
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                WorkerCommand::UpdateProfile(new_prof) => {
                    profile = *new_prof;
                    engine = GamepadEngine::new(profile.clone());
                    mapped_queries = plan_queries(&engine.required_keys());
                    settings_changed = true;
                }
                WorkerCommand::SetGamepadEnabled(enabled) => {
                    gamepad_enabled = enabled;
                    if gamepad_enabled && uinput_opt.is_none() {
                        uinput_opt = UinputGamepad::new().ok();
                    } else if !gamepad_enabled {
                        uinput_opt = None;
                    }
                    settings_changed = true;
                }
                WorkerCommand::SetPollingRateHz(hz) => {
                    polling_rate_hz = hz;
                }
                WorkerCommand::SetAdaptiveQueryMode(adaptive) => {
                    adaptive_query_mode = adaptive;
                }
                WorkerCommand::SetMappingOverride(ov) => {
                    mapping_override = ov;
                    settings_changed = true;
                }
                WorkerCommand::SetEnableTyping(en) => {
                    enable_typing = en;
                    settings_changed = true;
                }
                WorkerCommand::ToggleMappingOverride => {
                    mapping_override = !mapping_override;
                    settings_changed = true;
                }
                WorkerCommand::ToggleGamepad => {
                    gamepad_enabled = !gamepad_enabled;
                    if gamepad_enabled && uinput_opt.is_none() {
                        uinput_opt = UinputGamepad::new().ok();
                    } else if !gamepad_enabled {
                        uinput_opt = None;
                    }
                    settings_changed = true;
                }
                WorkerCommand::Shutdown => {
                    if let Some(ref mut kb) = keyboard_opt {
                        let _ = kb.restore_original_rgb();
                    }
                    is_running.store(false, Ordering::SeqCst);
                    return;
                }
            }
        }

        if settings_changed {
            sync_interceptor(
                &mut interceptor_opt,
                gamepad_enabled,
                mapping_override,
                enable_typing,
                toggle_key,
                &engine.required_keys(),
                &on_toggle_action,
            );
            rgb_synced = false;
        }

        // check device connection n reconnect if dropped
        if keyboard_opt.is_none() && last_reconnect_check.elapsed() >= Duration::from_millis(500) {
            keyboard_opt = MadlionKeyboard::auto_discover().ok();
            last_reconnect_check = Instant::now();
            if keyboard_opt.is_some() {
                rgb_synced = false;
                sync_interceptor(
                    &mut interceptor_opt,
                    gamepad_enabled,
                    mapping_override,
                    enable_typing,
                    toggle_key,
                    &engine.required_keys(),
                    &on_toggle_action,
                );
            }
        }

        // update hardware rgb highlights if custom mode is on
        if !rgb_synced && let Some(ref mut kb) = keyboard_opt {
            if gamepad_enabled && mapping_override {
                let _ = kb.apply_mapped_rgb(&engine.required_keys(), ColorRGB::RED);
            } else {
                let _ = kb.restore_original_rgb();
            }
            rgb_synced = true;
        }

        // read switch travel
        let queries = if adaptive_query_mode {
            &full_matrix_queries
        } else {
            &mapped_queries
        };

        let mut read_success = false;
        let mut key_depths = [0.0f32; 70];
        let mut gamepad_state = GamepadState::default();

        if let Some(ref mut kb) = keyboard_opt {
            let t_poll = Instant::now();
            match kb.read_keys(queries) {
                Ok(readings) => {
                    last_latency_us = t_poll.elapsed().as_micros() as u64;
                    read_success = true;
                    poll_count += 1;

                    for r in &readings {
                        let idx = r.matrix_index as usize;
                        if idx < key_depths.len() {
                            key_depths[idx] = r.normalized;
                        }
                    }

                    // feed readings into engine
                    gamepad_state = engine.update(&readings);

                    // send state to uinput
                    if gamepad_enabled {
                        if let Some(ref mut uinput) = uinput_opt {
                            let _ = engine.dispatch(&gamepad_state, uinput);
                        } else {
                            // reconnect uinput if needed
                            uinput_opt = UinputGamepad::new().ok();
                        }
                    }
                }
                Err(_) => {
                    // device write failed
                    keyboard_opt = None;
                }
            }
        }

        // calculate poll rate
        if hz_timer.elapsed() >= Duration::from_millis(500) {
            let elapsed_sec = hz_timer.elapsed().as_secs_f64();
            current_hz = (poll_count as f64) / elapsed_sec;
            poll_count = 0;
            hz_timer = Instant::now();
        }

        // update telemetry snapshot for ui
        if let Ok(mut tel) = telemetry.try_lock() {
            tel.device_connected = read_success;
            tel.virtual_gamepad_active = gamepad_enabled && uinput_opt.is_some();
            if read_success {
                tel.key_depths = key_depths;
                tel.gamepad_state = gamepad_state;
                tel.poll_hz = current_hz;
                tel.latency_us = last_latency_us;
            }
            if let Some(ref kb) = keyboard_opt {
                tel.device_path = kb.path().display().to_string();
                tel.device_name = "Madlion Keyboard (Analog Interface)".to_string();
            } else {
                tel.device_path.clear();
                tel.device_name = "No Madlion keyboard detected".to_string();
            }
        }

        // sleep to hit target poll rate
        let target_interval = if polling_rate_hz > 0 {
            Duration::from_secs_f64(1.0 / (polling_rate_hz as f64))
        } else {
            Duration::from_micros(500)
        };

        let elapsed = loop_start.elapsed();
        std::thread::sleep(target_interval.saturating_sub(elapsed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamepad::{
        ButtonBinding, GamepadButton, ResponseCurve, StickBinding, TriggerBinding,
    };

    #[test]
    fn test_engine_telemetry_default_and_clone() {
        let tel = EngineTelemetry::default();
        assert_eq!(tel.key_depths.len(), 70);
        for &depth in &tel.key_depths {
            assert_eq!(depth, 0.0);
        }
        assert_eq!(tel.poll_hz, 0.0);
        assert_eq!(tel.latency_us, 0);
        assert!(!tel.device_connected);
        assert_eq!(tel.device_name, "Scanning for device...");
        assert!(tel.device_path.is_empty());
        assert!(!tel.virtual_gamepad_active);
        assert_eq!(tel.gamepad_state.left_stick_x, 0);
        assert_eq!(tel.gamepad_state.left_stick_y, 0);
        assert_eq!(tel.gamepad_state.right_stick_x, 0);
        assert_eq!(tel.gamepad_state.right_stick_y, 0);
        assert_eq!(tel.gamepad_state.left_trigger, 0);
        assert_eq!(tel.gamepad_state.right_trigger, 0);
        assert!(tel.gamepad_state.buttons.is_empty());

        let mut cloned = tel.clone();
        cloned.device_connected = true;
        cloned.device_name = "Custom Device".to_string();
        cloned.poll_hz = 1000.0;
        cloned.key_depths[0] = 0.75;
        assert_ne!(cloned.device_name, tel.device_name);
        assert_ne!(cloned.device_connected, tel.device_connected);
        assert_eq!(tel.key_depths[0], 0.0);
    }

    #[test]
    fn test_worker_command_debug_and_clone() {
        let cmds = vec![
            WorkerCommand::UpdateProfile(Box::default()),
            WorkerCommand::SetGamepadEnabled(true),
            WorkerCommand::SetGamepadEnabled(false),
            WorkerCommand::SetPollingRateHz(500),
            WorkerCommand::SetAdaptiveQueryMode(true),
            WorkerCommand::SetAdaptiveQueryMode(false),
            WorkerCommand::SetMappingOverride(true),
            WorkerCommand::SetMappingOverride(false),
            WorkerCommand::SetEnableTyping(true),
            WorkerCommand::SetEnableTyping(false),
            WorkerCommand::ToggleMappingOverride,
            WorkerCommand::ToggleGamepad,
            WorkerCommand::Shutdown,
        ];

        for cmd in cmds {
            let debug_str = format!("{:?}", cmd);
            assert!(!debug_str.is_empty());
            let cloned = cmd.clone();
            assert_eq!(format!("{:?}", cloned), debug_str);
        }
    }

    #[test]
    fn test_worker_initial_config_clone_and_debug() {
        let config = WorkerInitialConfig {
            profile: GamepadProfile::default(),
            gamepad_enabled: false,
            polling_rate_hz: 500,
            mapping_override: true,
            enable_typing: false,
            toggle_key: Some(KeyId::LeftControl),
        };

        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("WorkerInitialConfig"));
        let cloned = config.clone();
        assert_eq!(cloned.polling_rate_hz, 500);
        assert!(cloned.mapping_override);
        assert!(!cloned.enable_typing);
        assert_eq!(cloned.toggle_key, Some(KeyId::LeftControl));
    }

    #[test]
    fn test_spawn_worker_and_graceful_shutdown() {
        let handle = spawn_worker(GamepadProfile::default(), false, 1000, false, true, None);
        std::thread::sleep(Duration::from_millis(30));

        {
            let tel = handle.telemetry.lock().unwrap();
            if !tel.device_connected {
                assert!(
                    tel.device_name == "Scanning for device..."
                        || tel.device_name == "No Madlion keyboard detected"
                );
            } else {
                assert!(!tel.device_name.is_empty());
            }
        }

        handle.shutdown();
        assert!(!handle.is_running.load(Ordering::SeqCst));

        // shutdown should be idempotent
        handle.shutdown();
    }

    #[test]
    fn test_worker_handle_drop_triggers_shutdown() {
        let is_running_ref;
        {
            let handle = spawn_worker(GamepadProfile::default(), false, 1000, false, true, None);
            is_running_ref = handle.is_running.clone();
            assert!(is_running_ref.load(Ordering::SeqCst));
            std::thread::sleep(Duration::from_millis(20));
        }
        // dropping handle stops worker
        assert!(!is_running_ref.load(Ordering::SeqCst));
    }

    #[test]
    fn test_worker_handle_command_channel_dispatch() {
        let handle = spawn_worker(GamepadProfile::default(), false, 1000, false, true, None);

        // test command dispatch methods
        handle.update_profile(GamepadProfile::default());
        handle.set_gamepad_enabled(true);
        handle.set_gamepad_enabled(false);
        handle.set_polling_rate_hz(500);
        handle.set_adaptive_query_mode(false);
        handle.set_adaptive_query_mode(true);
        handle.set_mapping_override(true);
        handle.set_mapping_override(false);
        handle.set_enable_typing(false);
        handle.set_enable_typing(true);
        handle.toggle_mapping_override();
        handle.toggle_gamepad();

        std::thread::sleep(Duration::from_millis(30));
        handle.shutdown();
    }

    #[test]
    fn test_worker_rapid_command_throughput() {
        let handle = spawn_worker(GamepadProfile::default(), false, 2000, false, true, None);

        for i in 0..100 {
            if i % 4 == 0 {
                handle.toggle_mapping_override();
            } else if i % 4 == 1 {
                handle.toggle_gamepad();
            } else if i % 4 == 2 {
                handle.set_polling_rate_hz(500 + (i as u64) * 10);
            } else {
                handle.set_adaptive_query_mode(i % 2 == 0);
            }
        }

        std::thread::sleep(Duration::from_millis(40));
        handle.shutdown();
    }

    #[test]
    fn test_worker_spawn_configurations_matrix() {
        let configs = vec![
            (true, 1000, true, false, Some(KeyId::LeftControl)),
            (false, 500, false, true, None),
            (false, 0, false, false, None),
            (false, 8000, true, true, Some(KeyId::CapsLock)),
        ];

        for (gamepad_enabled, rate, mapping_override, enable_typing, toggle_key) in configs {
            let handle = spawn_worker(
                GamepadProfile::default(),
                gamepad_enabled,
                rate,
                mapping_override,
                enable_typing,
                toggle_key,
            );
            std::thread::sleep(Duration::from_millis(15));
            handle.shutdown();
        }
    }

    #[test]
    fn test_worker_telemetry_concurrent_reads() {
        let handle = spawn_worker(GamepadProfile::default(), false, 1000, false, true, None);

        let telemetry_clone1 = handle.telemetry.clone();
        let telemetry_clone2 = handle.telemetry.clone();

        let reader1 = std::thread::spawn(move || {
            for _ in 0..50 {
                if let Ok(tel) = telemetry_clone1.lock() {
                    let _ = tel.device_connected;
                    let _ = tel.key_depths[0];
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        });

        let reader2 = std::thread::spawn(move || {
            for _ in 0..50 {
                if let Ok(tel) = telemetry_clone2.lock() {
                    let _ = tel.device_name.len();
                    let _ = tel.poll_hz;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        });

        for _ in 0..20 {
            handle.toggle_mapping_override();
            std::thread::sleep(Duration::from_millis(2));
        }

        reader1.join().unwrap();
        reader2.join().unwrap();
        handle.shutdown();
    }

    #[test]
    fn test_worker_profile_update_with_custom_bindings() {
        let custom_profile = GamepadProfile {
            left_stick: Some(StickBinding {
                up: Some(KeyId::W),
                down: Some(KeyId::S),
                left: Some(KeyId::A),
                right: Some(KeyId::D),
                deadzone: 0.15,
                curve: ResponseCurve::Exponential(1.5),
                square_joystick: true,
                snappy_joystick: false,
                angle_adjustment_deg: Some(10.0),
            }),
            right_trigger: Some(TriggerBinding {
                key: KeyId::Space,
                deadzone: 0.1,
                curve: ResponseCurve::Linear,
                max_value: 255,
            }),
            buttons: vec![ButtonBinding {
                key: KeyId::F,
                button: GamepadButton::A,
                actuation_mm: 0.4,
                release_mm: 0.3,
                rapid_trigger: true,
                rapid_press_delta_mm: 0.1,
                rapid_release_delta_mm: 0.1,
            }],
            ..Default::default()
        };

        let handle = spawn_worker(GamepadProfile::default(), false, 1000, false, true, None);
        std::thread::sleep(Duration::from_millis(15));

        handle.update_profile(custom_profile);
        std::thread::sleep(Duration::from_millis(30));

        handle.shutdown();
    }

    #[test]
    fn test_worker_direct_shutdown_command() {
        let handle = spawn_worker(GamepadProfile::default(), false, 1000, false, true, None);

        // send shutdown over channel
        handle.tx.send(WorkerCommand::Shutdown).unwrap();
        let start = Instant::now();
        while handle.is_running.load(Ordering::SeqCst)
            && start.elapsed() < Duration::from_millis(500)
        {
            std::thread::sleep(Duration::from_millis(10));
        }

        // worker should stop running
        assert!(!handle.is_running.load(Ordering::SeqCst));

        // clean up handle
        handle.shutdown();
    }

    #[test]
    fn test_worker_post_shutdown_calls_safety() {
        let handle = spawn_worker(GamepadProfile::default(), false, 1000, false, true, None);
        handle.shutdown();

        // calling methods after shutdown shouldnt panic
        handle.update_profile(GamepadProfile::default());
        handle.set_gamepad_enabled(true);
        handle.set_polling_rate_hz(500);
        handle.set_adaptive_query_mode(true);
        handle.set_mapping_override(true);
        handle.set_enable_typing(true);
        handle.toggle_mapping_override();
        handle.toggle_gamepad();
        handle.shutdown();
    }
}
