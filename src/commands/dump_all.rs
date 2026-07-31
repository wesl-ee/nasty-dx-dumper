use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{BufWriter, Write},
    os::unix::fs::FileExt,
    path::PathBuf,
};

use anyhow::Result;

use crate::constants::{
    NameCap, EXCLUDED_NOTES, INLINE_FIELDS, LONG_NAME_GLYPHS,
    TEXT_TABLES_EXPLICIT, WALKING_TABLES,
};
use crate::shape::DolHeader;
use crate::utils::load_character_table;
use crate::{ppc, spt};

pub fn dump_all(dir: PathBuf) -> Result<()> {
    for f in crate::utils::walk_dir(&dir) {
        let f = f?;
        let f_path = f.path();
        let f_extension = f_path.extension().unwrap_or_default();
        if !f.metadata()?.is_file() {
            continue;
        }

        // every file we parse is given a sidecard <something>.<ext>.patch file
        // for TL purposes
        let mut out_file = f_path.parent().expect("parent").to_path_buf();
        out_file.push(f_path.file_name().expect("file_name"));
        out_file.set_extension(format!(
            "{}.{}",
            f_extension.display(),
            "patch"
        ));

        if f_path
            .file_name()
            .unwrap_or_default()
            .eq_ignore_ascii_case("MAIN.DOL")
        {
            dump_main_dol(&f_path, &out_file)?;
        }

        if f_path
            .extension()
            .unwrap_or_default()
            .eq_ignore_ascii_case("SPT")
        {
            dump_spt(&f_path, &out_file)?;
        }
    }

    Ok(())
}

fn dump_main_dol(f_path: &PathBuf, out_file: &PathBuf) -> Result<()> {
    let mut fh = std::fs::File::open(f_path)?;
    let header = DolHeader::read(&mut fh)?;

    let character_table = load_character_table()?;
    // keyed by the file offset of the pointer word. ordered, because this file
    // is read and diffed by translators
    let mut all_indexed_ptrs = BTreeMap::<u32, (u32, &'static str)>::new();

    // Tables with no length: read entries until one stops resolving into a
    // data section, which is our only end-of-table signal.
    let abutting: Vec<u32> = WALKING_TABLES
        .iter()
        .filter(|t| t.abuts)
        .map(|t| t.base)
        .collect();
    let mut word = [0u8; 4];
    for table in WALKING_TABLES {
        let Some(mut entry) = header.file_offset(table.base) else {
            continue;
        };

        loop {
            let mut pointers = Vec::new();
            for &field in table.fields {
                fh.read_exact_at(&mut word, (entry + field) as u64)?;
                match header.file_offset(u32::from_be_bytes(word)) {
                    // a pointer into code is not a string, so the table ended
                    Some(off) if off >= header.data_start() => {
                        pointers.push((entry + field, off))
                    }
                    _ => break,
                }
            }
            if pointers.len() != table.fields.len() {
                break;
            }
            for &field in table.optional {
                fh.read_exact_at(&mut word, (entry + field) as u64)?;
                if let Some(off) = header
                    .file_offset(u32::from_be_bytes(word))
                    .filter(|&off| off >= header.data_start())
                {
                    pointers.push((entry + field, off));
                }
            }
            all_indexed_ptrs.extend(
                pointers.into_iter().map(|(at, off)| (at, (off, table.tag))),
            );

            entry += table.stride;
            if table.abuts
                && header
                    .ram_addr(entry)
                    .is_some_and(|ram| abutting.contains(&ram))
            {
                break;
            }
        }
    }

    // Tables we know the exact extent of. These run last and only fill slots the
    // walking tables above did not already claim, so their output is unchanged.
    let mut caps = HashMap::<u32, NameCap>::default();
    for table in TEXT_TABLES_EXPLICIT {
        for i in 0..table.count {
            for field in table.fields {
                let ref_addr = table.base + i * table.stride + field;
                let Some(ref_offset) = header.file_offset(ref_addr) else {
                    eprintln!(
                        "{}: entry {i} at RAM 0x{ref_addr:x} is outside every section",
                        table.tag
                    );
                    continue;
                };

                fh.read_exact_at(&mut word, ref_offset as u64)?;
                let ptr = u32::from_be_bytes(word);

                // NULL slots are normal: several of these tables are indexed by
                // an id whose range is sparse.
                if ptr == 0 {
                    continue;
                }

                let text_offset = match header.file_offset(ptr) {
                    Some(o) if o >= header.data_start() => o,
                    _ => {
                        eprintln!(
                            "{}: entry {i} at RAM 0x{ref_addr:x} holds 0x{ptr:x}, not a data pointer; skipping",
                            table.tag
                        );
                        continue;
                    }
                };

                // Never emit a pointer we cannot decode: rewriting a word that
                // is not really a string pointer corrupts the game.
                if let Err(e) =
                    dx_str_at_offset(&fh, text_offset, &character_table)
                {
                    eprintln!(
                        "{}: entry {i} points at 0x{text_offset:x} which does not read as text ({e}); skipping",
                        table.tag
                    );
                    continue;
                }

                all_indexed_ptrs
                    .entry(ref_offset)
                    .or_insert((text_offset, table.tag));
                caps.insert(text_offset, table.cap);
            }
        }
    }

    // strings the code addresses with a lis/addi immediate pair. no pointer
    // word exists for these, so the patcher rewrites the instructions instead.
    let buf = std::fs::read(f_path)?;
    let scan = ppc::scan(&buf, &header);
    let imm_refs = scan.usable();
    report_imm_scan(&scan, &imm_refs);

    let mut sorted_text_ptrs = all_indexed_ptrs
        .values()
        .map(|(off, _)| *off)
        .chain(imm_refs.keys().copied())
        .collect::<Vec<_>>();
    sorted_text_ptrs.sort();
    sorted_text_ptrs.dedup();

    let out_fh = std::fs::File::create(out_file)?;
    let mut out_writer = BufWriter::new(out_fh);

    let refs =
        all_indexed_ptrs.len() + imm_refs.values().map(Vec::len).sum::<usize>();
    // the inline records are emitted after the loop below, so they are not in
    // sorted_text_ptrs
    let inline: usize = INLINE_FIELDS.iter().map(|f| f.count as usize).sum();
    writeln!(
        out_writer,
        "# main.dol: {} strings, {refs} references. Translate by repeating the \
         0xOFFSET line with English.",
        sorted_text_ptrs.len() + inline
    )?;
    writeln!(
        out_writer,
        "# every string is relocated into a section appended to the DOL, so a \
         translation may grow freely except where a LIMIT line says otherwise."
    )?;
    writeln!(
        out_writer,
        "# text left in Japanese is listed at the bottom of this file."
    )?;
    writeln!(out_writer)?;

    for text_off in sorted_text_ptrs {
        let str = dx_str_at_offset(&fh, text_off, &character_table)?;
        for (ref_off, (inner_text_off, entry_type)) in all_indexed_ptrs.iter() {
            if text_off == *inner_text_off {
                writeln!(
                    out_writer,
                    "# reference at main.dol:0x{:x} (RAM 0x{:x}) [{}]",
                    ref_off,
                    header.ram_addr(*ref_off).unwrap(),
                    entry_type
                )?;
            }
        }
        for r in imm_refs.get(&text_off).into_iter().flatten() {
            writeln!(
                out_writer,
                "# reference at main.dol:0x{:x} (RAM 0x{:x}) [{} paired with \
                 the lis at RAM 0x{:x}]",
                r.lo_off,
                r.lo_ram,
                r.kind.name(),
                r.hi_ram
            )?;
        }
        match caps.get(&text_off) {
            Some(NameCap::Stub) => writeln!(
                out_writer,
                "# LIMIT {LONG_NAME_GLYPHS} glyphs - a fixed-size memcpy \
                 copies a redirect stub, not these glyphs"
            )?,
            Some(NameCap::Free) | None => {}
        }
        writeln!(out_writer, "0x{text_off:x} {str}")?;
    }

    // fixed-width inline records. no reference line is written because there is
    // no pointer to rewrite: the patcher recognises these by address and
    // overwrites the record where it lies.
    let mut rec = vec![0u8; 2];
    for field in INLINE_FIELDS {
        for i in 0..field.count {
            let ram = field.base + i * field.stride;
            let Some(off) = header.file_offset(ram) else {
                eprintln!(
                    "{}: record {i} at RAM 0x{ram:x} is outside every section",
                    field.tag
                );
                continue;
            };
            let mut text = String::new();
            for g in 0..field.glyphs {
                fh.read_exact_at(&mut rec, (off + g * 2) as u64)?;
                let code = u16::from_be_bytes([rec[0], rec[1]]);
                text.push_str(
                    character_table
                        .get(&code)
                        .unwrap_or(&format!("[0x{code:x}]")),
                );
            }
            writeln!(
                out_writer,
                "# inline {} record {i} at main.dol:0x{off:x} (RAM 0x{ram:x})",
                field.tag
            )?;
            if field.stub {
                writeln!(
                    out_writer,
                    "# LIMIT {LONG_NAME_GLYPHS} glyphs - the record is \
                     overwritten with a redirect stub, not with these glyphs"
                )?;
            } else {
                writeln!(
                    out_writer,
                    "# LIMIT {} glyphs - fixed-width record with no terminator",
                    field.glyphs
                )?;
            }
            // the terminator supplies the blank line that separates entries
            // everywhere else in this file; these records have none.
            writeln!(out_writer, "0x{off:x} {text}\n")?;
        }
    }

    // last, so a translator opening the file lands on work rather than on a
    // wall of addresses. `load_patch` only reads "# reference at" and "0x"
    // lines, so these are inert.
    writeln!(out_writer, "# NOT extracted, and why:")?;
    for (addr, note) in EXCLUDED_NOTES {
        writeln!(out_writer, "# 0x{addr:08X} - {}", note[0])?;
        for line in &note[1..] {
            writeln!(out_writer, "#   {line}")?;
        }
    }
    for (why, addrs) in rejected_by_reason(&scan) {
        writeln!(
            out_writer,
            "# {} glyph run(s) addressed by code we will not rewrite, written \
             below as run@instruction: {why}.",
            addrs.len()
        )?;
        for line in addrs.chunks(4) {
            let line: Vec<_> = line
                .iter()
                .map(|(a, at)| format!("0x{a:08X}@0x{at:08X}"))
                .collect();
            writeln!(out_writer, "#   {}", line.join(" "))?;
        }
    }

    Ok(())
}

/// Why the instruction scan left text behind, on stderr where the other
/// skipped-entry notices already go. A silent exclusion is a translation that
/// never happens.
fn report_imm_scan(scan: &ppc::Scan, usable: &HashMap<u32, Vec<&ppc::ImmRef>>) {
    let reasons = rejected_by_reason(scan);
    eprintln!(
        "instruction-stream text: {} strings from {} immediate pairs, {} \
         addressed runs left out",
        usable.len(),
        scan.refs.len(),
        scan.rejected.len()
    );
    for (why, addrs) in reasons {
        eprintln!("  {:4} left out: {why}", addrs.len());
    }
}

fn rejected_by_reason(scan: &ppc::Scan) -> Vec<(&str, Vec<(u32, u32)>)> {
    let mut by_reason = HashMap::<&str, Vec<(u32, u32)>>::new();
    for (addr, r) in &scan.rejected {
        by_reason
            .entry(r.why.as_str())
            .or_default()
            .push((*addr, r.at_ram));
    }
    let mut out: Vec<_> = by_reason.into_iter().collect();
    for (_, addrs) in out.iter_mut() {
        addrs.sort();
    }
    out.sort_by_key(|(why, addrs)| (std::cmp::Reverse(addrs.len()), *why));
    out
}

fn dx_str_at_offset(
    fh: &File,
    mut offset: u32,
    table: &HashMap<u16, String>,
) -> Result<String> {
    // read text. the longest string the game ships is 163 glyphs, so a run that
    // gets this far is not a string and the caller needs to hear about it
    // rather than get a megabyte of garbage.
    const MAX_GLYPHS: usize = 4096;

    let mut ch_buf = [0u8; 2];
    let mut text_data = String::new();
    for _ in 0..MAX_GLYPHS {
        fh.read_exact_at(&mut ch_buf, offset as u64)?;
        let ch_raw = u16::from_be_bytes(ch_buf);
        let ch_interpreted = table
            .get(&ch_raw)
            .unwrap_or(&format!("[0x{ch_raw:x}]"))
            .clone();
        text_data.push_str(&ch_interpreted);

        if ch_raw == 0xf800 {
            return Ok(text_data);
        }

        offset += 2;
    }

    anyhow::bail!("no terminator within {MAX_GLYPHS} glyphs of 0x{offset:x}")
}

/// Walk the script the way the interpreter does and write one entry per string,
/// with every reference to it. Grouping by string rather than by reference is
/// what makes the all-or-none rule the recycling writer imposes satisfiable:
/// one `en` line satisfies all of a string's readers at once.
fn dump_spt(f_path: &PathBuf, out_file: &PathBuf) -> Result<()> {
    let buf = std::fs::read(f_path)?;
    let character_table = load_character_table().expect("no character table");
    let plan = spt::plan(&buf, &character_table);
    let name = f_path.file_name().expect("file_name").display().to_string();

    let out_fh = std::fs::File::create(out_file)?;
    let mut out_writer = BufWriter::new(out_fh);

    let by_offset = plan.by_offset();
    let arena = plan.arena_bytes();
    writeln!(
        out_writer,
        "# {}: {} strings, {} references. Translate by repeating the 0xOFFSET \
         line with English.",
        name,
        by_offset.len(),
        plan.strings.len()
    )?;
    writeln!(
        out_writer,
        "# record cursor 0x{:04x}, script ends 0x{:04x}, {}",
        plan.record_cursor,
        plan.script_end,
        if arena > 0 {
            format!("{arena} bytes of arena shared by every replacement")
        } else {
            "no arena: a string may not grow past the Japanese it replaces"
                .to_string()
        }
    )?;
    writeln!(out_writer)?;

    for (offset, users) in &by_offset {
        for u in users {
            writeln!(
                out_writer,
                "# reference at {}:0x{:x} (cmd {} {}, slot {})",
                name,
                u.at,
                u.kind.cmd(),
                u.kind,
                match u.slot {
                    Some(s) => format!("0x{s:x}"),
                    None => "unwritable".to_string(),
                }
            )?;
        }
        if let Some(cap) = plan.cap(users, (users[0].end - offset) / 2 - 1) {
            writeln!(
                out_writer,
                "# cap {cap} glyphs: this is a name field, and over the cap the \
                 0x12-byte copy loses its terminator"
            )?;
        }
        writeln!(out_writer, "0x{:x} {}", offset, users[0].jp)?;
        writeln!(out_writer)?;
    }

    Ok(())
}
