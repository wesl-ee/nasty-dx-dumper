//! Text the code addresses with an immediate pair (`lis` + `addi`/`ori`)
//! instead of loading from a pointer word.
//!
//! Both the dumper and the patcher call [`scan`] on the same untouched
//! `main.dol`, so the pairing never has to survive in the `.patch` file: the
//! reference comment records the low half's file offset and the patcher looks
//! the pair up again.
//!
//! Anything left out is recorded in [`Scan::rejected`] and the string it points
//! at is dropped from the dump rather than half-relocated.

use std::collections::{HashMap, HashSet};

use crate::shape::DolHeader;

/// `_SDA_BASE_` (r13) and `_SDA2_BASE_` (r2), set by the init code at
/// 0x8000544c and 0x80005444. Nothing in game code reloads them.
const SDA_BASE: u32 = 0x802d4e00;
const SDA2_BASE: u32 = 0x802d6920;

/// A run this long is not a string; the longest the game ships is 163 glyphs.
const MAX_GLYPHS: u32 = 4096;

const TERMINATOR: u16 = 0xf800;

/// The reverse-engineered verdict for every address the code materialises as a
/// constant, keyed by RAM address.
const TEXT_REFS: &str = include_str!("../data/text-refs.json");

/// Addresses `text-refs.json` says can be moved, and the reason for those it
/// says cannot.
fn relocatable_constants() -> (HashSet<u32>, HashMap<u32, String>) {
    let mut ok = HashSet::new();
    let mut blocked = HashMap::new();
    for line in TEXT_REFS.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("\"0x") else {
            continue;
        };
        let Some((addr, body)) = rest.split_once("\":") else {
            continue;
        };
        let Ok(addr) = u32::from_str_radix(addr, 16) else {
            continue;
        };
        if body.contains("\"relocatable\": true") {
            ok.insert(addr);
        } else {
            let why = body
                .split_once("\"blocked_by\": [")
                .and_then(|(_, t)| t.split_once(']'))
                .map(|(w, _)| w.replace('"', ""))
                .unwrap_or_default();
            blocked.insert(addr, why);
        }
    }
    assert!(
        ok.len() + blocked.len() > 3000,
        "text-refs.json did not parse: {} ok, {} blocked",
        ok.len(),
        blocked.len()
    );
    (ok, blocked)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImmKind {
    /// low half is sign-extended, so the high half needs the `@ha` carry
    Addi,
    /// low half is zero-extended
    Ori,
}

impl ImmKind {
    pub fn name(&self) -> &'static str {
        match self {
            ImmKind::Addi => "addi",
            ImmKind::Ori => "ori",
        }
    }
}

/// One `lis` + low-half pair that builds the address of a glyph run.
#[derive(Clone, Debug)]
pub struct ImmRef {
    pub hi_off: u32,
    pub hi_ram: u32,
    pub lo_off: u32,
    pub lo_ram: u32,
    pub target_off: u32,
    pub target_ram: u32,
    pub kind: ImmKind,
}

/// A glyph run the code addresses in a way we will not rewrite. Dropped from
/// the dump even if another site for it looked fine: translating it would
/// leave the site we cannot rewrite pointing at the Japanese.
pub struct Rejection {
    pub why: String,
    pub at_ram: u32,
}

pub struct Scan {
    /// pairs we are willing to rewrite, keyed by the low half's file offset.
    pub refs: HashMap<u32, ImmRef>,
    /// target RAM address -> why we will not touch it.
    pub rejected: HashMap<u32, Rejection>,
}

impl Scan {
    /// Pairs whose target survived every rejection, grouped by target file
    /// offset.
    pub fn usable(&self) -> HashMap<u32, Vec<&ImmRef>> {
        let mut out: HashMap<u32, Vec<&ImmRef>> = HashMap::new();
        for r in self.refs.values() {
            if self.rejected.contains_key(&r.target_ram) {
                continue;
            }
            out.entry(r.target_off).or_default().push(r);
        }
        for v in out.values_mut() {
            v.sort_by_key(|r| r.lo_off);
        }
        out
    }
}

pub fn scan(buf: &[u8], header: &DolHeader) -> Scan {
    let mut scan = Scan {
        refs: HashMap::new(),
        rejected: HashMap::new(),
    };
    let targets = branch_targets(buf, header);

    for section in header.live() {
        if section.file >= header.data_start() {
            continue; // data, not instructions
        }
        let mut regs: [Option<Pending>; 32] = std::array::from_fn(|_| None);

        for i in (0..section.size).step_by(4) {
            let off = section.file + i;
            let ram = section.ram + i;
            let Some(w) = word(buf, off) else { continue };

            let op = w >> 26;
            let d = ((w >> 21) & 31) as usize;
            let a = ((w >> 16) & 31) as usize;

            // `lis rD,imm` is `addis rD,r0,imm`
            if op == 15 && a == 0 {
                retire(regs[d].take(), &targets, &mut scan);
                regs[d] = Some(Pending {
                    hi_off: off,
                    hi_ram: ram,
                    value: (w & 0xffff) << 16,
                    consumers: Vec::new(),
                    poison: None,
                });
                continue;
            }

            let e = eff(w);

            // an address built off the small-data bases is not a pair we can
            // move: the displacement is a fixed +-32K window around a register
            // the init code sets once.
            if op == 14 && (a == 13 || a == 2) && regs[a].is_none() {
                let base = if a == 13 { SDA_BASE } else { SDA2_BASE };
                let t = base.wrapping_add((w & 0xffff) as i16 as i32 as u32);
                if glyph_run(buf, header, t).is_some() {
                    scan.rejected.insert(
                        t,
                        Rejection {
                            why: "addressed off the r2/r13 small-data base, \
                                  whose +-32K displacement cannot reach the \
                                  arena"
                                .into(),
                            at_ram: ram,
                        },
                    );
                }
            }

            if let Some((base, disp, kind)) = e.consumer {
                if let Some(p) = regs[base as usize].as_mut() {
                    let t = match kind {
                        ImmKind::Addi => p.value.wrapping_add(disp as u32),
                        ImmKind::Ori => p.value | (disp as u32),
                    };
                    match glyph_run(buf, header, t) {
                        Some(target_off) => p.consumers.push(Consumer {
                            lo_off: off,
                            lo_ram: ram,
                            target_ram: t,
                            target_off,
                            kind,
                        }),
                        // the same high half also builds a non-text address, so
                        // it is not ours to move
                        None => p.poison(
                            "the lis is shared with an address that is not text",
                        ),
                    }
                }
            }

            if !e.known {
                for r in regs.iter_mut() {
                    if let Some(p) = r.as_mut() {
                        p.poison("an instruction we cannot decode reads or writes the register");
                    }
                    retire(r.take(), &targets, &mut scan);
                }
                continue;
            }

            for (r, pending) in regs.iter_mut().enumerate() {
                if e.reads & (1 << r) != 0 {
                    if let Some(p) = pending.as_mut() {
                        p.poison(
                            "the lis result is used other than to build one \
                             address",
                        );
                    }
                }
            }
            for (r, pending) in regs.iter_mut().enumerate() {
                if e.writes & (1 << r) != 0 {
                    retire(pending.take(), &targets, &mut scan);
                }
            }

            match e.flow {
                Flow::Straight => {}
                // volatiles do not survive a call; everything else does
                Flow::Call => {
                    for r in [0usize, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12] {
                        retire(regs[r].take(), &targets, &mut scan);
                    }
                }
                // control leaves: the straight-line window ends here. Whether
                // the address itself may move is not a question this walk can
                // answer, and it no longer tries -- see the module docs.
                Flow::Branch => {
                    for r in regs.iter_mut() {
                        retire(r.take(), &targets, &mut scan);
                    }
                }
            }
        }

        for r in regs.iter_mut() {
            retire(r.take(), &targets, &mut scan);
        }
    }

    // the encoding-level walk above only proves which two words move together.
    // whether the address may move at all is Ghidra's answer, not ours.
    let (ok, blocked) = relocatable_constants();
    let mut verdicts: Vec<(u32, Rejection)> = Vec::new();
    let mut refs: Vec<&ImmRef> = scan.refs.values().collect();
    refs.sort_by_key(|r| r.lo_off);
    for r in refs {
        if ok.contains(&r.target_ram) {
            continue;
        }
        let why = match blocked.get(&r.target_ram) {
            Some(w) => format!("the address is used as a value: {w}"),
            None => "no code in the decompiled program materialises this \
                     address"
                .to_string(),
        };
        verdicts.push((
            r.target_ram,
            Rejection {
                why,
                at_ram: r.lo_ram,
            },
        ));
    }
    for (target, why) in verdicts {
        scan.rejected.insert(target, why);
    }

    // a target reached both safely and unsafely is not safe
    scan.refs
        .retain(|_, r| !scan.rejected.contains_key(&r.target_ram));
    scan
}

struct Consumer {
    lo_off: u32,
    lo_ram: u32,
    target_ram: u32,
    target_off: u32,
    kind: ImmKind,
}

struct Pending {
    hi_off: u32,
    hi_ram: u32,
    value: u32,
    consumers: Vec<Consumer>,
    poison: Option<&'static str>,
}

impl Pending {
    fn poison(&mut self, why: &'static str) {
        self.poison.get_or_insert(why);
    }
}

fn retire(p: Option<Pending>, targets: &HashSet<u32>, scan: &mut Scan) {
    let Some(p) = p else { return };
    if p.consumers.is_empty() {
        return;
    }

    let last = p.consumers.iter().map(|c| c.lo_ram).max().unwrap();
    // a branch landing between the two words means the register may hold
    // something else by the time the low half reads it
    let entered = ((p.hi_ram + 4)..=last)
        .step_by(4)
        .any(|a| targets.contains(&a));
    let shared = p
        .consumers
        .iter()
        .any(|c| c.target_ram != p.consumers[0].target_ram);

    let why = match (p.poison, entered, shared) {
        (Some(w), _, _) => Some(w),
        (_, true, _) => Some(
            "a branch lands between the lis and the low half, so the register \
             may hold something else by then",
        ),
        (_, _, true) => Some(
            "one lis is shared by low halves that build different addresses",
        ),
        _ => None,
    };

    for c in &p.consumers {
        match why {
            Some(w) => {
                scan.rejected.insert(
                    c.target_ram,
                    Rejection {
                        why: w.to_string(),
                        at_ram: c.lo_ram,
                    },
                );
            }
            None => {
                scan.refs.insert(
                    c.lo_off,
                    ImmRef {
                        hi_off: p.hi_off,
                        hi_ram: p.hi_ram,
                        lo_off: c.lo_off,
                        lo_ram: c.lo_ram,
                        target_off: c.target_off,
                        target_ram: c.target_ram,
                        kind: c.kind,
                    },
                );
            }
        }
    }
}

/// Every address a direct branch in the DOL can jump to. Used to prove nothing
/// jumps into the middle of a pair.
fn branch_targets(buf: &[u8], header: &DolHeader) -> HashSet<u32> {
    let mut out = HashSet::new();
    for section in header.live() {
        if section.file >= header.data_start() {
            continue;
        }
        for i in (0..section.size).step_by(4) {
            let ram = section.ram + i;
            let Some(w) = word(buf, section.file + i) else {
                continue;
            };
            let aa = w & 2 != 0;
            let t = match w >> 26 {
                18 => ((w & 0x03fffffc) as i32) << 6 >> 6,
                16 => ((w & 0xfffc) as i16) as i32,
                _ => continue,
            } as u32;
            out.insert(if aa { t } else { ram.wrapping_add(t) });
        }
    }
    out
}

/// Does this halfword make sense inside a string?
///
/// Every 16-bit value decodes as *something*: the renderer switches on
/// `code & 0xF800` and quietly ignores the families it has no case for
/// (docs/showtextasbox.md), so "the run ends in 0xF800" does not tell a string
/// apart from a float pool, a pointer table or an ASCII filename. The payload
/// does. The tables a payload indexes have known sizes, and the families that
/// ignore their payload only ever ship with it zeroed. All 2231 strings the
/// pointer tables already reach pass this; the float pool in data4, the
/// `0x80xx` halves of pointer tables and the C strings in data5 do not.
fn text_code(g: u16) -> bool {
    let n = g & 0x07ff;
    match g & 0xf800 {
        0x0000 => n < 0x0400, // four font page tables exist
        0x1000 | 0x1800 | 0x7800 => n <= 31, // digits; more overruns the scratch buffer
        0x2000 | 0x2800 | 0x6800 => n <= 64, // field width in characters
        0x3000 => n <= 0xff, // per-character delay, 0xff meaning none
        0x4000 => n <= 0xf,  // 16-entry colour table at 0x801b7404
        0x7000 => n <= 8,    // 9-entry scale table at 0x801b73e0
        // pen x, pen y and the shrink budget, all in pixels on a 640-wide screen
        0x5000 | 0x6000 | 0xc800 | 0xe000 | 0xf000 => n <= 640,
        0xc000 => false, // no case in the handler tree
        // newline, cursor push and pop, page break: the payload is ignored, so
        // real text leaves it zero
        0x3800 | 0x4800 | 0x5800 | 0x8000 | 0x8800 | 0x9000 | 0x9800
        | 0xa000 | 0xb000 => n == 0,
        // sound, icon and voice ids, whose tables we have not sized
        _ => true,
    }
}

/// File offset of a 0xF800-terminated run at `addr`, if that is really what
/// lives there.
fn glyph_run(buf: &[u8], header: &DolHeader, addr: u32) -> Option<u32> {
    if addr & 1 != 0 {
        return None;
    }
    if crate::constants::EXCLUDED_NOTES
        .iter()
        .any(|(a, _)| *a == addr)
        || crate::constants::INLINE_FIELDS
            .iter()
            .any(|f| f.contains(addr))
    {
        return None;
    }
    let section = header.live().find(|s| {
        s.file >= header.data_start() && addr >= s.ram && addr - s.ram < s.size
    })?;
    let off = section.file + (addr - section.ram);

    let mut at = off;
    let end = section.file + section.size;
    for n in 0..MAX_GLYPHS {
        if at + 2 > end {
            return None;
        }
        let g = u16::from_be_bytes([buf[at as usize], buf[at as usize + 1]]);
        if g == TERMINATOR {
            // a bare terminator is a hole between strings, not a string
            return (n > 0).then_some(off);
        }
        if !text_code(g) {
            return None;
        }
        at += 2;
    }
    None
}

fn word(buf: &[u8], off: u32) -> Option<u32> {
    buf.get(off as usize..off as usize + 4)
        .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
}

enum Flow {
    Straight,
    Call,
    Branch,
}

struct Eff {
    reads: u32,
    writes: u32,
    consumer: Option<(u8, i32, ImmKind)>,
    flow: Flow,
    known: bool,
}

fn rw(writes: u32, reads: u32) -> Eff {
    Eff {
        reads,
        writes,
        consumer: None,
        flow: Flow::Straight,
        known: true,
    }
}

fn flow(f: Flow) -> Eff {
    Eff {
        reads: 0,
        writes: 0,
        consumer: None,
        flow: f,
        known: true,
    }
}

fn unknown() -> Eff {
    Eff {
        reads: 0,
        writes: 0,
        consumer: None,
        flow: Flow::Straight,
        known: false,
    }
}

/// Which GPRs an instruction reads and writes.
///
/// Writes have to be exact: pretending a register is written hides a later
/// consumer and would let us rewrite a `lis` somebody else still reads. Reads
/// may be over-reported, since an extra read only costs us a string.
fn eff(w: u32) -> Eff {
    let op = w >> 26;
    let d = (w >> 21) & 31;
    let a = (w >> 16) & 31;
    let b = (w >> 11) & 31;
    let bit = |r: u32| 1u32 << r;
    let upto31 = |r: u32| !0u32 << r;
    let simm = (w & 0xffff) as i16 as i32;
    let uimm = (w & 0xffff) as i32;

    match op {
        3 => rw(0, bit(a)),                    // twi
        7 | 8 | 12 | 13 => rw(bit(d), bit(a)), // mulli subfic addic addic.
        10 | 11 => rw(0, bit(a)),              // cmpli cmpi
        14 => {
            // addi / li / subi
            if a == 0 {
                rw(bit(d), 0)
            } else {
                Eff {
                    writes: bit(d),
                    consumer: Some((a as u8, simm, ImmKind::Addi)),
                    ..rw(0, 0)
                }
            }
        }
        15 => rw(bit(d), bit(a)), // addis with rA != 0; lis is handled by scan
        16 => flow(Flow::Branch), // bc
        18 => flow(if w & 1 != 0 { Flow::Call } else { Flow::Branch }),
        19 => match (w >> 1) & 0x3ff {
            16 | 528 => {
                flow(if w & 1 != 0 { Flow::Call } else { Flow::Branch })
            }
            0 | 33 | 129 | 150 | 193 | 225 | 257 | 289 | 417 | 449 => rw(0, 0),
            _ => unknown(),
        },
        20 => rw(bit(a), bit(d) | bit(a)), // rlwimi
        21 => rw(bit(a), bit(d)),          // rlwinm
        23 => rw(bit(a), bit(d) | bit(b)), // rlwnm
        24 => {
            // ori; `ori r0,r0,0` is the nop
            if w == 0x60000000 {
                rw(0, 0)
            } else {
                Eff {
                    writes: bit(a),
                    consumer: Some((d as u8, uimm, ImmKind::Ori)),
                    ..rw(0, 0)
                }
            }
        }
        25..=29 => rw(bit(a), bit(d)), // oris xori xoris andi. andis.
        31 => x_form(w, d, a, b),
        32 | 34 | 40 | 42 => rw(bit(d), bit(a)), // lwz lbz lhz lha
        33 | 35 | 41 | 43 => rw(bit(d) | bit(a), bit(a)), // ...u
        36 | 38 | 44 => rw(0, bit(d) | bit(a)),  // stw stb sth
        37 | 39 | 45 => rw(bit(a), bit(d) | bit(a)), // ...u
        46 => rw(upto31(d), bit(a)),             // lmw
        47 => rw(0, upto31(d) | bit(a)),         // stmw
        48 | 50 | 52 | 54 | 56 | 60 => rw(0, bit(a)), // lfs lfd stfs stfd psq_l psq_st
        49 | 51 | 53 | 55 | 57 | 61 => rw(bit(a), bit(a)), // ...u
        4 | 59 | 63 => rw(0, bit(a) | bit(b)), // ps / float: no GPR result
        _ => unknown(),
    }
}

fn x_form(w: u32, d: u32, a: u32, b: u32) -> Eff {
    let bit = |r: u32| 1u32 << r;
    let xo = (w >> 1) & 0x3ff;
    // the OE bit sits above the 9-bit XO of the XO-form arithmetic
    let arith =
        matches!(xo & 0x1ff, 8 | 10 | 40 | 136 | 138 | 235 | 266 | 459 | 491)
            || matches!(xo & 0x1ff, 104 | 200 | 202 | 232 | 234);

    match xo {
        0 | 4 | 32 => rw(0, bit(a) | bit(b)), // cmp tw cmpl
        // indexed loads
        20 | 23 | 87 | 279 | 343 | 533 | 534 | 790 => {
            rw(bit(d), bit(a) | bit(b))
        }
        55 | 119 | 311 | 375 => rw(bit(d) | bit(a), bit(a) | bit(b)),
        // indexed stores
        150 | 151 | 215 | 407 | 661 | 662 | 918 => {
            rw(0, bit(d) | bit(a) | bit(b))
        }
        183 | 247 | 439 => rw(bit(a), bit(d) | bit(a) | bit(b)),
        // lswi / stswi touch a span we cannot name, so they are a barrier
        // indexed float loads and stores
        535 | 599 | 663 | 727 | 983 => rw(0, bit(a) | bit(b)),
        567 | 631 | 695 | 759 => rw(bit(a), bit(a) | bit(b)),
        // rD = f(rA, rB)
        8 | 10 | 11 | 40 | 75 | 136 | 138 | 235 | 266 | 459 | 491 => {
            rw(bit(d), bit(a) | bit(b))
        }
        104 | 200 | 202 | 232 | 234 => rw(bit(d), bit(a)), // neg subfze addze subfme addme
        // rA = f(rS, rB)
        24 | 28 | 60 | 124 | 284 | 316 | 412 | 444 | 476 | 536 | 792 => {
            rw(bit(a), bit(d) | bit(b))
        }
        26 | 824 | 922 | 954 => rw(bit(a), bit(d)), // cntlzw srawi extsh extsb
        19 | 83 | 339 | 371 => rw(bit(d), 0),       // mfcr mfmsr mfspr mftb
        144 | 146 | 467 | 512 => rw(0, bit(d)),     // mtcrf mtmsr mtspr mcrxr
        // cache, sync, tlb
        54 | 86 | 246 | 278 | 306 | 370 | 470 | 566 | 598 | 854 | 982
        | 1014 => rw(0, bit(a) | bit(b)),
        _ if xo >= 512 && arith => rw(bit(d), bit(a) | bit(b)),
        _ => unknown(),
    }
}

/// The two instruction words that make `pair` build `addr` instead.
///
/// `addi` sign-extends its low half, so a low half with bit 15 set borrows one
/// from the high half; `ori` zero-extends and must not get that carry.
pub fn rewrite(
    hi_insn: u32,
    lo_insn: u32,
    kind: ImmKind,
    addr: u32,
) -> (u32, u32) {
    let (hi, lo) = match kind {
        ImmKind::Addi => {
            (((addr >> 16) + ((addr >> 15) & 1)) & 0xffff, addr & 0xffff)
        }
        ImmKind::Ori => (addr >> 16, addr & 0xffff),
    };
    ((hi_insn & 0xffff0000) | hi, (lo_insn & 0xffff0000) | lo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addi_low_half_borrows_from_the_high_half() {
        // lis r5,0x8025 / addi r5,r5,0x5ee4
        let (hi, lo) =
            rewrite(0x3ca08025, 0x38a55ee4, ImmKind::Addi, 0x802d0698);
        assert_eq!(hi, 0x3ca0802d);
        assert_eq!(lo, 0x38a50698);
        // bit 15 set: the addi subtracts 0x10000, so the lis must add it back
        let (hi, lo) =
            rewrite(0x3ca08025, 0x38a55ee4, ImmKind::Addi, 0x802dfedc);
        assert_eq!(hi, 0x3ca0802e);
        assert_eq!(lo, 0x38a5fedc);
        assert_eq!(
            (0x802eu32 << 16).wrapping_add(0xfedcu16 as i16 as u32),
            0x802dfedc
        );
    }

    /// The egg-space digging lines were translated and their pointer word was
    /// relocated, while the code that builds their address directly was left
    /// alone, so one route through the game showed English and the other
    /// Japanese. Both are on the branch-crossing shape the old linear walk
    /// rejected out of hand.
    #[test]
    fn the_excavation_point_lines_are_relocatable() {
        let (ok, blocked) = relocatable_constants();
        for addr in [0x8025_791c, 0x8025_7948] {
            assert!(
                ok.contains(&addr),
                "0x{addr:08x} is not relocatable: {:?}",
                blocked.get(&addr)
            );
        }
    }

    /// A table base is not one string and cannot move on its own. The old walk
    /// accepted this one; Ghidra sees the indexing.
    #[test]
    fn a_table_base_is_not_relocatable() {
        let (ok, blocked) = relocatable_constants();
        assert!(!ok.contains(&0x8025_7808));
        assert!(blocked[&0x8025_7808].contains("PTRADD"));
    }

    #[test]
    fn ori_low_half_does_not() {
        let (hi, lo) =
            rewrite(0x3ca08025, 0x60a55ee4, ImmKind::Ori, 0x802dfedc);
        assert_eq!(hi, 0x3ca0802d);
        assert_eq!(lo, 0x60a5fedc);
    }
}
