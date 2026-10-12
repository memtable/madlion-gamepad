use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use super::layout::KeyId;

const EVIOCGRAB: libc::c_ulong = 0x40044590;

const UI_DEV_CREATE: libc::c_ulong = 0x5501;
const UI_DEV_DESTROY: libc::c_ulong = 0x5502;
const UI_SET_EVBIT: libc::c_ulong = 0x40045564;
const UI_SET_KEYBIT: libc::c_ulong = 0x40045565;
const UI_SET_MSCBIT: libc::c_ulong = 0x40045568;
const UI_SET_LEDBIT: libc::c_ulong = 0x40045569;

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_MSC: u16 = 0x04;
const EV_LED: u16 = 0x11;
const EV_REP: u16 = 0x14;

const SYN_REPORT: u16 = 0x00;
const BUS_USB: u16 = 0x03;
const UINPUT_MAX_NAME_SIZE: usize = 80;

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
    absmax: [libc::c_int; 64],
    absmin: [libc::c_int; 64],
    absfuzz: [libc::c_int; 64],
    absflat: [libc::c_int; 64],
}

pub fn key_id_to_linux_keycode(key: KeyId) -> Option<u16> {
    match key {
        KeyId::Esc => Some(1),
        KeyId::Key1 => Some(2),
        KeyId::Key2 => Some(3),
        KeyId::Key3 => Some(4),
        KeyId::Key4 => Some(5),
        KeyId::Key5 => Some(6),
        KeyId::Key6 => Some(7),
        KeyId::Key7 => Some(8),
        KeyId::Key8 => Some(9),
        KeyId::Key9 => Some(10),
        KeyId::Key0 => Some(11),
        KeyId::Minus => Some(12),
        KeyId::Equal => Some(13),
        KeyId::Backspace => Some(14),
        KeyId::Tab => Some(15),
        KeyId::Q => Some(16),
        KeyId::W => Some(17),
        KeyId::E => Some(18),
        KeyId::R => Some(19),
        KeyId::T => Some(20),
        KeyId::Y => Some(21),
        KeyId::U => Some(22),
        KeyId::I => Some(23),
        KeyId::O => Some(24),
        KeyId::P => Some(25),
        KeyId::LeftBracket => Some(26),
        KeyId::RightBracket => Some(27),
        KeyId::Enter => Some(28),
        KeyId::LeftControl => Some(29),
        KeyId::A => Some(30),
        KeyId::S => Some(31),
        KeyId::D => Some(32),
        KeyId::F => Some(33),
        KeyId::G => Some(34),
        KeyId::H => Some(35),
        KeyId::J => Some(36),
        KeyId::K => Some(37),
        KeyId::L => Some(38),
        KeyId::Semicolon => Some(39),
        KeyId::Quote => Some(40),
        KeyId::LeftShift => Some(42),
        KeyId::Backslash => Some(43),
        KeyId::Z => Some(44),
        KeyId::X => Some(45),
        KeyId::C => Some(46),
        KeyId::V => Some(47),
        KeyId::B => Some(48),
        KeyId::N => Some(49),
        KeyId::M => Some(50),
        KeyId::Comma => Some(51),
        KeyId::Dot => Some(52),
        KeyId::Slash => Some(53),
        KeyId::RightShift => Some(54),
        KeyId::LeftAlt => Some(56),
        KeyId::Space => Some(57),
        KeyId::CapsLock => Some(58),
        KeyId::RightControl => Some(97),
        KeyId::RightAlt => Some(100),
        KeyId::LeftMeta => Some(125),
        KeyId::RightMeta => Some(126),
        KeyId::Menu => Some(127),
        KeyId::Fn => None,
    }
}

pub fn find_all_keyboard_event_devices() -> Vec<PathBuf> {
    let mut devices = Vec::new();

    // check /dev/input/by-id/ for keyboards
    if let Ok(by_id_entries) = fs::read_dir("/dev/input/by-id") {
        for entry in by_id_entries.flatten() {
            let fname = entry.file_name();
            let fname_str = fname.to_string_lossy();
            let is_madlion = fname_str.contains("MAD60")
                || fname_str.contains("MAD68")
                || fname_str.contains("MAD75")
                || fname_str.contains("Yizhita")
                || fname_str.contains("373b")
                || fname_str.contains("373B");
            if is_madlion
                && fname_str.contains("event-kbd")
                && let Ok(canonical) = fs::canonicalize(entry.path())
                && !devices.contains(&canonical)
            {
                devices.push(canonical);
            }
        }
    }

    // scan /sys/class/input/event* for keyboards
    let input_dir = Path::new("/sys/class/input");
    if let Ok(entries) = fs::read_dir(input_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.starts_with("event") {
                continue;
            }

            let device_path = entry.path().join("device");
            let uevent_path = device_path.join("uevent");
            if let Ok(uevent_content) = fs::read_to_string(&uevent_path) {
                let is_virtual = uevent_content.contains("Passthrough")
                    || uevent_content.contains("uinput")
                    || !uevent_content.contains("PHYS=usb");
                if is_virtual {
                    continue;
                }

                let is_madlion = uevent_content.contains("373B")
                    || uevent_content.contains("373b")
                    || uevent_content.contains("MAD60")
                    || uevent_content.contains("MAD68")
                    || uevent_content.contains("MAD75");
                if !is_madlion {
                    continue;
                }

                let is_mouse = uevent_content.contains("Mouse");
                let has_keys = uevent_content.contains("EV=120013")
                    || (uevent_content.contains("KEY=") && !is_mouse);

                if has_keys && !is_mouse {
                    let dev_node = PathBuf::from(format!("/dev/input/{}", name_str));
                    if dev_node.exists() && !devices.contains(&dev_node) {
                        devices.push(dev_node);
                    }
                }
            }
        }
    }

    devices.sort();
    devices
}

pub struct VirtualPassthroughKeyboard {
    file: File,
    created: bool,
}

impl VirtualPassthroughKeyboard {
    pub fn new() -> io::Result<Self> {
        let uinput_path = if Path::new("/dev/uinput").exists() {
            "/dev/uinput"
        } else if Path::new("/dev/input/uinput").exists() {
            "/dev/input/uinput"
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "uinput device not found",
            ));
        };

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(uinput_path)?;

        let fd = file.as_raw_fd();

        unsafe {
            Self::ioctl_set(fd, UI_SET_EVBIT, EV_SYN as libc::c_int)?;
            Self::ioctl_set(fd, UI_SET_EVBIT, EV_KEY as libc::c_int)?;
            Self::ioctl_set(fd, UI_SET_EVBIT, EV_MSC as libc::c_int)?;
            Self::ioctl_set(fd, UI_SET_EVBIT, EV_LED as libc::c_int)?;
            Self::ioctl_set(fd, UI_SET_EVBIT, EV_REP as libc::c_int)?;

            Self::ioctl_set(fd, UI_SET_MSCBIT, 4)?; // msc scan

            // enable standard keycodes 1..255
            for keycode in 1..=255 {
                Self::ioctl_set(fd, UI_SET_KEYBIT, keycode)?;
            }
            // enable leds
            for led in 0..=5 {
                Self::ioctl_set(fd, UI_SET_LEDBIT, led)?;
            }
        }

        let mut user_dev: UinputUserDev = unsafe { std::mem::zeroed() };
        let name_bytes = b"Madlion Passthrough Keyboard\0";
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

    pub fn sync(&mut self) -> io::Result<()> {
        self.write_event(EV_SYN, SYN_REPORT, 0)?;
        self.file.flush()
    }
}

impl Drop for VirtualPassthroughKeyboard {
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

pub struct InterceptorSettings {
    pub suppressed_keycodes: HashSet<u16>,
    pub mapping_override: bool,
    pub enable_typing: bool,
    pub toggle_keycode: Option<u16>,
    pub on_toggle: Option<Arc<dyn Fn() + Send + Sync>>,
}

pub struct KeyboardInterceptor {
    is_running: Arc<AtomicBool>,
    settings: Arc<Mutex<InterceptorSettings>>,
    thread_handle: Option<JoinHandle<()>>,
}

impl KeyboardInterceptor {
    pub fn spawn_all(
        event_device_paths: &[PathBuf],
        suppressed_keys: &[KeyId],
        mapping_override: bool,
        enable_typing: bool,
        toggle_key: Option<KeyId>,
        on_toggle: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> io::Result<Self> {
        let mut fds: Vec<libc::c_int> = Vec::new();
        let mut input_files = Vec::new();

        for path in event_device_paths {
            if let Ok(file) = OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(path)
            {
                fds.push(file.as_raw_fd());
                input_files.push(file);
            }
        }

        if input_files.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "No keyboard event devices could be opened",
            ));
        }

        // wait up to 250ms for keys to release so wayland doesnt get stuck modifiers before grabbing
        let grab_wait_start = std::time::Instant::now();
        while grab_wait_start.elapsed() < std::time::Duration::from_millis(250) {
            let pressed = query_pressed_keys_from_fds(&fds);
            if pressed.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(15));
        }

        // grab all matching event nodes
        let mut grabbed_files = Vec::new();
        for file in input_files {
            let fd = file.as_raw_fd();
            unsafe {
                if libc::ioctl(fd, EVIOCGRAB, 1 as libc::c_int) >= 0 {
                    grabbed_files.push(file);
                }
            }
        }

        if grabbed_files.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "No keyboard event devices could be grabbed",
            ));
        }

        // spin up virtual uinput keyboard
        let v_kbd = match VirtualPassthroughKeyboard::new() {
            Ok(k) => k,
            Err(e) => {
                for file in &grabbed_files {
                    let fd = file.as_raw_fd();
                    unsafe {
                        libc::ioctl(fd, EVIOCGRAB, 0 as libc::c_int);
                    }
                }
                return Err(e);
            }
        };

        let mut suppressed_keycodes = HashSet::new();
        for &k in suppressed_keys {
            if let Some(code) = key_id_to_linux_keycode(k) {
                suppressed_keycodes.insert(code);
            }
        }

        let toggle_keycode = toggle_key.and_then(key_id_to_linux_keycode);

        let is_running = Arc::new(AtomicBool::new(true));
        let settings = Arc::new(Mutex::new(InterceptorSettings {
            suppressed_keycodes,
            mapping_override,
            enable_typing,
            toggle_keycode,
            on_toggle,
        }));

        let is_running_clone = is_running.clone();
        let settings_clone = settings.clone();

        let thread_handle = thread::spawn(move || {
            interceptor_multi_loop(grabbed_files, v_kbd, is_running_clone, settings_clone);
        });

        Ok(Self {
            is_running,
            settings,
            thread_handle: Some(thread_handle),
        })
    }

    #[allow(dead_code)]
    pub fn spawn(
        event_device_path: impl AsRef<Path>,
        suppressed_keys: &[KeyId],
        mapping_override: bool,
        enable_typing: bool,
        toggle_key: Option<KeyId>,
        on_toggle: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> io::Result<Self> {
        let all_devices = find_all_keyboard_event_devices();
        let target_devices = if all_devices.is_empty() {
            vec![event_device_path.as_ref().to_path_buf()]
        } else {
            all_devices
        };
        Self::spawn_all(
            &target_devices,
            suppressed_keys,
            mapping_override,
            enable_typing,
            toggle_key,
            on_toggle,
        )
    }

    pub fn update_settings(
        &self,
        suppressed_keys: &[KeyId],
        mapping_override: bool,
        enable_typing: bool,
        toggle_key: Option<KeyId>,
    ) {
        let mut suppressed_keycodes = HashSet::new();
        for &k in suppressed_keys {
            if let Some(code) = key_id_to_linux_keycode(k) {
                suppressed_keycodes.insert(code);
            }
        }

        let toggle_keycode = toggle_key.and_then(key_id_to_linux_keycode);

        if let Ok(mut st) = self.settings.lock() {
            st.suppressed_keycodes = suppressed_keycodes;
            st.mapping_override = mapping_override;
            st.enable_typing = enable_typing;
            st.toggle_keycode = toggle_keycode;
        }
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
        if let Some(h) = self.thread_handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for KeyboardInterceptor {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn query_pressed_keys_from_fds(fds: &[libc::c_int]) -> HashSet<u16> {
    let mut pressed = HashSet::new();
    const EVIOCGKEY_96: libc::c_ulong = 0x80604518;
    for &fd in fds {
        let mut key_bits = [0u8; 96];
        unsafe {
            if libc::ioctl(fd, EVIOCGKEY_96, key_bits.as_mut_ptr()) >= 0 {
                for (byte_idx, &byte) in key_bits.iter().enumerate() {
                    if byte == 0 {
                        continue;
                    }
                    for bit in 0..8 {
                        if (byte & (1 << bit)) != 0 {
                            let keycode = (byte_idx * 8 + bit) as u16;
                            pressed.insert(keycode);
                        }
                    }
                }
            }
        }
    }
    pressed
}

fn interceptor_multi_loop(
    input_files: Vec<File>,
    mut v_kbd: VirtualPassthroughKeyboard,
    is_running: Arc<AtomicBool>,
    settings: Arc<Mutex<InterceptorSettings>>,
) {
    let fds: Vec<libc::c_int> = input_files.iter().map(|f| f.as_raw_fd()).collect();
    let mut held_before_grab = query_pressed_keys_from_fds(&fds);
    let mut virtual_pressed_keys = HashSet::<u16>::new();
    let ev_size = std::mem::size_of::<libc::input_event>();
    let mut ev_buf = vec![0u8; ev_size * 16];

    while is_running.load(Ordering::Relaxed) {
        let mut poll_fds: Vec<libc::pollfd> = fds
            .iter()
            .map(|&fd| libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            })
            .collect();

        let ret = unsafe { libc::poll(poll_fds.as_mut_ptr(), poll_fds.len() as libc::nfds_t, 15) };

        if ret <= 0 {
            continue;
        }

        let (suppressed_set, mapping_override, enable_typing, toggle_code, on_toggle_cb) = {
            let st = settings.lock().unwrap();
            (
                st.suppressed_keycodes.clone(),
                st.mapping_override,
                st.enable_typing,
                st.toggle_keycode,
                st.on_toggle.clone(),
            )
        };

        let mut forwarded_any = false;

        for pfd in &poll_fds {
            if (pfd.revents & libc::POLLIN) == 0 {
                continue;
            }

            let bytes_read = unsafe {
                libc::read(
                    pfd.fd,
                    ev_buf.as_mut_ptr() as *mut libc::c_void,
                    ev_buf.len(),
                )
            };

            if bytes_read <= 0 {
                continue;
            }

            let num_events = (bytes_read as usize) / ev_size;
            let events: &[libc::input_event] = unsafe {
                std::slice::from_raw_parts(ev_buf.as_ptr() as *const libc::input_event, num_events)
            };

            for ev in events {
                if ev.type_ == EV_KEY {
                    // check toggle key press
                    if ev.value == 1
                        && let Some(t_code) = toggle_code
                        && ev.code == t_code
                    {
                        if let Some(ref cb) = on_toggle_cb {
                            cb();
                        }
                        continue;
                    }

                    if ev.value == 0 {
                        // key release event
                        // forward releases for pregrab held keys so modifiers dont get stuck
                        let was_held = held_before_grab.remove(&ev.code);
                        let was_virt_pressed = virtual_pressed_keys.remove(&ev.code);

                        if was_held || was_virt_pressed || enable_typing {
                            let _ = v_kbd.write_event(ev.type_, ev.code, 0);
                            forwarded_any = true;
                        }
                        continue;
                    }

                    // key down or repeat
                    if !enable_typing {
                        // typing is turned off so drop event
                        continue;
                    }

                    if mapping_override && suppressed_set.contains(&ev.code) {
                        // swallow key event since its mapped to gamepad
                        continue;
                    }

                    // pass through key event n track state
                    if ev.value == 1 {
                        virtual_pressed_keys.insert(ev.code);
                    }
                    let _ = v_kbd.write_event(ev.type_, ev.code, ev.value);
                    forwarded_any = true;
                } else if ev.type_ == EV_SYN {
                    if forwarded_any {
                        let _ = v_kbd.sync();
                        forwarded_any = false;
                    }
                } else {
                    // pass through msc, led, rep n other events
                    let _ = v_kbd.write_event(ev.type_, ev.code, ev.value);
                    forwarded_any = true;
                }
            }
        }

        if forwarded_any {
            let _ = v_kbd.sync();
        }
    }

    // release held virtual keys so nothing stays stuck on exit
    for &code in &virtual_pressed_keys {
        let _ = v_kbd.write_event(EV_KEY, code, 0);
    }
    for &code in &held_before_grab {
        let _ = v_kbd.write_event(EV_KEY, code, 0);
    }
    let _ = v_kbd.sync();

    // ungrab all devices before worker thread exits
    for &fd in &fds {
        unsafe {
            libc::ioctl(fd, EVIOCGRAB, 0 as libc::c_int);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_id_to_linux_keycode() {
        assert_eq!(key_id_to_linux_keycode(KeyId::Esc), Some(1));
        assert_eq!(key_id_to_linux_keycode(KeyId::Enter), Some(28));
        assert_eq!(key_id_to_linux_keycode(KeyId::Space), Some(57));
        assert_eq!(key_id_to_linux_keycode(KeyId::Fn), None);
    }

    #[test]
    fn test_wasd_keycodes() {
        assert_eq!(key_id_to_linux_keycode(KeyId::W), Some(17));
        assert_eq!(key_id_to_linux_keycode(KeyId::A), Some(30));
        assert_eq!(key_id_to_linux_keycode(KeyId::S), Some(31));
        assert_eq!(key_id_to_linux_keycode(KeyId::D), Some(32));
    }

    #[test]
    fn test_find_all_keyboard_event_devices_on_host() {
        let devs = find_all_keyboard_event_devices();
        println!("Discovered physical keyboard event devices: {devs:?}");
        for d in &devs {
            assert!(d.exists());
        }
    }
}
