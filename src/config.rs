use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter};
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize};

use crate::gamepad::{
    ButtonBinding, GamepadButton, GamepadProfile, ResponseCurve, StickBinding, TriggerBinding,
};
use crate::keyboard::KeyId;

fn default_true() -> bool {
    true
}

fn default_polling_rate() -> u64 {
    1000
}

fn default_controller_type() -> String {
    "Xbox Controller".to_string()
}

fn default_curve_preset() -> String {
    "Linear".to_string()
}

fn default_curve_points() -> [(f64, f64); 4] {
    ResponseCurve::Linear
        .to_points()
        .map(|(x, y)| (x as f64, y as f64))
}

fn default_angle_adj() -> f64 {
    45.0
}

fn default_key_bindings() -> HashMap<KeyId, String> {
    let mut key_bindings = HashMap::new();
    key_bindings.insert(KeyId::W, "LS-U".to_string());
    key_bindings.insert(KeyId::S, "LS-D".to_string());
    key_bindings.insert(KeyId::A, "LS-L".to_string());
    key_bindings.insert(KeyId::D, "LS-R".to_string());
    key_bindings.insert(KeyId::Q, "LT".to_string());
    key_bindings.insert(KeyId::E, "RT".to_string());
    key_bindings.insert(KeyId::Space, "A".to_string());
    key_bindings.insert(KeyId::LeftShift, "LB".to_string());
    key_bindings
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnalogCategory {
    LeftStick,
    RightStick,
    LeftTrigger,
    RightTrigger,
}

impl AnalogCategory {
    pub fn display_name(self) -> &'static str {
        match self {
            AnalogCategory::LeftStick => "Left Stick",
            AnalogCategory::RightStick => "Right Stick",
            AnalogCategory::LeftTrigger => "Left Trigger",
            AnalogCategory::RightTrigger => "Right Trigger",
        }
    }

    // figure out analog category from badge code
    pub fn from_badge(badge: &str) -> Option<Self> {
        match badge {
            "LS-U" | "LS-D" | "LS-L" | "LS-R" => Some(AnalogCategory::LeftStick),
            "RS-U" | "RS-D" | "RS-L" | "RS-R" => Some(AnalogCategory::RightStick),
            "LT" => Some(AnalogCategory::LeftTrigger),
            "RT" => Some(AnalogCategory::RightTrigger),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CurveConfig {
    #[serde(default = "default_curve_preset")]
    pub preset: String,
    #[serde(default = "default_curve_points")]
    pub points: [(f64, f64); 4],
}

impl Default for CurveConfig {
    fn default() -> Self {
        Self {
            preset: default_curve_preset(),
            points: default_curve_points(),
        }
    }
}

impl CurveConfig {
    pub fn new(preset: impl Into<String>, points: [(f64, f64); 4]) -> Self {
        Self {
            preset: preset.into(),
            points,
        }
    }

    pub fn to_response_curve(&self) -> ResponseCurve {
        let pts_f32 = self.points.map(|(x, y)| (x as f32, y as f32));
        ResponseCurve::Custom(pts_f32)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileConfig {
    pub name: String,
    #[serde(default = "default_true")]
    pub gamepad_enabled: bool,
    #[serde(default = "default_polling_rate")]
    pub polling_rate_hz: u64,
    #[serde(default = "default_controller_type")]
    pub controller_type: String,
    #[serde(default = "default_true")]
    pub enable_typing: bool,
    #[serde(default)]
    pub mapping_override: bool,
    #[serde(default = "default_key_bindings")]
    pub key_bindings: HashMap<KeyId, String>,
    #[serde(default = "default_curve_preset")]
    pub curve_preset: String,
    #[serde(default = "default_curve_points")]
    pub curve_points: [(f64, f64); 4],
    #[serde(default)]
    pub left_stick_curve: Option<CurveConfig>,
    #[serde(default)]
    pub right_stick_curve: Option<CurveConfig>,
    #[serde(default)]
    pub left_trigger_curve: Option<CurveConfig>,
    #[serde(default)]
    pub right_trigger_curve: Option<CurveConfig>,
    #[serde(default)]
    pub enable_angle_adj: bool,
    #[serde(default = "default_angle_adj")]
    pub angle_adjustment: f64,
    #[serde(default)]
    pub square_joystick: bool,
    #[serde(default = "default_true")]
    pub snappy_joystick: bool,
    #[serde(default)]
    pub toggle_override_key: Option<KeyId>,
    #[serde(default = "default_true")]
    pub poll_only_mapped_keys: bool,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self::default_profile()
    }
}

impl ProfileConfig {
    pub fn default_profile() -> Self {
        Self {
            name: "Default".to_string(),
            gamepad_enabled: true,
            polling_rate_hz: 1000,
            controller_type: "Xbox Controller".to_string(),
            enable_typing: true,
            mapping_override: false,
            key_bindings: default_key_bindings(),
            curve_preset: "Linear".to_string(),
            curve_points: ResponseCurve::Linear
                .to_points()
                .map(|(x, y)| (x as f64, y as f64)),
            left_stick_curve: None,
            right_stick_curve: None,
            left_trigger_curve: None,
            right_trigger_curve: None,
            enable_angle_adj: false,
            angle_adjustment: 45.0,
            square_joystick: false,
            snappy_joystick: true,
            toggle_override_key: None,
            poll_only_mapped_keys: true,
        }
    }

    pub fn fps_profile() -> Self {
        let mut key_bindings = HashMap::new();
        key_bindings.insert(KeyId::W, "LS-U".to_string());
        key_bindings.insert(KeyId::S, "LS-D".to_string());
        key_bindings.insert(KeyId::A, "LS-L".to_string());
        key_bindings.insert(KeyId::D, "LS-R".to_string());
        key_bindings.insert(KeyId::Q, "LT".to_string());
        key_bindings.insert(KeyId::E, "RT".to_string());
        key_bindings.insert(KeyId::Space, "A".to_string());
        key_bindings.insert(KeyId::LeftShift, "LB".to_string());
        key_bindings.insert(KeyId::LeftControl, "B".to_string());
        key_bindings.insert(KeyId::R, "X".to_string());

        Self {
            name: "FPS Tactical".to_string(),
            gamepad_enabled: true,
            polling_rate_hz: 1000,
            controller_type: "Xbox Controller".to_string(),
            enable_typing: true,
            mapping_override: false,
            key_bindings,
            curve_preset: "Aggro".to_string(),
            curve_points: ResponseCurve::Aggro
                .to_points()
                .map(|(x, y)| (x as f64, y as f64)),
            left_stick_curve: None,
            right_stick_curve: None,
            left_trigger_curve: None,
            right_trigger_curve: None,
            enable_angle_adj: false,
            angle_adjustment: 45.0,
            square_joystick: false,
            snappy_joystick: true,
            toggle_override_key: None,
            poll_only_mapped_keys: true,
        }
    }

    pub fn racing_profile() -> Self {
        let mut key_bindings = HashMap::new();
        key_bindings.insert(KeyId::A, "LS-L".to_string());
        key_bindings.insert(KeyId::D, "LS-R".to_string());
        key_bindings.insert(KeyId::W, "RT".to_string());
        key_bindings.insert(KeyId::S, "LT".to_string());
        key_bindings.insert(KeyId::Space, "A".to_string());
        key_bindings.insert(KeyId::LeftShift, "LB".to_string());
        key_bindings.insert(KeyId::E, "RB".to_string());

        Self {
            name: "Racing".to_string(),
            gamepad_enabled: true,
            polling_rate_hz: 1000,
            controller_type: "Xbox Controller".to_string(),
            enable_typing: true,
            mapping_override: false,
            key_bindings,
            curve_preset: "Smooth".to_string(),
            curve_points: ResponseCurve::Smooth
                .to_points()
                .map(|(x, y)| (x as f64, y as f64)),
            left_stick_curve: None,
            right_stick_curve: None,
            left_trigger_curve: None,
            right_trigger_curve: None,
            enable_angle_adj: false,
            angle_adjustment: 45.0,
            square_joystick: false,
            snappy_joystick: false,
            toggle_override_key: None,
            poll_only_mapped_keys: true,
        }
    }

    pub fn rocket_league_profile() -> Self {
        let mut key_bindings = HashMap::new();
        key_bindings.insert(KeyId::W, "RT".to_string());
        key_bindings.insert(KeyId::S, "LT".to_string());
        key_bindings.insert(KeyId::A, "LS-L".to_string());
        key_bindings.insert(KeyId::D, "LS-R".to_string());
        key_bindings.insert(KeyId::Space, "A".to_string());
        key_bindings.insert(KeyId::LeftShift, "X".to_string());
        key_bindings.insert(KeyId::LeftControl, "B".to_string());
        key_bindings.insert(KeyId::Q, "LB".to_string());
        key_bindings.insert(KeyId::E, "RB".to_string());

        Self {
            name: "Rocket League".to_string(),
            gamepad_enabled: true,
            polling_rate_hz: 1000,
            controller_type: "Xbox Controller".to_string(),
            enable_typing: true,
            mapping_override: false,
            key_bindings,
            curve_preset: "Linear".to_string(),
            curve_points: ResponseCurve::Linear
                .to_points()
                .map(|(x, y)| (x as f64, y as f64)),
            left_stick_curve: None,
            right_stick_curve: None,
            left_trigger_curve: None,
            right_trigger_curve: None,
            enable_angle_adj: false,
            angle_adjustment: 45.0,
            square_joystick: true,
            snappy_joystick: true,
            toggle_override_key: None,
            poll_only_mapped_keys: true,
        }
    }

    pub fn from_preset(name: &str) -> Option<Self> {
        let trimmed = name.trim();
        if trimmed.eq_ignore_ascii_case("default") {
            Some(Self::default_profile())
        } else if trimmed.eq_ignore_ascii_case("fps")
            || trimmed.eq_ignore_ascii_case("fps tactical")
            || trimmed.eq_ignore_ascii_case("fpstactical")
        {
            Some(Self::fps_profile())
        } else if trimmed.eq_ignore_ascii_case("racing") {
            Some(Self::racing_profile())
        } else if trimmed.eq_ignore_ascii_case("rocket league")
            || trimmed.eq_ignore_ascii_case("rocketleague")
            || trimmed.eq_ignore_ascii_case("rocket_league")
        {
            Some(Self::rocket_league_profile())
        } else {
            None
        }
    }

    pub fn get_curve(&self, category: AnalogCategory) -> CurveConfig {
        match category {
            AnalogCategory::LeftStick => self
                .left_stick_curve
                .clone()
                .unwrap_or_else(|| self.legacy_curve()),
            AnalogCategory::RightStick => self
                .right_stick_curve
                .clone()
                .unwrap_or_else(|| self.legacy_curve()),
            AnalogCategory::LeftTrigger => self
                .left_trigger_curve
                .clone()
                .unwrap_or_else(|| self.legacy_curve()),
            AnalogCategory::RightTrigger => self
                .right_trigger_curve
                .clone()
                .unwrap_or_else(|| self.legacy_curve()),
        }
    }

    pub fn set_curve(&mut self, category: AnalogCategory, curve: CurveConfig) {
        match category {
            AnalogCategory::LeftStick => self.left_stick_curve = Some(curve),
            AnalogCategory::RightStick => self.right_stick_curve = Some(curve),
            AnalogCategory::LeftTrigger => self.left_trigger_curve = Some(curve),
            AnalogCategory::RightTrigger => self.right_trigger_curve = Some(curve),
        }
    }

    pub fn keys_for_category(&self, category: AnalogCategory) -> Vec<KeyId> {
        self.key_bindings
            .iter()
            .filter_map(|(&key, badge)| {
                if AnalogCategory::from_badge(badge) == Some(category) {
                    Some(key)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn category_for_key(&self, key: KeyId) -> Option<AnalogCategory> {
        self.key_bindings
            .get(&key)
            .and_then(|badge| AnalogCategory::from_badge(badge))
    }

    fn legacy_curve(&self) -> CurveConfig {
        CurveConfig {
            preset: self.curve_preset.clone(),
            points: self.curve_points,
        }
    }

    pub fn to_gamepad_profile(&self) -> GamepadProfile {
        let ls_curve = self
            .get_curve(AnalogCategory::LeftStick)
            .to_response_curve();
        let rs_curve = self
            .get_curve(AnalogCategory::RightStick)
            .to_response_curve();
        let lt_curve = self
            .get_curve(AnalogCategory::LeftTrigger)
            .to_response_curve();
        let rt_curve = self
            .get_curve(AnalogCategory::RightTrigger)
            .to_response_curve();

        // find keys bound to left stick
        let mut ls_up = None;
        let mut ls_down = None;
        let mut ls_left = None;
        let mut ls_right = None;

        // find keys bound to right stick
        let mut rs_up = None;
        let mut rs_down = None;
        let mut rs_left = None;
        let mut rs_right = None;

        // triggers
        let mut lt_key = None;
        let mut rt_key = None;

        // buttons
        let mut buttons = Vec::new();

        for (&key, badge) in &self.key_bindings {
            match badge.as_str() {
                "LS-U" => ls_up = Some(key),
                "LS-D" => ls_down = Some(key),
                "LS-L" => ls_left = Some(key),
                "LS-R" => ls_right = Some(key),
                "RS-U" => rs_up = Some(key),
                "RS-D" => rs_down = Some(key),
                "RS-L" => rs_left = Some(key),
                "RS-R" => rs_right = Some(key),
                "LT" => lt_key = Some(key),
                "RT" => rt_key = Some(key),
                "A" => buttons.push(ButtonBinding::new(key, GamepadButton::A)),
                "B" => buttons.push(ButtonBinding::new(key, GamepadButton::B)),
                "X" => buttons.push(ButtonBinding::new(key, GamepadButton::X)),
                "Y" => buttons.push(ButtonBinding::new(key, GamepadButton::Y)),
                "LB" => buttons.push(ButtonBinding::new(key, GamepadButton::LeftBumper)),
                "RB" => buttons.push(ButtonBinding::new(key, GamepadButton::RightBumper)),
                "L3" => buttons.push(ButtonBinding::new(key, GamepadButton::LeftThumb)),
                "R3" => buttons.push(ButtonBinding::new(key, GamepadButton::RightThumb)),
                "Start" => buttons.push(ButtonBinding::new(key, GamepadButton::Start)),
                "Back" => buttons.push(ButtonBinding::new(key, GamepadButton::Select)),
                "Guide" => buttons.push(ButtonBinding::new(key, GamepadButton::Mode)),
                "D-U" => buttons.push(ButtonBinding::new(key, GamepadButton::DPadUp)),
                "D-D" => buttons.push(ButtonBinding::new(key, GamepadButton::DPadDown)),
                "D-L" => buttons.push(ButtonBinding::new(key, GamepadButton::DPadLeft)),
                "D-R" => buttons.push(ButtonBinding::new(key, GamepadButton::DPadRight)),
                _ => {}
            }
        }

        let left_stick =
            if ls_up.is_some() || ls_down.is_some() || ls_left.is_some() || ls_right.is_some() {
                Some(StickBinding {
                    up: ls_up,
                    down: ls_down,
                    left: ls_left,
                    right: ls_right,
                    deadzone: 0.05,
                    curve: ls_curve,
                    square_joystick: self.square_joystick,
                    snappy_joystick: self.snappy_joystick,
                    angle_adjustment_deg: if self.enable_angle_adj {
                        Some(self.angle_adjustment as f32)
                    } else {
                        None
                    },
                })
            } else {
                None
            };

        let right_stick =
            if rs_up.is_some() || rs_down.is_some() || rs_left.is_some() || rs_right.is_some() {
                Some(StickBinding {
                    up: rs_up,
                    down: rs_down,
                    left: rs_left,
                    right: rs_right,
                    deadzone: 0.05,
                    curve: rs_curve,
                    square_joystick: self.square_joystick,
                    snappy_joystick: self.snappy_joystick,
                    angle_adjustment_deg: if self.enable_angle_adj {
                        Some(self.angle_adjustment as f32)
                    } else {
                        None
                    },
                })
            } else {
                None
            };

        let left_trigger = lt_key.map(|key| TriggerBinding {
            key,
            deadzone: 0.05,
            curve: lt_curve,
            max_value: 255,
        });

        let right_trigger = rt_key.map(|key| TriggerBinding {
            key,
            deadzone: 0.05,
            curve: rt_curve,
            max_value: 255,
        });

        GamepadProfile {
            left_stick,
            right_stick,
            left_trigger,
            right_trigger,
            buttons,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AppConfig {
    pub active_profile: String,
    pub profiles: Vec<ProfileConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            active_profile: "Default".to_string(),
            profiles: vec![
                ProfileConfig::default_profile(),
                ProfileConfig::fps_profile(),
                ProfileConfig::racing_profile(),
                ProfileConfig::rocket_league_profile(),
            ],
        }
    }
}

impl Deref for AppConfig {
    type Target = ProfileConfig;

    fn deref(&self) -> &Self::Target {
        self.active()
    }
}

impl DerefMut for AppConfig {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.active_mut()
    }
}

impl<'de> Deserialize<'de> for AppConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ConfigHelper {
            Multi {
                #[serde(default)]
                active_profile: Option<String>,
                profiles: Vec<ProfileConfig>,
            },
            Legacy(LegacyProfileHelper),
        }

        #[derive(Deserialize)]
        struct LegacyProfileHelper {
            #[serde(default)]
            name: Option<String>,
            #[serde(default = "default_true")]
            gamepad_enabled: bool,
            #[serde(default = "default_polling_rate")]
            polling_rate_hz: u64,
            #[serde(default = "default_controller_type")]
            controller_type: String,
            #[serde(default = "default_true")]
            enable_typing: bool,
            #[serde(default)]
            mapping_override: bool,
            #[serde(default = "default_key_bindings")]
            key_bindings: HashMap<KeyId, String>,
            #[serde(default = "default_curve_preset")]
            curve_preset: String,
            #[serde(default = "default_curve_points")]
            curve_points: [(f64, f64); 4],
            #[serde(default)]
            enable_angle_adj: bool,
            #[serde(default = "default_angle_adj")]
            angle_adjustment: f64,
            #[serde(default)]
            square_joystick: bool,
            #[serde(default = "default_true")]
            snappy_joystick: bool,
            #[serde(default)]
            toggle_override_key: Option<KeyId>,
            #[serde(default = "default_true")]
            poll_only_mapped_keys: bool,
        }

        let helper = ConfigHelper::deserialize(deserializer)?;
        match helper {
            ConfigHelper::Multi {
                active_profile,
                profiles,
            } => {
                if profiles.is_empty() {
                    return Ok(AppConfig::default());
                }
                let active = active_profile
                    .filter(|name| profiles.iter().any(|p| &p.name == name))
                    .unwrap_or_else(|| profiles[0].name.clone());
                Ok(AppConfig {
                    active_profile: active,
                    profiles,
                })
            }
            ConfigHelper::Legacy(leg) => {
                let name = leg.name.unwrap_or_else(|| "Default".to_string());
                let profile = ProfileConfig {
                    name: name.clone(),
                    gamepad_enabled: leg.gamepad_enabled,
                    polling_rate_hz: leg.polling_rate_hz,
                    controller_type: leg.controller_type,
                    enable_typing: leg.enable_typing,
                    mapping_override: leg.mapping_override,
                    key_bindings: leg.key_bindings,
                    curve_preset: leg.curve_preset,
                    curve_points: leg.curve_points,
                    left_stick_curve: None,
                    right_stick_curve: None,
                    left_trigger_curve: None,
                    right_trigger_curve: None,
                    enable_angle_adj: leg.enable_angle_adj,
                    angle_adjustment: leg.angle_adjustment,
                    square_joystick: leg.square_joystick,
                    snappy_joystick: leg.snappy_joystick,
                    toggle_override_key: leg.toggle_override_key,
                    poll_only_mapped_keys: leg.poll_only_mapped_keys,
                };
                Ok(AppConfig {
                    active_profile: name,
                    profiles: vec![profile],
                })
            }
        }
    }
}

impl AppConfig {
    pub fn config_dir() -> PathBuf {
        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME")
            && !xdg_config.trim().is_empty()
        {
            return PathBuf::from(xdg_config).join("madlion-gamepad");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".config").join("madlion-gamepad");
        }
        PathBuf::from(".config").join("madlion-gamepad")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("profile.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if !path.exists() {
            return Self::default();
        }

        match File::open(&path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                match serde_json::from_reader(reader) {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to parse config file {}: {}. Using defaults.",
                            path.display(),
                            e
                        );
                        Self::default()
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "Warning: Failed to open config file {}: {}. Using defaults.",
                    path.display(),
                    e
                );
                Self::default()
            }
        }
    }

    pub fn save(&self) -> io::Result<()> {
        let dir = Self::config_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        let path = Self::config_path();
        let file = File::create(&path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, self)?;
        Ok(())
    }

    pub fn active(&self) -> &ProfileConfig {
        self.get_profile(&self.active_profile)
            .or_else(|| self.profiles.first())
            .expect("AppConfig must have at least one profile")
    }

    pub fn active_mut(&mut self) -> &mut ProfileConfig {
        if let Some(pos) = self
            .profiles
            .iter()
            .position(|p| p.name == self.active_profile)
        {
            &mut self.profiles[pos]
        } else if !self.profiles.is_empty() {
            &mut self.profiles[0]
        } else {
            panic!("AppConfig must have at least one profile")
        }
    }

    pub fn get_profile(&self, name: &str) -> Option<&ProfileConfig> {
        self.profiles.iter().find(|p| p.name == name)
    }

    pub fn get_profile_mut(&mut self, name: &str) -> Option<&mut ProfileConfig> {
        self.profiles.iter_mut().find(|p| p.name == name)
    }

    pub fn switch_profile(&mut self, name: &str) -> bool {
        if self.profiles.iter().any(|p| p.name == name) {
            self.active_profile = name.to_string();
            true
        } else {
            false
        }
    }

    pub fn add_profile(&mut self, profile: ProfileConfig) -> bool {
        if profile.name.is_empty() || self.profiles.iter().any(|p| p.name == profile.name) {
            return false;
        }
        self.profiles.push(profile);
        true
    }

    pub fn duplicate_profile(&mut self, name: &str, new_name: String) -> bool {
        if new_name.is_empty() || self.profiles.iter().any(|p| p.name == new_name) {
            return false;
        }
        if let Some(profile) = self.get_profile(name) {
            let mut cloned = profile.clone();
            cloned.name = new_name;
            self.profiles.push(cloned);
            true
        } else {
            false
        }
    }

    pub fn delete_profile(&mut self, name: &str) -> bool {
        if self.profiles.len() <= 1 {
            return false;
        }
        if let Some(pos) = self.profiles.iter().position(|p| p.name == name) {
            self.profiles.remove(pos);
            if self.active_profile == name {
                self.active_profile = self.profiles[0].name.clone();
            }
            true
        } else {
            false
        }
    }

    pub fn rename_profile(&mut self, old_name: &str, new_name: String) -> bool {
        if new_name.is_empty() {
            return false;
        }
        if old_name == new_name {
            return true;
        }
        if self.profiles.iter().any(|p| p.name == new_name) {
            return false;
        }
        if let Some(p) = self.get_profile_mut(old_name) {
            p.name = new_name.clone();
            if self.active_profile == old_name {
                self.active_profile = new_name;
            }
            true
        } else {
            false
        }
    }

    pub fn to_gamepad_profile(&self) -> GamepadProfile {
        self.active().to_gamepad_profile()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamepad::GamepadEngine;
    use crate::keyboard::AnalogReading;

    #[test]
    fn test_default_config_conversion() {
        let config = AppConfig::default();
        let profile = config.to_gamepad_profile();

        assert!(profile.left_stick.is_some());
        let ls = profile.left_stick.unwrap();
        assert_eq!(ls.up, Some(KeyId::W));
        assert_eq!(ls.down, Some(KeyId::S));
        assert_eq!(ls.left, Some(KeyId::A));
        assert_eq!(ls.right, Some(KeyId::D));
        assert!(ls.snappy_joystick);
        assert!(!ls.square_joystick);

        assert!(profile.left_trigger.is_some());
        assert_eq!(profile.left_trigger.unwrap().key, KeyId::Q);

        assert!(profile.right_trigger.is_some());
        assert_eq!(profile.right_trigger.unwrap().key, KeyId::E);

        assert!(
            profile
                .buttons
                .iter()
                .any(|b| b.key == KeyId::Space && b.button == GamepadButton::A)
        );
        assert!(
            profile
                .buttons
                .iter()
                .any(|b| b.key == KeyId::LeftShift && b.button == GamepadButton::LeftBumper)
        );
    }

    #[test]
    fn test_preset_profiles_creation() {
        let app_config = AppConfig::default();
        assert_eq!(app_config.active_profile, "Default");
        assert_eq!(app_config.profiles.len(), 4);

        let default_prof = app_config.get_profile("Default").unwrap();
        assert_eq!(default_prof.name, "Default");
        assert_eq!(default_prof.curve_preset, "Linear");
        assert!(!default_prof.square_joystick);
        assert!(default_prof.snappy_joystick);

        let fps_prof = app_config.get_profile("FPS Tactical").unwrap();
        assert_eq!(fps_prof.name, "FPS Tactical");
        assert_eq!(fps_prof.curve_preset, "Aggro");
        assert!(!fps_prof.square_joystick);
        assert!(fps_prof.snappy_joystick);
        assert_eq!(
            fps_prof.key_bindings.get(&KeyId::W),
            Some(&"LS-U".to_string())
        );
        assert_eq!(
            fps_prof.key_bindings.get(&KeyId::LeftControl),
            Some(&"B".to_string())
        );

        let racing_prof = app_config.get_profile("Racing").unwrap();
        assert_eq!(racing_prof.name, "Racing");
        assert_eq!(racing_prof.curve_preset, "Smooth");
        assert!(!racing_prof.square_joystick);
        assert!(!racing_prof.snappy_joystick);
        assert_eq!(
            racing_prof.key_bindings.get(&KeyId::W),
            Some(&"RT".to_string())
        );
        assert_eq!(
            racing_prof.key_bindings.get(&KeyId::S),
            Some(&"LT".to_string())
        );
        assert_eq!(
            racing_prof.key_bindings.get(&KeyId::A),
            Some(&"LS-L".to_string())
        );
        assert_eq!(
            racing_prof.key_bindings.get(&KeyId::D),
            Some(&"LS-R".to_string())
        );

        let rl_prof = app_config.get_profile("Rocket League").unwrap();
        assert_eq!(rl_prof.name, "Rocket League");
        assert!(rl_prof.square_joystick);
        assert!(rl_prof.snappy_joystick);
        assert_eq!(rl_prof.key_bindings.get(&KeyId::W), Some(&"RT".to_string()));
        assert_eq!(rl_prof.key_bindings.get(&KeyId::S), Some(&"LT".to_string()));

        assert!(ProfileConfig::from_preset("Racing").is_some());
        assert!(ProfileConfig::from_preset("FPS Tactical").is_some());
        assert!(ProfileConfig::from_preset("Rocket League").is_some());
        assert!(ProfileConfig::from_preset("Default").is_some());
        assert!(ProfileConfig::from_preset("NonExistent").is_none());
    }

    #[test]
    fn test_profile_switching_duplication_deletion_renaming() {
        let mut app_config = AppConfig::default();

        // switch profile
        assert!(app_config.switch_profile("FPS Tactical"));
        assert_eq!(app_config.active_profile, "FPS Tactical");
        assert_eq!(app_config.active().name, "FPS Tactical");
        assert_eq!(app_config.curve_preset, "Aggro"); // via Deref
        assert!(!app_config.switch_profile("NonExistentProfile"));
        assert_eq!(app_config.active_profile, "FPS Tactical");

        // duplicate profile
        assert!(app_config.duplicate_profile("FPS Tactical", "FPS Custom".to_string()));
        assert_eq!(app_config.profiles.len(), 5);
        let dup = app_config.get_profile("FPS Custom").unwrap();
        assert_eq!(dup.curve_preset, "Aggro");
        assert_eq!(dup.name, "FPS Custom");
        // cant duplicate with existing or empty name
        assert!(!app_config.duplicate_profile("FPS Tactical", "FPS Custom".to_string()));
        assert!(!app_config.duplicate_profile("FPS Tactical", "".to_string()));
        assert!(!app_config.duplicate_profile("NonExistent", "UniqueName".to_string()));

        // rename profile
        assert!(app_config.switch_profile("FPS Custom"));
        assert!(app_config.rename_profile("FPS Custom", "FPS Competitive".to_string()));
        assert_eq!(app_config.active_profile, "FPS Competitive");
        assert!(app_config.get_profile("FPS Competitive").is_some());
        assert!(app_config.get_profile("FPS Custom").is_none());
        // renaming to same name is fine
        assert!(app_config.rename_profile("FPS Competitive", "FPS Competitive".to_string()));
        // cant rename to existing name
        assert!(!app_config.rename_profile("FPS Competitive", "Default".to_string()));

        // add profile
        let mut custom = ProfileConfig::default_profile();
        custom.name = "My Profile".to_string();
        assert!(app_config.add_profile(custom.clone()));
        assert!(!app_config.add_profile(custom));

        // delete profile
        assert!(app_config.switch_profile("FPS Competitive"));
        assert!(app_config.delete_profile("FPS Competitive"));
        assert_eq!(app_config.get_profile("FPS Competitive"), None);
        // active profile falls back to default
        assert_eq!(app_config.active_profile, "Default");

        // delete all down to 1
        assert!(app_config.delete_profile("FPS Tactical"));
        assert!(app_config.delete_profile("Racing"));
        assert!(app_config.delete_profile("Rocket League"));
        assert!(app_config.delete_profile("My Profile"));
        assert_eq!(app_config.profiles.len(), 1);
        // cant delete last profile
        assert!(!app_config.delete_profile("Default"));
        assert_eq!(app_config.profiles.len(), 1);
    }

    #[test]
    fn test_partial_stick_binding_racing_layout() {
        // w = rt, s = lt, a = ls-l, d = ls-r
        let mut key_bindings = HashMap::new();
        key_bindings.insert(KeyId::A, "LS-L".to_string());
        key_bindings.insert(KeyId::D, "LS-R".to_string());
        key_bindings.insert(KeyId::W, "RT".to_string());
        key_bindings.insert(KeyId::S, "LT".to_string());

        let profile_cfg = ProfileConfig {
            name: "Racing Custom".to_string(),
            key_bindings,
            ..ProfileConfig::default()
        };
        let profile = profile_cfg.to_gamepad_profile();

        assert!(profile.left_stick.is_some());
        let ls = profile.left_stick.as_ref().unwrap();
        assert_eq!(ls.up, None);
        assert_eq!(ls.down, None);
        assert_eq!(ls.left, Some(KeyId::A));
        assert_eq!(ls.right, Some(KeyId::D));

        assert!(profile.right_trigger.is_some());
        assert_eq!(profile.right_trigger.as_ref().unwrap().key, KeyId::W);

        assert!(profile.left_trigger.is_some());
        assert_eq!(profile.left_trigger.as_ref().unwrap().key, KeyId::S);

        // test engine run with profile
        let mut engine = GamepadEngine::new(profile);
        let req = engine.required_keys();
        assert!(req.contains(&KeyId::A));
        assert!(req.contains(&KeyId::D));
        assert!(req.contains(&KeyId::W));
        assert!(req.contains(&KeyId::S));

        // press a at half depth
        let readings = vec![
            AnalogReading::new(KeyId::A, 29, 175),
            AnalogReading::new(KeyId::D, 31, 0),
            AnalogReading::new(KeyId::W, 16, 350), // 100% rt
            AnalogReading::new(KeyId::S, 30, 0),   // 0% lt
        ];
        let state = engine.update(&readings);
        assert!(state.left_stick_x < -10000); // steer left
        assert_eq!(state.left_stick_y, 0);
        assert_eq!(state.right_trigger, 255);
        assert_eq!(state.left_trigger, 0);
    }

    #[test]
    fn test_legacy_single_profile_migration() {
        let legacy_json = r#"{
            "gamepad_enabled": true,
            "polling_rate_hz": 500,
            "controller_type": "DualShock 4",
            "enable_typing": false,
            "mapping_override": true,
            "key_bindings": {
                "W": "LS-U",
                "S": "LS-D",
                "A": "LS-L",
                "D": "LS-R"
            },
            "curve_preset": "Aggro",
            "curve_points": [[0.0, 33.0], [63.0, 114.0], [139.0, 231.0], [253.0, 255.0]],
            "enable_angle_adj": true,
            "angle_adjustment": 30.0,
            "square_joystick": true,
            "snappy_joystick": false
        }"#;

        let migrated: AppConfig = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(migrated.active_profile, "Default");
        assert_eq!(migrated.profiles.len(), 1);

        let prof = migrated.active();
        assert_eq!(prof.name, "Default");
        assert!(prof.gamepad_enabled);
        assert_eq!(prof.polling_rate_hz, 500);
        assert_eq!(prof.controller_type, "DualShock 4");
        assert!(!prof.enable_typing);
        assert!(prof.mapping_override);
        assert_eq!(prof.curve_preset, "Aggro");
        assert!(prof.enable_angle_adj);
        assert_eq!(prof.angle_adjustment, 30.0);
        assert!(prof.square_joystick);
        assert!(!prof.snappy_joystick);
        assert_eq!(prof.key_bindings.len(), 4);
    }

    #[test]
    fn test_config_json_roundtrip() {
        let mut config = AppConfig::default();
        config.switch_profile("FPS Tactical");
        config.active_mut().polling_rate_hz = 2000;

        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
        assert_eq!(parsed.active_profile, "FPS Tactical");
        assert_eq!(parsed.active().polling_rate_hz, 2000);
        assert_eq!(parsed.profiles.len(), 4);
    }

    #[test]
    fn test_per_category_analog_curves() {
        let mut prof = ProfileConfig::default_profile();

        // check fallback to default curve when category curve is none
        assert_eq!(prof.get_curve(AnalogCategory::LeftStick).preset, "Linear");
        assert_eq!(prof.get_curve(AnalogCategory::RightStick).preset, "Linear");

        // set distinct curves for categories
        prof.set_curve(
            AnalogCategory::LeftStick,
            CurveConfig::new(
                "Aggro",
                ResponseCurve::Aggro
                    .to_points()
                    .map(|(x, y)| (x as f64, y as f64)),
            ),
        );
        prof.set_curve(
            AnalogCategory::LeftTrigger,
            CurveConfig::new(
                "Smooth",
                ResponseCurve::Smooth
                    .to_points()
                    .map(|(x, y)| (x as f64, y as f64)),
            ),
        );

        assert_eq!(prof.get_curve(AnalogCategory::LeftStick).preset, "Aggro");
        assert_eq!(prof.get_curve(AnalogCategory::LeftTrigger).preset, "Smooth");
        assert_eq!(prof.get_curve(AnalogCategory::RightStick).preset, "Linear");

        // verify key lookup by category n vice versa
        let ls_keys = prof.keys_for_category(AnalogCategory::LeftStick);
        assert!(ls_keys.contains(&KeyId::W));
        assert!(ls_keys.contains(&KeyId::A));
        assert!(ls_keys.contains(&KeyId::S));
        assert!(ls_keys.contains(&KeyId::D));
        assert_eq!(
            prof.category_for_key(KeyId::W),
            Some(AnalogCategory::LeftStick)
        );
        assert_eq!(
            prof.category_for_key(KeyId::Q),
            Some(AnalogCategory::LeftTrigger)
        );
        assert_eq!(prof.category_for_key(KeyId::Space), None);

        // check to_gamepad_profile builds separate curves
        let gp = prof.to_gamepad_profile();
        let ls = gp.left_stick.unwrap();
        let lt = gp.left_trigger.unwrap();
        assert_eq!(
            ls.curve,
            ResponseCurve::Custom(ResponseCurve::Aggro.to_points())
        );
        assert_eq!(
            lt.curve,
            ResponseCurve::Custom(ResponseCurve::Smooth.to_points())
        );
    }
}
