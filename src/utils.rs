use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};

use crate::shape::DolHeader;

pub struct TLEntry {
    pub og_ptr: u32,
    pub references: Vec<u32>,
    pub jp_string: String,
    pub jp_bytes: Vec<u8>,
    pub en_string: Option<String>,
    pub en_bytes: Option<Vec<u8>>,
}

pub fn print_hex_dump(data: &[u8], max_bytes: usize) {
    let bytes_to_show = std::cmp::min(data.len(), max_bytes);
    for (i, byte) in data[..bytes_to_show].iter().enumerate() {
        if i % 16 == 0 && i > 0 {
            println!();
        }
        if i % 16 == 0 {
            print!("{:08x}: ", i);
        }
        print!("{:02x} ", byte);
    }
    if bytes_to_show > 0 {
        println!();
    }
}

pub fn pull_dol_header(fh: &mut File) -> Result<DolHeader> {
    let mut buf = [0u8; 4];
    let mut header = DolHeader::default();

    // offsets
    fh.read_exact(&mut buf)?;
    header.text0_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text1_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text2_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text3_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text4_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text5_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text6_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data0_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data1_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data2_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data3_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data4_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data5_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data6_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data7_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data8_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data9_offset = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data10_offset = u32::from_be_bytes(buf);

    // addresses
    fh.read_exact(&mut buf)?;
    header.text0_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text1_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text2_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text3_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text4_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text5_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text6_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data0_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data1_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data2_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data3_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data4_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data5_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data6_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data7_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data8_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data9_address = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data10_address = u32::from_be_bytes(buf);

    // sizes
    fh.read_exact(&mut buf)?;
    header.text0_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text1_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text2_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text3_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text4_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text5_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.text6_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data0_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data1_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data2_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data3_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data4_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data5_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data6_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data7_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data8_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data9_size = u32::from_be_bytes(buf);
    fh.read_exact(&mut buf)?;
    header.data10_size = u32::from_be_bytes(buf);

    Ok(header)
}

pub fn load_character_table() -> Result<HashMap<u16, String>> {
    let mut table = HashMap::new();
    let content = std::fs::read_to_string("Table.txt")
        .with_context(|| "Failed to read Table.txt")?;

    for line in content.lines() {
        if let Some((hex_str, char_str)) = line.split_once('=') {
            if let Ok(code) = u16::from_str_radix(hex_str, 16) {
                table.insert(code, char_str.to_string());
            }
        }
    }

    table.insert(0xF800, "\n".to_string());
    Ok(table)
}

pub fn load_reverse_character_table() -> Result<HashMap<String, u16>> {
    let mut table = HashMap::new();
    let content = std::fs::read_to_string("Table.txt")
        .with_context(|| "Failed to read Table.txt")?;

    for line in content.lines() {
        if let Some((hex_str, char_str)) = line.split_once('=') {
            if let Ok(code) = u16::from_str_radix(hex_str, 16) {
                table.insert(char_str.to_string(), code);
            }
        }
    }

    table.insert("\n".to_string(), 0xF800);
    Ok(table)
}

pub fn string_to_dx_bytes(s: &str, table: &HashMap<String, u16>) -> Vec<u8> {
    let mut result = Vec::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '[' {
            let bracket_content: String =
                chars.by_ref().take_while(|&ch| ch != ']').collect();
            let full = format!("[{}]", bracket_content);

            if bracket_content.starts_with("0x") {
                if let Ok(code) = u16::from_str_radix(&bracket_content[2..], 16)
                {
                    result.extend(code.to_be_bytes());
                }
            } else if let Some(&code) = table.get(&full) {
                result.extend(code.to_be_bytes());
            }
        } else {
            if let Some(&code) = table.get(&c.to_string()) {
                result.extend(code.to_be_bytes());
            }
        }
    }

    result
}

pub fn load_patch(patch_file: &Path) -> Result<Vec<TLEntry>> {
    let in_fh = File::open(patch_file)?;
    let reader = BufReader::new(in_fh);

    let mut entries = Vec::new();
    let mut current = TLEntry {
        og_ptr: 0,
        references: Vec::new(),
        jp_string: String::new(),
        jp_bytes: Vec::new(),
        en_string: None,
        en_bytes: None,
    };
    let mut seen_jp = false;

    for l in reader.lines() {
        let l = l?;

        if l.trim().is_empty() {
            if seen_jp {
                entries.push(current);
                current = TLEntry {
                    og_ptr: 0,
                    references: Vec::new(),
                    jp_string: String::new(),
                    jp_bytes: Vec::new(),
                    en_string: None,
                    en_bytes: None,
                };
                seen_jp = false;
            }
        } else if l.starts_with("# reference at") {
            let reference_to_eol =
                &l[l.find(':').expect("malformed reference line") + 3..];
            let reference_offset = u32::from_str_radix(
                reference_to_eol.split_once(' ').unwrap_or_default().0,
                16,
            )?;
            current.references.push(reference_offset);
        } else if l.starts_with("0x") {
            let ptr_to_eol = &l[2..];
            let (ptr_offset, text_content) =
                ptr_to_eol.split_once(' ').unwrap_or_default();
            let ptr_offset = u32::from_str_radix(ptr_offset, 16)?;

            if !seen_jp {
                current.og_ptr = ptr_offset;
                current.jp_string = text_content.to_string();
                seen_jp = true;
            } else if ptr_offset == current.og_ptr {
                current.en_string = Some(text_content.to_string());
            }
        }
    }

    if seen_jp {
        entries.push(current);
    }

    Ok(entries)
}
