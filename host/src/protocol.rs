use std::io::Cursor;
use byteorder::{LittleEndian, ReadBytesExt};
use thiserror::Error;

#[derive(Debug, PartialEq, Clone)]
pub enum InputPacket {
    MouseMove { dx: i16, dy: i16 },
    MouseDown { button: u8 },
    MouseUp { button: u8 },
    MouseScroll { delta_y: i16, delta_x: i16 },
    KeyDown { vkey: u16, modifiers: u8 },
    KeyUp { vkey: u16 },
    TextInput { text: String },
    SwitchMonitor { monitor_index: u8 },
    SetQuality { mode: u8 },
    TouchDirect { norm_x: u16, norm_y: u16, action: u8 },
    SetPrivacyCurtain { enabled: bool },
    ChangeResolution { mode: u8 },
    ChangeOrientation { vertical: bool },
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("packet too short")]
    TooShort,
    #[error("unrecognized opcode: {0:#04x}")]
    UnknownOpCode(u8),
    #[error("invalid utf-8 text payload")]
    InvalidUtf8,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl InputPacket {
    pub fn parse(buf: &[u8]) -> Result<Self, ProtocolError> {
        if buf.is_empty() {
            return Err(ProtocolError::TooShort);
        }

        let mut rdr = Cursor::new(&buf[1..]);
        let opcode = buf[0];

        match opcode {
            0x01 => {
                let dx = rdr.read_i16::<LittleEndian>()?;
                let dy = rdr.read_i16::<LittleEndian>()?;
                Ok(InputPacket::MouseMove { dx, dy })
            }
            0x02 => {
                let button = rdr.read_u8()?;
                Ok(InputPacket::MouseDown { button })
            }
            0x03 => {
                let button = rdr.read_u8()?;
                Ok(InputPacket::MouseUp { button })
            }
            0x04 => {
                let delta_y = rdr.read_i16::<LittleEndian>()?;
                let delta_x = rdr.read_i16::<LittleEndian>().unwrap_or(0);
                Ok(InputPacket::MouseScroll { delta_y, delta_x })
            }
            0x05 => {
                let vkey = rdr.read_u16::<LittleEndian>()?;
                let modifiers = rdr.read_u8().unwrap_or(0);
                Ok(InputPacket::KeyDown { vkey, modifiers })
            }
            0x06 => {
                let vkey = rdr.read_u16::<LittleEndian>()?;
                Ok(InputPacket::KeyUp { vkey })
            }
            0x07 => {
                let len = rdr.read_u16::<LittleEndian>()? as usize;
                let remaining = &buf[3..];
                if remaining.len() < len {
                    return Err(ProtocolError::TooShort);
                }
                let text = String::from_utf8(remaining[..len].to_vec())
                    .map_err(|_| ProtocolError::InvalidUtf8)?;
                Ok(InputPacket::TextInput { text })
            }
            0x08 => {
                let monitor_index = rdr.read_u8()?;
                Ok(InputPacket::SwitchMonitor { monitor_index })
            }
            0x09 => {
                let mode = rdr.read_u8()?;
                Ok(InputPacket::SetQuality { mode })
            }
            0x0A => {
                let norm_x = rdr.read_u16::<LittleEndian>()?;
                let norm_y = rdr.read_u16::<LittleEndian>()?;
                let action = rdr.read_u8()?;
                Ok(InputPacket::TouchDirect { norm_x, norm_y, action })
            }
            0x0B => {
                let enabled = rdr.read_u8()? != 0;
                Ok(InputPacket::SetPrivacyCurtain { enabled })
            }
            0x0C => {
                let mode = rdr.read_u8()?;
                Ok(InputPacket::ChangeResolution { mode })
            }
            0x0D => {
                let vertical = rdr.read_u8()? != 0;
                Ok(InputPacket::ChangeOrientation { vertical })
            }
            _ => Err(ProtocolError::UnknownOpCode(opcode)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mouse_move() {
        let bytes = vec![0x01, 0x0C, 0x00, 0xFB, 0xFF];
        let packet = InputPacket::parse(&bytes).expect("parse failed");
        assert_eq!(packet, InputPacket::MouseMove { dx: 12, dy: -5 });
    }

    #[test]
    fn test_parse_mouse_down_and_up() {
        let down_bytes = vec![0x02, 0x01];
        assert_eq!(
            InputPacket::parse(&down_bytes).unwrap(),
            InputPacket::MouseDown { button: 1 }
        );

        let up_bytes = vec![0x03, 0x02];
        assert_eq!(
            InputPacket::parse(&up_bytes).unwrap(),
            InputPacket::MouseUp { button: 2 }
        );
    }

    #[test]
    fn test_parse_scroll() {
        let bytes = vec![0x04, 0x78, 0x00, 0x00, 0x00];
        assert_eq!(
            InputPacket::parse(&bytes).unwrap(),
            InputPacket::MouseScroll { delta_y: 120, delta_x: 0 }
        );
    }

    #[test]
    fn test_parse_text_input() {
        let mut bytes = vec![0x07, 0x05, 0x00];
        bytes.extend_from_slice(b"Hello");
        assert_eq!(
            InputPacket::parse(&bytes).unwrap(),
            InputPacket::TextInput { text: "Hello".to_string() }
        );
    }

    #[test]
    fn test_parse_monitor_and_quality() {
        let mon = vec![0x08, 0x01];
        assert_eq!(
            InputPacket::parse(&mon).unwrap(),
            InputPacket::SwitchMonitor { monitor_index: 1 }
        );

        let qual = vec![0x09, 0x02];
        assert_eq!(
            InputPacket::parse(&qual).unwrap(),
            InputPacket::SetQuality { mode: 2 }
        );
    }
}
