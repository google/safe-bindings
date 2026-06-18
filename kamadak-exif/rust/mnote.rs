use crate::value::Value;
use exif::Value as KamadakValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum MakerNoteType {
    #[default]
    None = 0,
    Unknown = 1,
    Canon = 2,
    Casio1 = 3,
    Casio2 = 4,
    Casio3 = 5,
    Fuji = 6,
    Nikon1 = 7,
    Nikon3 = 8,
    Olympus = 9,
    Panasonic = 10,
    Pentax = 11,
    Sony = 12,
}

#[derive(Debug, Clone)]
pub struct MnoteField {
    pub tag: u16,
    pub value: Value,
}

pub fn parse_makernote(
    make: &str,
    mnote_bytes: &[u8],
    tiff_buf: &[u8],
    little_endian: bool,
) -> (MakerNoteType, Vec<MnoteField>) {
    let make_lower = make.to_lowercase();

    let mut mnote_type = MakerNoteType::Unknown;
    let mut ifd_offset = 0;
    let mut value_offset_base = 0;
    let mut value_offset_rel_to_mnote = true;

    if make_lower.contains("canon") {
        mnote_type = MakerNoteType::Canon;
        ifd_offset = 0;
        value_offset_rel_to_mnote = false;
    } else if make_lower.contains("nikon") {
        if mnote_bytes.starts_with(b"Nikon\0\x01") {
            mnote_type = MakerNoteType::Nikon1;
            ifd_offset = 8;
            value_offset_base = 0;
        } else if mnote_bytes.starts_with(b"Nikon\0\x02") || mnote_bytes.starts_with(b"Nikon\0\x03")
        {
            mnote_type = MakerNoteType::Nikon3;
            value_offset_base = 10;
            // Read IFD offset from sub-TIFF header at offset 10
            if mnote_bytes.len() >= 18 {
                let ofs = if little_endian {
                    u32::from_le_bytes([
                        mnote_bytes[14],
                        mnote_bytes[15],
                        mnote_bytes[16],
                        mnote_bytes[17],
                    ])
                } else {
                    u32::from_be_bytes([
                        mnote_bytes[14],
                        mnote_bytes[15],
                        mnote_bytes[16],
                        mnote_bytes[17],
                    ])
                } as usize;
                ifd_offset = 10 + ofs;
            } else {
                ifd_offset = 18;
            }
        } else {
            mnote_type = MakerNoteType::Nikon3;
            ifd_offset = 0;
            value_offset_base = 0;
        }
    } else if make_lower.contains("olympus") || make_lower.contains("epson") {
        if mnote_bytes.starts_with(b"OLYMP\0\x01\0") {
            mnote_type = MakerNoteType::Olympus;
            ifd_offset = 8;
            value_offset_base = 0;
        } else if mnote_bytes.starts_with(b"OLYMPUS\0\x01\0") {
            mnote_type = MakerNoteType::Olympus;
            ifd_offset = 12;
            value_offset_base = 8;
        } else {
            mnote_type = MakerNoteType::Olympus;
            ifd_offset = 0;
            value_offset_base = 0;
        }
    } else if make_lower.contains("panasonic") {
        mnote_type = MakerNoteType::Panasonic;
        ifd_offset = 12;
        value_offset_rel_to_mnote = false;
    } else if make_lower.contains("pentax") {
        mnote_type = MakerNoteType::Pentax;
        ifd_offset = 6;
        value_offset_base = 0;
    } else if make_lower.contains("sony") {
        mnote_type = MakerNoteType::Sony;
        ifd_offset = 12;
        value_offset_base = 0;
    } else if make_lower.contains("fuji") {
        if mnote_bytes.starts_with(b"FUJIFILM") && mnote_bytes.len() >= 12 {
            mnote_type = MakerNoteType::Fuji;
            let ofs = u32::from_le_bytes([
                mnote_bytes[8],
                mnote_bytes[9],
                mnote_bytes[10],
                mnote_bytes[11],
            ]) as usize;
            ifd_offset = ofs;
            value_offset_base = 0;
            value_offset_rel_to_mnote = true;
            return (
                mnote_type,
                parse_ifd(
                    tiff_buf,
                    mnote_bytes,
                    value_offset_base,
                    ifd_offset,
                    value_offset_rel_to_mnote,
                    true,
                )
                .unwrap_or_default(),
            );
        }
    } else if make_lower.contains("casio") {
        if mnote_bytes.starts_with(b"QVC\0") {
            mnote_type = MakerNoteType::Casio2;
            ifd_offset = 6;
            value_offset_base = 0;
        } else {
            mnote_type = MakerNoteType::Casio1;
            ifd_offset = 6;
            value_offset_base = 0;
        }
    }

    if mnote_type == MakerNoteType::Unknown && mnote_bytes.len() < 4 {
        return (MakerNoteType::None, Vec::new());
    }

    let mut fields = Vec::new();
    if let Some(parsed_fields) = parse_ifd(
        tiff_buf,
        mnote_bytes,
        value_offset_base,
        ifd_offset,
        value_offset_rel_to_mnote,
        little_endian,
    ) {
        fields = parsed_fields;
    }

    (mnote_type, fields)
}

fn parse_ifd(
    tiff_buf: &[u8],
    mnote_bytes: &[u8],
    value_offset_base: usize,
    ifd_offset: usize,
    value_offset_rel_to_mnote: bool,
    little_endian: bool,
) -> Option<Vec<MnoteField>> {
    let n_bytes = mnote_bytes.get(ifd_offset..ifd_offset.checked_add(2)?)?;
    let n = if little_endian {
        u16::from_le_bytes([n_bytes[0], n_bytes[1]])
    } else {
        u16::from_be_bytes([n_bytes[0], n_bytes[1]])
    } as usize;

    let mut fields = Vec::new();
    for i in 0..n {
        let i_mul_12 = i.checked_mul(12)?;
        let entry_start = ifd_offset.checked_add(2)?.checked_add(i_mul_12)?;
        let entry_end = entry_start.checked_add(12)?;
        let entry_bytes = mnote_bytes.get(entry_start..entry_end)?;

        let tag = if little_endian {
            u16::from_le_bytes([entry_bytes[0], entry_bytes[1]])
        } else {
            u16::from_be_bytes([entry_bytes[0], entry_bytes[1]])
        };
        let typ = if little_endian {
            u16::from_le_bytes([entry_bytes[2], entry_bytes[3]])
        } else {
            u16::from_be_bytes([entry_bytes[2], entry_bytes[3]])
        };
        let count = if little_endian {
            u32::from_le_bytes([entry_bytes[4], entry_bytes[5], entry_bytes[6], entry_bytes[7]])
        } else {
            u32::from_be_bytes([entry_bytes[4], entry_bytes[5], entry_bytes[6], entry_bytes[7]])
        };
        let val_or_ofs_bytes = [entry_bytes[8], entry_bytes[9], entry_bytes[10], entry_bytes[11]];

        let type_size: usize = match typ {
            1 => 1,  // BYTE
            2 => 1,  // ASCII
            3 => 2,  // SHORT
            4 => 4,  // LONG
            5 => 8,  // RATIONAL
            7 => 1,  // UNDEFINED
            8 => 2,  // SSHORT
            9 => 4,  // SLONG
            10 => 8, // SRATIONAL
            _ => continue,
        };

        let Some(val_size) = type_size.checked_mul(count as usize) else {
            continue;
        };
        let val_bytes_opt = if val_size <= 4 {
            Some(&val_or_ofs_bytes[0..val_size])
        } else {
            let offset = if little_endian {
                u32::from_le_bytes(val_or_ofs_bytes) as usize
            } else {
                u32::from_be_bytes(val_or_ofs_bytes) as usize
            };

            if value_offset_rel_to_mnote {
                let start = value_offset_base.checked_add(offset)?;
                let end = start.checked_add(val_size)?;
                mnote_bytes.get(start..end)
            } else {
                let end = offset.checked_add(val_size)?;
                tiff_buf.get(offset..end)
            }
        };

        let val_bytes = match val_bytes_opt {
            Some(b) => b,
            None => continue,
        };

        if let Some(val) = parse_value_payload(val_bytes, typ, little_endian) {
            fields.push(MnoteField { tag, value: Value(val) });
        }
    }

    Some(fields)
}

fn parse_value_payload(bytes: &[u8], typ: u16, little_endian: bool) -> Option<KamadakValue> {
    match typ {
        1 => Some(KamadakValue::Byte(bytes.to_vec())),
        2 => {
            let mut ascii_parts = Vec::new();
            let mut current = Vec::new();
            for &b in bytes {
                if b == 0 {
                    if !current.is_empty() {
                        ascii_parts.push(current.clone());
                        current.clear();
                    }
                } else {
                    current.push(b);
                }
            }
            if !current.is_empty() {
                ascii_parts.push(current);
            }
            Some(KamadakValue::Ascii(ascii_parts))
        }
        3 => {
            let mut v = Vec::new();
            for chunk in bytes.chunks_exact(2) {
                let x = if little_endian {
                    u16::from_le_bytes([chunk[0], chunk[1]])
                } else {
                    u16::from_be_bytes([chunk[0], chunk[1]])
                };
                v.push(x);
            }
            Some(KamadakValue::Short(v))
        }
        4 => {
            let mut v = Vec::new();
            for chunk in bytes.chunks_exact(4) {
                let x = if little_endian {
                    u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                } else {
                    u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                };
                v.push(x);
            }
            Some(KamadakValue::Long(v))
        }
        5 => {
            let mut v = Vec::new();
            for chunk in bytes.chunks_exact(8) {
                let num = if little_endian {
                    u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                } else {
                    u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                };
                let denom = if little_endian {
                    u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]])
                } else {
                    u32::from_be_bytes([chunk[4], chunk[5], chunk[6], chunk[7]])
                };
                v.push(exif::Rational::from((num, denom)));
            }
            Some(KamadakValue::Rational(v))
        }
        7 => Some(KamadakValue::Undefined(bytes.to_vec(), 0)),
        8 => {
            let mut v = Vec::new();
            for chunk in bytes.chunks_exact(2) {
                let x = if little_endian {
                    i16::from_le_bytes([chunk[0], chunk[1]])
                } else {
                    i16::from_be_bytes([chunk[0], chunk[1]])
                };
                v.push(x);
            }
            Some(KamadakValue::SShort(v))
        }
        9 => {
            let mut v = Vec::new();
            for chunk in bytes.chunks_exact(4) {
                let x = if little_endian {
                    i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                } else {
                    i32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                };
                v.push(x);
            }
            Some(KamadakValue::SLong(v))
        }
        10 => {
            let mut v = Vec::new();
            for chunk in bytes.chunks_exact(8) {
                let num = if little_endian {
                    i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                } else {
                    i32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                };
                let denom = if little_endian {
                    i32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]])
                } else {
                    i32::from_be_bytes([chunk[4], chunk[5], chunk[6], chunk[7]])
                };
                v.push(exif::SRational::from((num, denom)));
            }
            Some(KamadakValue::SRational(v))
        }
        _ => None,
    }
}
