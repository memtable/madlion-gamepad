# Madlion Gamepad

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Platform: Linux](https://img.shields.io/badge/Platform-Linux-FCC624?logo=linux&logoColor=white)

Turns Madlion Hall-effect keyboards (MAD60 / MAD60HE) into analog Linux gamepads over USB HID and `/dev/uinput`.
Includes a GTK4 / Libadwaita desktop app for visual remapping and curve tuning, plus a headless CLI daemon.

<p align="center">
<img width="70%" alt="remap" src="https://github.com/user-attachments/assets/a4cfca78-ac8f-4e02-8acf-483f38462802" />
</p>

## Curve Tuning

<p align="center">
<img width="70% alt="curve" src="https://github.com/user-attachments/assets/8d04b6b4-e4ec-4ea7-9d05-3e34935bd707" />
</p>

Tune analog travel (0.10 mm – 3.50 mm) per stick and trigger with 4-point curve editing, presets (`Linear`, `Aggro`, `Slow`, `Smooth`, `Steep`, `Instant`, `Turbo`), deadzones, and joystick dynamics (diagonal angle adjustment, square output, snappy response).

## Requirements

- **Hardware**: Madlion MAD60 / MAD60HE (VID `0x373b`, PID `0x105d`)
- **System package**: `libadwaita`

### Permissions (udev)

To access the device and `/dev/uinput` without root:

```bash
sudo tee /etc/udev/rules.d/99-madlion.rules << 'EOF'
KERNEL=="hidraw*", ATTRS{idVendor}=="373b", MODE="0660", TAG+="uaccess"
KERNEL=="uinput", SUBSYSTEM=="misc", MODE="0660", GROUP="input", TAG+="uaccess"
EOF

sudo udevadm control --reload-rules && sudo udevadm trigger
sudo usermod -aG input $USER
```

## Usage

### Desktop app

```bash
./madlion-gamepad
```

### CLI

```bash
# Run headless daemon
./madlion-gamepad run

# Inspect live switch travel in terminal
./madlion-gamepad inspect

# Benchmark USB polling latency
./madlion-gamepad benchmark

# Switch profiles
./madlion-gamepad profile list
./madlion-gamepad profile switch "Rocket League"
```

## Build from source (optional)

Requires Rust, `gtk4`, `libadwaita`, `cairo`, and `pkg-config`:

```bash
cargo build --release
```

## Credits

Keyboard communication and switch readout protocol referenced from [AnalogSense/universal-analog-plugin](https://github.com/AnalogSense/universal-analog-plugin).

## License

[MIT](LICENSE)
