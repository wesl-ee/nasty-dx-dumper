use anyhow::Result;
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::FileExt,
    path::{Path, PathBuf},
};

use crate::shape::DolHeader;
use crate::utils::{
    load_patch, load_reverse_character_table, pull_dol_header,
    string_to_dx_bytes, TLEntry,
};

pub fn patch_all(dir: PathBuf, out_dir: PathBuf) -> Result<()> {
    for f in std::fs::read_dir(&dir)?.filter(|f| {
        match f {
            Ok(f) => f,
            _ => return false,
        }
        .path() // yeah only process .patch files
        .extension()
        .unwrap_or_default()
        .to_ascii_uppercase()
            == "PATCH"
    }) {
        let f = f?;
        let f_path = f.path();
        if !f.metadata()?.is_file() {
            continue;
        }

        if f_path.file_name().unwrap_or_default().to_ascii_uppercase()
            == "MAIN.DOL.PATCH"
        {
            let dol_f_path = f_path.with_extension("");

            let patcher = DolPatcher::load(&f_path);
            let _dol_added_size = patcher.patch(&dol_f_path, &out_dir)?;

            // let header_f_path = dol_f_path.parent().expect("some parent").join("boot.bin");
            // patch_disk_header_for_extended_main_dol(&header_f_path, dol_added_size, &out_dir)?;
        }
    }

    Ok(())
}

fn patch_disk_header_for_extended_main_dol(
    header_f_path: &PathBuf,
    dol_added_size: u32,
    out_dir: &PathBuf,
) -> Result<()> {
    const FST_OFFSET_OFFSET: usize = 0x0424;
    let out_path = out_dir.join("boot.bin");

    let mut buf = vec![];
    let mut in_fh = File::open(header_f_path)?;
    in_fh.read_to_end(&mut buf)?;

    let mut fst_offset = buf
        .get(FST_OFFSET_OFFSET..FST_OFFSET_OFFSET + 4)
        .and_then(|s| s.try_into().ok())
        .map(u32::from_be_bytes)
        .expect("unable to parse fst offset from boot.bin");

    fst_offset += dol_added_size;
    buf[FST_OFFSET_OFFSET..FST_OFFSET_OFFSET + 4]
        .copy_from_slice(&fst_offset.to_be_bytes());
    let mut out_fh = File::create(out_path)?;
    out_fh.write_all(&buf)?;

    Ok(())
}

fn extend_section_by_size(
    header: &mut DolHeader,
    og_ptr: u32,
    additional_size: u32,
) {
    let mut sections = [
        (&mut header.text0_offset, &mut header.text0_size),
        (&mut header.text1_offset, &mut header.text1_size),
        (&mut header.text2_offset, &mut header.text2_size),
        (&mut header.text3_offset, &mut header.text3_size),
        (&mut header.text4_offset, &mut header.text4_size),
        (&mut header.text5_offset, &mut header.text5_size),
        (&mut header.text6_offset, &mut header.text6_size),
        (&mut header.data0_offset, &mut header.data0_size),
        (&mut header.data1_offset, &mut header.data1_size),
        (&mut header.data2_offset, &mut header.data2_size),
        (&mut header.data3_offset, &mut header.data3_size),
        (&mut header.data4_offset, &mut header.data4_size),
        (&mut header.data5_offset, &mut header.data5_size),
        (&mut header.data6_offset, &mut header.data6_size),
        (&mut header.data7_offset, &mut header.data7_size),
        (&mut header.data8_offset, &mut header.data8_size),
        (&mut header.data9_offset, &mut header.data9_size),
        (&mut header.data10_offset, &mut header.data10_size),
    ];

    // find section to extend
    let mut found_idx = None;
    for (i, (offset, size)) in sections.iter().enumerate() {
        if **size != 0 && og_ptr >= **offset && og_ptr < **offset + **size {
            found_idx = Some(i);
            break;
        }
    }

    let found_idx =
        found_idx.expect(&format!("0x{:x} not in any section", og_ptr));
    let section_end = *sections[found_idx].0 + *sections[found_idx].1;

    // extend this section
    *sections[found_idx].1 += additional_size;

    // shift subsequent sections by that additional amt
    for (offset, size) in &mut sections {
        if **size != 0 && **offset >= section_end {
            **offset += additional_size;
        }
    }

    // lastly shift bss since it follows those sections also
    header.bss_address += additional_size;
}

fn write_dol_header<W: Write>(
    writer: &mut W,
    header: &DolHeader,
) -> Result<()> {
    let offsets = [
        header.text0_offset,
        header.text1_offset,
        header.text2_offset,
        header.text3_offset,
        header.text4_offset,
        header.text5_offset,
        header.text6_offset,
        header.data0_offset,
        header.data1_offset,
        header.data2_offset,
        header.data3_offset,
        header.data4_offset,
        header.data5_offset,
        header.data6_offset,
        header.data7_offset,
        header.data8_offset,
        header.data9_offset,
        header.data10_offset,
    ];
    let addresses = [
        header.text0_address,
        header.text1_address,
        header.text2_address,
        header.text3_address,
        header.text4_address,
        header.text5_address,
        header.text6_address,
        header.data0_address,
        header.data1_address,
        header.data2_address,
        header.data3_address,
        header.data4_address,
        header.data5_address,
        header.data6_address,
        header.data7_address,
        header.data8_address,
        header.data9_address,
        header.data10_address,
    ];
    let sizes = [
        header.text0_size,
        header.text1_size,
        header.text2_size,
        header.text3_size,
        header.text4_size,
        header.text5_size,
        header.text6_size,
        header.data0_size,
        header.data1_size,
        header.data2_size,
        header.data3_size,
        header.data4_size,
        header.data5_size,
        header.data6_size,
        header.data7_size,
        header.data8_size,
        header.data9_size,
        header.data10_size,
    ];

    // offsets
    for &val in &offsets {
        writer.write_all(&val.to_be_bytes())?;
    }
    // addresses
    for &val in &addresses {
        writer.write_all(&val.to_be_bytes())?;
    }
    // sizes
    for &val in &sizes {
        writer.write_all(&val.to_be_bytes())?;
    }

    // bss + entry + padding to 0x100
    writer.write_all(&header.bss_address.to_be_bytes())?;
    writer.write_all(&header.bss_size.to_be_bytes())?;
    writer.write_all(&header.entry_point.to_be_bytes())?;
    writer.write_all(&[0u8; 28])?;

    Ok(())
}

trait BinaryPatcher {
    fn load(patch_file: &Path) -> Self;
    fn patch(&self, in_file: &Path, out_dir: &Path) -> Result<u32>;
}

struct DolPatcher {
    tl: HashMap<u32, TLEntry>,
}

impl BinaryPatcher for DolPatcher {
    fn load(patch_file: &Path) -> Self {
        let rev_table = load_reverse_character_table()
            .expect("failed to load reverse table");
        let entries = load_patch(patch_file).expect("failed to load patch");

        let mut tl = HashMap::new();

        for mut entry in entries {
            entry.jp_bytes = string_to_dx_bytes(&entry.jp_string, &rev_table);
            if let Some(ref en) = entry.en_string {
                entry.en_bytes = Some(string_to_dx_bytes(en, &rev_table));
            }
            tl.insert(entry.og_ptr, entry);
        }

        DolPatcher { tl }
    }

    fn patch(&self, in_file: &Path, out_dir: &Path) -> Result<u32> {
        let mut in_fh = File::open(in_file)?;
        let mut header = pull_dol_header(&mut in_fh)?;
        let original_len = in_fh.metadata()?.len();

        // group new text patching by section
        let mut section_patches: HashMap<
            usize,
            (Vec<u8>, Vec<(u32, u32)>, u32),
        > = HashMap::new();

        for (_, entry) in &self.tl {
            let en_bytes = match &entry.en_bytes {
                Some(b) => b,
                None => continue,
            };

            let sections: Vec<_> = header.section_iter().collect();
            let section_idx = sections
                .iter()
                .position(|s| {
                    entry.og_ptr >= s.file && entry.og_ptr < s.file + s.size
                })
                .expect("ptr not in any section");

            let section = &sections[section_idx];
            let entry_og_ptr = entry.og_ptr;
            let (appended, ptr_updates, _) = section_patches
                .entry(section_idx)
                .or_insert_with(|| (Vec::new(), Vec::new(), entry_og_ptr));

            let new_ram = section.ram + section.size + appended.len() as u32;
            appended.extend_from_slice(en_bytes);

            for &ref_off in &entry.references {
                ptr_updates.push((ref_off, new_ram));
            }
        }

        let mut all_ptr_updates = HashMap::new();
        for (_, ptr_updates, _) in section_patches.values() {
            for &(file_off, new_ram) in ptr_updates {
                all_ptr_updates.insert(file_off, new_ram);
            }
        }

        let original_sections: Vec<_> = header.section_iter().collect();

        // extend header
        for (_, (appended, _, og_ptr)) in &section_patches {
            extend_section_by_size(&mut header, *og_ptr, appended.len() as u32);
        }
        let new_sections: Vec<_> = header.section_iter().collect();

        let out_path = out_dir.join("main.dol");
        let mut out_fh = File::create(&out_path)?;
        write_dol_header(&mut out_fh, &header)?;

        // write sections one by one
        for (section_idx, (orig_section, new_section)) in original_sections
            .iter()
            .zip(new_sections.iter())
            .enumerate()
        {
            if orig_section.size == 0 {
                continue;
            }

            // seek
            out_fh.seek(SeekFrom::Start(new_section.file as u64))?;

            // read at sought offset
            in_fh.seek(SeekFrom::Start(orig_section.file as u64))?;
            let mut section_data = vec![0u8; orig_section.size as usize];
            in_fh.read_exact(&mut section_data)?;

            // update pointers in this section
            for (&file_offset, &new_ram) in &all_ptr_updates {
                if file_offset >= orig_section.file
                    && file_offset + 4 <= orig_section.file + orig_section.size
                {
                    let i = (file_offset - orig_section.file) as usize;
                    section_data[i..i + 4]
                        .copy_from_slice(&new_ram.to_be_bytes());
                }
            }

            // write
            out_fh.write_all(&section_data)?;

            // add translated strings if this section has them
            if let Some((appended, _, _)) = section_patches.get(&section_idx) {
                out_fh.write_all(appended)?;
            }
        }

        // report because I'll probably have to update fst.bin to reflect overall
        // new main.dol size
        let report_path = out_dir.join("report.txt");
        let growth = out_fh.metadata()?.len() - original_len;
        std::fs::write(
            &report_path,
            format!("main.dol grew by {} bytes\n", growth),
        )?;

        Ok(growth.try_into()?)
    }
}
