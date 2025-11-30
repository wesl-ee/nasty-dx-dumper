use anyhow::Result;
use std::{
    io::{BufRead, BufReader, Read},
    path::PathBuf,
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
        {}
    }

    Ok(())
}

#[derive(Default)]
struct TextPtr {
    ptr: u32,
    references: Vec<u32>,
}
