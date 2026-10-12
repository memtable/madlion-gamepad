use std::io;

pub const LIGHTING_REPORT_LEN: usize = 33;
pub const LIGHTING_RESPONSE_LEN: usize = 32;

pub const CMD_LIGHTING_GET: u8 = 0x08;
pub const CMD_LIGHTING_SET: u8 = 0x07;
pub const SUBCMD_LIGHT_INFO: u8 = 0x41;
pub const SUBCMD_CUSTOM_LAMPLIGHT_SET: u8 = 0x42;
pub const SUBCMD_CUSTOM_LAMPLIGHT_GET: u8 = 0x45;

pub const EFFECT_MODE_CUSTOMIZATION: u8 = 0x01;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorRGB {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl ColorRGB {
    pub const RED: Self = Self { r: 255, g: 0, b: 0 };
    #[cfg(test)]
    pub const GREEN: Self = Self { r: 0, g: 255, b: 0 };
    pub const OFF: Self = Self { r: 0, g: 0, b: 0 };

    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightingInfo {
    pub effect_mode: u8,
    pub speed: u8,
    pub brightness: u8,
    pub color: ColorRGB,
}

impl LightingInfo {
    pub fn is_customization_mode(self) -> bool {
        self.effect_mode == EFFECT_MODE_CUSTOMIZATION
    }
}

pub fn build_lighting_query_packet() -> [u8; LIGHTING_REPORT_LEN] {
    let mut packet = [0u8; LIGHTING_REPORT_LEN];
    packet[0] = 0x00; // report id 0
    packet[1] = CMD_LIGHTING_GET; // 0x08
    packet[2] = SUBCMD_LIGHT_INFO; // 0x41
    packet
}

pub fn parse_lighting_response(resp: &[u8]) -> io::Result<LightingInfo> {
    if resp.len() < LIGHTING_RESPONSE_LEN {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!(
                "lighting response too short (expected {} bytes, got {})",
                LIGHTING_RESPONSE_LEN,
                resp.len()
            ),
        ));
    }

    if resp[0] != CMD_LIGHTING_GET || resp[1] != SUBCMD_LIGHT_INFO {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "invalid lighting response header: [0x{:02x}, 0x{:02x}]",
                resp[0], resp[1]
            ),
        ));
    }

    let effect_mode = resp[2];
    let speed = resp[4];
    let r = resp[5];
    let g = resp[6];
    let b = resp[7];
    let brightness = resp[8];

    Ok(LightingInfo {
        effect_mode,
        speed,
        brightness,
        color: ColorRGB::new(r, g, b),
    })
}

pub fn build_get_custom_key_packet(row: u8, start_col: u8, count: u8) -> [u8; LIGHTING_REPORT_LEN] {
    let mut packet = [0u8; LIGHTING_REPORT_LEN];
    packet[0] = 0x00; // report id
    packet[1] = CMD_LIGHTING_GET; // 0x08 (id_custom_get_value)
    packet[2] = SUBCMD_CUSTOM_LAMPLIGHT_GET; // 0x45 (getCustomLamplight)
    packet[3] = row;
    packet[4] = start_col;
    packet[5] = count;
    packet
}

pub fn parse_get_custom_key_response(resp: &[u8]) -> io::Result<Vec<ColorRGB>> {
    if resp.len() < LIGHTING_RESPONSE_LEN {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!(
                "get custom key response too short (expected {} bytes, got {})",
                LIGHTING_RESPONSE_LEN,
                resp.len()
            ),
        ));
    }

    if resp[0] != CMD_LIGHTING_GET || resp[1] != SUBCMD_CUSTOM_LAMPLIGHT_GET {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "invalid get custom key response header: [0x{:02x}, 0x{:02x}]",
                resp[0], resp[1]
            ),
        ));
    }

    let count = (resp[4] as usize).min(8);
    let mut colors = Vec::with_capacity(count);

    for i in 0..count {
        let base = 5 + i * 3;
        if base + 2 < resp.len() {
            let r = resp[base];
            let g = resp[base + 1];
            let b = resp[base + 2];
            colors.push(ColorRGB::new(r, g, b));
        }
    }

    Ok(colors)
}

pub fn build_set_custom_key_packet(
    row: u8,
    start_col: u8,
    colors: &[ColorRGB],
) -> io::Result<[u8; LIGHTING_REPORT_LEN]> {
    if colors.is_empty() || colors.len() > 8 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "colors slice must have between 1 and 8 entries",
        ));
    }

    let mut packet = [0u8; LIGHTING_REPORT_LEN];
    packet[0] = 0x00; // report id
    packet[1] = CMD_LIGHTING_SET; // 0x07
    packet[2] = SUBCMD_CUSTOM_LAMPLIGHT_SET; // 0x42
    packet[3] = row; // row index (0..4)
    packet[4] = start_col; // start column index (0..13)
    packet[5] = colors.len() as u8; // count of keys

    for (i, color) in colors.iter().enumerate() {
        let base = 6 + i * 3;
        packet[base] = color.r;
        packet[base + 1] = color.g;
        packet[base + 2] = color.b;
    }

    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_lighting_query_packet() {
        let p = build_lighting_query_packet();
        assert_eq!(p.len(), 33);
        assert_eq!(p[0], 0x00);
        assert_eq!(p[1], 0x08);
        assert_eq!(p[2], 0x41);
        assert_eq!(&p[3..], &[0u8; 30]);
    }

    #[test]
    fn test_parse_lighting_response() {
        let mut resp = [0u8; 32];
        resp[0] = 0x08;
        resp[1] = 0x41;
        resp[2] = 0x01; // customization mode
        resp[3] = 0x00; // sub-mode
        resp[4] = 0x05; // speed = 5
        resp[5] = 0x9c; // r = 156
        resp[6] = 0x21; // g = 33
        resp[7] = 0x14; // b = 20
        resp[8] = 0xd2; // brightness = 210

        let info = parse_lighting_response(&resp).unwrap();
        assert_eq!(info.effect_mode, 0x01);
        assert!(info.is_customization_mode());
        assert_eq!(info.speed, 5);
        assert_eq!(info.brightness, 210);
        assert_eq!(info.color.r, 156);
        assert_eq!(info.color.g, 33);
        assert_eq!(info.color.b, 20);
    }

    #[test]
    fn test_build_get_custom_key_packet_and_parse() {
        let req = build_get_custom_key_packet(1, 0, 8);
        assert_eq!(req[0], 0x00);
        assert_eq!(req[1], 0x08);
        assert_eq!(req[2], 0x45);
        assert_eq!(req[3], 1);
        assert_eq!(req[4], 0);
        assert_eq!(req[5], 8);

        let mut resp = [0u8; 32];
        resp[0] = 0x08;
        resp[1] = 0x45;
        resp[2] = 1;
        resp[3] = 0;
        resp[4] = 2; // 2 keys in response
        resp[5] = 255;
        resp[6] = 0;
        resp[7] = 0; // key 0: red
        resp[8] = 0;
        resp[9] = 255;
        resp[10] = 0; // key 1: green

        let parsed = parse_get_custom_key_response(&resp).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], ColorRGB::RED);
        assert_eq!(parsed[1], ColorRGB::GREEN);
    }

    #[test]
    fn test_build_set_custom_key_packet() {
        // single key: w at row 1, col 2
        let p = build_set_custom_key_packet(1, 2, &[ColorRGB::RED]).unwrap();
        assert_eq!(p[0], 0x00);
        assert_eq!(p[1], 0x07);
        assert_eq!(p[2], 0x42);
        assert_eq!(p[3], 1); // row 1
        assert_eq!(p[4], 2); // col 2 (w)
        assert_eq!(p[5], 1); // 1 key
        assert_eq!(p[6], 255); // r
        assert_eq!(p[7], 0); // g
        assert_eq!(p[8], 0); // b

        // multiple keys: asd at row 2, cols 1..3
        let colors = [ColorRGB::RED, ColorRGB::RED, ColorRGB::RED];
        let p2 = build_set_custom_key_packet(2, 1, &colors).unwrap();
        assert_eq!(p2[3], 2); // row 2
        assert_eq!(p2[4], 1); // col 1 (a)
        assert_eq!(p2[5], 3);
        assert_eq!(p2[6], 255);
        assert_eq!(p2[9], 255);
        assert_eq!(p2[12], 255);
    }

    #[test]
    fn test_build_set_custom_key_packet_bounds() {
        assert!(build_set_custom_key_packet(0, 0, &[]).is_err());
        let nine_colors = [ColorRGB::RED; 9];
        assert!(build_set_custom_key_packet(0, 0, &nine_colors).is_err());
    }
}
