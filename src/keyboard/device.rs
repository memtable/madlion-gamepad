use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use super::layout::{KeyId, find_key_matrix_index};
use super::lighting::{
    ColorRGB, LightingInfo, build_get_custom_key_packet, build_lighting_query_packet,
    build_set_custom_key_packet, parse_get_custom_key_response, parse_lighting_response,
};

pub const MADLION_VID: u16 = 0x373b;
pub const KNOWN_PIDS: &[u16] = &[0x1053, 0x1054, 0x1055, 0x1056, 0x105d];
pub const REPORT_DESCRIPTOR_MAGIC: &[u8] = &[0x06, 0x60, 0xff, 0x09, 0x61];
pub const MAX_TRAVEL_MM: f32 = 3.50;
pub const MAX_RAW_UNITS: f32 = 350.0;
pub const QUERY_PACKET_LEN: usize = 33;
pub const RESPONSE_PACKET_LEN: usize = 32;

#[derive(Debug, Clone, PartialEq)]
pub struct AnalogReading {
    pub key: KeyId,
    pub matrix_index: u8,
    pub raw: u16,
    pub depth_mm: f32,
    pub normalized: f32,
}

impl AnalogReading {
    pub fn new(key: KeyId, matrix_index: u8, raw: u16) -> Self {
        let depth_mm = (raw as f32 / 100.0).clamp(0.0, MAX_TRAVEL_MM);
        let normalized = (raw as f32 / MAX_RAW_UNITS).clamp(0.0, 1.0);
        Self {
            key,
            matrix_index,
            raw,
            depth_mm,
            normalized,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkQuery {
    pub offset: u8,
    // keyid, matrix index, slot index in chunk 0..3
    pub targets: Vec<(KeyId, u8, usize)>,
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub dev_path: PathBuf,
    pub sysfs_path: PathBuf,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
}

pub fn build_query_packet(offset: u8) -> [u8; QUERY_PACKET_LEN] {
    let mut packet = [0u8; QUERY_PACKET_LEN];
    packet[0] = 0x00; // report id preamble
    packet[1] = 0x02;
    packet[2] = 0x96;
    packet[3] = 0x1c;
    packet[4] = 0x00;
    packet[5] = 0x00;
    packet[6] = 0x00;
    packet[7] = offset;
    packet[8] = 0x04;
    packet
}

pub fn parse_chunk_response(resp: &[u8]) -> io::Result<[u16; 4]> {
    if resp.len() < RESPONSE_PACKET_LEN {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!(
                "response packet too short (expected {} bytes, got {})",
                RESPONSE_PACKET_LEN,
                resp.len()
            ),
        ));
    }

    let mut values = [0u16; 4];
    for (i, val) in values.iter_mut().enumerate() {
        let idx0 = 10 + i * 5;
        let idx1 = idx0 + 1;
        *val = ((resp[idx0] as u16) << 8) | (resp[idx1] as u16);
    }
    Ok(values)
}

pub fn plan_queries(keys: &[KeyId]) -> Vec<ChunkQuery> {
    if keys.is_empty() {
        return Vec::new();
    }

    let mut key_indices: Vec<(KeyId, u8)> = Vec::new();
    for &k in keys {
        if let Some(idx) = find_key_matrix_index(k)
            && !key_indices.iter().any(|(existing_k, _)| *existing_k == k)
        {
            key_indices.push((k, idx));
        }
    }

    key_indices.sort_by_key(|(_, idx)| *idx);

    let mut queries = Vec::new();
    let mut i = 0;
    while i < key_indices.len() {
        let start_idx = key_indices[i].1;
        let offset = start_idx;
        let mut targets = Vec::new();

        while i < key_indices.len() {
            let (k, idx) = key_indices[i];
            if idx >= offset && idx < offset + 4 {
                let slot = (idx - offset) as usize;
                targets.push((k, idx, slot));
                i += 1;
            } else {
                break;
            }
        }

        queries.push(ChunkQuery { offset, targets });
    }

    queries
}

pub struct MadlionKeyboard {
    file: File,
    path: PathBuf,
    backup_colors: Option<[[ColorRGB; 14]; 5]>,
}

impl MadlionKeyboard {
    pub fn open_path(path: impl AsRef<Path>) -> io::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let file = OpenOptions::new().read(true).write(true).open(&path_buf)?;

        Ok(Self {
            file,
            path: path_buf,
            backup_colors: None,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn find_devices() -> Vec<DeviceInfo> {
        let mut devices = Vec::new();
        let hidraw_dir = Path::new("/sys/class/hidraw");
        if !hidraw_dir.is_dir() {
            return devices;
        }

        let entries = match fs::read_dir(hidraw_dir) {
            Ok(e) => e,
            Err(_) => return devices,
        };

        for entry in entries.flatten() {
            let fname = entry.file_name();
            let hidraw_name = fname.to_string_lossy();
            let dev_node = PathBuf::from(format!("/dev/{}", hidraw_name));
            let sysfs_path = entry.path();
            let device_dir = sysfs_path.join("device");

            // check report descriptor
            let report_desc_path = device_dir.join("report_descriptor");
            let has_magic = if let Ok(desc) = fs::read(&report_desc_path) {
                desc.windows(REPORT_DESCRIPTOR_MAGIC.len())
                    .any(|window| window == REPORT_DESCRIPTOR_MAGIC)
            } else {
                false
            };

            // parse uevent for vid/pid/name
            let uevent_path = device_dir.join("uevent");
            let mut vid = 0u16;
            let mut pid = 0u16;
            let mut name = String::new();

            if let Ok(uevent_str) = fs::read_to_string(&uevent_path) {
                for line in uevent_str.lines() {
                    if let Some(rest) = line.strip_prefix("HID_ID=") {
                        // format: 0003:0000373b:0000105d
                        let parts: Vec<&str> = rest.split(':').collect();
                        if parts.len() >= 3 {
                            if let Ok(v) =
                                u16::from_str_radix(parts[1].trim_start_matches("0000"), 16)
                            {
                                vid = v;
                            }
                            if let Ok(p) =
                                u16::from_str_radix(parts[2].trim_start_matches("0000"), 16)
                            {
                                pid = p;
                            }
                        }
                    } else if let Some(rest) = line.strip_prefix("HID_NAME=") {
                        name = rest.to_string();
                    }
                }
            }

            let is_madlion = vid == MADLION_VID || KNOWN_PIDS.contains(&pid) || has_magic;
            if is_madlion && has_magic {
                devices.push(DeviceInfo {
                    dev_path: dev_node,
                    sysfs_path,
                    name: if name.is_empty() {
                        "Madlion Keyboard (Analog Interface)".to_string()
                    } else {
                        name
                    },
                    vendor_id: vid,
                    product_id: pid,
                });
            }
        }

        devices.sort_by(|a, b| a.dev_path.cmp(&b.dev_path));
        devices
    }

    pub fn auto_discover() -> io::Result<Self> {
        let devices = Self::find_devices();
        if let Some(dev) = devices.first() {
            Self::open_path(&dev.dev_path)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "No Madlion analog keyboard interface found in /sys/class/hidraw",
            ))
        }
    }

    pub fn query_chunk(&mut self, offset: u8) -> io::Result<[u16; 4]> {
        let packet = build_query_packet(offset);
        self.file.write_all(&packet)?;
        self.file.flush()?;

        let mut resp = [0u8; RESPONSE_PACKET_LEN];
        self.file.read_exact(&mut resp)?;
        parse_chunk_response(&resp)
    }

    pub fn read_keys(&mut self, queries: &[ChunkQuery]) -> io::Result<Vec<AnalogReading>> {
        let mut readings = Vec::new();
        for chunk in queries {
            let chunk_data = self.query_chunk(chunk.offset)?;
            for &(key, matrix_index, slot) in &chunk.targets {
                let raw = chunk_data[slot];
                readings.push(AnalogReading::new(key, matrix_index, raw));
            }
        }
        Ok(readings)
    }

    pub fn query_lighting_info(&mut self) -> io::Result<LightingInfo> {
        let packet = build_lighting_query_packet();
        self.file.write_all(&packet)?;
        self.file.flush()?;

        let mut resp = [0u8; RESPONSE_PACKET_LEN];
        self.file.read_exact(&mut resp)?;
        parse_lighting_response(&resp)
    }

    pub fn set_custom_key_colors(
        &mut self,
        row: u8,
        start_col: u8,
        colors: &[ColorRGB],
    ) -> io::Result<()> {
        let packet = build_set_custom_key_packet(row, start_col, colors)?;
        self.file.write_all(&packet)?;
        self.file.flush()?;

        let mut resp = [0u8; RESPONSE_PACKET_LEN];
        self.file.read_exact(&mut resp)?;
        Ok(())
    }

    pub fn read_custom_key_colors(
        &mut self,
        row: u8,
        start_col: u8,
        count: u8,
    ) -> io::Result<Vec<ColorRGB>> {
        let packet = build_get_custom_key_packet(row, start_col, count);
        self.file.write_all(&packet)?;
        self.file.flush()?;

        let mut resp = [0u8; RESPONSE_PACKET_LEN];
        self.file.read_exact(&mut resp)?;
        parse_get_custom_key_response(&resp)
    }

    pub fn read_full_custom_lighting_matrix(&mut self) -> io::Result<[[ColorRGB; 14]; 5]> {
        let mut matrix = [[ColorRGB::OFF; 14]; 5];
        for (r, row) in matrix.iter_mut().enumerate() {
            // chunk 0: cols 0..8 (8 keys)
            let chunk0 = self.read_custom_key_colors(r as u8, 0, 8)?;
            for (i, &col) in chunk0.iter().enumerate() {
                if i < 8 {
                    row[i] = col;
                }
            }
            // chunk 1: cols 8..14 (6 keys)
            let chunk1 = self.read_custom_key_colors(r as u8, 8, 6)?;
            for (i, &col) in chunk1.iter().enumerate() {
                if 8 + i < 14 {
                    row[8 + i] = col;
                }
            }
        }
        Ok(matrix)
    }

    // sets mapped keys to static rgb while keeping users other colors in custom mode
    // saves hardware rgb first so we can restore later
    pub fn apply_mapped_rgb(&mut self, mapped_keys: &[KeyId], color: ColorRGB) -> io::Result<bool> {
        let info = self.query_lighting_info()?;

        if !info.is_customization_mode() {
            // hardware animation like rainbow or breathing is running so skip override
            return Ok(false);
        }

        // save current rgb matrix if we havent yet
        if self.backup_colors.is_none()
            && let Ok(original) = self.read_full_custom_lighting_matrix()
        {
            self.backup_colors = Some(original);
        }

        // use saved matrix as baseline or start from all off
        let mut matrix_colors = self.backup_colors.unwrap_or([[ColorRGB::OFF; 14]; 5]);

        // highlight just the mapped keys
        for &k in mapped_keys {
            if let Some(idx) = find_key_matrix_index(k) {
                let row = (idx / 14) as usize;
                let col = (idx % 14) as usize;
                if row < 5 && col < 14 {
                    matrix_colors[row][col] = color;
                }
            }
        }

        // split each row into 2 chunks (cols 0..8 then 8..14)
        for (r, row) in matrix_colors.iter().enumerate() {
            self.set_custom_key_colors(r as u8, 0, &row[0..8])?;
            self.set_custom_key_colors(r as u8, 8, &row[8..14])?;
        }

        Ok(true)
    }

    // restore users original lighting backup
    pub fn restore_original_rgb(&mut self) -> io::Result<bool> {
        if let Some(original) = self.backup_colors.take() {
            for (r, row) in original.iter().enumerate() {
                self.set_custom_key_colors(r as u8, 0, &row[0..8])?;
                self.set_custom_key_colors(r as u8, 8, &row[8..14])?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl Drop for MadlionKeyboard {
    fn drop(&mut self) {
        let _ = self.restore_original_rgb();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_query_packet() {
        let packet = build_query_packet(16);
        assert_eq!(packet.len(), 33);
        assert_eq!(packet[0], 0x00);
        assert_eq!(packet[1], 0x02);
        assert_eq!(packet[2], 0x96);
        assert_eq!(packet[3], 0x1c);
        assert_eq!(packet[7], 16);
        assert_eq!(packet[8], 0x04);
        assert_eq!(&packet[9..], &[0u8; 24]);
    }

    #[test]
    fn test_parse_chunk_response() {
        let mut resp = [0u8; 32];
        // slot 0 (bytes 10, 11) = 250 (2.50mm)
        resp[10] = 0x00;
        resp[11] = 0xfa;
        // slot 1 (bytes 15, 16) = 350 (3.50mm)
        resp[15] = 0x01;
        resp[16] = 0x5e;
        // slot 2 (bytes 20, 21) = 0
        resp[20] = 0x00;
        resp[21] = 0x00;
        // slot 3 (bytes 25, 26) = 100 (1.00mm)
        resp[25] = 0x00;
        resp[26] = 0x64;

        let parsed = parse_chunk_response(&resp).unwrap();
        assert_eq!(parsed, [250, 350, 0, 100]);

        let reading0 = AnalogReading::new(KeyId::W, 16, parsed[0]);
        assert_eq!(reading0.raw, 250);
        assert!((reading0.depth_mm - 2.50).abs() < 1e-4);
        assert!((reading0.normalized - (250.0 / 350.0)).abs() < 1e-4);

        let reading1 = AnalogReading::new(KeyId::S, 30, parsed[1]);
        assert_eq!(reading1.raw, 350);
        assert!((reading1.depth_mm - 3.50).abs() < 1e-4);
        assert!((reading1.normalized - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_plan_queries_wasd() {
        let keys = [KeyId::W, KeyId::A, KeyId::S, KeyId::D];
        let queries = plan_queries(&keys);
        // w is 16
        // a is 29, s is 30, d is 31
        assert_eq!(queries.len(), 2);
        assert_eq!(queries[0].offset, 16);
        assert_eq!(queries[0].targets, vec![(KeyId::W, 16, 0)]);

        assert_eq!(queries[1].offset, 29);
        assert_eq!(
            queries[1].targets,
            vec![(KeyId::A, 29, 0), (KeyId::S, 30, 1), (KeyId::D, 31, 2)]
        );
    }

    #[test]
    fn test_plan_queries_contiguous() {
        let keys = [KeyId::Q, KeyId::W, KeyId::E, KeyId::R];
        // 15, 16, 17, 18
        let queries = plan_queries(&keys);
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].offset, 15);
        assert_eq!(queries[0].targets.len(), 4);
    }

    #[test]
    fn test_plan_queries_empty() {
        let queries = plan_queries(&[]);
        assert!(queries.is_empty());
    }

    #[test]
    fn test_plan_queries_duplicates_and_order() {
        let keys = [KeyId::D, KeyId::W, KeyId::A, KeyId::W, KeyId::D];
        let queries = plan_queries(&keys);
        assert_eq!(queries.len(), 2);
        assert_eq!(queries[0].offset, 16);
        assert_eq!(queries[0].targets, vec![(KeyId::W, 16, 0)]);
        assert_eq!(queries[1].offset, 29);
        assert_eq!(
            queries[1].targets,
            vec![(KeyId::A, 29, 0), (KeyId::D, 31, 2)]
        );
    }

    #[test]
    fn test_parse_chunk_response_short() {
        let short = [0u8; 10];
        assert!(parse_chunk_response(&short).is_err());
    }

    #[test]
    fn test_analog_reading_clamping() {
        let r = AnalogReading::new(KeyId::Space, 62, 500);
        assert_eq!(r.depth_mm, 3.50);
        assert_eq!(r.normalized, 1.0);
    }

    #[test]
    fn test_live_keyboard_lighting_info() {
        if let Ok(mut kb) = MadlionKeyboard::auto_discover()
            && let Ok(info) = kb.query_lighting_info()
        {
            println!("Live keyboard lighting info: {:?}", info);
            println!("Is customization mode: {}", info.is_customization_mode());
        }
    }
}
