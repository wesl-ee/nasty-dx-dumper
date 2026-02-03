use anyhow::Result;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    os::unix::fs::FileExt,
    path::{Path, PathBuf},
};

use crate::utils::{
    load_patch, load_reverse_character_table, pull_dol_header,
    string_to_dx_bytes, TLEntry,
};

pub fn patch_all(dir: PathBuf, patch_dir: PathBuf) -> Result<()> {
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
        let f_extension = f_path.extension().unwrap_or_default();
        if !f.metadata()?.is_file() {
            continue;
        }

        let in_fh = std::fs::File::open(f_path.clone())?;
        let reader = BufReader::new(in_fh);

        // this is the format of every .PATCH file we wrote in dump_all
        let mut ptr = TextPtr::default();
        for l in reader.lines() {
            let l = l?;
            if l.starts_with("#") {
                // # reference at <f_name>.<ext>:0x
                let reference_to_eol = &l[l
                    .find(|c| c == ':')
                    .expect("malformed reference line")
                    + 3..];
                let reference_offset = u32::from_str_radix(
                    reference_to_eol.split_once(" ").unwrap_or_default().0,
                    16,
                )?;

                ptr.references.push(reference_offset);

                // reference lines
            } else if l.starts_with("0x") {
                // ptr line
                let ptr_to_eol = &l[2..];
                let (ptr_offset, text_content) =
                    ptr_to_eol.split_once(" ").unwrap_or_default();
                let ptr_offset = u32::from_str_radix(ptr_offset, 16)?;

                ptr.ptr = ptr_offset;
                println!("0x{:x} {}", ptr_offset, text_content);
            } else if l == "\n" {
                ptr = TextPtr::default();
            }
        }

        if f_path.file_name().unwrap_or_default().to_ascii_uppercase()
            == "MAIN.DOL.PATCH"
        {
            let dol_f_path = f_path.with_extension("");

            println!("\n=== Testing DolPatcher ===");
            let patcher = DolPatcher::load(&dol_f_path, &f_path);
            patcher.patch(&dol_f_path)?;
            println!("=== End DolPatcher test ===\n");

            let mut dol_fh = std::fs::File::open(dol_f_path)?;
            let header = pull_dol_header(&mut dol_fh)?;

            println!("offset 0x{:x}", header.data5_offset);
            println!("RAM 0x{:x}", header.data5_address);

            // Our strategy for translating dol files is to copy the DOL, take the section that
            // text exists in and append extra strings to the end of this section, then update the
            // pointers we extracted earlier to point to the new strings we inserted
            //
            // Since we control the header offsets and sizes we are able to extend a section
            // arbitrarily.
        }
    }

    Ok(())
}

trait BinaryPatcher {
    fn load(in_file: &Path, patch_file: &Path) -> Self;
    fn patch(&self, out_file: &Path) -> Result<()>;
}

struct DolPatcher {
    original_ptr: HashMap<u32, Vec<u32>>,
    tl: HashMap<u32, TLEntry>,
}

impl BinaryPatcher for DolPatcher {
    fn load(_in_file: &Path, patch_file: &Path) -> Self {
        let rev_table = load_reverse_character_table()
            .expect("failed to load reverse table");
        let entries = load_patch(patch_file).expect("failed to load patch");

        let mut original_ptr = HashMap::new();
        let mut tl = HashMap::new();

        for mut entry in entries {
            entry.jp_bytes = string_to_dx_bytes(&entry.jp_string, &rev_table);
            if let Some(ref en) = entry.en_string {
                entry.en_bytes = Some(string_to_dx_bytes(en, &rev_table));
            }
            original_ptr.insert(entry.og_ptr, entry.references.clone());
            tl.insert(entry.og_ptr, entry);
        }

        DolPatcher { original_ptr, tl }
    }

    fn patch(&self, _out_file: &Path) -> Result<()> {
        for (ptr, entry) in &self.tl {
            if let Some(ref en) = entry.en_string {
                println!("0x{:x}: {} -> {}", ptr, entry.jp_string, en);
                println!("  refs: {:?}", entry.references);
                println!("  jp_bytes: {:02x?}", entry.jp_bytes);
                println!(
                    "  en_bytes: {:02x?}",
                    entry.en_bytes.as_ref().unwrap()
                );
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct TextPtr {
    ptr: u32,
    references: Vec<u32>,
}
