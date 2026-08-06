use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use crate::constants::LATIN_SPACE;

#[derive(Default)]
pub struct TLEntry {
    pub og_ptr: u32,
    pub references: Vec<u32>,
    pub en_string: Option<String>,
}

/// `Table.txt` as (code, rendering) pairs, in file order.
fn character_table() -> Result<Vec<(u16, String)>> {
    let content = std::fs::read_to_string("Table.txt")
        .with_context(|| "Failed to read Table.txt")?;

    Ok(content
        .lines()
        .filter_map(|line| {
            let (hex, glyph) = line.split_once('=')?;
            Some((u16::from_str_radix(hex, 16).ok()?, glyph.to_string()))
        })
        .collect())
}

pub fn load_character_table() -> Result<HashMap<u16, String>> {
    let mut table: HashMap<u16, String> =
        character_table()?.into_iter().collect();
    table.insert(0xF800, "\n".to_string());
    table.insert(LATIN_SPACE, " ".to_string());
    Ok(table)
}

pub fn load_reverse_character_table() -> Result<HashMap<String, u16>> {
    let mut table = HashMap::new();
    for (code, glyph) in character_table()? {
        // twelve renderings are shared by two codes; take the lower one so
        // decode -> encode is stable
        table
            .entry(glyph)
            .and_modify(|c: &mut u16| *c = (*c).min(code))
            .or_insert(code);
    }

    table.insert("\n".to_string(), 0xF800);
    // The one place `Table.txt` is overruled rather than corrected: it maps a
    // space to 0x0000, the full-width Japanese one the padder still writes, and
    // that entry has to stay for `[0x0000]` to keep meaning what it means.
    table.insert(" ".to_string(), LATIN_SPACE);
    Ok(table)
}

/// Longest-match encoder over the reverse character table, which refuses text
/// it cannot encode rather than dropping the character.
///
/// A writer that quietly shortens text can corrupt reviewed copy and hide
/// exactly the errors the fixed-width caps exist to catch. `"`, `#`, `$`, `:`,
/// `;`, backtick and `|` have no glyph, so this fires on real English copy.
pub struct Encoder {
    by_first: HashMap<char, Vec<(String, u16)>>,
}

impl Encoder {
    pub fn new(table: &HashMap<String, u16>) -> Encoder {
        let mut by_first: HashMap<char, Vec<(String, u16)>> = HashMap::new();
        for (token, &code) in table {
            // the terminator renders as "][" in Table.txt, a decode artefact
            // that would let a bracket pair end the string early
            match token.chars().next() {
                Some(c) if code != 0xF800 => {
                    by_first.entry(c).or_default().push((token.clone(), code))
                }
                _ => continue,
            }
        }
        for tokens in by_first.values_mut() {
            tokens
                .sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(a.1.cmp(&b.1)));
        }
        Encoder { by_first }
    }

    pub fn encode(&self, text: &str) -> std::result::Result<Vec<u16>, String> {
        let mut codes = Vec::new();
        let mut i = 0;
        while i < text.len() {
            let rest = &text[i..];
            if let Some((code, len)) = raw_escape(rest) {
                codes.push(code);
                i += len;
                continue;
            }
            let ch = rest.chars().next().expect("non-empty");
            let hit = self.by_first.get(&ch).and_then(|t| {
                t.iter().find(|(tok, _)| rest.starts_with(tok.as_str()))
            });
            match hit {
                Some((tok, code)) => {
                    codes.push(*code);
                    i += tok.len();
                }
                None => {
                    return Err(format!(
                    "cannot encode {ch:?} at position {i} (not in the glyph \
                         table; use [0xhhhh] for a raw code)"
                ))
                }
            }
        }
        Ok(codes)
    }
}

/// `[0xhhhh]` -> (code, byte length consumed)
fn raw_escape(s: &str) -> Option<(u16, usize)> {
    let body = s.strip_prefix("[0x")?;
    let end = body.find(']')?;
    (1..=4)
        .contains(&end)
        .then(|| u16::from_str_radix(&body[..end], 16).ok())
        .flatten()
        .map(|c| (c, 4 + end))
}

/// One entry per blank-line-separated block: the Japanese `0xPTR` line, any
/// `# reference at` lines above it, and the English repeat of the same pointer.
pub fn load_patch(patch_file: &Path) -> Result<Vec<TLEntry>> {
    let reader = BufReader::new(File::open(patch_file)?);

    let mut entries = Vec::new();
    let mut current = TLEntry::default();
    let mut seen_jp = false;

    for l in reader.lines() {
        let l = l?;

        if l.trim().is_empty() {
            if seen_jp {
                entries.push(std::mem::take(&mut current));
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
        } else if let Some(rest) = l.strip_prefix("0x") {
            let (ptr, text) = rest.split_once(' ').unwrap_or_default();
            let ptr = u32::from_str_radix(ptr, 16)?;

            if !seen_jp {
                current.og_ptr = ptr;
                seen_jp = true;
            } else if ptr == current.og_ptr {
                current.en_string = Some(text.to_string());
            }
        }
    }

    if seen_jp {
        entries.push(current);
    }

    Ok(entries)
}

/// Every file under `dir`, depth first. Directories are descended into, never
/// yielded.
pub fn walk_dir(
    dir: &Path,
) -> impl Iterator<Item = std::io::Result<std::fs::DirEntry>> {
    let mut stack = vec![];
    let mut pending_err = None;

    match std::fs::read_dir(dir) {
        Ok(rd) => stack.push(rd),
        Err(e) => pending_err = Some(e),
    }

    std::iter::from_fn(move || {
        if let Some(e) = pending_err.take() {
            return Some(Err(e));
        }

        while let Some(rd) = stack.last_mut() {
            match rd.next() {
                Some(Ok(e)) => {
                    let p = e.path();
                    if p.is_dir() {
                        match std::fs::read_dir(&p) {
                            Ok(sub) => stack.push(sub),
                            Err(e) => return Some(Err(e)),
                        }
                    } else {
                        return Some(Ok(e));
                    }
                }
                Some(Err(e)) => return Some(Err(e)),
                None => {
                    stack.pop();
                }
            }
        }
        None
    })
}
