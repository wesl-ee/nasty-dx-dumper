//! SPT script text: walk the command stream the way the interpreter does, and
//! write English back without moving a single existing byte.
//!
//! derived from machine code via ghidra
//!
//! The patched runtime treats SPT addresses as unsigned 16-bit file-relative
//! offsets. The retail game sign-extends them in FUN_800D2CCC/FUN_800D2CE8/
//! FUN_800D2D0C
//!
//! I did that to get more room in TITLE.SPT, other files had ample room and
//! did not need this hack (but I left it in for all SPT anyway)

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub const REACH: usize = 0x10000;
pub const TERMINATOR: u16 = 0xF800;
const END: u16 = 0xFFFF;
const PAD_OP: u16 = 0xFFFE;
/// printable glyphs are 0x0000-0x03FF: only four font page tables exist
const GLYPH_MAX: u16 = 0x03FF;
/// every u16@0 in the corpus is 32-aligned
const ALIGN: usize = 32;
/// the 0x12-byte name field: 8 glyphs plus the 0xF800.
///
/// Stays at 8 while DOL names go to 16. A DOL name reaches 16 by having its
/// reference repointed at a redirect stub in the translation bank, so the
/// 0x12-byte copy transports two words instead of glyphs. An SPT name has no
/// such reference to repoint -- it is reached through a 16-bit directory offset
/// within its own file, and the bank is in the DOL -- so its glyphs really are
/// what the copy moves, and 8 is still where the terminator is lost. Raising
/// this without a stub path for SPT would ship the corruption below rather than
/// avoid it. Keep in step with `dx-ghidra/tools/sptpack.py`.
const NAME_CAP: usize = 8;
/// 0x0B18 writes words 1..32 in place and never writes a terminator
const B18_WORDS: usize = 34;
const MAX_TABLE: usize = 512;
const MAX_STR: usize = 512;
/// FUN_800DDB84 expands a complete string into a 512-byte stack buffer. The
/// terminator needs the final word, leaving room for at most 255 source codes.
const EXPANSION_CAP: usize = 255;
const DECODE_LIMIT: usize = 1024;

// FUN_80014E48 splits a code as family = code & 0xF800, argument = code & 0x7FF
const FAMILY: u16 = 0xF800;
const ARG: u16 = 0x07FF;
/// the seven codes that consume a positional variadic argument of showTextAsBox
const VARIADIC: [u16; 7] =
    [0x1000, 0x1800, 0x7800, 0x2000, 0x2800, 0x6800, 0xE800];
/// arg > 31 overruns the 64-byte scratch at 0x801B7440
const NUM_FIELD: [u16; 3] = [0x1000, 0x1800, 0x7800];
/// arg > 30 is silently truncated by FUN_800177BC
const PAD_FIELD: [u16; 3] = [0x2000, 0x2800, 0x6800];

/// opcode -> word index holding a direct u16 text offset
static TEXT_OPERAND: &[(u16, usize)] = &[
    (0x0201, 6),
    (0x0202, 6),
    (0x0203, 4),
    (0x0204, 8),
    (0x020d, 8),
    (0x0218, 7),
    (0x0303, 2),
    (0x0306, 3),
    (0x0701, 11),
    (0x0703, 3),
];
/// 0x0307 (setObjectText) word 2: the base of an array of u16 string offsets,
/// which something outside the command indexes -- a character id in the drama
/// files, a game mode in TITLE, a memory-card result in SAVEGAME. `docs/
/// spt-commands.md` reads it off `FUN_800D2CE8` as a directory id instead; the
/// bytes say otherwise, because the only value in the corpus small enough to be
/// an id is 0, which clears the field, and every other value lands on a run of
/// offsets that decode.
const OBJ_TEXT: u16 = 0x0307;
const OBJ_TEXT_WORD: usize = 2;
/// unconditional jumps: opcode -> operand word holding the u16 target
static GOTO: &[(u16, usize)] =
    &[(0x0302, 2), (0x0801, 1), (0x0806, 1), (0x080b, 2)];
/// conditional: opcode -> (target word, fallthrough length in words)
static BRANCH: &[(u16, (usize, usize))] = &[
    (0x0402, (2, 3)),
    (0x0403, (2, 3)),
    (0x0804, (2, 3)),
    (0x0805, (2, 3)),
];
/// jumps through an inline array of u16 targets whose element count is in
/// neither the command nor the binary -- the index is a menu cursor or a script
/// variable -- so the array's extent is recovered from its own shape by
/// `jump_array`
static JUMP_ARRAY: [u16; 2] = [0x0302, 0x080b];
const RET: u16 = 0x0807;
/// 0x0806: opcode + target, pushed as the return address
const CALL_LEN: usize = 2;

// lengths in 16-bit words including the opcode. shipped binary by dx-ghidra/tools/opcodes.py. 288
// of the 302 commands have a constant length; the other three are computed below and 0x0807 is a
// stack pop.
static LENGTHS: &[(u16, u16)] = &[
    (0x0100, 2),
    (0x0101, 1),
    (0x0102, 1),
    (0x0103, 7),
    (0x0104, 3),
    (0x0105, 4),
    (0x0106, 3),
    (0x0107, 3),
    (0x0108, 2),
    (0x0109, 2),
    (0x010a, 2),
    (0x010b, 3),
    (0x010c, 8),
    (0x010d, 3),
    (0x010e, 3),
    (0x010f, 3),
    (0x0110, 7),
    (0x0111, 3),
    (0x0112, 3),
    (0x0113, 3),
    (0x0114, 3),
    (0x0115, 4),
    (0x0116, 3),
    (0x0117, 6),
    (0x0118, 2),
    (0x0119, 3),
    (0x011a, 3),
    (0x011b, 5),
    (0x011c, 2),
    (0x011d, 2),
    (0x011e, 4),
    (0x011f, 2),
    (0x0120, 2),
    (0x0121, 2),
    (0x0122, 2),
    (0x0200, 7),
    (0x0201, 7),
    (0x0202, 7),
    (0x0203, 7),
    (0x0204, 9),
    (0x0205, 6),
    (0x0206, 2),
    (0x0207, 15),
    (0x0208, 13),
    (0x0209, 4),
    (0x020a, 4),
    (0x020b, 4),
    (0x020c, 3),
    (0x020d, 10),
    (0x020e, 3),
    (0x020f, 3),
    (0x0210, 4),
    (0x0211, 7),
    (0x0212, 3),
    (0x0213, 4),
    (0x0214, 4),
    (0x0215, 4),
    (0x0216, 5),
    (0x0217, 6),
    (0x0218, 9),
    (0x0219, 8),
    (0x021a, 3),
    (0x021b, 3),
    (0x021c, 3),
    (0x021d, 3),
    (0x021e, 12),
    (0x0300, 2),
    (0x0301, 3),
    (0x0303, 3),
    (0x0304, 3),
    (0x0305, 3),
    (0x0306, 4),
    (0x0307, 4),
    (0x0308, 2),
    (0x0400, 2),
    (0x0401, 2),
    (0x0500, 1),
    (0x0501, 1),
    (0x0502, 6),
    (0x0503, 1),
    (0x0504, 1),
    (0x0505, 3),
    (0x0601, 2),
    (0x0602, 2),
    (0x0603, 3),
    (0x0604, 3),
    (0x0605, 3),
    (0x0606, 2),
    (0x0700, 1),
    (0x0701, 15),
    (0x0702, 14),
    (0x0703, 4),
    (0x0704, 2),
    (0x0705, 11),
    (0x0706, 2),
    (0x0707, 11),
    (0x0708, 3),
    (0x0800, 2),
    (0x0802, 2),
    (0x0803, 2),
    (0x0807, 96),
    (0x0808, 3),
    (0x0809, 3),
    (0x080a, 3),
    (0x080c, 3),
    (0x080d, 3),
    (0x080e, 3),
    (0x080f, 3),
    (0x0810, 1),
    (0x0811, 2),
    (0x0812, 2),
    (0x0813, 4),
    (0x0814, 3),
    (0x0815, 6),
    (0x0816, 2),
    (0x0900, 3),
    (0x0901, 1),
    (0x0902, 1),
    (0x0903, 1),
    (0x0904, 1),
    (0x0905, 1),
    (0x0906, 1),
    (0x0907, 2),
    (0x0908, 1),
    (0x0909, 1),
    (0x0a00, 2),
    (0x0a01, 2),
    (0x0a02, 2),
    (0x0a03, 1),
    (0x0a04, 2),
    (0x0a05, 3),
    (0x0a06, 1),
    (0x0a07, 1),
    (0x0a08, 1),
    (0x0a09, 2),
    (0x0a0a, 1),
    (0x0a0b, 1),
    (0x0a0c, 1),
    (0x0a0d, 1),
    (0x0a0e, 1),
    (0x0a0f, 2),
    (0x0a10, 1),
    (0x0a11, 2),
    (0x0b00, 2),
    (0x0b01, 4),
    (0x0b02, 4),
    (0x0b03, 2),
    (0x0b04, 1),
    (0x0b05, 1),
    (0x0b07, 1),
    (0x0b08, 5),
    (0x0b09, 1),
    (0x0b0a, 3),
    (0x0b0b, 3),
    (0x0b0c, 2),
    (0x0b0d, 2),
    (0x0b0e, 2),
    (0x0b0f, 1),
    (0x0b10, 2),
    (0x0b11, 1),
    (0x0b12, 1),
    (0x0b13, 2),
    (0x0b14, 1),
    (0x0b15, 1),
    (0x0b16, 2),
    (0x0b17, 3),
    (0x0b18, 2),
    (0x0b19, 3),
    (0x0b1a, 1),
    (0x0b1b, 2),
    (0x0b1c, 1),
    (0x0b1d, 3),
    (0x0b1e, 5),
    (0x0b1f, 1),
    (0x0b20, 3),
    (0x0b21, 3),
    (0x0b22, 2),
    (0x0b23, 2),
    (0x0b24, 2),
    (0x0b25, 1),
    (0x0b26, 1),
    (0x0b27, 3),
    (0x0b28, 2),
    (0x0b29, 1),
    (0x0b2a, 2),
    (0x0b2b, 2),
    (0x0b2c, 2),
    (0x0b2d, 2),
    (0x0b2e, 1),
    (0x0b2f, 1),
    (0x0b30, 3),
    (0x0b31, 2),
    (0x0c00, 4),
    (0x0c01, 4),
    (0x0c02, 2),
    (0x0c03, 2),
    (0x0c04, 2),
    (0x0d00, 2),
    (0x0d01, 2),
    (0x0d02, 2),
    (0x0d03, 1),
    (0x0d04, 1),
    (0x0d05, 1),
    (0x0d06, 2),
    (0x0d07, 2),
    (0x0d08, 2),
    (0x0d09, 2),
    (0x0d0a, 6),
    (0x0d0b, 5),
    (0x0d0c, 2),
    (0x0d0d, 1),
    (0x0d0e, 1),
    (0x0d0f, 1),
    (0x0d10, 1),
    (0x0d11, 1),
    (0x0d12, 1),
    (0x0e00, 3),
    (0x0e01, 5),
    (0x0e02, 5),
    (0x0e03, 5),
    (0x0e04, 3),
    (0x0e05, 2),
    (0x0e06, 9),
    (0x0e07, 3),
    (0x0e08, 8),
    (0x0e09, 3),
    (0x0e0a, 3),
    (0x0e0b, 8),
    (0x0f00, 5),
    (0x0f01, 5),
    (0x0f02, 3),
    (0x0f03, 1),
    (0x0f04, 11),
    (0x0f05, 11),
    (0x0f06, 1),
    (0x0f07, 1),
    (0x0f08, 1),
    (0x0f09, 1),
    (0x0f0a, 3),
    (0x0f0b, 1),
    (0x0f0c, 1),
    (0x0f0d, 1),
    (0x0f0e, 1),
    (0x0f0f, 1),
    (0x0f10, 2),
    (0x0f11, 1),
    (0x0f12, 1),
    (0x0f13, 5),
    (0x0f14, 2),
    (0x0f15, 4),
    (0x0f16, 1),
    (0x0f17, 3),
    (0x0f18, 3),
    (0x0f19, 1),
    (0x0f1a, 1),
    (0x0f1b, 1),
    (0x0f1c, 3),
    (0x0f1d, 1),
    (0x0f1e, 1),
    (0x1000, 2),
    (0x1001, 2),
    (0x1002, 1),
    (0x1003, 2),
    (0x1004, 3),
    (0x1005, 3),
    (0x1007, 1),
    (0x100d, 3),
    (0x100e, 2),
    (0x1011, 2),
    (0x1012, 1),
    (0x1013, 3),
    (0x1100, 2),
    (0x1101, 2),
    (0x1102, 2),
    (0x1103, 1),
    (0x1104, 1),
    (0x1105, 1),
    (0x1106, 2),
    (0x1107, 2),
    (0x1108, 1),
    (0x1109, 1),
    (0x110a, 2),
    (0x110b, 2),
    (0x110c, 2),
    (0x110d, 3),
    (0x110e, 2),
    (0x110f, 1),
    (0x1110, 2),
];

fn lookup(table: &[(u16, u16)], op: u16) -> Option<u16> {
    table
        .binary_search_by_key(&op, |&(k, _)| k)
        .ok()
        .map(|i| table[i].1)
}

fn word<T: Copy>(table: &[(u16, T)], op: u16) -> Option<T> {
    table.iter().find(|&&(k, _)| k == op).map(|&(_, v)| v)
}

/// Length in words of a command that is not control flow, `None` when the
/// opcode has none. The three computed lengths the binary works out at runtime:
///   0x0401 falls through all n pairs, 2 + 2n words
///   0x0600 registers a task; 800ce360 reads the argc at word 3
///   0x0B06 calls a native; 800cfb28 advances by argc + 3 words
fn advance(buf: &[u8], cur: usize, op: u16) -> Option<usize> {
    let w = |i: usize| u16at(buf, cur + i * 2).unwrap_or(0) as usize;
    let n = match op {
        0x0401 => 2 + 2 * w(1),
        0x0600 => w(3) + 5,
        0x0b06 => w(2) + 3,
        _ => lookup(LENGTHS, op)? as usize,
    };
    (n > 0).then_some(n)
}

/// The elements of a `JUMP_ARRAY`'s inline target array, which nothing declares
/// a length for.
fn jump_array(buf: &[u8], base: usize, limit: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut bound = limit;
    while base + out.len() * 2 < bound {
        let Some(t) = u16at(buf, base + out.len() * 2).map(usize::from) else {
            break;
        };
        if t == 0 || t >= limit || t % 2 != 0 {
            break;
        }
        if !out.is_empty() && !runs_as_code(buf, t, limit) {
            break;
        }
        if (base..bound).contains(&t) {
            bound = t;
        }
        out.push(t);
    }
    out
}

/// Could the interpreter run from here? Real code reaches a sentinel or a
/// return; a misread runs into an opcode that does not exist. Nested arrays are
/// left at element 0, which is enough to decide the question and keeps this
/// from recursing through the whole script.
fn runs_as_code(buf: &[u8], start: usize, limit: usize) -> bool {
    let mut work = vec![start];
    let mut seen = HashSet::new();
    while let Some(cur) = work.pop() {
        if cur >= limit || !seen.insert(cur) {
            continue;
        }
        let Some(op) = u16at(buf, cur) else {
            return false;
        };
        if op == END || op == RET {
            continue;
        }
        if op == PAD_OP {
            work.push(cur + 2);
            continue;
        }
        let target = |w: usize| {
            u16at(buf, cur + w * 2)
                .map(usize::from)
                .filter(|&t| t > 0 && t < limit)
        };
        if let Some(i) = word(GOTO, op) {
            work.extend(target(i));
            if op == 0x0806 {
                work.push(cur + CALL_LEN * 2);
            }
            continue;
        }
        if let Some((wd, fall)) = word(BRANCH, op) {
            work.extend(target(wd));
            work.push(cur + fall * 2);
            continue;
        }
        if op == 0x0401 {
            let pairs = u16at(buf, cur + 2).unwrap_or(0) as usize;
            work.extend((0..pairs).filter_map(|k| target(3 + 2 * k)));
        }
        match advance(buf, cur, op) {
            Some(n) => work.push(cur + n * 2),
            None => return false,
        }
    }
    true
}

pub fn u16at(buf: &[u8], off: usize) -> Option<u16> {
    buf.get(off..off + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
}

/// How a reference names its string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefKind {
    /// the command operand is the offset
    Direct(u16),
    /// slot of the array a 0x0307 operand points at
    ObjText(usize),
    /// slot of the string table dir entry `t` points at
    Table(usize),
    /// slot of an inner table reached through 0x0808's nested arrays
    Native(usize),
}

impl RefKind {
    /// the `cmd` column of a reference comment
    pub fn cmd(&self) -> String {
        match self {
            RefKind::Direct(op) => format!("{op:04x}"),
            RefKind::ObjText(_) => format!("{OBJ_TEXT:04x}-array"),
            RefKind::Table(_) => "table".to_string(),
            RefKind::Native(_) => "0808-table".to_string(),
        }
    }

    /// dir 11 is the name table, whose entries the 0x12-byte copy caps
    fn is_name_table(&self) -> bool {
        matches!(*self, RefKind::Table(11))
    }

    /// A table whose extent is recovered by decoding its entries, so a blank
    /// one ends the walk and hides everything after it.
    fn is_walked_table(&self) -> bool {
        matches!(self, RefKind::Table(_) | RefKind::ObjText(_))
    }
}

impl std::fmt::Display for RefKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            RefKind::Direct(_) => write!(f, "direct"),
            RefKind::ObjText(base) => write!(f, "obj-text[0x{base:x}]"),
            RefKind::Table(t) => write!(f, "table[{t}]"),
            RefKind::Native(outer) => write!(f, "native-table[0x{outer:x}]"),
        }
    }
}

/// A reference to a string: one command operand or one directory-table slot.
#[derive(Clone, Debug)]
pub struct StringRef {
    /// the command address, or the table slot address for a table reference
    pub at: usize,
    pub offset: usize,
    /// one past the terminator
    pub end: usize,
    pub kind: RefKind,
    /// the halfword that has to be rewritten to repoint this reference
    pub slot: Option<usize>,
    pub jp: String,
}

#[derive(Default)]
pub struct WalkStats {
    pub steps: usize,
    pub sentinel: bool,
    pub unknown: BTreeMap<u16, usize>,
    pub arrays: usize,
    pub targets: usize,
    /// byte position -> length in words, 0 when the length is not known
    pub cmds: BTreeMap<usize, usize>,
}

/// Read u16 codes to the 0xF800 terminator
fn read_codes(buf: &[u8], mut off: usize, limit: usize) -> Option<Vec<u16>> {
    let mut out = Vec::new();
    while out.len() < limit {
        match u16at(buf, off) {
            None => return None,
            Some(TERMINATOR) => return Some(out),
            Some(c) => out.push(c),
        }
        off += 2;
    }
    None
}

pub fn render(codes: &[u16], glyphs: &HashMap<u16, String>) -> String {
    codes
        .iter()
        .map(|c| {
            glyphs
                .get(c)
                .cloned()
                .unwrap_or_else(|| format!("[0x{c:04x}]"))
        })
        .collect()
}

fn count_glyphs(codes: &[u16]) -> usize {
    codes.iter().filter(|&&c| c <= GLYPH_MAX).count()
}

/// Discover every reachable command from the entry point.
pub fn walk(
    buf: &[u8],
    glyphs: &HashMap<u16, String>,
) -> (Vec<StringRef>, WalkStats) {
    let limit = buf.len().min(REACH);
    let mut st = WalkStats::default();
    let mut strings = Vec::new();
    let mut seen = HashSet::new();
    // the entry point: the directory ends where the command stream begins
    let mut work = vec![u16at(buf, 2).unwrap_or(0) as usize];

    while let Some(cur) = work.pop() {
        if cur >= limit || !seen.insert(cur) {
            continue;
        }
        let op = match u16at(buf, cur) {
            Some(o) => o,
            None => continue,
        };
        st.steps += 1;
        if op == END {
            st.sentinel = true;
            st.cmds.insert(cur, 1);
            continue;
        }
        if op == PAD_OP {
            st.cmds.insert(cur, 1);
            work.push(cur + 2);
            continue;
        }
        let w = |i: usize| u16at(buf, cur + i * 2).unwrap_or(0) as usize;

        if let Some(i) = word(TEXT_OPERAND, op) {
            // 0x0306 word 3 is polymorphic: FUN_800E19A8 runs it through a
            // binary search over the value, and only the default arm passes
            // it to FUN_800D2CCC as a file offset. 0..22 and 100..115 select
            // a game-state source instead.
            let raw = u16at(buf, cur + i * 2);
            // 0x0306 makes its dispatch decision on the signed operand,
            // then the default arm calls the now-unsigned resolver.
            let signed = raw.map(|x| x as i16 as i32);
            let off = match (op, raw, signed) {
                (0x0306, Some(_), Some(x)) if (0..116).contains(&x) => None,
                (_, Some(x), _) => Some(x as i32),
                _ => None,
            };
            // a glyph array is halfword-aligned by construction, so an odd
            // offset is a misread rather than text
            if let Some(off) =
                off.filter(|&o| o > 0 && (o as usize) < limit && o % 2 == 0)
            {
                let off = off as usize;
                if let Some(codes) = read_codes(buf, off, DECODE_LIMIT) {
                    if count_glyphs(&codes) > 0 && !codes.contains(&END) {
                        strings.push(StringRef {
                            at: cur,
                            offset: off,
                            end: off + codes.len() * 2 + 2,
                            kind: RefKind::Direct(op),
                            slot: None,
                            jp: render(&codes, glyphs),
                        });
                    }
                }
            }
        }

        if let Some(i) = word(GOTO, op) {
            if JUMP_ARRAY.contains(&op) {
                let targets = jump_array(buf, cur + i * 2, limit);
                st.targets += targets.len();
                st.arrays += 1;
                st.cmds.insert(cur, i + targets.len());
                work.extend(targets);
                continue;
            }
            if let Some(t) = u16at(buf, cur + i * 2)
                .map(usize::from)
                .filter(|&t| t > 0 && t < limit)
            {
                work.push(t);
                st.targets += 1;
            }
            if op == 0x0806 {
                work.push(cur + CALL_LEN * 2);
            }
            st.cmds.insert(cur, i + 1);
            continue;
        }
        if let Some((wd, fall)) = word(BRANCH, op) {
            if let Some(t) = u16at(buf, cur + wd * 2)
                .map(usize::from)
                .filter(|&t| t > 0 && t < limit)
            {
                work.push(t);
                st.targets += 1;
            }
            work.push(cur + fall * 2);
            st.cmds.insert(cur, fall);
            continue;
        }
        if op == RET {
            st.cmds.insert(cur, 1);
            continue;
        }

        if op == 0x0401 {
            // n (mask, target) pairs from word 1; 800ce0f4 commits the cursor
            // wherever the walk stopped
            for k in 0..w(1) {
                if let Some(t) = u16at(buf, cur + (3 + 2 * k) * 2)
                    .map(usize::from)
                    .filter(|&t| t > 0 && t < limit)
                {
                    work.push(t);
                    st.targets += 1;
                }
            }
        }

        let Some(n) = advance(buf, cur, op) else {
            *st.unknown.entry(op).or_insert(0) += 1;
            st.cmds.insert(cur, 0);
            continue;
        };
        st.cmds.insert(cur, n);
        work.push(cur + n * 2);
    }
    (strings, st)
}

pub struct Plan {
    pub strings: Vec<StringRef>,
    pub stats: WalkStats,
    pub dir_words: usize,
    /// one past the last byte the script region claims
    pub script_end: usize,
    /// the record cursor: where the arena goes and where the archives start
    pub record_cursor: usize,
}

/// The string tables FUN_800D2D0C reads: dirEntry(t) is an array of u16 string
/// offsets. Only a handful of files have real ones, so every entry has to earn
/// its place and the walk stops at the first that does not.
fn tables(
    buf: &[u8],
    dir_words: usize,
    cmds: &[bool],
    known: &[StringRef],
    glyphs: &HashMap<u16, String>,
) -> Vec<StringRef> {
    let (lo, hi) = (dir_words * 2, cmds.len());
    let mut out: Vec<StringRef> = Vec::new();
    for t in 2..dir_words {
        let base = match u16at(buf, t * 2) {
            Some(b) => b as usize,
            _ => continue,
        };
        if base % 2 != 0 || !(lo..hi).contains(&base) || cmds[base] {
            continue;
        }
        for i in 0..MAX_TABLE {
            let slot = base + i * 2;
            if slot + 2 > hi || cmds[slot] {
                break;
            }
            let off = match u16at(buf, slot) {
                Some(o) => o as usize,
                _ => break,
            };
            if off % 2 != 0 || !(lo..hi).contains(&off) || cmds[off] {
                break;
            }
            let codes = match read_codes(buf, off, usize::MAX) {
                Some(c) => c,
                None => break,
            };
            let end = off + codes.len() * 2 + 2;
            if !(1..=MAX_STR).contains(&codes.len()) || end > hi {
                break;
            }
            let overlaps = known.iter().chain(out.iter()).any(|s| {
                (off < s.offset && s.offset < end)
                    || (s.offset < off && off < s.end)
            });
            if overlaps {
                break;
            }
            out.push(StringRef {
                at: slot,
                offset: off,
                end,
                kind: RefKind::Table(t),
                slot: Some(slot),
                jp: render(&codes, glyphs),
            });
        }
    }
    out
}

/// The strings an `OBJ_TEXT` command's array names.
///
/// The array has no declared length and no terminator, so it runs until a slot
/// stops looking like one: past the addressable window, into bytes the walk
/// already claimed as a command, or onto something that does not decode. The
/// arrays that end early in the file pad with zeros, which the same test
/// rejects. Every slot is writable, so a translation repoints entries here
/// exactly the way it does for a directory table.
fn obj_text_arrays(
    buf: &[u8],
    bases: &[usize],
    lo: usize,
    cmds: &[bool],
    known: &[StringRef],
    glyphs: &HashMap<u16, String>,
) -> Vec<StringRef> {
    let hi = cmds.len();
    let mut out: Vec<StringRef> = Vec::new();
    let mut seen = HashSet::new();
    for &base in bases {
        if !seen.insert(base)
            || base % 2 != 0
            || !(lo..hi).contains(&base)
            || cmds[base]
        {
            continue;
        }
        for i in 0..MAX_TABLE {
            let slot = base + i * 2;
            if slot + 2 > hi || cmds[slot] {
                break;
            }
            let off = match u16at(buf, slot) {
                Some(o) => o as usize,
                _ => break,
            };
            if off % 2 != 0 || !(lo..hi).contains(&off) || cmds[off] {
                break;
            }
            let codes = match read_codes(buf, off, MAX_STR) {
                Some(c) => c,
                None => break,
            };
            let end = off + codes.len() * 2 + 2;
            if codes.is_empty() || count_glyphs(&codes) == 0 || end > hi {
                break;
            }
            let overlaps = known.iter().chain(out.iter()).any(|s| {
                (off < s.offset && s.offset < end)
                    || (s.offset < off && off < s.end)
            });
            if overlaps {
                break;
            }
            out.push(StringRef {
                at: slot,
                offset: off,
                end,
                kind: RefKind::ObjText(base),
                slot: Some(slot),
                jp: render(&codes, glyphs),
            });
        }
    }
    out
}

/// Discover the nested string tables passed to native routines by 0x0808.
///
/// SAVEBOX uses `0808 000c 02fe`: 0x02fe is an outer array of inner-array
/// offsets, and each inner array is a 0xffff-terminated list of string offsets.
/// The first inner offset also proves the outer count: `(first - outer) / 2`.
/// Requiring that complete shape, terminated decodable strings, monotonic inner
/// arrays, and no overlaps keeps ordinary 0x0808 data from becoming a guessed
/// writable reference.
fn native_tables(
    buf: &[u8],
    outers: &[usize],
    lo: usize,
    hi: usize,
    cmds: &[bool],
    known: &[StringRef],
    glyphs: &HashMap<u16, String>,
) -> Vec<StringRef> {
    let mut out = Vec::new();
    let mut seen_outer = HashSet::new();

    for &outer in outers {
        if !seen_outer.insert(outer)
            || outer % 2 != 0
            || !(lo..hi).contains(&outer)
            || cmds.get(outer).copied().unwrap_or(true)
        {
            continue;
        }
        let first = match u16at(buf, outer).map(usize::from) {
            Some(v) if v > outer && v < hi && (v - outer) % 2 == 0 => v,
            _ => continue,
        };
        let count = (first - outer) / 2;
        if !(1..=MAX_TABLE).contains(&count) {
            continue;
        }
        let bases: Vec<usize> = (0..count)
            .filter_map(|i| u16at(buf, outer + i * 2).map(usize::from))
            .collect();
        if bases.len() != count
            || bases[0] != first
            || bases.iter().any(|&base| {
                base % 2 != 0
                    || !(first..hi).contains(&base)
                    || cmds.get(base).copied().unwrap_or(true)
            })
            || bases.windows(2).any(|pair| pair[0] >= pair[1])
        {
            continue;
        }

        let mut candidate = Vec::new();
        let mut valid = true;
        for (i, &base) in bases.iter().enumerate() {
            let table_end = bases.get(i + 1).copied().unwrap_or(hi);
            let mut terminated = false;
            for n in 0..MAX_TABLE {
                let slot = base + n * 2;
                if slot + 2 > table_end
                    || cmds.get(slot).copied().unwrap_or(true)
                {
                    break;
                }
                let off = match u16at(buf, slot) {
                    Some(0xffff) => {
                        terminated = true;
                        break;
                    }
                    Some(v) => v as usize,
                    None => break,
                };
                if off % 2 != 0 || !(lo..hi).contains(&off) || cmds[off] {
                    valid = false;
                    break;
                }
                let codes = match read_codes(buf, off, MAX_STR) {
                    Some(c) if !c.is_empty() && !c.contains(&END) => c,
                    _ => {
                        valid = false;
                        break;
                    }
                };
                let end = off + codes.len() * 2 + 2;
                if end > hi
                    || known
                        .iter()
                        .chain(out.iter())
                        .chain(candidate.iter())
                        .any(|s| {
                            (off < s.offset && s.offset < end)
                                || (s.offset < off && off < s.end)
                        })
                {
                    valid = false;
                    break;
                }
                candidate.push(StringRef {
                    at: slot,
                    offset: off,
                    end,
                    kind: RefKind::Native(outer),
                    slot: Some(slot),
                    jp: render(&codes, glyphs),
                });
            }
            if !terminated {
                valid = false;
            }
            if !valid {
                break;
            }
        }
        if valid {
            out.extend(candidate);
        }
    }
    out
}

/// Walk once, then mark every byte of the addressable window that is a command
/// or a string, and resolve the word that names each string.
pub fn plan(buf: &[u8], glyphs: &HashMap<u16, String>) -> Plan {
    let (mut strings, stats) = walk(buf, glyphs);
    let limit = buf.len().min(REACH);
    let dir_words = (u16at(buf, 2).unwrap_or(0) as usize / 2).max(1);
    let mut cmds = vec![false; limit];
    let mut claimed = vec![false; limit];

    for (&at, &n) in &stats.cmds {
        // an unknown length still owns its opcode
        let (a, b) = (at.min(limit), (at + n.max(1) * 2).min(limit));
        cmds[a..b].fill(true);
    }
    claimed.copy_from_slice(&cmds);
    for s in &mut strings {
        let (a, b) = (s.offset.min(limit), s.end.min(limit));
        claimed[a..b].fill(true);
        s.slot = match s.kind {
            RefKind::Direct(op) => {
                Some(s.at + word(TEXT_OPERAND, op).unwrap_or(0) * 2)
            }
            // tables carry their own slot from the moment they are found
            RefKind::ObjText(_) | RefKind::Table(_) | RefKind::Native(_) => {
                s.slot
            }
        };
    }
    for s in tables(buf, dir_words, &cmds, &strings, glyphs) {
        let (a, b) = (s.offset.min(limit), s.end.min(limit));
        claimed[a..b].fill(true);
        strings.push(s);
    }
    let obj_text_bases: Vec<usize> = stats
        .cmds
        .keys()
        .copied()
        .filter(|&at| u16at(buf, at) == Some(OBJ_TEXT))
        .filter_map(|at| u16at(buf, at + OBJ_TEXT_WORD * 2).map(usize::from))
        .collect();
    for s in obj_text_arrays(
        buf,
        &obj_text_bases,
        dir_words * 2,
        &cmds,
        &strings,
        glyphs,
    ) {
        let (a, b) = (s.offset.min(limit), s.end.min(limit));
        claimed[a..b].fill(true);
        strings.push(s);
    }
    let native_outers: Vec<usize> = stats
        .cmds
        .keys()
        .copied()
        .filter(|&at| u16at(buf, at) == Some(0x0808))
        .filter_map(|at| u16at(buf, at + 4).map(usize::from))
        .collect();
    for s in native_tables(
        buf,
        &native_outers,
        dir_words * 2,
        limit,
        &cmds,
        &strings,
        glyphs,
    ) {
        let (a, b) = (s.offset.min(limit), s.end.min(limit));
        claimed[a..b].fill(true);
        strings.push(s);
    }
    let script_end = claimed.iter().rposition(|&b| b).map_or(0, |i| i + 1);
    Plan {
        strings,
        stats,
        dir_words,
        script_end,
        record_cursor: u16at(buf, 0).unwrap_or(0) as usize,
    }
}

impl Plan {
    /// Group every reference by the string it names. One "en" per string
    /// satisfies all of its readers at once.
    pub fn by_offset(&self) -> BTreeMap<usize, Vec<&StringRef>> {
        let mut m: BTreeMap<usize, Vec<&StringRef>> = BTreeMap::new();
        for s in &self.strings {
            m.entry(s.offset).or_default().push(s);
        }
        m
    }

    /// 0x0B1B copies 0x12 bytes from whatever table id 0x0B resolves to, so an
    /// entry of that table which the index can select is a name. The tables
    /// overlap in a shared array, so dir 11's tail runs on into a table of
    /// descriptions; an entry longer than the field is not a name, or the
    /// shipped game would already be overrunning it.
    pub fn cap(&self, users: &[&StringRef], jp_words: usize) -> Option<usize> {
        let named = users.iter().any(|u| u.kind.is_name_table());
        (named && jp_words <= NAME_CAP).then_some(NAME_CAP)
    }

    /// Headroom in bytes before the patched unsigned 16-bit address space ends.
    pub fn arena_bytes(&self) -> usize {
        REACH.saturating_sub(self.record_cursor)
    }
}

/// A refusal, with the reference it belongs to.
#[derive(Debug)]
pub struct Refusal {
    pub key: Option<(usize, usize)>,
    pub why: String,
}

impl Refusal {
    fn new(key: Option<(usize, usize)>, why: String) -> Refusal {
        Refusal { key, why }
    }
}

/// Refuse a translation that changes which positional vararg each formatting
/// code consumes. Non-consuming controls such as colour, newline and 0x3800
/// may move freely; only the family sequence matters because the low 11 bits
/// select field width/alignment rather than a different argument type.
pub(crate) fn check_variadic_args(
    orig: &[u16],
    codes: &[u16],
) -> Result<(), String> {
    let variadic = |cs: &[u16]| {
        cs.iter()
            .map(|c| c & FAMILY)
            .filter(|f| VARIADIC.contains(f))
            .collect::<Vec<_>>()
    };
    let (was, now) = (variadic(orig), variadic(codes));
    if was != now {
        return Err(format!(
            "variadic argument sequence changed: {was:04x?} -> {now:04x?}. showTextAsBox \
             consumes these positionally, so count and order must both survive \
             translation"
        ));
    }
    Ok(())
}

/// Text limits enforced on replacements
fn check(
    codes: &[u16],
    orig: &[u16],
    cap: Option<usize>,
) -> Result<(), String> {
    if codes.len() > EXPANSION_CAP {
        return Err(format!(
            "{} codes exceeds the 255-code expansion cap. FUN_800DDB84 writes the \
             complete string and terminator into a 512-byte stack buffer, so a \
             longer string corrupts saved registers and the return address",
            codes.len()
        ));
    }
    if codes.iter().any(|&c| c >= TERMINATOR) {
        return Err(
            "code at or above 0xF800; that is the terminator, which the \
                    packer appends, and nothing above it is a defined family"
                .into(),
        );
    }
    for &c in codes {
        let (fam, n) = (c & FAMILY, c & ARG);
        if NUM_FIELD.contains(&fam) && n > 31 {
            return Err(format!(
                "numeric field code {c:04x} has width {n}; N>31 writes the terminator \
                 past the 64-byte scratch at 0x801B7440 and corrupts the frame's \
                 display list"
            ));
        }
        if PAD_FIELD.contains(&fam) && n > 30 {
            return Err(format!(
                "inserted-string code {c:04x} has width {n}; FUN_800177BC clamps at 30 \
                 words, so anything above is silently truncated"
            ));
        }
    }
    check_variadic_args(orig, codes)?;
    if let Some(cap) = cap {
        let n = count_glyphs(codes);
        if n > cap {
            return Err(format!(
                "{n} glyphs exceeds the {cap}-glyph cap. Over the cap the 0x12-byte copy \
                 loses the 0xF800 and the renderer walks off the field -- corruption, \
                 not a clipped name"
            ));
        }
    }
    Ok(())
}

/// The one word each reference uses to name its string, and the offset it has
/// to hold.
fn slotmap(
    groups: &BTreeMap<(usize, String), Vec<&StringRef>>,
    offsets: &BTreeMap<(usize, String), usize>,
    errors: &mut Vec<Refusal>,
) -> BTreeMap<usize, usize> {
    let mut slots = BTreeMap::new();
    for (key, users) in groups {
        let new = match offsets.get(key) {
            Some(&n) => n,
            None => continue,
        };
        for u in users {
            let slot = u.slot.expect("unwritable reference reached the writer");
            if *slots.entry(slot).or_insert(new) != new {
                errors.push(Refusal::new(
                    Some((u.at, key.0)),
                    format!(
                        "word 0x{slot:04x} would have to hold two different offsets; give \
                         every reader of that slot the same text"
                    ),
                ));
            }
        }
    }
    slots
}

fn codes_to_bytes(codes: &[u16]) -> Vec<u8> {
    let mut b: Vec<u8> = codes.iter().flat_map(|c| c.to_be_bytes()).collect();
    b.extend(TERMINATOR.to_be_bytes());
    b
}

pub struct Packed {
    pub bytes: Vec<u8>,
    pub placements: Vec<String>,
}

/// Append the new text at the record cursor, sliding the asset archives down.
/// Nothing that already exists moves and nothing is overwritten.
///
/// The archives' lengths are self-relative, so sliding them by a multiple of 32
/// and bumping the halfword at 0 is what the original build tool would have
/// produced had the script been longer.
fn arena(
    buf: &[u8],
    p: &Plan,
    groups: &BTreeMap<(usize, String), Vec<&StringRef>>,
    coded: &BTreeMap<(usize, String), Vec<u16>>,
) -> Result<Packed, Vec<Refusal>> {
    let at0 = p.record_cursor;
    let mut errors = Vec::new();
    if at0 % 2 != 0 {
        errors.push(Refusal::new(
            None,
            format!(
                "record cursor 0x{at0:04x} is odd; refusing to guess a layout"
            ),
        ));
    }
    if at0 < p.script_end {
        errors.push(Refusal::new(
            None,
            format!(
                "record cursor 0x{:04x} is below the end of the script region \
                 0x{:04x}; this file is not laid out the way the rest are",
                at0, p.script_end
            ),
        ));
    }
    for t in 1..p.dir_words {
        if let Some(v) =
            u16at(buf, t * 2).map(usize::from).filter(|&v| v >= at0)
        {
            errors.push(Refusal::new(
                None,
                format!(
                    "directory entry {t} is 0x{v:04x}, at or past the insertion point; \
                     sliding the record region would break it"
                ),
            ));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut blob: Vec<u8> = Vec::new();
    let mut offsets = BTreeMap::new();
    let mut placements = Vec::new();
    for (key, codes) in coded {
        let start = at0 + blob.len();
        if start >= REACH {
            errors.extend(groups[key].iter().map(|u| {
                Refusal::new(
                    Some((u.at, key.0)),
                    format!(
                        "arena is full -- this string would start at 0x{:04x}, past \
                         the u16 reach of 0x{:04x}. The file has {} bytes of headroom",
                        start,
                        REACH,
                        p.arena_bytes()
                    ),
                )
            }));
            continue;
        }
        offsets.insert(key.clone(), start);
        blob.extend(codes_to_bytes(codes));
        placements.push(format!(
            "0x{:04x} -> 0x{:04x}  {}w -> {}w x{}",
            key.0,
            start,
            read_codes(buf, key.0, usize::MAX).map_or(0, |c| c.len() + 1),
            codes.len() + 1,
            groups[key].len()
        ));
    }
    let slots = slotmap(groups, &offsets, &mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }

    blob.resize(blob.len() + (ALIGN - blob.len() % ALIGN) % ALIGN, 0);
    let new_cursor = at0 + blob.len();
    if new_cursor > u16::MAX as usize {
        return Err(vec![Refusal::new(
            None,
            format!(
                "arena needs an archive cursor of 0x{new_cursor:x}, beyond the unsigned \
                 16-bit limit 0xffff"
            ),
        )]);
    }
    let mut out = Vec::with_capacity(buf.len() + blob.len());
    out.extend_from_slice(&buf[..at0]);
    out.extend_from_slice(&blob);
    out.extend_from_slice(&buf[at0..]);
    out[0..2].copy_from_slice(&(new_cursor as u16).to_be_bytes());
    for (slot, new) in slots {
        out[slot..slot + 2].copy_from_slice(&(new as u16).to_be_bytes());
    }
    Ok(Packed {
        bytes: out,
        placements,
    })
}

/// The replaced strings' own bytes, coalesced. Adjacent strings share one
/// segment when asked to; on their own they never do, which is what makes a
/// fixed placement fixed.
fn segments(
    p: &Plan,
    want: &BTreeMap<usize, String>,
    share: bool,
) -> Vec<Vec<usize>> {
    let ext: HashMap<usize, usize> =
        p.strings.iter().map(|s| (s.offset, s.end)).collect();
    let mut segs: Vec<Vec<usize>> = Vec::new();
    for &off in want.keys() {
        match segs.last_mut() {
            Some(last) if share && ext[last.last().unwrap()] == off => {
                last.push(off)
            }
            _ => segs.push(vec![off]),
        }
    }
    segs
}

/// Rewrite in place, reusing only the bytes the replaced Japanese occupies.
///
/// Fallback for a file whose record cursor is outside even the patched
/// unsigned reach. The recycled bytes are the entire budget, so the file
/// neither grows nor moves and the archives are untouched.
fn recycle(
    buf: &[u8],
    p: &Plan,
    groups: &BTreeMap<(usize, String), Vec<&StringRef>>,
    coded: &BTreeMap<(usize, String), Vec<u16>>,
    share: bool,
) -> Result<Packed, Vec<Refusal>> {
    let byoff = p.by_offset();
    let ext: HashMap<usize, usize> =
        p.strings.iter().map(|s| (s.offset, s.end)).collect();
    let done: HashSet<(usize, usize)> = groups
        .iter()
        .flat_map(|((off, _), us)| us.iter().map(move |u| (u.at, *off)))
        .collect();
    let mut errors = Vec::new();

    let mut want: BTreeMap<usize, String> = BTreeMap::new();
    for ((off, text), users) in groups {
        if want.entry(*off).or_insert_with(|| text.clone()) != text {
            errors.push(Refusal::new(
                Some((users[0].at, *off)),
                "two different replacements for one offset. Without an arena there is \
                 nowhere to put a second copy, so every reader of a string has to be \
                 given the same text"
                    .into(),
            ));
        }
    }
    for &off in want.keys() {
        let loose: Vec<_> = byoff[&off]
            .iter()
            .filter(|s| !done.contains(&(s.at, off)))
            .collect();
        if let Some(first) = loose.first() {
            errors.push(Refusal::new(
                Some((first.at, off)),
                format!(
                    "{} of {} references to this string were left as Japanese (first \
                     at 0x{:04x}). Recycling frees its bytes, so either every \
                     reference to it is replaced or none is",
                    loose.len(),
                    byoff[&off].len(),
                    first.at
                ),
            ));
        }
        if let Some(over) = p
            .strings
            .iter()
            .find(|s| off < s.offset && s.offset < ext[&off])
        {
            errors.push(Refusal::new(
                Some((byoff[&off][0].at, off)),
                format!(
                    "the run at 0x{:04x} reaches past the string at 0x{:04x}, so those \
                     bytes are not ours to reuse",
                    off, over.offset
                ),
            ));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut out = buf.to_vec();
    let mut offsets = BTreeMap::new();
    let mut placements = Vec::new();
    for seg in segments(p, &want, share) {
        let top = ext[seg.last().unwrap()];
        let room = top - seg[0];
        let need: usize = seg
            .iter()
            .map(|o| coded[&(*o, want[o].clone())].len() * 2 + 2)
            .sum();
        if need > room {
            let what = if seg.len() > 1 { "segment" } else { "string" };
            let why = format!(
                "{} 0x{:04x}-0x{:04x} holds {} words and the replacement needs {}: {} \
                 too many. This file has no arena, so a string can be no longer than \
                 the Japanese it replaces{}",
                what,
                seg[0],
                top,
                room / 2,
                need / 2,
                (need - room) / 2,
                if seg.len() > 1 { " plus its neighbours in the segment" } else { "" }
            );
            errors.extend(seg.iter().flat_map(|o| {
                byoff[o]
                    .iter()
                    .map(|u| Refusal::new(Some((u.at, *o)), why.clone()))
            }));
            continue;
        }
        let mut cur = seg[0];
        for o in &seg {
            let key = (*o, want[o].clone());
            let codes = &coded[&key];
            let was =
                read_codes(buf, *o, usize::MAX).map_or(0, |c| c.len() + 1);
            offsets.insert(key, cur);
            let bytes = codes_to_bytes(codes);
            out[cur..cur + bytes.len()].copy_from_slice(&bytes);
            if was >= B18_WORDS && codes.len() + 1 < B18_WORDS {
                placements.push(format!(
                    "warn 0x{:04x}: was {} words, now {}. If this is the string command \
                     0x0B18 writes into, it needs {} and will overrun",
                    o,
                    was,
                    codes.len() + 1,
                    B18_WORDS
                ));
            }
            placements.push(format!(
                "0x{:04x} -> 0x{:04x}  {}w -> {}w x{}",
                o,
                cur,
                was,
                codes.len() + 1,
                byoff[o].len()
            ));
            cur += bytes.len();
        }
        // slack reads as an empty string
        for i in (cur..top).step_by(2) {
            out[i..i + 2].copy_from_slice(&TERMINATOR.to_be_bytes());
        }
    }
    let slots = slotmap(groups, &offsets, &mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }
    for (slot, new) in slots {
        out[slot..slot + 2].copy_from_slice(&(new as u16).to_be_bytes());
    }
    Ok(Packed {
        bytes: out,
        placements,
    })
}

/// `repl` maps (reference position, original offset) -> replacement codes. The
/// position is the command address for a direct or directory reference and the
/// table slot address for a table one.
pub fn pack(
    buf: &[u8],
    p: &Plan,
    repl: &BTreeMap<(usize, usize), Vec<u16>>,
) -> Result<Packed, Vec<Refusal>> {
    if repl.is_empty() {
        return Ok(Packed {
            bytes: buf.to_vec(),
            placements: Vec::new(),
        });
    }
    let refs: HashMap<(usize, usize), &StringRef> =
        p.strings.iter().map(|s| ((s.at, s.offset), s)).collect();
    let mut errors = Vec::new();
    for key in repl.keys() {
        match refs.get(key) {
            None => errors.push(Refusal::new(
                Some(*key),
                format!(
                    "no string at reference 0x{:04x} offset 0x{:04x}",
                    key.0, key.1
                ),
            )),
            Some(_) if key.1 % 2 != 0 => errors.push(Refusal::new(
                Some(*key),
                format!(
                    "offset 0x{:04x} is odd, so it is not a halfword-aligned code \
                     array. The walker reached it through an operand that is not a \
                     text offset",
                    key.1
                ),
            )),
            Some(s) if s.slot.is_none() => errors.push(Refusal::new(
                Some(*key),
                format!(
                    "reached through a directory id whose slot is outside the declared \
                     directory [0x0004, 0x{:04x}); that word is command stream or \
                     string bytes, so it cannot be rewritten as an offset",
                    p.dir_words * 2
                ),
            )),
            _ => {}
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    // one copy per (original offset, replacement text). Refs that shared an
    // offset and get the same text keep sharing; refs given different text get
    // separate copies where there is room for them.
    let mut groups: BTreeMap<(usize, String), Vec<&StringRef>> =
        BTreeMap::new();
    for ((at, off), codes) in repl {
        let tag = codes.iter().map(|c| format!("{c:04x}")).collect::<String>();
        groups
            .entry((*off, tag))
            .or_default()
            .push(refs[&(*at, *off)]);
    }
    let mut coded = BTreeMap::new();
    for (key, users) in &groups {
        let orig = read_codes(buf, key.0, usize::MAX).unwrap_or_default();
        let codes = repl[&(users[0].at, key.0)].clone();
        let why = check(&codes, &orig, p.cap(users, orig.len())).err().or_else(|| {
            (codes.is_empty() && users.iter().any(|u| u.kind.is_walked_table()))
                .then(|| {
                    "a table entry cannot be blanked. Tables are recovered by \
                     decoding their entries, so an empty one ends the walk and hides \
                     every entry after it from the next run"
                        .to_string()
                })
        });
        match why {
            Some(why) => errors.extend(
                users
                    .iter()
                    .map(|u| Refusal::new(Some((u.at, key.0)), why.clone())),
            ),
            None => {
                coded.insert(key.clone(), codes);
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    if p.record_cursor < REACH {
        arena(buf, p, &groups, &coded)
    } else {
        recycle(buf, p, &groups, &coded, false)
    }
}

/// Re-walk the output the way the interpreter would and compare. It also
/// asserts the design's central claim directly: below the insertion point the
/// only bytes that differ are the record cursor and the operand words that name
/// a replaced string.
pub fn verify(
    buf: &[u8],
    out: &[u8],
    repl: &BTreeMap<(usize, usize), Vec<u16>>,
    glyphs: &HashMap<u16, String>,
) -> Vec<String> {
    let (a, b) = (plan(buf, glyphs), plan(out, glyphs));
    let mut bad = Vec::new();
    let at0 = a.record_cursor;
    // the arena writer always adds at least one aligned block, so an output the
    // same length as its input is a recycled one, and recycling is the only mode
    // allowed to touch the bytes of the strings it replaces
    let reused = !repl.is_empty() && out.len() == buf.len();
    let mut allowed: BTreeSet<usize> = [0, 1].into_iter().collect();
    for s in &a.strings {
        if repl.contains_key(&(s.at, s.offset)) {
            if let Some(slot) = s.slot {
                allowed.extend([slot, slot + 1]);
            }
            if reused {
                allowed.extend(s.offset..s.end);
            }
        }
    }
    let stray: Vec<usize> = (0..at0.min(buf.len()))
        .filter(|&i| buf[i] != out[i] && !allowed.contains(&i))
        .collect();
    if let Some(&first) = stray.first() {
        bad.push(format!(
            "{} bytes changed that should not have, first at 0x{:04x}",
            stray.len(),
            first
        ));
    }
    if out[at0 + (out.len() - buf.len())..] != buf[at0..] {
        bad.push(
            "the region at and after the record cursor is not a verbatim slide"
                .into(),
        );
    }
    if a.stats.sentinel && !b.stats.sentinel {
        bad.push("output never reaches the 0xFFFF sentinel".into());
    }
    if a.stats.steps != b.stats.steps {
        bad.push(format!(
            "command count changed: {} -> {}",
            a.stats.steps, b.stats.steps
        ));
    }
    if a.stats.unknown != b.stats.unknown {
        bad.push(format!("new dead ends: {:04x?}", b.stats.unknown));
    }
    let seen: HashMap<usize, &str> =
        b.strings.iter().map(|s| (s.at, s.jp.as_str())).collect();
    for ((at, _), codes) in repl {
        let want = render(codes, glyphs);
        if seen.get(at).copied() != Some(want.as_str()) {
            bad.push(format!(
                "command 0x{:04x}: expected {:?}, walker read {:?}",
                at,
                want.chars().take(40).collect::<String>(),
                seen.get(at)
                    .unwrap_or(&"<nothing>")
                    .chars()
                    .take(40)
                    .collect::<String>()
            ));
        }
    }
    bad
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_string_expansion_reserves_a_word_for_the_terminator() {
        assert!(check(&vec![0; 255], &[], None).is_ok());
        let error = check(&vec![0; 256], &[], None).unwrap_err();
        assert!(error.contains("255-code expansion cap"));
    }

    #[test]
    fn variadic_argument_families_keep_their_count_and_order() {
        // The status-damage crash: the call supplies player text, ailment
        // text, then damage. Moving the integer before the second string made
        // the damage value (0x84 in the observed crash) become a text pointer.
        let original = [0x4006, 0x2000, 0x4003, 0x2000, 0x3800, 0x1000];
        let reordered = [0x4006, 0x2000, 0x3800, 0x1000, 0x4003, 0x2000];
        let error = check_variadic_args(&original, &reordered).unwrap_err();
        assert!(error.contains("[2000, 2000, 1000] -> [2000, 1000, 2000]"));

        // Moving non-consuming controls is fine, and changing a field width
        // keeps the same argument type because only the family is compared.
        let safe = [0x2007, 0x3800, 0x4006, 0x2001, 0x1003];
        assert!(check_variadic_args(&original, &safe).is_ok());

        assert!(check_variadic_args(&[0x2000], &[0x2000, 0x1000]).is_err());
        assert!(check_variadic_args(&[0x2000, 0x1000], &[0x2000]).is_err());
    }

    #[test]
    fn nested_native_string_tables_are_discovered_with_leaf_slots() {
        let mut buf = vec![0u8; 0x200];
        let put = |buf: &mut [u8], at: usize, value: u16| {
            buf[at..at + 2].copy_from_slice(&value.to_be_bytes());
        };
        // Two outer entries. The first value proves the outer table ends at
        // 0x24; each inner table then ends at 0xffff.
        put(&mut buf, 0x20, 0x24);
        put(&mut buf, 0x22, 0x2a);
        put(&mut buf, 0x24, 0x100);
        put(&mut buf, 0x26, 0x104);
        put(&mut buf, 0x28, 0xffff);
        put(&mut buf, 0x2a, 0x108);
        put(&mut buf, 0x2c, 0xffff);
        put(&mut buf, 0x100, 1);
        put(&mut buf, 0x102, TERMINATOR);
        put(&mut buf, 0x104, 2);
        put(&mut buf, 0x106, TERMINATOR);
        put(&mut buf, 0x108, 1);
        put(&mut buf, 0x10a, 2);
        put(&mut buf, 0x10c, TERMINATOR);

        let glyphs =
            HashMap::from([(1, "A".to_string()), (2, "B".to_string())]);
        let found = native_tables(
            &buf,
            &[0x20],
            4,
            buf.len(),
            &vec![false; buf.len()],
            &[],
            &glyphs,
        );
        assert_eq!(found.len(), 3);
        assert_eq!(
            found.iter().map(|s| (s.slot, s.offset)).collect::<Vec<_>>(),
            [
                (Some(0x24), 0x100),
                (Some(0x26), 0x104),
                (Some(0x2a), 0x108)
            ]
        );
        assert_eq!(
            found.iter().map(|s| s.jp.as_str()).collect::<Vec<_>>(),
            ["A", "B", "AB"]
        );
    }

    /// The two shapes a jump table comes in: bodies laid out right after it,
    /// and a table the next command abuts.
    #[test]
    fn a_jump_table_ends_at_its_first_body_or_at_the_next_command() {
        let mut buf = vec![0u8; 0x40];
        let put = |buf: &mut [u8], at: usize, value: u16| {
            buf[at..at + 2].copy_from_slice(&value.to_be_bytes());
        };
        // three cases whose bodies begin at 0x0a, where the table stops
        put(&mut buf, 0x04, 0x0a);
        put(&mut buf, 0x06, 0x0c);
        put(&mut buf, 0x08, 0x0e);
        put(&mut buf, 0x0a, END);
        put(&mut buf, 0x0c, END);
        put(&mut buf, 0x0e, END);
        assert_eq!(jump_array(&buf, 0x04, buf.len()), [0x0a, 0x0c, 0x0e]);

        // two cases far away, and then a 0x0702 command whose opcode reads as
        // the offset 0x0702 -- past the buffer, so it cannot be a target
        put(&mut buf, 0x20, 0x30);
        put(&mut buf, 0x22, 0x32);
        put(&mut buf, 0x24, 0x0702);
        put(&mut buf, 0x30, END);
        put(&mut buf, 0x32, END);
        assert_eq!(jump_array(&buf, 0x20, buf.len()), [0x30, 0x32]);

        // the same, with the stray offset in range but landing on nothing the
        // interpreter could run
        put(&mut buf, 0x24, 0x12);
        put(&mut buf, 0x12, 0xdead);
        assert_eq!(jump_array(&buf, 0x20, buf.len()), [0x30, 0x32]);
    }

    /// An `OBJ_TEXT` array runs to the zero padding, and every entry is a
    /// writable slot -- the shape `drama09.spt:0x63c` has.
    #[test]
    fn an_obj_text_array_ends_where_its_entries_stop_decoding() {
        let mut buf = vec![0u8; 0x100];
        let put = |buf: &mut [u8], at: usize, value: u16| {
            buf[at..at + 2].copy_from_slice(&value.to_be_bytes());
        };
        put(&mut buf, 0x40, 0x60);
        put(&mut buf, 0x42, 0x66);
        // 0x44 stays zero: the pad the arrays that end early are written with
        put(&mut buf, 0x60, 1);
        put(&mut buf, 0x62, 2);
        put(&mut buf, 0x64, TERMINATOR);
        put(&mut buf, 0x66, 2);
        put(&mut buf, 0x68, TERMINATOR);

        let glyphs =
            HashMap::from([(1, "A".to_string()), (2, "B".to_string())]);
        let mut cmds = vec![false; buf.len()];
        cmds[..0x10].fill(true);
        let found =
            obj_text_arrays(&buf, &[0x40, 0x40], 4, &cmds, &[], &glyphs);
        assert_eq!(
            found
                .iter()
                .map(|s| (s.slot, s.offset, s.jp.as_str()))
                .collect::<Vec<_>>(),
            [(Some(0x40), 0x60, "AB"), (Some(0x42), 0x66, "B")]
        );
        // a base the walk already claimed as a command is not an array
        assert!(
            obj_text_arrays(&buf, &[0x08], 4, &cmds, &[], &glyphs).is_empty()
        );
    }
}
