use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;

const SYN_REPORT: u16 = 0x00;

const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
const ABS_Z: u16 = 0x02;
const ABS_RX: u16 = 0x03;
const ABS_RY: u16 = 0x04;
const ABS_RZ: u16 = 0x05;

const BTN_A: u16 = 0x130;
const BTN_B: u16 = 0x131;
const BTN_X: u16 = 0x133;
const BTN_Y: u16 = 0x134;
const BTN_TL: u16 = 0x136;
const BTN_TR: u16 = 0x137;
const BTN_SELECT: u16 = 0x13a;
const BTN_START: u16 = 0x13b;
const BTN_MODE: u16 = 0x13c;
const BTN_THUMBL: u16 = 0x13d;
const BTN_THUMBR: u16 = 0x13e;
const BTN_DPAD_UP: u16 = 0x220;
const BTN_DPAD_DOWN: u16 = 0x221;
const BTN_DPAD_LEFT: u16 = 0x222;
const BTN_DPAD_RIGHT: u16 = 0x223;

const UI_DEV_CREATE: libc::c_ulong = 0x5501;
const UI_DEV_DESTROY: libc::c_ulong = 0x5502;
const UI_SET_EVBIT: libc::c_ulong = 0x40045564;
const UI_SET_KEYBIT: libc::c_ulong = 0x40045565;
const UI_SET_ABSBIT: libc::c_ulong = 0x40045567;

const BUS_USB: u16 = 0x03;
const UINPUT_MAX_NAME_SIZE: usize = 80;
const ABS_CNT: usize = 64;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

#[repr(C)]
struct UinputUserDev {
    name: [libc::c_char; UINPUT_MAX_NAME_SIZE],
    id: InputId,
    ff_effects_max: libc::c_int,
    absmax: [libc::c_int; ABS_CNT],
    absmin: [libc::c_int; ABS_CNT],
    absfuzz: [libc::c_int; ABS_CNT],
    absflat: [libc::c_int; ABS_CNT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    A,
    B,
    X,
    Y,
    LeftBumper,
    RightBumper,
    Select,
    Start,
    Mode,
    LeftThumb,
    RightThumb,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

impl GamepadButton {
    pub fn to_event_code(self) -> u16 {
        match self {
            GamepadButton::A => BTN_A,
            GamepadButton::B => BTN_B,
            GamepadButton::X => BTN_X,
            GamepadButton::Y => BTN_Y,
            GamepadButton::LeftBumper => BTN_TL,
            GamepadButton::RightBumper => BTN_TR,
            GamepadButton::Select => BTN_SELECT,
            GamepadButton::Start => BTN_START,
            GamepadButton::Mode => BTN_MODE,
            GamepadButton::LeftThumb => BTN_THUMBL,
            GamepadButton::RightThumb => BTN_THUMBR,
            GamepadButton::DPadUp => BTN_DPAD_UP,
            GamepadButton::DPadDown => BTN_DPAD_DOWN,
            GamepadButton::DPadLeft => BTN_DPAD_LEFT,
            GamepadButton::DPadRight => BTN_DPAD_RIGHT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadAxis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
    LeftTrigger,
    RightTrigger,
}

impl GamepadAxis {
    pub fn to_event_code(self) -> u16 {
        match self {
            GamepadAxis::LeftStickX => ABS_X,
            GamepadAxis::LeftStickY => ABS_Y,
            GamepadAxis::LeftTrigger => ABS_Z,
            GamepadAxis::RightStickX => ABS_RX,
            GamepadAxis::RightStickY => ABS_RY,
            GamepadAxis::RightTrigger => ABS_RZ,
        }
    }
}

pub struct UinputGamepad {
    file: File,
    created: bool,
}

impl UinputGamepad {
    pub fn new() -> io::Result<Self> {
        let uinput_path = if Path::new("/dev/uinput").exists() {
            "/dev/uinput"
        } else if Path::new("/dev/input/uinput").exists() {
            "/dev/input/uinput"
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "uinput device not found in /dev/uinput or /dev/input/uinput",
            ));
        };

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(uinput_path)?;

        let fd = file.as_raw_fd();

        unsafe {
            // enable event types
            Self::ioctl_set(fd, UI_SET_EVBIT, EV_SYN as libc::c_int)?;
            Self::ioctl_set(fd, UI_SET_EVBIT, EV_KEY as libc::c_int)?;
            Self::ioctl_set(fd, UI_SET_EVBIT, EV_ABS as libc::c_int)?;

            // enable buttons
            let buttons = [
                BTN_A,
                BTN_B,
                BTN_X,
                BTN_Y,
                BTN_TL,
                BTN_TR,
                BTN_SELECT,
                BTN_START,
                BTN_MODE,
                BTN_THUMBL,
                BTN_THUMBR,
                BTN_DPAD_UP,
                BTN_DPAD_DOWN,
                BTN_DPAD_LEFT,
                BTN_DPAD_RIGHT,
            ];
            for &btn in &buttons {
                Self::ioctl_set(fd, UI_SET_KEYBIT, btn as libc::c_int)?;
            }

            // enable axes
            let axes = [ABS_X, ABS_Y, ABS_Z, ABS_RX, ABS_RY, ABS_RZ];
            for &axis in &axes {
                Self::ioctl_set(fd, UI_SET_ABSBIT, axis as libc::c_int)?;
            }
        }

        // set up device info
        let mut user_dev: UinputUserDev = unsafe { std::mem::zeroed() };
        let name_bytes = b"Madlion Virtual Gamepad\0";
        for (i, &b) in name_bytes.iter().enumerate() {
            if i < UINPUT_MAX_NAME_SIZE {
                user_dev.name[i] = b as libc::c_char;
            }
        }

        user_dev.id = InputId {
            bustype: BUS_USB,
            vendor: 0x373b,
            product: 0x105d,
            version: 1,
        };

        // thumbsticks range -32767..32767
        for &axis in &[ABS_X, ABS_Y, ABS_RX, ABS_RY] {
            let a = axis as usize;
            user_dev.absmin[a] = -32767;
            user_dev.absmax[a] = 32767;
            user_dev.absfuzz[a] = 0;
            user_dev.absflat[a] = 0;
        }

        // triggers range 0..255
        for &axis in &[ABS_Z, ABS_RZ] {
            let a = axis as usize;
            user_dev.absmin[a] = 0;
            user_dev.absmax[a] = 255;
            user_dev.absfuzz[a] = 0;
            user_dev.absflat[a] = 0;
        }

        let user_dev_slice = unsafe {
            std::slice::from_raw_parts(
                &user_dev as *const _ as *const u8,
                std::mem::size_of::<UinputUserDev>(),
            )
        };

        let mut dev = Self {
            file,
            created: false,
        };

        dev.file.write_all(user_dev_slice)?;
        dev.file.flush()?;

        unsafe {
            if libc::ioctl(fd, UI_DEV_CREATE) < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        dev.created = true;
        Ok(dev)
    }

    unsafe fn ioctl_set(
        fd: libc::c_int,
        request: libc::c_ulong,
        value: libc::c_int,
    ) -> io::Result<()> {
        let ret = unsafe { libc::ioctl(fd, request, value) };
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub fn write_event(&mut self, type_: u16, code: u16, value: i32) -> io::Result<()> {
        let mut ev: libc::input_event = unsafe { std::mem::zeroed() };
        ev.type_ = type_;
        ev.code = code;
        ev.value = value;

        let ev_slice = unsafe {
            std::slice::from_raw_parts(
                &ev as *const _ as *const u8,
                std::mem::size_of::<libc::input_event>(),
            )
        };

        self.file.write_all(ev_slice)
    }

    pub fn send_button(&mut self, button: GamepadButton, pressed: bool) -> io::Result<()> {
        self.write_event(EV_KEY, button.to_event_code(), if pressed { 1 } else { 0 })
    }

    pub fn send_axis(&mut self, axis: GamepadAxis, value: i32) -> io::Result<()> {
        self.write_event(EV_ABS, axis.to_event_code(), value)
    }

    pub fn sync(&mut self) -> io::Result<()> {
        self.write_event(EV_SYN, SYN_REPORT, 0)?;
        self.file.flush()
    }
}

impl Drop for UinputGamepad {
    fn drop(&mut self) {
        if self.created {
            let fd = self.file.as_raw_fd();
            unsafe {
                libc::ioctl(fd, UI_DEV_DESTROY);
            }
            self.created = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_event_codes() {
        assert_eq!(GamepadButton::A.to_event_code(), BTN_A);
        assert_eq!(GamepadButton::B.to_event_code(), BTN_B);
        assert_eq!(GamepadButton::X.to_event_code(), BTN_X);
        assert_eq!(GamepadButton::Y.to_event_code(), BTN_Y);
        assert_eq!(GamepadButton::LeftBumper.to_event_code(), BTN_TL);
        assert_eq!(GamepadButton::RightBumper.to_event_code(), BTN_TR);
        assert_eq!(GamepadButton::DPadUp.to_event_code(), BTN_DPAD_UP);
    }

    #[test]
    fn test_axis_event_codes() {
        assert_eq!(GamepadAxis::LeftStickX.to_event_code(), ABS_X);
        assert_eq!(GamepadAxis::LeftStickY.to_event_code(), ABS_Y);
        assert_eq!(GamepadAxis::LeftTrigger.to_event_code(), ABS_Z);
        assert_eq!(GamepadAxis::RightStickX.to_event_code(), ABS_RX);
        assert_eq!(GamepadAxis::RightStickY.to_event_code(), ABS_RY);
        assert_eq!(GamepadAxis::RightTrigger.to_event_code(), ABS_RZ);
    }
}
