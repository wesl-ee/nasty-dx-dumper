use anyhow::{anyhow, Result};
use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use crate::constants::{
    NameCap, INLINE_FIELDS, TEXT_TABLES_EXPLICIT, WALKER_HOOKS,
};
use crate::longname::LongNameBank;
use crate::shape::{DolHeader, Section, DATA7, DATA8, TEXT2};
use crate::utils::{
    load_character_table, load_patch, load_reverse_character_table, Encoder,
    TLEntry,
};
use crate::{ppc, spt, utils};

fn word(buf: &[u8], off: u32) -> u32 {
    u32::from_be_bytes(buf[off as usize..off as usize + 4].try_into().unwrap())
}

// Retail memory immediately above BSS is not free: the linker reserves a
// 64 KiB main stack and an 8 KiB debugger stack there.  The runtime arena (and
// therefore the first genuinely available address) starts after both.
const TRANSLATION_ARENA_RAM: u32 = 0x802e_26a0;

/// Sanity-check for some strings I confirmed were good
const REVIEWED_IMMEDIATE_REFS: &[(u32, u32)] = &[
    (0x01ee6c, 0x1cf810), // alchemist duplication result
    (0x040440, 0x1d39fc), // roulette prompt
    (0x04052c, 0x1d397c), // Item Roulette
    (0x0405c4, 0x1d3990), // Magic Roulette
    (0x04065c, 0x1d39a4), // Red Chest Roulette
    (0x0406f4, 0x1d39bc), // White Chest Roulette
    (0x04078c, 0x1d39d4), // Mystery Chest Roulette
    (0x040824, 0x1d39ec), // Safe Roulette
    (0x044a9c, 0x1d4478), // Dig Space: insufficient money
    (0x0457d0, 0x1d4398), // Dig Space: nothing found
    (0x045824, 0x1d4380), // Dig Space: item unearthed
    (0x0458b8, 0x1d4414), // Dig Space: owner/payment prompt
    (0x069a80, 0x243450), // battle card: Attack
    (0x069a8c, 0x24345c), // battle card: Defense
    (0x069b68, 0x243474), // battle card: Counter
    (0x069bf0, 0x243480), // battle card: Give Up
    (0x06c4ac, 0x244090), // punishment: steal nothing
    (0x06c504, 0x243ec4), // punishment: steal money
    (0x06c52c, 0x243ec4), // punishment: steal money
    (0x06c550, 0x243f00), // punishment: discard money
    (0x06c574, 0x243fe4), // punishment: status ailment
    (0x06c588, 0x243f3c), // punishment: discard five
    (0x06c59c, 0x243f80), // punishment: discard all
    (0x06c5b0, 0x243fb8), // punishment: negative Items
    (0x06c5c4, 0x243edc), // punishment: steal equipment
    (0x06c5d8, 0x243f1c), // punishment: discard selection
    (0x06c608, 0x243f1c), // punishment: discard selection
    (0x06c638, 0x243f1c), // punishment: discard selection
    (0x06c66c, 0x243f1c), // punishment: discard selection
    (0x06c6a0, 0x244038), // punishment: steal D-Goods
    (0x06c6bc, 0x244038), // punishment: steal D-Goods
    (0x06c6d8, 0x244064), // punishment: steal D-Parts
    (0x06c6ec, 0x244010), // punishment: put something on
    (0x06c71c, 0x243e38), // punishment: confirmation
    (0x06ca14, 0x244120), // punishment result: stole money
    (0x06ca88, 0x244120), // punishment result: stole money
    (0x06cac4, 0x244138), // punishment result: discarded money
    (0x06ccac, 0x2441f8), // punishment result: head item
    (0x06cf08, 0x2433d8), // punishment: Yes/No
    (0x06d2cc, 0x24414c), // punishment result: stole selection
    (0x06d328, 0x244164), // punishment result: discarded selection
    (0x06d368, 0x244178), // punishment result: discarded all
    (0x06d3c4, 0x2441ac), // punishment result: forced item
    (0x06d404, 0x2441c4), // punishment result: forced negative Items
    (0x06dba4, 0x244024), // punishment: confirmation
    (0x06dbd0, 0x243e6c), // punishment: remaining selections
    (0x098754, 0x2440f4), // punishment result: two held items
    (0x0ac260, 0x2440b0), // punishment result: held item
    (0x0ac288, 0x2440d0), // punishment result: dropped item
    (0x0ac2b0, 0x243cd0), // punishment result: D-Parts
    (0x0ac2f4, 0x243e4c), // punishment: choose what to steal
    (0x0ac3fc, 0x243bf0), // one dropped item
    (0x0ac42c, 0x243c14), // two dropped items
    (0x0ac468, 0x243c40), // three dropped items
    (0x0ac4b0, 0x243c74), // dropped money
    (0x0ac4d8, 0x243c98), // dropped all Items/Magic
    (0x10e7b4, 0x2712d0), // Dropped Items heading
];

fn align_32(value: u32) -> u32 {
    value.checked_add(31).expect("RAM address overflow") & !31
}

fn dol_entry_cap(header: &DolHeader, entry: &TLEntry) -> NameCap {
    entry
        .references
        .iter()
        .find_map(|&ref_off| {
            let ref_ram = header.ram_addr(ref_off)?;
            TEXT_TABLES_EXPLICIT.iter().find_map(|table| {
                (0..table.count).find_map(|i| {
                    table.fields.iter().find_map(|field| {
                        (ref_ram == table.base + i * table.stride + *field)
                            .then_some(table.cap)
                    })
                })
            })
        })
        .unwrap_or(NameCap::Free)
}

/// Move every retail ArenaLo path past the appended translation bank.
///
/// `func_80141ED4` first installs the linker default at 0x802e26a0.  On the
/// normal non-debug path it then reclaims the debugger stack by installing
/// aligned(0x802e0698).  Patching only the first pair lets later heap
/// allocations overwrite data8 even though the game initially boots.
fn patch_runtime_arena_lo(
    buf: &[u8],
    header: &DolHeader,
    updates: &mut HashMap<u32, u32>,
    arena_lo: u32,
) -> Result<()> {
    const SITES: [(u32, u32, u32, u32); 2] = [
        // linker default
        (0x8014_1eec, 0x8014_1ef0, 0x3c60_802e, 0x3863_26a0),
        // normal retail path, followed by addi +31 / align-down-32
        (0x8014_1f24, 0x8014_1f28, 0x3c60_802e, 0x3863_0698),
    ];

    for (hi_ram, lo_ram, expected_hi, expected_lo) in SITES {
        let hi_off = header.file_offset(hi_ram).ok_or_else(|| {
            anyhow!("ArenaLo lis at 0x{hi_ram:08x} is outside the DOL")
        })?;
        let lo_off = header.file_offset(lo_ram).ok_or_else(|| {
            anyhow!("ArenaLo addi at 0x{lo_ram:08x} is outside the DOL")
        })?;
        let old_hi = word(buf, hi_off);
        let old_lo = word(buf, lo_off);
        if (old_hi, old_lo) != (expected_hi, expected_lo) {
            return Err(anyhow!(
                "ArenaLo instructions at 0x{hi_ram:08x} are \
                 0x{old_hi:08x}/0x{old_lo:08x}, expected \
                 0x{expected_hi:08x}/0x{expected_lo:08x}"
            ));
        }

        let (new_hi, new_lo) =
            ppc::rewrite(old_hi, old_lo, ppc::ImmKind::Addi, arena_lo);
        updates.insert(hi_off, new_hi);
        updates.insert(lo_off, new_lo);
    }
    Ok(())
}

static SPT_UNSIGNED_OFFSET_CODE: &[u8; 12] =
    include_bytes!(concat!(env!("OUT_DIR"), "/spt_unsigned_offsets.bin"));

/// Retail SPT offsets are signed. Install the three independently assembled
/// instructions that make direct, directory and table offsets unsigned.
fn patch_spt_resolvers(
    buf: &[u8],
    header: &DolHeader,
    updates: &mut HashMap<u32, u32>,
) -> Result<()> {
    const SITES: [(u32, u32); 3] = [
        (0x800d_2cdc, 0x7c60_0734), // extsh r0,r3
        (0x800d_2d00, 0x7c04_02ae), // lhax r0,r4,r0
        (0x800d_2d2c, 0x7c63_02ae), // lhax r3,r3,r0
    ];
    const EXPECTED_NEW: [u32; 3] = [
        0x5460_043e, // clrlwi r0,r3,16
        0x7c04_022e, // lhzx r0,r4,r0
        0x7c63_022e, // lhzx r3,r3,r0
    ];

    for (i, ((ram, expected_old), assembled)) in SITES
        .into_iter()
        .zip(SPT_UNSIGNED_OFFSET_CODE.chunks_exact(4))
        .enumerate()
    {
        let off = header.file_offset(ram).ok_or_else(|| {
            anyhow!("SPT resolver RAM 0x{ram:08x} is outside the DOL")
        })?;
        let found = word(buf, off);
        if found != expected_old {
            return Err(anyhow!(
                "SPT resolver at RAM 0x{ram:08x} (file 0x{off:x}) is \
                 0x{found:08x}, expected retail instruction 0x{expected_old:08x}"
            ));
        }
        let new_word = u32::from_be_bytes(assembled.try_into().unwrap());
        if new_word != EXPECTED_NEW[i] {
            return Err(anyhow!(
                "assembler produced 0x{new_word:08x} for SPT resolver {i}, \
                 expected 0x{:08x}",
                EXPECTED_NEW[i]
            ));
        }
        updates.insert(off, new_word);
    }
    Ok(())
}

/// Make every reference to one string resolve to `ram`: a pointer word gets
/// the address, an immediate pair gets both its halves rebuilt.
fn point_at(
    buf: &[u8],
    imm: &HashMap<u32, ppc::ImmRef>,
    updates: &mut HashMap<u32, u32>,
    refs: &[u32],
    ram: u32,
) {
    for &ref_off in refs {
        match imm.get(&ref_off) {
            Some(r) => {
                let (hi, lo) = ppc::rewrite(
                    word(buf, r.hi_off),
                    word(buf, r.lo_off),
                    r.kind,
                    ram,
                );
                updates.insert(r.hi_off, hi);
                updates.insert(r.lo_off, lo);
            }
            None => {
                updates.insert(ref_off, ram);
            }
        }
    }
}

static LONG_NAME_CODE: &[u8; 240] =
    include_bytes!(concat!(env!("OUT_DIR"), "/long_names.bin"));

/// Words in one redirect cave, and the three of them Rust fills in: the
/// `lis`/`addi` pair that builds `longNamePtrs` and the branch back into the
/// walker. See `asm/long_names.s`.
const CAVE_WORDS: usize = 10;
const CAVE_PTRS_HI: usize = 5;
const CAVE_PTRS_LO: usize = 6;
const CAVE_RESUME: usize = 9;

/// Install one redirect cave per text walker and return the text2 payload.
///
/// Each cave is entered by the branch this writes over the walker's loop-head
/// `lhz`, and leaves either through the resume branch -- one instruction past
/// the hook, with the code already loaded into the register the walker expects
/// -- or, having followed a redirect, through its own loop head with the cursor
/// moved. Nothing else in the walker observes the difference.
fn install_redirect_caves(
    buf: &[u8],
    header: &DolHeader,
    updates: &mut HashMap<u32, u32>,
    text2_ram: u32,
    ptrs_ram: u32,
) -> Result<Vec<u8>> {
    let mut code: Vec<u32> = LONG_NAME_CODE
        .chunks_exact(4)
        .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
        .collect();
    if code.len() != WALKER_HOOKS.len() * CAVE_WORDS {
        return Err(anyhow!(
            "long_names.s assembled to {} words, expected {} caves of \
             {CAVE_WORDS}",
            code.len(),
            WALKER_HOOKS.len()
        ));
    }

    for (i, hook) in WALKER_HOOKS.iter().enumerate() {
        let off = header.file_offset(hook.ram).ok_or_else(|| {
            anyhow!(
                "{} loop head at 0x{:08x} is outside the DOL",
                hook.what,
                hook.ram
            )
        })?;
        let found = word(buf, off);
        if found != hook.expect {
            return Err(anyhow!(
                "{} loop head at RAM 0x{:08x} is 0x{found:08x}, expected \
                 retail instruction 0x{:08x}",
                hook.what,
                hook.ram,
                hook.expect
            ));
        }

        let at = i * CAVE_WORDS;
        // the cave repeats the instruction it displaced, so a cave assembled
        // for the wrong registers cannot be installed over this walker
        if code[at] != hook.expect {
            return Err(anyhow!(
                "cave {i} opens with 0x{:08x} but {} loop head is \
                 0x{:08x}; the cave's registers do not match the walker",
                code[at],
                hook.what,
                hook.expect
            ));
        }

        let cave_ram = text2_ram + (at * 4) as u32;
        let (hi, lo) = ppc::rewrite(
            code[at + CAVE_PTRS_HI],
            code[at + CAVE_PTRS_LO],
            ppc::ImmKind::Addi,
            ptrs_ram,
        );
        code[at + CAVE_PTRS_HI] = hi;
        code[at + CAVE_PTRS_LO] = lo;
        code[at + CAVE_RESUME] =
            ppc::branch(cave_ram + (CAVE_RESUME * 4) as u32, hook.ram + 4)?;

        updates.insert(off, ppc::branch(hook.ram, cave_ram)?);
    }

    Ok(code.iter().flat_map(|w| w.to_be_bytes()).collect())
}

pub fn patch_all(dir: PathBuf, out_dir: PathBuf) -> Result<()> {
    let mut refused = 0usize;

    for f in utils::walk_dir(&dir) {
        let f = f?;
        let f_path = f.path();
        // only .patch files carry translations; everything else is game data
        if !f_path
            .extension()
            .unwrap_or_default()
            .eq_ignore_ascii_case("PATCH")
            || !f.metadata()?.is_file()
        {
            continue;
        }

        // must stay relative: joining an absolute path onto out_dir discards
        // out_dir and writes straight back over the input tree
        let rel_path = f_path.strip_prefix(&dir).unwrap_or(&f_path);

        if f_path
            .file_name()
            .unwrap_or_default()
            .eq_ignore_ascii_case("MAIN.DOL.PATCH")
        {
            let dol_f_path = f_path.with_extension("");
            let out_path = out_dir
                .join(rel_path)
                .parent()
                .expect("parent")
                .join("main.dol");

            let refusals =
                DolPatcher::load(&f_path)?.patch(&dol_f_path, &out_path)?;
            for r in &refusals {
                println!("  REFUSED main.dol {r}");
            }
            refused += refusals.len();
        }

        let spt_f_path = f_path.with_extension("");
        if spt_f_path
            .extension()
            .unwrap_or_default()
            .eq_ignore_ascii_case("SPT")
        {
            let out_path = out_dir.join(rel_path).with_extension("");
            if !patch_spt(&f_path, &spt_f_path, &out_path)? {
                refused += 1;
            }
        }
    }

    if refused > 0 {
        // a plausible-looking corrupt SPT costs more than a clear error, so the
        // untouched Japanese was written instead and the run fails
        return Err(anyhow!(
            "{refused} translation(s) refused; their Japanese was written \
             through unchanged"
        ));
    }
    Ok(())
}

/// Apply one `.SPT.patch`. Returns false when the writer refused, in which case
/// the original file is written through untouched.
///
/// Every check the writer makes is a refusal rather than a truncation: over the
/// 8-glyph name cap the 0x12-byte copy loses its terminator and the renderer
/// walks off the field, so silently clipping would corrupt the game.
fn patch_spt(
    patch_file: &Path,
    in_file: &Path,
    out_path: &Path,
) -> Result<bool> {
    let glyphs = load_character_table()?;
    let encoder = Encoder::new(&load_reverse_character_table()?);
    let buf = std::fs::read(in_file)?;
    let plan = spt::plan(&buf, &glyphs);
    let name = in_file
        .file_name()
        .unwrap_or_default()
        .display()
        .to_string();

    let entries = load_patch(patch_file)?;
    let mut repl = BTreeMap::new();
    let mut refusals = Vec::new();
    for entry in &entries {
        let en = match entry.en_string.as_deref() {
            Some(s) => s,
            None => continue,
        };
        match encoder.encode(en) {
            Ok(codes) => {
                for &at in &entry.references {
                    repl.insert(
                        (at as usize, entry.og_ptr as usize),
                        codes.clone(),
                    );
                }
            }
            Err(why) => refusals.push(format!("0x{:x}: {}", entry.og_ptr, why)),
        }
    }

    let mut out = &buf;
    let mut packed;
    if refusals.is_empty() {
        match spt::pack(&buf, &plan, &repl) {
            Ok(p) => {
                packed = p;
                refusals.extend(
                    spt::verify(&buf, &packed.bytes, &repl, &glyphs)
                        .into_iter()
                        .map(|b| format!("verify: {b}")),
                );
                if refusals.is_empty() {
                    if name.eq_ignore_ascii_case("TITLE.SPT") {
                        patch_title_default_names(&mut packed.bytes, &encoder)?;
                    }
                    out = &packed.bytes;
                }
            }
            Err(rs) => refusals.extend(rs.into_iter().map(|r| match r.key {
                Some((at, off)) => {
                    format!("0x{:x} (ref 0x{:x}): {}", off, at, r.why)
                }
                None => format!("file: {}", r.why),
            })),
        }
    }

    std::fs::create_dir_all(out_path.parent().expect("parent"))?;
    if refusals.is_empty() {
        std::fs::write(out_path, out)?;
        if !repl.is_empty() {
            println!(
                "{}: {} replacements, {} -> {} bytes",
                name,
                repl.len(),
                buf.len(),
                out.len()
            );
        }
        return Ok(true);
    }

    std::fs::write(out_path, &buf)?;
    for r in &refusals {
        println!("  REFUSED {name} {r}");
    }
    println!("{name}: refused, wrote the original through");
    Ok(false)
}

/// TITLE's normal string references follow the relocated directory entries,
/// but the character-select name prefill ultimately reads the seven retail
/// slots at 0x7260..0x729f.  Keep full names in the relocated strings and put
/// short Latin defaults in those legacy slots.  Each replacement is exactly
/// the original slot width, including the terminator; refusing on any byte
/// mismatch prevents this post-pass from overwriting a future packer's data.
fn patch_title_default_names(buf: &mut [u8], encoder: &Encoder) -> Result<()> {
    const NAMES: [(usize, &str, &str); 7] = [
        (0x7260, "タップ", "Tap"),
        (0x7268, "ルチル", "Luc"),
        (0x7270, "ブリキン", "Brik"),
        (0x727a, "ガママル", "Gama"),
        (0x7284, "ラズリ", "Raz"),
        (0x728c, "ウィウィ", "Wiwi"),
        (0x7296, "ヴィント", "Vint"),
    ];

    for (offset, japanese, latin) in NAMES {
        let original = encoder
            .encode(japanese)
            .map_err(|why| anyhow!("TITLE default {japanese}: {why}"))?;
        let replacement = encoder
            .encode(latin)
            .map_err(|why| anyhow!("TITLE default {latin}: {why}"))?;
        if original.len() != replacement.len() {
            return Err(anyhow!(
                "TITLE default at 0x{offset:x}: {japanese:?} and {latin:?} have different widths"
            ));
        }

        let mut expected = Vec::with_capacity((original.len() + 1) * 2);
        let mut encoded = Vec::with_capacity((replacement.len() + 1) * 2);
        for code in original {
            expected.extend(code.to_be_bytes());
        }
        for code in replacement {
            encoded.extend(code.to_be_bytes());
        }
        expected.extend(spt::TERMINATOR.to_be_bytes());
        encoded.extend(spt::TERMINATOR.to_be_bytes());

        let end = offset + expected.len();
        let found = buf.get(offset..end).ok_or_else(|| {
            anyhow!("TITLE default slot 0x{offset:x} is outside the file")
        })?;
        if found != expected {
            return Err(anyhow!(
                "TITLE default slot 0x{offset:x} no longer contains the expected retail name {japanese:?}"
            ));
        }
        buf[offset..end].copy_from_slice(&encoded);
    }
    Ok(())
}

struct DolPatcher {
    tl: HashMap<u32, TLEntry>,
    /// Strict encoder for every translated DOL string. Silently dropping an
    /// unsupported glyph can turn reviewed copy into different text and can
    /// also hide fixed-width overflows.
    encoder: Encoder,
}

impl DolPatcher {
    fn load(patch_file: &Path) -> Result<DolPatcher> {
        let rev_table = load_reverse_character_table()?;
        let tl = load_patch(patch_file)?
            .into_iter()
            .map(|e| (e.og_ptr, e))
            .collect();

        Ok(DolPatcher {
            tl,
            encoder: Encoder::new(&rev_table),
        })
    }

    /// Returns one refusal per string that could not be encoded; those keep
    /// their Japanese, everything else is still translated.
    fn patch(&self, in_file: &Path, out_path: &Path) -> Result<Vec<String>> {
        let mut in_fh = File::open(in_file)?;
        let mut header = DolHeader::read(&mut in_fh)?;
        let buf = std::fs::read(in_file)?;

        // strings the code addresses with an immediate pair have no pointer
        // word; re-derive the pairs from this same DOL rather than trusting
        // anything the .patch file says about them
        let scan = ppc::scan(&buf, &header);
        let rejected: HashMap<u32, &str> = scan
            .rejected
            .iter()
            .map(|(&a, r)| (a, r.why.as_str()))
            .collect();
        // pairs grouped by the string they build, so an entry's code references
        // come from this DOL rather than from whatever the .patch file happened
        // to record when it was last dumped
        let imm_by_target: HashMap<u32, Vec<u32>> = scan
            .usable()
            .into_iter()
            .map(|(target, refs)| {
                (target, refs.into_iter().map(|r| r.lo_off).collect())
            })
            .collect();
        let imm = scan.refs;
        for &(ref_off, expected_target) in REVIEWED_IMMEDIATE_REFS {
            let found = imm.get(&ref_off).ok_or_else(|| {
                anyhow!(
                    "reviewed immediate reference 0x{ref_off:x} was not \
                     recovered from this DOL"
                )
            })?;
            if found.target_off != expected_target {
                return Err(anyhow!(
                    "reviewed immediate reference 0x{ref_off:x} targets \
                     0x{:x}, expected 0x{expected_target:x}",
                    found.target_off
                ));
            }
        }

        // data8 is empty on the original ROM, so use it for the translated
        // text.  It must begin after the two linker-reserved stacks, not at the
        // end of BSS (which is the bottom of the main stack).
        let data7 = header.sections[DATA7];
        if data7.size == 0 {
            return Err(anyhow!("data7 is empty; this is not the retail DOL"));
        }
        let original_bss_end = header.bss_address + header.bss_size;
        if original_bss_end != 0x802d_0698 {
            return Err(anyhow!(
                "unexpected retail BSS end 0x{original_bss_end:08x}; refusing \
                 to guess where the stacks end"
            ));
        }
        header.sections[DATA8] = Section {
            file: data7.file + data7.size,
            ram: TRANSLATION_ARENA_RAM,
            size: 0,
        };

        let data8_base_ram = TRANSLATION_ARENA_RAM;

        // collect all new text into single buffer for data8
        let mut appended = Vec::new();
        // file offset -> the word to write there: a relocated pointer in data,
        // a rebuilt `lis` or low half in code
        let mut word_updates = HashMap::new();
        // names too long for the 0x12-byte field their reference feeds. The
        // bank is laid out after every ordinary string, so their references
        // cannot be rewritten until the loop below has finished: hold
        // (references, bank index) and resolve once the base is known.
        let mut bank = LongNameBank::new();
        let mut stub_refs: Vec<(Vec<u32>, u32)> = Vec::new();

        // sort by og_ptr for deterministic ordering
        let mut entries: Vec<_> = self.tl.values().collect();
        entries.sort_by_key(|e| e.og_ptr);

        // one bad glyph used to abort the whole run, so a translator learned
        // about them one build at a time. Refuse the string, keep going, and
        // report every offender at the end of the pass.
        let mut refusals = Vec::new();

        for entry in entries {
            let en = match entry.en_string.as_deref() {
                Some(s) => s,
                None => continue,
            };
            let codes = match self.encoder.encode(en) {
                Ok(codes) => codes,
                Err(why) => {
                    refusals
                        .push(format!("0x{:x}: {why}: {en:?}", entry.og_ptr));
                    continue;
                }
            };
            let mut en_bytes = Vec::with_capacity((codes.len() + 1) * 2);
            for &code in &codes {
                en_bytes.extend(code.to_be_bytes());
            }
            en_bytes.extend(spt::TERMINATOR.to_be_bytes());

            let cap = dol_entry_cap(&header, entry);
            // fixed-width inline records have no reference of any kind, so
            // falling through to the relocation path below would append the
            // translation to the arena and leave nothing pointing at it. A
            // stubbed field is overwritten where it lies with a redirect and
            // the glyphs go to the bank; the rest stay Japanese, because
            // overwriting one in place means fitting the record's own width and
            // nothing has playtested that.
            let inline = header.ram_addr(entry.og_ptr).and_then(|ram| {
                INLINE_FIELDS.iter().find(|f| f.record(ram).is_some())
            });
            if let Some(field) = inline {
                if !field.stub {
                    continue;
                }
                if field.stride % 4 != 0 {
                    return Err(anyhow!(
                        "inline field {} has stride 0x{:x}; a stub is written \
                         a word at a time and needs a multiple of four",
                        field.tag,
                        field.stride
                    ));
                }
                let idx = match bank.intern(&codes) {
                    Ok(idx) => idx,
                    Err(why) => {
                        refusals.push(format!(
                            "0x{:x}: {why}: {en:?}",
                            entry.og_ptr
                        ));
                        continue;
                    }
                };
                let record = LongNameBank::record(idx, field.stride as usize);
                for (i, w) in record.chunks_exact(4).enumerate() {
                    word_updates.insert(
                        entry.og_ptr + (i * 4) as u32,
                        u32::from_be_bytes(w.try_into().unwrap()),
                    );
                }
                continue;
            }

            // pointer words come from the .patch file; instruction pairs come
            // from the scan of this DOL. A reference the .patch file puts inside
            // code that the scan does not know is a pair is a stale entry, not a
            // translation, and it is not ours to guess at.
            let mut all_refs: Vec<u32> = Vec::new();
            let mut unrewritable = Vec::new();
            for &ref_off in &entry.references {
                if ref_off >= header.data_start() {
                    all_refs.push(ref_off);
                } else if !imm.contains_key(&ref_off) {
                    unrewritable.push(format!("0x{ref_off:x}"));
                }
            }
            // Relocating a string moves it, so every reference to it has to
            // follow. Rewriting only some leaves the rest pointing at the
            // Japanese, which is worse than not translating it at all: one
            // route through the game shows English and another does not.
            if !unrewritable.is_empty() {
                eprintln!(
                    "0x{:x}: leaving the original, code references it at {} and \
                     the scan will not rewrite that",
                    entry.og_ptr,
                    unrewritable.join(" ")
                );
                continue;
            }
            if let Some(pairs) = imm_by_target.get(&entry.og_ptr) {
                all_refs.extend(pairs);
            }
            all_refs.sort_unstable();
            all_refs.dedup();
            if all_refs.is_empty() {
                continue;
            }
            if let Some(ram) = header.ram_addr(entry.og_ptr) {
                if let Some(why) = rejected.get(&ram) {
                    eprintln!(
                        "0x{:x}: leaving the original, code addresses it in a way \
                         we will not rewrite: {why}",
                        entry.og_ptr
                    );
                    continue;
                }
            }

            // a stubbed reference is pointed at two words in the bank rather
            // than at the glyphs, so the 0x12-byte copy downstream transports a
            // redirect instead of truncating a name. The address is not known
            // until the bank is laid out.
            if cap == NameCap::Stub {
                match bank.intern(&codes) {
                    Ok(idx) => stub_refs.push((all_refs, idx)),
                    Err(why) => refusals
                        .push(format!("0x{:x}: {why}: {en:?}", entry.og_ptr)),
                }
                continue;
            }

            let new_ram = data8_base_ram + appended.len() as u32;
            appended.extend_from_slice(&en_bytes);
            point_at(&buf, &imm, &mut word_updates, &all_refs, new_ram);
        }

        // the bank follows every relocated string, so its base is only known
        // now. Stubs first, because that is what the deferred references point
        // at; `emit` reports where the pointer table the caves index landed.
        let bank_ram = data8_base_ram + appended.len() as u32;
        let (bank_bytes, bank_layout) = bank.emit(bank_ram);
        appended.extend_from_slice(&bank_bytes);
        for (refs, idx) in &stub_refs {
            point_at(
                &buf,
                &imm,
                &mut word_updates,
                refs,
                LongNameBank::stub_ram(bank_ram, *idx),
            );
        }

        patch_spt_resolvers(&buf, &header, &mut word_updates)?;

        // text2 is empty on retail and holds the redirect caves. Code rather
        // than a corner of data8 because the DOL loader invalidates icache per
        // text section, and putting instructions in a data section relies on
        // that not mattering.
        header.sections[DATA8].size = appended.len().try_into()?;
        let data8 = header.sections[DATA8];
        if header.sections[TEXT2].size != 0 {
            return Err(anyhow!(
                "text2 is not empty; this is not the retail DOL"
            ));
        }
        // both halves 32-aligned, as every retail section is: the apploader
        // DMAs a section straight from disc to RAM
        header.sections[TEXT2] = Section {
            file: align_32(data8.file + data8.size),
            ram: align_32(data8.ram + data8.size),
            size: 0,
        };
        let caves = install_redirect_caves(
            &buf,
            &header,
            &mut word_updates,
            header.sections[TEXT2].ram,
            bank_layout.ptrs,
        )?;
        header.sections[TEXT2].size = caves.len().try_into()?;

        let text2 = header.sections[TEXT2];
        let appended_end = text2.ram + text2.size;
        let new_arena_lo = align_32(appended_end);
        const MEM1_END_RAM: u32 = 0x8180_0000;
        if new_arena_lo > MEM1_END_RAM {
            return Err(anyhow!(
                "translation arena ends at 0x{new_arena_lo:08x}, beyond MEM1"
            ));
        }
        for section in header.live() {
            let section_end = section
                .ram
                .checked_add(section.size)
                .ok_or_else(|| anyhow!("DOL section address overflow"))?;
            if section.ram != data8_base_ram
                && section.ram != text2.ram
                && data8_base_ram < section_end
                && section.ram < appended_end
            {
                return Err(anyhow!(
                    "translation arena 0x{data8_base_ram:08x}..0x{appended_end:08x} \
                     overlaps DOL section 0x{:08x}..0x{section_end:08x}",
                    section.ram
                ));
            }
        }
        patch_runtime_arena_lo(&buf, &header, &mut word_updates, new_arena_lo)?;

        // The DOL loader clears BSS before loading initialized sections. Grow
        // the reservation through the aligned end of text2 so neither stack nor
        // heap ownership can overlap the bank or the caves.
        header.bss_size = new_arena_lo - header.bss_address;

        let sections: Vec<_> = header.live().collect();

        std::fs::create_dir_all(out_path.parent().expect("parent"))?;
        let mut out_fh = File::create(out_path)?;
        header.write(&mut out_fh)?;

        // write sections
        for section in &sections {
            if section.size == 0 {
                continue;
            }

            out_fh.seek(SeekFrom::Start(section.file as u64))?;

            if section.ram == data8_base_ram {
                out_fh.write_all(&appended)?;
                continue;
            }
            if section.ram == text2.ram {
                out_fh.write_all(&caves)?;
                continue;
            }

            // read original section
            in_fh.seek(SeekFrom::Start(section.file as u64))?;
            let mut section_data = vec![0u8; section.size as usize];
            in_fh.read_exact(&mut section_data)?;

            for (&file_offset, &new_word) in &word_updates {
                if file_offset >= section.file
                    && file_offset + 4 <= section.file + section.size
                {
                    let i = (file_offset - section.file) as usize;
                    section_data[i..i + 4]
                        .copy_from_slice(&new_word.to_be_bytes());
                }
            }

            out_fh.write_all(&section_data)?;
        }

        Ok(refusals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::DATA0;
    use std::collections::HashSet;

    #[test]
    fn ppc_assembler_emits_the_reviewed_unsigned_resolver_words() {
        let words: Vec<u32> = SPT_UNSIGNED_OFFSET_CODE
            .chunks_exact(4)
            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
            .collect();
        assert_eq!(words, [0x5460_043e, 0x7c04_022e, 0x7c63_022e]);
    }

    fn cave_words() -> Vec<u32> {
        LONG_NAME_CODE
            .chunks_exact(4)
            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
            .collect()
    }

    #[test]
    fn ppc_assembler_emits_the_reviewed_redirect_cave() {
        assert_eq!(
            &cave_words()[..CAVE_WORDS],
            [
                0xa3dd_0000, // lhz    r30,0x0(r29)      the displaced retail word
                0x57c0_0428, // rlwinm r0,r30,0,16,20    family
                0x2800_c000, // cmplwi r0,0xc000
                0x4082_0018, // bne    +0x18             -> the resume branch
                0x57c0_14fa, // rlwinm r0,r30,2,19,29    (code & 0x7ff) * 4
                0x3fc0_0000, // lis    r30,0             -> longNamePtrs@ha
                0x3bde_0000, // addi   r30,r30,0         -> longNamePtrs@l
                0x7fbe_002e, // lwzx   r29,r30,r0        cursor = longNamePtrs[i]
                0x4bff_ffe0, // b      -0x20             re-read at the new cursor
                0x4800_0000, // b      .                 -> the hooked loop head + 4
            ]
        );
    }

    /// The cave repeats the instruction it displaces, so this is the check that
    /// a cave's registers really are the ones its walker uses. Getting it wrong
    /// would have the trampoline load the code into a register the walker does
    /// not read.
    #[test]
    fn every_cave_opens_with_the_instruction_it_displaces() {
        let code = cave_words();
        assert_eq!(code.len(), WALKER_HOOKS.len() * CAVE_WORDS);
        for (i, hook) in WALKER_HOOKS.iter().enumerate() {
            assert_eq!(
                code[i * CAVE_WORDS],
                hook.expect,
                "cave {i} does not open with {}'s loop head",
                hook.what
            );
        }
    }

    /// Every hook lands on the retail instruction it claims to. Skipped when
    /// the ROM is not extracted, since it is not in the repository.
    #[test]
    fn walker_hooks_match_retail() {
        let Ok(buf) = std::fs::read("dx-iso-base/sys/main.dol") else {
            eprintln!("no extracted ROM; skipping");
            return;
        };
        let header = DolHeader::read(&mut buf.as_slice()).unwrap();
        for hook in WALKER_HOOKS {
            let off = header.file_offset(hook.ram).unwrap();
            assert_eq!(
                word(&buf, off),
                hook.expect,
                "{} at RAM 0x{:08x}",
                hook.what,
                hook.ram
            );
        }
    }

    /// The two branches that make a cave reachable and survivable: into it from
    /// the walker, and back out one instruction past the hook.
    #[test]
    fn a_cave_branches_into_the_walker_and_back() {
        const TEXT: u32 = 0x8001_4000;
        const SIZE: u32 = 0x000d_0000;
        const TEXT2_RAM: u32 = 0x802f_0000;
        const PTRS: u32 = 0x802e_4004;

        let mut header = DolHeader::default();
        header.sections[0] = Section {
            file: 0x100,
            ram: TEXT,
            size: SIZE,
        };
        let mut buf = vec![0u8; (0x100 + SIZE) as usize];
        for hook in WALKER_HOOKS {
            let at = (0x100 + hook.ram - TEXT) as usize;
            buf[at..at + 4].copy_from_slice(&hook.expect.to_be_bytes());
        }

        let mut updates = HashMap::new();
        let caves = install_redirect_caves(
            &buf,
            &header,
            &mut updates,
            TEXT2_RAM,
            PTRS,
        )
        .unwrap();
        let code: Vec<u32> = caves
            .chunks_exact(4)
            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
            .collect();

        for (i, hook) in WALKER_HOOKS.iter().enumerate() {
            let cave = TEXT2_RAM + (i * CAVE_WORDS * 4) as u32;
            let at = i * CAVE_WORDS;

            let hooked = updates[&header.file_offset(hook.ram).unwrap()];
            assert_eq!(hooked, ppc::branch(hook.ram, cave).unwrap());
            assert_eq!(
                code[at + CAVE_RESUME],
                ppc::branch(cave + (CAVE_RESUME * 4) as u32, hook.ram + 4)
                    .unwrap()
            );
            // PTRS has bit 15 clear, so the addi takes no carry
            assert_eq!(code[at + CAVE_PTRS_HI] & 0xffff, PTRS >> 16);
            assert_eq!(code[at + CAVE_PTRS_LO] & 0xffff, PTRS & 0xffff);
        }
    }

    /// A cave assembled for one walker refuses to install over another.
    #[test]
    fn a_hook_refuses_a_dol_whose_loop_head_moved() {
        let mut header = DolHeader::default();
        header.sections[0] = Section {
            file: 0x100,
            ram: 0x8001_4000,
            size: 0x000d_0000,
        };
        let buf = vec![0u8; 0x100 + 0x000d_0000];
        let err = install_redirect_caves(
            &buf,
            &header,
            &mut HashMap::new(),
            0x802f_0000,
            0x802e_8004,
        )
        .unwrap_err();
        assert!(err.to_string().contains("expected retail instruction"));
    }

    #[test]
    fn playtested_immediate_references_are_exact_and_unique() {
        let refs: HashSet<u32> = REVIEWED_IMMEDIATE_REFS
            .iter()
            .map(|&(ref_off, _)| ref_off)
            .collect();
        assert_eq!(REVIEWED_IMMEDIATE_REFS.len(), 57);
        assert_eq!(refs.len(), REVIEWED_IMMEDIATE_REFS.len());
        assert_eq!(REVIEWED_IMMEDIATE_REFS[0], (0x01ee6c, 0x1cf810));
        assert_eq!(REVIEWED_IMMEDIATE_REFS[56], (0x10e7b4, 0x2712d0));
    }

    #[test]
    fn monster_name_references_are_stubbed_rather_than_capped() {
        let mut header = DolHeader::default();
        header.sections[DATA0] = Section {
            file: 0x100,
            ram: 0x8024_0000,
            size: 0x1_0000,
        };
        let entry = TLEntry {
            og_ptr: 0,
            references: vec![0x100 + (0x8024_1d8c - 0x8024_0000)],
            en_string: Some("Pakkun Grass".into()),
        };

        assert_eq!(dol_entry_cap(&header, &entry), NameCap::Stub);
    }

    #[test]
    fn title_legacy_name_slots_receive_width_preserving_latin_defaults() {
        let encoder = Encoder::new(&load_reverse_character_table().unwrap());
        let cases = [
            (0x7260, "タップ", "Tap"),
            (0x7268, "ルチル", "Luc"),
            (0x7270, "ブリキン", "Brik"),
            (0x727a, "ガママル", "Gama"),
            (0x7284, "ラズリ", "Raz"),
            (0x728c, "ウィウィ", "Wiwi"),
            (0x7296, "ヴィント", "Vint"),
        ];
        let mut buf = vec![0u8; 0x72a0];

        for (offset, japanese, _) in cases {
            let mut at = offset;
            for code in encoder.encode(japanese).unwrap() {
                buf[at..at + 2].copy_from_slice(&code.to_be_bytes());
                at += 2;
            }
            buf[at..at + 2].copy_from_slice(&spt::TERMINATOR.to_be_bytes());
        }

        patch_title_default_names(&mut buf, &encoder).unwrap();

        for (offset, _, latin) in cases {
            let mut expected = Vec::new();
            for code in encoder.encode(latin).unwrap() {
                expected.extend(code.to_be_bytes());
            }
            expected.extend(spt::TERMINATOR.to_be_bytes());
            assert_eq!(&buf[offset..offset + expected.len()], expected);
        }
    }

    #[test]
    fn dol_encoder_is_strict_and_uses_longest_table_tokens() {
        let encoder = Encoder::new(&load_reverse_character_table().unwrap());

        assert_eq!(encoder.encode("II").unwrap(), vec![0x01f8]);
        assert!(encoder.encode("Wallace's").is_err());
    }
}
