use std::io;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::keyboard::{KeyId, MadlionKeyboard};

pub mod benchmark;
pub mod daemon;
pub mod inspect;
pub mod profile;

#[derive(Parser, Debug, PartialEq)]
#[command(name = "madlion-gamepad")]
#[command(about = "Madlion raw analog switch reader and gamepad engine", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum Commands {
    /// Launch the native GTK4 / Libadwaita GUI application (default)
    Gui,

    /// List detected Madlion devices and their sysfs / hidraw paths
    List,

    /// Manage configuration profiles
    Profile(ProfileArgs),

    /// Real-time terminal visualizer displaying analog travel bars for keys
    Inspect(InspectArgs),

    /// Run the analog gamepad mapper daemon
    Run(RunArgs),

    /// Benchmark query latency and polling rate against connected keyboard
    Benchmark(BenchmarkArgs),

    /// Toggle mapping override (digital key suppression and RGB indicator)
    #[command(name = "toggle-override", alias = "override")]
    ToggleOverride,

    /// Toggle virtual gamepad emulation on/off
    #[command(name = "toggle-gamepad", alias = "gamepad")]
    ToggleGamepad,

    /// Show current daemon/GUI and profile status
    Status,
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct ProfileArgs {
    #[command(subcommand)]
    pub command: ProfileCommands,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum ProfileCommands {
    /// List all available profiles
    List,

    /// Set the active profile
    Switch {
        /// Profile name to activate
        name: String,
    },

    /// Create a new profile
    Create {
        /// Profile name
        name: String,

        /// Base profile to copy settings from
        #[arg(long)]
        from: Option<String>,
    },

    /// Delete a profile
    Delete {
        /// Profile name to delete
        name: String,
    },

    /// Show profile details
    Show {
        /// Profile name (defaults to active profile)
        name: Option<String>,
    },
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct InspectArgs {
    /// Path to hidraw device (defaults to auto-discovery)
    #[arg(short, long)]
    pub device: Option<PathBuf>,

    /// Comma-separated list of keys to monitor (e.g. "W,A,S,D,Q,E,Space,LShift")
    #[arg(short, long, default_value = "W,A,S,D,Q,E,Space,LShift")]
    pub keys: String,

    /// Target refresh rate in Hz
    #[arg(short, long, default_value_t = 60)]
    pub rate: u64,
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct RunArgs {
    /// Path to hidraw device (defaults to auto-discovery)
    #[arg(short, long)]
    pub device: Option<PathBuf>,

    /// Profile to use (defaults to active profile)
    #[arg(long)]
    pub profile: Option<String>,

    /// Target polling rate in Hz (defaults to saved profile or 1000)
    #[arg(short, long)]
    pub poll_hz: Option<u64>,

    /// Left stick keys (format: Up,Down,Left,Right)
    #[arg(long)]
    pub left_stick: Option<String>,

    /// Left trigger key
    #[arg(long)]
    pub left_trigger: Option<String>,

    /// Right trigger key
    #[arg(long)]
    pub right_trigger: Option<String>,
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct BenchmarkArgs {
    /// Path to hidraw device (defaults to auto-discovery)
    #[arg(short, long)]
    pub device: Option<PathBuf>,

    /// Comma-separated list of keys to query
    #[arg(short, long, default_value = "W,A,S,D")]
    pub keys: String,

    /// Duration of benchmark in seconds
    #[arg(short = 't', long, default_value_t = 3)]
    pub duration: u64,
}

pub fn open_keyboard(custom_path: Option<&PathBuf>) -> io::Result<MadlionKeyboard> {
    if let Some(path) = custom_path {
        MadlionKeyboard::open_path(path)
    } else {
        MadlionKeyboard::auto_discover()
    }
}

pub fn parse_key_list(s: &str) -> Result<Vec<KeyId>, String> {
    let mut list = Vec::new();
    for part in s.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed
            .parse::<KeyId>()
            .map_err(|e| format!("Invalid key '{}': {}", trimmed, e))?;
        if !list.contains(&key) {
            list.push(key);
        }
    }
    Ok(list)
}

pub fn cmd_list() -> Result<(), Box<dyn std::error::Error>> {
    let devices = MadlionKeyboard::find_devices();
    if devices.is_empty() {
        println!("No Madlion analog keyboard interfaces found.");
        println!("Ensure the keyboard is plugged in and udev permissions allow access.");
        return Ok(());
    }

    println!("Detected {} Madlion device interface(s):", devices.len());
    println!("{:-<70}", "");
    for (i, dev) in devices.iter().enumerate() {
        println!("Device #{}:", i + 1);
        println!("  Device Path : {}", dev.dev_path.display());
        println!("  Sysfs Path  : {}", dev.sysfs_path.display());
        println!("  Name        : {}", dev.name);
        println!(
            "  VID:PID     : 0x{:04x}:0x{:04x}",
            dev.vendor_id, dev.product_id
        );
        println!("{:-<70}", "");
    }

    Ok(())
}

pub fn cmd_toggle_override() -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(resp) = crate::ipc::send_ipc_command("toggle-override") {
        println!("{resp}");
        Ok(())
    } else {
        let mut config = crate::config::AppConfig::load();
        let new_val = {
            let prof = config.active_mut();
            prof.mapping_override = !prof.mapping_override;
            prof.mapping_override
        };
        config.save()?;
        println!("Mapping Override toggled in configuration: {new_val}");
        Ok(())
    }
}

pub fn cmd_toggle_gamepad() -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(resp) = crate::ipc::send_ipc_command("toggle-gamepad") {
        println!("{resp}");
        Ok(())
    } else {
        let mut config = crate::config::AppConfig::load();
        let new_val = {
            let prof = config.active_mut();
            prof.gamepad_enabled = !prof.gamepad_enabled;
            prof.gamepad_enabled
        };
        config.save()?;
        println!("Gamepad Enabled toggled in configuration: {new_val}");
        Ok(())
    }
}

pub fn cmd_status() -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(resp) = crate::ipc::send_ipc_command("status") {
        println!("Daemon / GUI Status: Running");
        println!("{resp}");
        Ok(())
    } else {
        let config = crate::config::AppConfig::load();
        let prof = config.active();
        println!("Daemon / GUI Status: Not Running");
        println!(
            "Config Profile: {}, Gamepad: {}, Override: {}, Typing: {}",
            prof.name, prof.gamepad_enabled, prof.mapping_override, prof.enable_typing
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_profile_subcommands_parse() {
        // list
        let cli = Cli::try_parse_from(["madlion-gamepad", "profile", "list"]).unwrap();
        assert_eq!(
            cli.command,
            Some(Commands::Profile(ProfileArgs {
                command: ProfileCommands::List,
            }))
        );

        // switch
        let cli =
            Cli::try_parse_from(["madlion-gamepad", "profile", "switch", "FPS Tactical"]).unwrap();
        assert_eq!(
            cli.command,
            Some(Commands::Profile(ProfileArgs {
                command: ProfileCommands::Switch {
                    name: "FPS Tactical".to_string(),
                },
            }))
        );

        // create without from
        let cli = Cli::try_parse_from(["madlion-gamepad", "profile", "create", "Custom"]).unwrap();
        assert_eq!(
            cli.command,
            Some(Commands::Profile(ProfileArgs {
                command: ProfileCommands::Create {
                    name: "Custom".to_string(),
                    from: None,
                },
            }))
        );

        // create with from
        let cli = Cli::try_parse_from([
            "madlion-gamepad",
            "profile",
            "create",
            "Custom",
            "--from",
            "Racing",
        ])
        .unwrap();
        assert_eq!(
            cli.command,
            Some(Commands::Profile(ProfileArgs {
                command: ProfileCommands::Create {
                    name: "Custom".to_string(),
                    from: Some("Racing".to_string()),
                },
            }))
        );

        // delete
        let cli = Cli::try_parse_from(["madlion-gamepad", "profile", "delete", "Custom"]).unwrap();
        assert_eq!(
            cli.command,
            Some(Commands::Profile(ProfileArgs {
                command: ProfileCommands::Delete {
                    name: "Custom".to_string(),
                },
            }))
        );

        // show without name
        let cli = Cli::try_parse_from(["madlion-gamepad", "profile", "show"]).unwrap();
        assert_eq!(
            cli.command,
            Some(Commands::Profile(ProfileArgs {
                command: ProfileCommands::Show { name: None },
            }))
        );

        // show with name
        let cli = Cli::try_parse_from(["madlion-gamepad", "profile", "show", "Racing"]).unwrap();
        assert_eq!(
            cli.command,
            Some(Commands::Profile(ProfileArgs {
                command: ProfileCommands::Show {
                    name: Some("Racing".to_string()),
                },
            }))
        );

        // toggle override
        let cli = Cli::try_parse_from(["madlion-gamepad", "toggle-override"]).unwrap();
        assert_eq!(cli.command, Some(Commands::ToggleOverride));

        // toggle gamepad
        let cli = Cli::try_parse_from(["madlion-gamepad", "toggle-gamepad"]).unwrap();
        assert_eq!(cli.command, Some(Commands::ToggleGamepad));

        // status
        let cli = Cli::try_parse_from(["madlion-gamepad", "status"]).unwrap();
        assert_eq!(cli.command, Some(Commands::Status));
    }

    #[test]
    fn test_cli_run_args_with_profile() {
        let cli = Cli::try_parse_from([
            "madlion-gamepad",
            "run",
            "--profile",
            "FPS Tactical",
            "-p",
            "500",
        ])
        .unwrap();
        match cli.command {
            Some(Commands::Run(args)) => {
                assert_eq!(args.profile.as_deref(), Some("FPS Tactical"));
                assert_eq!(args.poll_hz, Some(500));
            }
            _ => panic!("Expected Run command"),
        }
    }
}
