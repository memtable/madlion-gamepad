use super::{ProfileArgs, ProfileCommands};

pub fn cmd_profile(args: ProfileArgs) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        ProfileCommands::List => cmd_profile_list(),
        ProfileCommands::Switch { name } => cmd_profile_switch(&name),
        ProfileCommands::Create { name, from } => cmd_profile_create(&name, from.as_deref()),
        ProfileCommands::Delete { name } => cmd_profile_delete(&name),
        ProfileCommands::Show { name } => cmd_profile_show(name.as_deref()),
    }
}

pub fn cmd_profile_list() -> Result<(), Box<dyn std::error::Error>> {
    let config = crate::config::AppConfig::load();
    for p in &config.profiles {
        if p.name == config.active_profile {
            println!("* {}", p.name);
        } else {
            println!("  {}", p.name);
        }
    }
    Ok(())
}

pub fn cmd_profile_switch(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = crate::config::AppConfig::load();
    if config.switch_profile(name) {
        config.save()?;
        println!("Switched active profile to '{}'", name);
        Ok(())
    } else {
        Err(format!("Profile '{}' not found", name).into())
    }
}

pub fn cmd_profile_create(
    name: &str,
    from: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Profile name cannot be empty".into());
    }
    let mut config = crate::config::AppConfig::load();
    if config.get_profile(trimmed).is_some() {
        return Err(format!("Profile '{}' already exists", trimmed).into());
    }

    if let Some(from_name) = from {
        if config.duplicate_profile(from_name, trimmed.to_string()) {
            config.save()?;
            println!("Created profile '{}' copied from '{}'", trimmed, from_name);
        } else if let Some(mut preset) = crate::config::ProfileConfig::from_preset(from_name) {
            preset.name = trimmed.to_string();
            config.add_profile(preset);
            config.save()?;
            println!("Created profile '{}' copied from '{}'", trimmed, from_name);
        } else {
            return Err(format!("Source profile '{}' not found", from_name).into());
        }
    } else {
        let mut new_profile = crate::config::ProfileConfig::default_profile();
        new_profile.name = trimmed.to_string();
        config.add_profile(new_profile);
        config.save()?;
        println!("Created profile '{}'", trimmed);
    }
    Ok(())
}

pub fn cmd_profile_delete(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = crate::config::AppConfig::load();
    if config.get_profile(name).is_none() {
        return Err(format!("Profile '{}' not found", name).into());
    }
    if config.profiles.len() <= 1 {
        return Err("Cannot delete the only profile in configuration".into());
    }
    config.delete_profile(name);
    config.save()?;
    println!("Deleted profile '{}'", name);
    Ok(())
}

pub fn cmd_profile_show(name: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let config = crate::config::AppConfig::load();
    let profile = match name {
        Some(n) => config
            .get_profile(n)
            .ok_or_else(|| format!("Profile '{}' not found", n))?,
        None => config.active(),
    };

    let is_active = profile.name == config.active_profile;
    println!(
        "Profile: {}{}",
        profile.name,
        if is_active { " (active)" } else { "" }
    );
    println!("{:-<50}", "");
    println!("  Gamepad Enabled    : {}", profile.gamepad_enabled);
    println!("  Polling Rate       : {} Hz", profile.polling_rate_hz);
    println!("  Controller Type    : {}", profile.controller_type);
    println!("  Typing Enabled     : {}", profile.enable_typing);
    println!("  Mapping Override   : {}", profile.mapping_override);
    println!("  Poll Only Mapped   : {}", profile.poll_only_mapped_keys);
    println!("  Curve Preset       : {}", profile.curve_preset);
    println!("  Curve Points       : {:?}", profile.curve_points);
    println!("  Square Joystick    : {}", profile.square_joystick);
    println!("  Snappy Joystick    : {}", profile.snappy_joystick);
    println!(
        "  Angle Adjustment   : {} (enabled: {})",
        profile.angle_adjustment, profile.enable_angle_adj
    );
    println!("  Key Bindings ({})   :", profile.key_bindings.len());
    let mut sorted_bindings: Vec<_> = profile.key_bindings.iter().collect();
    sorted_bindings.sort_by_key(|(k, _)| format!("{}", k));
    for (key, action) in sorted_bindings {
        println!("    {:<12} -> {}", format!("{}", key), action);
    }
    println!("{:-<50}", "");
    Ok(())
}
