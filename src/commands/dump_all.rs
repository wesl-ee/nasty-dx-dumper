use std::{
    collections::HashMap,
    fs::File,
    io::{BufWriter, Read, Write},
    os::unix::fs::FileExt,
    path::PathBuf,
};

use anyhow::{Context, Result};

use crate::constants::{
    TEXT_TABLES_A, TEXT_TABLES_B, TEXT_TABLES_C, TEXT_TABLES_D, TEXT_TABLES_E,
    TEXT_TABLES_F, TEXT_TABLES_G,
};
use crate::{
    constants::TEXT_TABLES_H,
    shape::{
        TextTableEntry as _, TextTableEntryA, TextTableEntryB, TextTableEntryC,
        TextTableEntryD, TextTableEntryE, TextTableEntryF, TextTableEntryG,
        TextTableEntryH,
    },
};

#[derive(Default)]
struct DolHeader {
    text0_offset: u32,
    text1_offset: u32,
    text2_offset: u32,
    text3_offset: u32,
    text4_offset: u32,
    text5_offset: u32,
    text6_offset: u32,
    data0_offset: u32,
    data1_offset: u32,
    data2_offset: u32,
    data3_offset: u32,
    data4_offset: u32,
    data5_offset: u32,
    data6_offset: u32,
    data7_offset: u32,
    data8_offset: u32,
    data9_offset: u32,
    data10_offset: u32,

    text0_address: u32,
    text1_address: u32,
    text2_address: u32,
    text3_address: u32,
    text4_address: u32,
    text5_address: u32,
    text6_address: u32,
    data0_address: u32,
    data1_address: u32,
    data2_address: u32,
    data3_address: u32,
    data4_address: u32,
    data5_address: u32,
    data6_address: u32,
    data7_address: u32,
    data8_address: u32,
    data9_address: u32,
    data10_address: u32,

    text0_size: u32,
    text1_size: u32,
    text2_size: u32,
    text3_size: u32,
    text4_size: u32,
    text5_size: u32,
    text6_size: u32,
    data0_size: u32,
    data1_size: u32,
    data2_size: u32,
    data3_size: u32,
    data4_size: u32,
    data5_size: u32,
    data6_size: u32,
    data7_size: u32,
    data8_size: u32,
    data9_size: u32,
    data10_size: u32,
    // there's other stuff in a dol header but not relevant for text dumping
}

struct DolSection {
    file: u32,
    ram: u32,
    size: u32,
}

impl DolHeader {
    fn section_iter(&self) -> impl Iterator<Item = DolSection> + '_ {
        [
            // text
            (self.text0_offset, self.text0_address, self.text0_size),
            (self.text1_offset, self.text1_address, self.text1_size),
            (self.text2_offset, self.text2_address, self.text2_size),
            (self.text3_offset, self.text3_address, self.text3_size),
            (self.text4_offset, self.text4_address, self.text4_size),
            (self.text5_offset, self.text5_address, self.text5_size),
            (self.text6_offset, self.text6_address, self.text6_size),
            // data
            (self.data0_offset, self.data0_address, self.data0_size),
            (self.data1_offset, self.data1_address, self.data1_size),
            (self.data2_offset, self.data2_address, self.data2_size),
            (self.data3_offset, self.data3_address, self.data3_size),
            (self.data4_offset, self.data4_address, self.data4_size),
            (self.data5_offset, self.data5_address, self.data5_size),
            (self.data6_offset, self.data6_address, self.data6_size),
            (self.data7_offset, self.data7_address, self.data7_size),
            (self.data8_offset, self.data8_address, self.data8_size),
            (self.data9_offset, self.data9_address, self.data9_size),
            (self.data10_offset, self.data10_address, self.data10_size),
        ]
        .into_iter()
        .filter(|(_, _, size)| *size != 0)
        .map(|(file, ram, size)| DolSection { file, ram, size })
    }
}

pub fn dump_all(dir: PathBuf, dir_out: PathBuf) -> Result<()> {
    std::fs::create_dir_all(&dir_out)?;

    for f in std::fs::read_dir(dir)? {
        let f = f?;
        let f_path = f.path();
        let f_extension = f_path.extension().unwrap_or_default();
        if !f.metadata()?.is_file() {
            continue;
        }

        let mut out_file = dir_out.clone();
        out_file.push(f_path.file_name().expect("file_name"));
        out_file.set_extension(format!(
            "{}.{}",
            f_extension.display(),
            "patch"
        ));

        if f_path.file_name().unwrap_or_default().to_ascii_uppercase()
            == "MAIN.DOL"
        {
            dump_main_dol(&f_path, &out_file)?;
        }

        if f_path.extension().unwrap_or_default().to_ascii_uppercase() == "SPT"
        {
            dump_spt(&f_path, &out_file);
        }
    }

    Ok(())
}

fn dump_main_dol(f_path: &PathBuf, out_file: &PathBuf) -> Result<()> {
    let mut fh = std::fs::File::open(f_path)?;

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

    let character_table = load_character_table().expect("no character table");
    let mut all_indexed_ptrs = HashMap::<u32, u32>::default();

    for ptr_table_addr in TEXT_TABLES_A {
        let mut curr_offset =
            ram_2_dol_offset(&header, ptr_table_addr).expect("not in section");
        let mut buf = [0u8; TextTableEntryA::SIZE];

        loop {
            fh.read_exact_at(&mut buf, curr_offset as u64)?;

            let entry = match TextTableEntryA::from_bytes(&buf) {
                Some(e) => e,
                None => break,
            };

            // We don't have a length for these tables to work with, so we do our best to detect if
            // we're at the end of the table. This involves (1) Checking if the address we read is
            // a valid offset
            let text_entry_offset = match ram_2_dol_offset(&header, entry.ptr) {
                Some(o) => o,
                None => break,
            };
            // and (2) checking if the address we read is in a data section
            if text_entry_offset < header.data0_offset {
                break;
            }

            all_indexed_ptrs.insert(curr_offset, text_entry_offset);

            curr_offset += TextTableEntryA::SIZE as u32;

            // dat tables are right next to each other
            if TEXT_TABLES_A
                .iter()
                .find(|&p| {
                    dol_offset_2_ram(&header, curr_offset)
                        .expect("not in section")
                        == *p
                })
                .is_some()
            {
                break;
            }
        }
    }

    for ptr_table_addr in TEXT_TABLES_B {
        let mut curr_offset =
            ram_2_dol_offset(&header, ptr_table_addr).expect("not in section");
        let mut buf = [0u8; TextTableEntryB::SIZE];

        loop {
            fh.read_exact_at(&mut buf, curr_offset as u64)?;

            let entry = match TextTableEntryB::from_bytes(&buf) {
                Some(e) => e,
                None => break,
            };

            let text_1_offset = match ram_2_dol_offset(&header, entry.text_1) {
                Some(o) => o,
                None => break,
            };
            let text_2_offset = match ram_2_dol_offset(&header, entry.text_2) {
                Some(o) => o,
                None => break,
            };
            let text_3_offset = match ram_2_dol_offset(&header, entry.text_3) {
                Some(o) => o,
                None => break,
            };

            // sanity check
            if text_1_offset < header.data0_offset
                || text_2_offset < header.data0_offset
                || text_3_offset < header.data0_offset
            {
                break;
            }

            all_indexed_ptrs.insert(curr_offset, text_1_offset);
            all_indexed_ptrs.insert(curr_offset + 4, text_2_offset);
            all_indexed_ptrs.insert(curr_offset + 8, text_3_offset);

            curr_offset += TextTableEntryB::SIZE as u32;
        }
    }

    for ptr_table_addr in TEXT_TABLES_C {
        let mut curr_offset =
            ram_2_dol_offset(&header, ptr_table_addr).expect("not in section");
        let mut buf = [0u8; TextTableEntryC::SIZE];

        loop {
            fh.read_exact_at(&mut buf, curr_offset as u64)?;

            let entry = match TextTableEntryC::from_bytes(&buf) {
                Some(e) => e,
                None => break,
            };

            let text_1_offset = match ram_2_dol_offset(&header, entry.text_1) {
                Some(o) => o,
                None => break,
            };
            let text_2_offset = match ram_2_dol_offset(&header, entry.text_2) {
                Some(o) => o,
                None => break,
            };

            // sanity check
            if text_1_offset < header.data0_offset
                || text_2_offset < header.data0_offset
            {
                break;
            }

            all_indexed_ptrs.insert(curr_offset, text_1_offset);
            all_indexed_ptrs.insert(curr_offset + 4, text_2_offset);

            curr_offset += TextTableEntryC::SIZE as u32;
        }
    }

    for ptr_table_addr in TEXT_TABLES_D {
        let mut curr_offset =
            ram_2_dol_offset(&header, ptr_table_addr).expect("not in section");
        let mut buf = [0u8; TextTableEntryD::SIZE];

        loop {
            fh.read_exact_at(&mut buf, curr_offset as u64)?;

            let entry = match TextTableEntryD::from_bytes(&buf) {
                Some(e) => e,
                None => break,
            };

            let text_1_offset = match ram_2_dol_offset(&header, entry.text_1) {
                Some(o) => o,
                None => break,
            };
            let text_2_offset = match ram_2_dol_offset(&header, entry.text_2) {
                Some(o) => o,
                None => break,
            };

            // sanity check
            if text_1_offset < header.data0_offset
                || text_2_offset < header.data0_offset
            {
                break;
            }

            all_indexed_ptrs.insert(curr_offset, text_1_offset);
            all_indexed_ptrs.insert(curr_offset + 4, text_2_offset);

            curr_offset += TextTableEntryD::SIZE as u32;
        }
    }

    for ptr_table_addr in TEXT_TABLES_E {
        let mut curr_offset =
            ram_2_dol_offset(&header, ptr_table_addr).expect("not in section");
        let mut buf = [0u8; TextTableEntryE::SIZE];

        loop {
            fh.read_exact_at(&mut buf, curr_offset as u64)?;

            let entry = match TextTableEntryE::from_bytes(&buf) {
                Some(e) => e,
                None => break,
            };

            let text_1_offset = match ram_2_dol_offset(&header, entry.text_1) {
                Some(o) => o,
                None => break,
            };
            let text_2_offset = match ram_2_dol_offset(&header, entry.text_2) {
                Some(o) => o,
                None => break,
            };

            // sanity check
            if text_1_offset < header.data0_offset
                || text_2_offset < header.data0_offset
            {
                break;
            }

            all_indexed_ptrs.insert(curr_offset, text_1_offset);
            all_indexed_ptrs.insert(curr_offset + 4, text_2_offset);

            curr_offset += TextTableEntryE::SIZE as u32;
        }
    }

    for ptr_table_addr in TEXT_TABLES_F {
        let mut curr_offset =
            ram_2_dol_offset(&header, ptr_table_addr).expect("not in section");
        let mut buf = [0u8; TextTableEntryF::SIZE];

        loop {
            fh.read_exact_at(&mut buf, curr_offset as u64)?;

            let entry = match TextTableEntryF::from_bytes(&buf) {
                Some(e) => e,
                None => break,
            };

            let text_1_offset = match ram_2_dol_offset(&header, entry.text_1) {
                Some(o) => o,
                None => break,
            };
            let maybe_text_2_offset =
                ram_2_dol_offset(&header, entry.maybe_text_2);

            // sanity check
            if text_1_offset < header.data0_offset {
                break;
            }

            if maybe_text_2_offset.unwrap_or_default() >= header.data0_offset {
                // really not sure why some entries have a pointer and others do not
                all_indexed_ptrs
                    .insert(curr_offset + 4, maybe_text_2_offset.unwrap());
            }

            all_indexed_ptrs.insert(curr_offset, text_1_offset);

            curr_offset += TextTableEntryF::SIZE as u32;
        }
    }

    for ptr_table_addr in TEXT_TABLES_G {
        let mut curr_offset =
            ram_2_dol_offset(&header, ptr_table_addr).expect("not in section");
        let mut buf = [0u8; TextTableEntryG::SIZE];

        loop {
            fh.read_exact_at(&mut buf, curr_offset as u64)?;

            let entry = match TextTableEntryG::from_bytes(&buf) {
                Some(e) => e,
                None => break,
            };

            let text_1_offset = match ram_2_dol_offset(&header, entry.text_1) {
                Some(o) => o,
                None => break,
            };

            // sanity check
            if text_1_offset < header.data0_offset {
                break;
            }

            all_indexed_ptrs.insert(curr_offset, text_1_offset);

            curr_offset += TextTableEntryG::SIZE as u32;
        }
    }

    for ptr_table_addr in TEXT_TABLES_H {
        let mut curr_offset =
            ram_2_dol_offset(&header, ptr_table_addr).expect("not in section");
        let mut buf = [0u8; TextTableEntryH::SIZE];

        loop {
            fh.read_exact_at(&mut buf, curr_offset as u64)?;

            let entry = match TextTableEntryH::from_bytes(&buf) {
                Some(e) => e,
                None => break,
            };

            let text_1_offset = match ram_2_dol_offset(&header, entry.text_1) {
                Some(o) => o,
                None => break,
            };

            // sanity check
            if text_1_offset < header.data0_offset {
                break;
            }

            all_indexed_ptrs.insert(curr_offset, text_1_offset);

            curr_offset += TextTableEntryH::SIZE as u32;
        }
    }

    let mut sorted_text_ptrs = all_indexed_ptrs.values().collect::<Vec<_>>();
    sorted_text_ptrs.sort();
    sorted_text_ptrs.dedup();

    let out_fh = std::fs::File::create(out_file)?;
    let mut out_writer = BufWriter::new(out_fh);

    for v in sorted_text_ptrs {
        let str = dx_str_at_offset(&fh, *v, &character_table)?;
        for (k, inner_v) in all_indexed_ptrs.iter() {
            if v == inner_v {
                writeln!(
                    out_writer,
                    "# reference at main.dol:0x{:x} (RAM 0x{:x})",
                    k,
                    dol_offset_2_ram(&header, *k).unwrap()
                )?;
            }
        }
        writeln!(out_writer, "0x{:x} {}", v, str)?;
    }

    Ok(())
}

fn dx_str_at_offset(
    fh: &File,
    mut offset: u32,
    table: &HashMap<u16, String>,
) -> Result<String> {
    // read text
    let mut ch_buf = [0u8; 2];
    let mut text_data = String::new();
    loop {
        fh.read_exact_at(&mut ch_buf, offset as u64)?;
        let ch_raw = u16::from_be_bytes(ch_buf);
        let ch_interpreted = table
            .get(&ch_raw)
            .unwrap_or(&format!("[0x{:x}]", ch_raw))
            .clone();
        text_data.push_str(&ch_interpreted);

        if ch_raw == 0xf800 {
            break;
        }

        offset += 2;
    }

    Ok(text_data)
}

fn dump_spt(_f_path: &PathBuf, _out_file: &PathBuf) {
    // let mut fh = std::fs::File::open(f_path)?;

    // let mut table_size = [0u8; 2];
    // fh.read_exact(&mut table_size)?;
    // let table_size = u16::from_be_bytes(table_size);

    // let mut ptr_cnt = [0u8; 2];
    // fh.read_exact(&mut ptr_cnt)?;
    // let table_size = u16::from_be_bytes(ptr_cnt);

    // let mut table_data = Vec::with_capacity(table_size.into());
    // fh.rewind()?;
    // fh.read_exact(&mut table_data)?;

    // let text_offset = match table_data
    //     .chunks_exact(4)
    //     .find(|c| {
    //         c[0] == 0 &&
    //         c[1] == 0 &&
    //         c[2] == 0xf8 &&
    //         c[3] == 0
    //     }) {
    //         Some(o) => o,
    //         None => {
    //             println!("could not find text table");
    //             continue
    //         }
    // };
}

fn load_character_table() -> Result<HashMap<u16, String>> {
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

fn ram_2_dol_offset(h: &DolHeader, addr: u32) -> Option<u32> {
    h.section_iter().find_map(|s| {
        addr.checked_sub(s.ram)
            .and_then(|d| (d < s.size).then_some(s.file + d))
    })
}

fn dol_offset_2_ram(h: &DolHeader, off: u32) -> Option<u32> {
    h.section_iter().find_map(|s| {
        off.checked_sub(s.file)
            .and_then(|d| (d < s.size).then_some(s.ram + d))
    })
}
