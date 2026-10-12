use std::collections::{HashMap, HashSet};
use std::io;

use crate::keyboard::{AnalogReading, KeyId};

use super::uinput::{GamepadAxis, GamepadButton, UinputGamepad};

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum ResponseCurve {
    Linear,
    Aggro,
    Slow,
    Smooth,
    Steep,
    Instant,
    Turbo,
    Exponential(f32),
    Custom([(f32, f32); 4]),
}

impl ResponseCurve {
    pub fn to_points(self) -> [(f32, f32); 4] {
        match self {
            ResponseCurve::Linear => [(0.0, 20.0), (60.0, 60.0), (194.0, 195.0), (253.0, 255.0)],
            ResponseCurve::Aggro => [(0.0, 33.0), (63.0, 114.0), (139.0, 231.0), (253.0, 255.0)],
            ResponseCurve::Slow => [(0.0, 23.0), (128.0, 63.0), (226.0, 145.0), (253.0, 255.0)],
            ResponseCurve::Smooth => [(0.0, 26.0), (78.0, 84.0), (199.0, 174.0), (253.0, 255.0)],
            ResponseCurve::Steep => [(0.0, 26.0), (25.0, 132.0), (192.0, 135.0), (253.0, 255.0)],
            ResponseCurve::Instant => [(0.0, 255.0), (2.0, 255.0), (192.0, 255.0), (255.0, 255.0)],
            ResponseCurve::Turbo => [(0.0, 255.0), (1.0, 255.0), (2.0, 255.0), (3.0, 255.0)],
            ResponseCurve::Exponential(_) => {
                [(0.0, 0.0), (85.0, 28.0), (170.0, 113.0), (255.0, 255.0)]
            }
            ResponseCurve::Custom(pts) => pts,
        }
    }

    pub fn apply(&self, normalized_val: f32) -> f32 {
        let val = normalized_val.clamp(0.0, 1.0);
        match self {
            ResponseCurve::Exponential(exp) => val.powf(*exp),
            _ => {
                let pts = self.to_points();
                let x = val * 255.0;
                let y = interpolate_piecewise(&pts, x);
                (y / 255.0).clamp(0.0, 1.0)
            }
        }
    }
}

pub fn interpolate_piecewise(pts: &[(f32, f32); 4], x: f32) -> f32 {
    let (x0, y0) = pts[0];
    let (x1, y1) = pts[1];
    let (x2, y2) = pts[2];
    let (x3, y3) = pts[3];

    if x < x0 {
        return 0.0;
    }
    if x >= x3 {
        return y3;
    }

    if x <= x1 {
        if (x1 - x0).abs() < 1e-5 {
            return y1;
        }
        let t = (x - x0) / (x1 - x0);
        return y0 + t * (y1 - y0);
    }

    if x <= x2 {
        if (x2 - x1).abs() < 1e-5 {
            return y2;
        }
        let t = (x - x1) / (x2 - x1);
        return y1 + t * (y2 - y1);
    }

    if (x3 - x2).abs() < 1e-5 {
        return y3;
    }
    let t = (x - x2) / (x3 - x2);
    y2 + t * (y3 - y2)
}

#[derive(Debug, Clone)]
pub struct StickBinding {
    pub up: Option<KeyId>,
    pub down: Option<KeyId>,
    pub left: Option<KeyId>,
    pub right: Option<KeyId>,
    pub deadzone: f32,
    pub curve: ResponseCurve,
    pub square_joystick: bool,
    pub snappy_joystick: bool,
    pub angle_adjustment_deg: Option<f32>,
}

impl Default for StickBinding {
    fn default() -> Self {
        Self {
            up: Some(KeyId::W),
            down: Some(KeyId::S),
            left: Some(KeyId::A),
            right: Some(KeyId::D),
            deadzone: 0.05,
            curve: ResponseCurve::Linear,
            square_joystick: false,
            snappy_joystick: true,
            angle_adjustment_deg: None,
        }
    }
}

impl StickBinding {
    pub fn compute_vector(&self, up: f32, down: f32, left: f32, right: f32) -> (i32, i32) {
        // pick dominant direction when pressing opposing inputs
        let (raw_x, raw_y) = if self.snappy_joystick {
            let x = if right >= left { right } else { -left };
            let y = if down >= up { down } else { -up };
            (x, y)
        } else {
            (right - left, down - up)
        };

        // tweak diagonal center angle if enabled
        let (adj_x, adj_y) = if let Some(deg) = self.angle_adjustment_deg {
            if raw_x.abs() > 1e-4 && raw_y.abs() > 1e-4 {
                let rad = deg.to_radians();
                // 45 deg is standard
                let scale_x = rad.cos() / std::f32::consts::FRAC_1_SQRT_2;
                let scale_y = rad.sin() / std::f32::consts::FRAC_1_SQRT_2;
                (raw_x * scale_x, raw_y * scale_y)
            } else {
                (raw_x, raw_y)
            }
        } else {
            (raw_x, raw_y)
        };

        // deadzone n clamping
        if self.square_joystick {
            // square stick mode: clamp axes independently for full diagonals
            let calc_axis = |val: f32| -> i32 {
                let sign = val.signum();
                let abs_val = val.abs();
                if abs_val <= self.deadzone || abs_val <= 1e-6 {
                    0
                } else {
                    let eff = ((abs_val - self.deadzone) / (1.0 - self.deadzone)).clamp(0.0, 1.0);
                    let curved = self.curve.apply(eff);
                    (sign * curved * 32767.0).round().clamp(-32767.0, 32767.0) as i32
                }
            };
            (calc_axis(adj_x), calc_axis(adj_y))
        } else {
            // circular stick mode: clamp magnitude to unit circle
            let magnitude = (adj_x * adj_x + adj_y * adj_y).sqrt();
            if magnitude <= self.deadzone || magnitude <= 1e-6 {
                return (0, 0);
            }

            // smooth scaling past deadzone up to 1
            let effective_norm =
                ((magnitude - self.deadzone) / (1.0 - self.deadzone)).clamp(0.0, 1.0);
            let curved_norm = self.curve.apply(effective_norm);

            let dir_x = adj_x / magnitude;
            let dir_y = adj_y / magnitude;

            let out_x = (dir_x * curved_norm * 32767.0)
                .round()
                .clamp(-32767.0, 32767.0) as i32;
            let out_y = (dir_y * curved_norm * 32767.0)
                .round()
                .clamp(-32767.0, 32767.0) as i32;

            (out_x, out_y)
        }
    }
}

#[derive(Debug, Clone)]
pub struct TriggerBinding {
    pub key: KeyId,
    pub deadzone: f32,
    pub curve: ResponseCurve,
    pub max_value: i32,
}

impl Default for TriggerBinding {
    fn default() -> Self {
        Self {
            key: KeyId::Q,
            deadzone: 0.05,
            curve: ResponseCurve::Linear,
            max_value: 255,
        }
    }
}

impl TriggerBinding {
    pub fn compute_value(&self, depth_normalized: f32) -> i32 {
        if depth_normalized <= self.deadzone {
            return 0;
        }

        let effective =
            ((depth_normalized - self.deadzone) / (1.0 - self.deadzone)).clamp(0.0, 1.0);
        let curved = self.curve.apply(effective);

        (curved * (self.max_value as f32))
            .round()
            .clamp(0.0, self.max_value as f32) as i32
    }
}

#[derive(Debug, Clone)]
pub struct ButtonBinding {
    pub key: KeyId,
    pub button: GamepadButton,
    pub actuation_mm: f32,
    pub release_mm: f32,
    pub rapid_trigger: bool,
    pub rapid_press_delta_mm: f32,
    pub rapid_release_delta_mm: f32,
}

impl ButtonBinding {
    pub fn new(key: KeyId, button: GamepadButton) -> Self {
        Self {
            key,
            button,
            actuation_mm: 1.2,
            release_mm: 1.0,
            rapid_trigger: false,
            rapid_press_delta_mm: 0.15,
            rapid_release_delta_mm: 0.15,
        }
    }

    #[cfg(test)]
    pub fn rapid(key: KeyId, button: GamepadButton, initial_actuation_mm: f32) -> Self {
        Self {
            key,
            button,
            actuation_mm: initial_actuation_mm,
            release_mm: initial_actuation_mm - 0.2,
            rapid_trigger: true,
            rapid_press_delta_mm: 0.15,
            rapid_release_delta_mm: 0.15,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ButtonState {
    pub pressed: bool,
    pub peak_depth_since_press: f32,
    pub valley_depth_since_release: f32,
}

impl Default for ButtonState {
    fn default() -> Self {
        Self {
            pressed: false,
            peak_depth_since_press: 0.0,
            valley_depth_since_release: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GamepadProfile {
    pub left_stick: Option<StickBinding>,
    pub right_stick: Option<StickBinding>,
    pub left_trigger: Option<TriggerBinding>,
    pub right_trigger: Option<TriggerBinding>,
    pub buttons: Vec<ButtonBinding>,
}

impl Default for GamepadProfile {
    fn default() -> Self {
        Self {
            left_stick: Some(StickBinding::default()),
            right_stick: Some(StickBinding {
                up: Some(KeyId::I),
                down: Some(KeyId::K),
                left: Some(KeyId::J),
                right: Some(KeyId::L),
                deadzone: 0.05,
                curve: ResponseCurve::Linear,
                square_joystick: false,
                snappy_joystick: true,
                angle_adjustment_deg: None,
            }),
            left_trigger: Some(TriggerBinding {
                key: KeyId::Q,
                deadzone: 0.05,
                curve: ResponseCurve::Linear,
                max_value: 255,
            }),
            right_trigger: Some(TriggerBinding {
                key: KeyId::E,
                deadzone: 0.05,
                curve: ResponseCurve::Linear,
                max_value: 255,
            }),
            buttons: vec![
                ButtonBinding::new(KeyId::Space, GamepadButton::A),
                ButtonBinding::new(KeyId::LeftAlt, GamepadButton::B),
                ButtonBinding::new(KeyId::LeftShift, GamepadButton::X),
                ButtonBinding::new(KeyId::R, GamepadButton::Y),
                ButtonBinding::new(KeyId::LeftControl, GamepadButton::LeftBumper),
                ButtonBinding::new(KeyId::F, GamepadButton::RightBumper),
                ButtonBinding::new(KeyId::Tab, GamepadButton::Select),
                ButtonBinding::new(KeyId::Enter, GamepadButton::Start),
                ButtonBinding::new(KeyId::Esc, GamepadButton::Mode),
                ButtonBinding::new(KeyId::Z, GamepadButton::LeftThumb),
                ButtonBinding::new(KeyId::X, GamepadButton::RightThumb),
                ButtonBinding::new(KeyId::Key1, GamepadButton::DPadUp),
                ButtonBinding::new(KeyId::Key2, GamepadButton::DPadDown),
                ButtonBinding::new(KeyId::Key3, GamepadButton::DPadLeft),
                ButtonBinding::new(KeyId::Key4, GamepadButton::DPadRight),
            ],
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct GamepadState {
    pub left_stick_x: i32,
    pub left_stick_y: i32,
    pub right_stick_x: i32,
    pub right_stick_y: i32,
    pub left_trigger: i32,
    pub right_trigger: i32,
    pub buttons: HashMap<GamepadButton, bool>,
}

pub struct GamepadEngine {
    pub profile: GamepadProfile,
    button_states: HashMap<KeyId, ButtonState>,
    last_sent_state: GamepadState,
}

impl GamepadEngine {
    pub fn new(profile: GamepadProfile) -> Self {
        Self {
            profile,
            button_states: HashMap::new(),
            last_sent_state: GamepadState::default(),
        }
    }

    pub fn required_keys(&self) -> Vec<KeyId> {
        let mut keys = HashSet::new();

        if let Some(ref ls) = self.profile.left_stick {
            if let Some(k) = ls.up {
                keys.insert(k);
            }
            if let Some(k) = ls.down {
                keys.insert(k);
            }
            if let Some(k) = ls.left {
                keys.insert(k);
            }
            if let Some(k) = ls.right {
                keys.insert(k);
            }
        }

        if let Some(ref rs) = self.profile.right_stick {
            if let Some(k) = rs.up {
                keys.insert(k);
            }
            if let Some(k) = rs.down {
                keys.insert(k);
            }
            if let Some(k) = rs.left {
                keys.insert(k);
            }
            if let Some(k) = rs.right {
                keys.insert(k);
            }
        }

        if let Some(ref lt) = self.profile.left_trigger {
            keys.insert(lt.key);
        }

        if let Some(ref rt) = self.profile.right_trigger {
            keys.insert(rt.key);
        }

        for btn in &self.profile.buttons {
            keys.insert(btn.key);
        }

        let mut list: Vec<KeyId> = keys.into_iter().collect();
        list.sort_by_key(|k| format!("{:?}", k));
        list
    }

    pub fn update(&mut self, readings: &[AnalogReading]) -> GamepadState {
        let mut norm_map: HashMap<KeyId, f32> = HashMap::new();
        let mut mm_map: HashMap<KeyId, f32> = HashMap::new();

        for r in readings {
            norm_map.insert(r.key, r.normalized);
            mm_map.insert(r.key, r.depth_mm);
        }

        let mut state = GamepadState::default();

        // handle left stick
        if let Some(ref ls) = self.profile.left_stick {
            let u = ls.up.and_then(|k| norm_map.get(&k)).copied().unwrap_or(0.0);
            let d = ls
                .down
                .and_then(|k| norm_map.get(&k))
                .copied()
                .unwrap_or(0.0);
            let l = ls
                .left
                .and_then(|k| norm_map.get(&k))
                .copied()
                .unwrap_or(0.0);
            let r = ls
                .right
                .and_then(|k| norm_map.get(&k))
                .copied()
                .unwrap_or(0.0);
            let (sx, sy) = ls.compute_vector(u, d, l, r);
            state.left_stick_x = sx;
            state.left_stick_y = sy;
        }

        // handle right stick
        if let Some(ref rs) = self.profile.right_stick {
            let u = rs.up.and_then(|k| norm_map.get(&k)).copied().unwrap_or(0.0);
            let d = rs
                .down
                .and_then(|k| norm_map.get(&k))
                .copied()
                .unwrap_or(0.0);
            let l = rs
                .left
                .and_then(|k| norm_map.get(&k))
                .copied()
                .unwrap_or(0.0);
            let r = rs
                .right
                .and_then(|k| norm_map.get(&k))
                .copied()
                .unwrap_or(0.0);
            let (sx, sy) = rs.compute_vector(u, d, l, r);
            state.right_stick_x = sx;
            state.right_stick_y = sy;
        }

        // handle left trigger
        if let Some(ref lt) = self.profile.left_trigger {
            let depth_norm = norm_map.get(&lt.key).copied().unwrap_or(0.0);
            state.left_trigger = lt.compute_value(depth_norm);
        }

        // handle right trigger
        if let Some(ref rt) = self.profile.right_trigger {
            let depth_norm = norm_map.get(&rt.key).copied().unwrap_or(0.0);
            state.right_trigger = rt.compute_value(depth_norm);
        }

        // handle buttons
        for binding in &self.profile.buttons {
            let depth_mm = mm_map.get(&binding.key).copied().unwrap_or(0.0);
            let btn_state = self.button_states.entry(binding.key).or_default();

            let is_pressed = if !binding.rapid_trigger {
                // static threshold with hysteresis
                if btn_state.pressed {
                    depth_mm >= binding.release_mm
                } else {
                    depth_mm >= binding.actuation_mm
                }
            } else {
                // rapid trigger tracking
                if !btn_state.pressed {
                    if depth_mm >= binding.actuation_mm
                        || depth_mm
                            > btn_state.valley_depth_since_release + binding.rapid_press_delta_mm
                    {
                        btn_state.peak_depth_since_press = depth_mm;
                        true
                    } else {
                        if depth_mm < btn_state.valley_depth_since_release {
                            btn_state.valley_depth_since_release = depth_mm;
                        }
                        false
                    }
                } else {
                    if depth_mm > btn_state.peak_depth_since_press {
                        btn_state.peak_depth_since_press = depth_mm;
                    }
                    if depth_mm < binding.release_mm.min(0.2)
                        || depth_mm
                            <= (btn_state.peak_depth_since_press - binding.rapid_release_delta_mm)
                    {
                        btn_state.valley_depth_since_release = depth_mm;
                        false
                    } else {
                        true
                    }
                }
            };

            btn_state.pressed = is_pressed;
            state.buttons.insert(binding.button, is_pressed);
        }

        state
    }

    pub fn dispatch(
        &mut self,
        state: &GamepadState,
        gamepad: &mut UinputGamepad,
    ) -> io::Result<()> {
        let mut changed = false;

        if state.left_stick_x != self.last_sent_state.left_stick_x {
            gamepad.send_axis(GamepadAxis::LeftStickX, state.left_stick_x)?;
            changed = true;
        }
        if state.left_stick_y != self.last_sent_state.left_stick_y {
            gamepad.send_axis(GamepadAxis::LeftStickY, state.left_stick_y)?;
            changed = true;
        }
        if state.right_stick_x != self.last_sent_state.right_stick_x {
            gamepad.send_axis(GamepadAxis::RightStickX, state.right_stick_x)?;
            changed = true;
        }
        if state.right_stick_y != self.last_sent_state.right_stick_y {
            gamepad.send_axis(GamepadAxis::RightStickY, state.right_stick_y)?;
            changed = true;
        }
        if state.left_trigger != self.last_sent_state.left_trigger {
            gamepad.send_axis(GamepadAxis::LeftTrigger, state.left_trigger)?;
            changed = true;
        }
        if state.right_trigger != self.last_sent_state.right_trigger {
            gamepad.send_axis(GamepadAxis::RightTrigger, state.right_trigger)?;
            changed = true;
        }

        for (&btn, &pressed) in &state.buttons {
            let prev_pressed = self
                .last_sent_state
                .buttons
                .get(&btn)
                .copied()
                .unwrap_or(false);
            if pressed != prev_pressed {
                gamepad.send_button(btn, pressed)?;
                changed = true;
            }
        }

        if changed {
            gamepad.sync()?;
            self.last_sent_state = state.clone();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stick_vector_cardinal() {
        let stick = StickBinding::default();
        // fully up (w=1.0, s=0, a=0, d=0)
        let (x, y) = stick.compute_vector(1.0, 0.0, 0.0, 0.0);
        assert_eq!(x, 0);
        assert_eq!(y, -32767);

        // fully right (w=0, s=0, a=0, d=1.0)
        let (x, y) = stick.compute_vector(0.0, 0.0, 0.0, 1.0);
        assert_eq!(x, 32767);
        assert_eq!(y, 0);

        // fully down (w=0, s=1.0, a=0, d=0)
        let (x, y) = stick.compute_vector(0.0, 1.0, 0.0, 0.0);
        assert_eq!(x, 0);
        assert_eq!(y, 32767);

        // fully left (w=0, s=0, a=1.0, d=0)
        let (x, y) = stick.compute_vector(0.0, 0.0, 1.0, 0.0);
        assert_eq!(x, -32767);
        assert_eq!(y, 0);
    }

    #[test]
    fn test_stick_deadzone() {
        let stick = StickBinding {
            deadzone: 0.10,
            ..Default::default()
        };
        let (x, y) = stick.compute_vector(0.08, 0.0, 0.0, 0.0);
        assert_eq!(x, 0);
        assert_eq!(y, 0);

        let (x, y) = stick.compute_vector(0.20, 0.0, 0.0, 0.0);
        assert!(y < 0);
        assert_eq!(x, 0);
    }

    #[test]
    fn test_trigger_mapping() {
        let trigger = TriggerBinding::default();
        assert_eq!(trigger.compute_value(0.0), 0);
        assert_eq!(trigger.compute_value(0.04), 0); // below deadzone
        assert_eq!(trigger.compute_value(1.0), 255);
        let mid = trigger.compute_value(0.525);
        assert!(mid > 100 && mid < 160);
    }

    #[test]
    fn test_button_hysteresis() {
        let mut engine = GamepadEngine::new(GamepadProfile {
            left_stick: None,
            right_stick: None,
            left_trigger: None,
            right_trigger: None,
            buttons: vec![ButtonBinding::new(KeyId::Space, GamepadButton::A)],
        });

        // start at 0mm
        let s0 = engine.update(&[AnalogReading::new(KeyId::Space, 62, 0)]);
        assert!(!*s0.buttons.get(&GamepadButton::A).unwrap());

        // 1.0mm below threshold
        let s1 = engine.update(&[AnalogReading::new(KeyId::Space, 62, 100)]);
        assert!(!*s1.buttons.get(&GamepadButton::A).unwrap());

        // 1.3mm past threshold so pressed
        let s2 = engine.update(&[AnalogReading::new(KeyId::Space, 62, 130)]);
        assert!(*s2.buttons.get(&GamepadButton::A).unwrap());

        // 1.1mm still held due to hysteresis
        let s3 = engine.update(&[AnalogReading::new(KeyId::Space, 62, 110)]);
        assert!(*s3.buttons.get(&GamepadButton::A).unwrap());

        // 0.9mm released
        let s4 = engine.update(&[AnalogReading::new(KeyId::Space, 62, 90)]);
        assert!(!*s4.buttons.get(&GamepadButton::A).unwrap());
    }

    #[test]
    fn test_stick_diagonal() {
        let stick = StickBinding {
            deadzone: 0.0,
            curve: ResponseCurve::Linear,
            ..Default::default()
        };
        // up n right equal (w=0.7, d=0.7)
        let (x, y) = stick.compute_vector(0.7, 0.0, 0.0, 0.7);
        assert!(x > 0);
        assert!(y < 0);
        // check vector magnitude scaling
        let mag = ((x as f64).powi(2) + (y as f64).powi(2)).sqrt();
        assert!(mag > 32000.0 && mag <= 32768.0);
    }

    #[test]
    fn test_exponential_curve() {
        let curve = ResponseCurve::Exponential(2.0);
        assert_eq!(curve.apply(0.0), 0.0);
        assert_eq!(curve.apply(1.0), 1.0);
        assert!((curve.apply(0.5) - 0.25).abs() < 1e-4);
    }

    #[test]
    fn test_rapid_trigger() {
        let mut engine = GamepadEngine::new(GamepadProfile {
            left_stick: None,
            right_stick: None,
            left_trigger: None,
            right_trigger: None,
            buttons: vec![ButtonBinding::rapid(KeyId::Space, GamepadButton::A, 0.5)],
        });

        // starts unpressed
        let s0 = engine.update(&[AnalogReading::new(KeyId::Space, 62, 0)]);
        assert!(!*s0.buttons.get(&GamepadButton::A).unwrap());

        // press past actuation point
        let s1 = engine.update(&[AnalogReading::new(KeyId::Space, 62, 60)]);
        assert!(*s1.buttons.get(&GamepadButton::A).unwrap());

        // press further down
        let s2 = engine.update(&[AnalogReading::new(KeyId::Space, 62, 150)]);
        assert!(*s2.buttons.get(&GamepadButton::A).unwrap());

        // small release resets immediately
        let s3 = engine.update(&[AnalogReading::new(KeyId::Space, 62, 130)]);
        assert!(!*s3.buttons.get(&GamepadButton::A).unwrap());

        // repressing immediately actuates again
        let s4 = engine.update(&[AnalogReading::new(KeyId::Space, 62, 150)]);
        assert!(*s4.buttons.get(&GamepadButton::A).unwrap());
    }

    #[test]
    fn test_snappy_vs_subtractive_joystick() {
        // snappy mode outputs dominant d over a
        let stick_snappy = StickBinding {
            deadzone: 0.0,
            snappy_joystick: true,
            ..Default::default()
        };
        let (x, _) = stick_snappy.compute_vector(0.0, 0.0, 0.6, 0.8);
        assert!(x > 0);

        // normal mode subtracts a from d
        let stick_subtractive = StickBinding {
            deadzone: 0.0,
            snappy_joystick: false,
            ..Default::default()
        };
        let (x_sub, _) = stick_subtractive.compute_vector(0.0, 0.0, 0.6, 0.8);
        assert!(x_sub > 0 && x_sub < x);
    }

    #[test]
    fn test_square_joystick() {
        let stick_square = StickBinding {
            deadzone: 0.0,
            square_joystick: true,
            ..Default::default()
        };
        // full diagonal hits max on both axes
        let (x, y) = stick_square.compute_vector(1.0, 0.0, 0.0, 1.0);
        assert_eq!(x, 32767);
        assert_eq!(y, -32767);
    }

    #[test]
    fn test_angle_adjustment() {
        let stick_angle = StickBinding {
            deadzone: 0.0,
            angle_adjustment_deg: Some(30.0), // more horizontal than vertical
            ..Default::default()
        };
        let (x, y) = stick_angle.compute_vector(1.0, 0.0, 0.0, 1.0);
        assert!(x.abs() > y.abs());
    }

    #[test]
    fn test_piecewise_curve_presets() {
        let linear = ResponseCurve::Linear;
        assert_eq!(linear.apply(0.0), 20.0 / 255.0);
        assert!((linear.apply(1.0) - 1.0).abs() < 1e-4);

        let instant = ResponseCurve::Instant;
        assert_eq!(instant.apply(0.0), 1.0);
        assert_eq!(instant.apply(0.5), 1.0);

        let custom =
            ResponseCurve::Custom([(20.0, 0.0), (100.0, 50.0), (200.0, 200.0), (240.0, 255.0)]);
        // below start deadzone outputs 0
        assert_eq!(custom.apply(0.05), 0.0);
        // past end deadzone outputs 1
        assert_eq!(custom.apply(0.98), 1.0);
    }

    #[test]
    fn test_required_keys() {
        let engine = GamepadEngine::new(GamepadProfile::default());
        let keys = engine.required_keys();
        assert!(keys.contains(&KeyId::W));
        assert!(keys.contains(&KeyId::S));
        assert!(keys.contains(&KeyId::A));
        assert!(keys.contains(&KeyId::D));
        assert!(keys.contains(&KeyId::Q));
        assert!(keys.contains(&KeyId::E));
        assert!(keys.contains(&KeyId::Space));
    }
}
