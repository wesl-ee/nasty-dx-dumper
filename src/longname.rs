//! Names longer than the 18-byte record field they are stored in.
//!
//! Every name in the game lands in a fixed `0x12`-byte slot at some point --
//! `spawnActorFromDef` copies one out of `actorDefs`, `initActorFromRecord`
//! carries it into the battle actor, `800d0aa4` restores it from the backup at
//! `+0x10A`. The slot cannot grow: the halfword at `+0x12` is the next struct
//! member.
//!
//! So the slot stops holding glyphs. It holds a two-word *stub*
//!
//! ```text
//!     0xC000 | idx      "continue this string at longNamePtrs[idx]"
//!     0xF800            terminator, for any reader that has not been hooked
//! ```
//!
//! and the real name lives here, in the bank. Every copy in the game transports
//! the stub unchanged, so none of them needs patching; the only code change is
//! one trampoline per text walker (`asm/long_names.s`).
//!
//! Family `0xC000` is free: it is dead in all six walkers and appears zero
//! times in the 1112 extracted strings.

use anyhow::{anyhow, Result};

use crate::constants::{
    LONG_NAME_GLYPHS, LONG_NAME_INDICES, REDIRECT_FAMILY, STUB_STRIDE,
};
use crate::spt::TERMINATOR;

/// Is this the first word of a redirect?
///
/// The chain invariant is that no interned name starts with one, which is what
/// makes an infinite redirect loop unrepresentable rather than merely unlikely
/// -- the caves branch back to their own loop head after following a redirect.
pub fn is_redirect(code: u16) -> bool {
    code & 0xf800 == REDIRECT_FAMILY
}

/// The word that stands in for name `idx`.
pub fn stub_word(idx: u32) -> u16 {
    REDIRECT_FAMILY | idx as u16
}

/// Where each piece of the bank ended up, in RAM.
pub struct BankLayout {
    /// `longNamePtrs`, the table the caves index.
    pub ptrs: u32,
}

#[derive(Default)]
pub struct LongNameBank {
    /// the names themselves, each terminated, concatenated.
    strings: Vec<u8>,
    /// per index, the byte offset of its name within `strings`.
    offsets: Vec<u32>,
}

impl LongNameBank {
    pub fn new() -> LongNameBank {
        LongNameBank::default()
    }

    /// Take a name into the bank. Returns its index, which is both the stub's
    /// argument and its position in the stub blob.
    pub fn intern(&mut self, codes: &[u16]) -> Result<u32> {
        if codes.len() > LONG_NAME_GLYPHS {
            return Err(anyhow!(
                "{} glyphs exceeds the {LONG_NAME_GLYPHS}-glyph redirected \
                 name cap",
                codes.len()
            ));
        }
        // a name starting with a redirect would send the cave straight back
        // into its own loop head with a cursor it has already followed
        if codes.first().is_some_and(|&c| is_redirect(c)) {
            return Err(anyhow!(
                "a redirected name may not itself begin with a 0x{REDIRECT_FAMILY:04X} \
                 family word"
            ));
        }
        let idx = self.offsets.len() as u32;
        if idx >= LONG_NAME_INDICES {
            return Err(anyhow!(
                "the redirect opcode names at most {LONG_NAME_INDICES} strings"
            ));
        }

        self.offsets.push(self.strings.len() as u32);
        for &code in codes {
            self.strings.extend(code.to_be_bytes());
        }
        self.strings.extend(TERMINATOR.to_be_bytes());
        Ok(idx)
    }

    /// The `0x12` bytes that replace a name wherever it is stored inline: the
    /// stub, then zeroes out to whatever width the copy moves. Past the
    /// terminator nothing is read; the padding exists so the blind copy has
    /// something defined to move.
    pub fn record(idx: u32, bytes: usize) -> Vec<u8> {
        let mut out = vec![0u8; bytes];
        out[0..2].copy_from_slice(&stub_word(idx).to_be_bytes());
        out[2..4].copy_from_slice(&TERMINATOR.to_be_bytes());
        out
    }

    /// Lay the bank out at `base_ram` and return its bytes.
    ///
    /// ```text
    ///   +0000  stubs         STUB_STRIDE each, one per interned name
    ///   +....  strings       the names, terminated
    ///   +....  empty         one 0xF800 word
    ///   +....  ptrs[0x800]   u32, every entry resolved
    /// ```
    ///
    /// The table is fully populated: an index nobody claimed points at `empty`,
    /// so a corrupt redirect renders nothing rather than walking off into the
    /// heap.
    pub fn emit(&self, base_ram: u32) -> (Vec<u8>, BankLayout) {
        let stubs_len = self.offsets.len() * STUB_STRIDE;
        let strings_at = stubs_len;
        let empty_at = strings_at + self.strings.len();
        let ptrs_at = (empty_at + 2 + 3) & !3;

        let mut out =
            Vec::with_capacity(ptrs_at + LONG_NAME_INDICES as usize * 4);
        for idx in 0..self.offsets.len() {
            out.extend(LongNameBank::record(idx as u32, STUB_STRIDE));
        }
        out.extend_from_slice(&self.strings);
        out.extend(TERMINATOR.to_be_bytes());
        out.resize(ptrs_at, 0);

        let empty_ram = base_ram + empty_at as u32;
        for i in 0..LONG_NAME_INDICES as usize {
            let ram = match self.offsets.get(i) {
                Some(off) => base_ram + strings_at as u32 + off,
                None => empty_ram,
            };
            out.extend(ram.to_be_bytes());
        }

        (
            out,
            BankLayout {
                ptrs: base_ram + ptrs_at as u32,
            },
        )
    }

    /// RAM of the stub that stands in for name `idx`.
    pub fn stub_ram(base_ram: u32, idx: u32) -> u32 {
        base_ram + idx * STUB_STRIDE as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stub_is_a_terminated_two_word_string() {
        let stub = LongNameBank::record(0x123, STUB_STRIDE);
        assert_eq!(stub.len(), STUB_STRIDE);
        assert_eq!(&stub[0..4], [0xc1, 0x23, 0xf8, 0x00]);
        // the blind 0x12-byte copy has to have something defined to move
        assert!(stub.len() >= crate::constants::NAME_FIELD_BYTES);
        assert!(stub[4..].iter().all(|&b| b == 0));
    }

    #[test]
    fn the_bank_refuses_a_name_that_begins_with_a_redirect() {
        let mut bank = LongNameBank::new();
        let err = bank.intern(&[stub_word(3), 0x0041]).unwrap_err();
        assert!(err.to_string().contains("may not itself begin"));
    }

    #[test]
    fn the_bank_refuses_a_name_over_the_glyph_cap() {
        let mut bank = LongNameBank::new();
        let long = vec![0x0041u16; LONG_NAME_GLYPHS + 1];
        assert!(bank.intern(&long).is_err());
        assert!(bank.intern(&long[..LONG_NAME_GLYPHS]).is_ok());
    }

    #[test]
    fn every_pointer_table_entry_resolves_into_the_bank() {
        let mut bank = LongNameBank::new();
        assert_eq!(bank.intern(&[0x0041, 0x0042]).unwrap(), 0);
        assert_eq!(bank.intern(&[0x0043]).unwrap(), 1);

        const BASE: u32 = 0x802e_0000;
        let (blob, layout) = bank.emit(BASE);
        let word = |ram: u32| {
            let at = (ram - BASE) as usize;
            u16::from_be_bytes(blob[at..at + 2].try_into().unwrap())
        };
        let ptr = |i: u32| {
            let at = (layout.ptrs - BASE + i * 4) as usize;
            u32::from_be_bytes(blob[at..at + 4].try_into().unwrap())
        };

        // the stub each reference is pointed at
        assert_eq!(word(LongNameBank::stub_ram(BASE, 0)), stub_word(0));
        assert_eq!(word(LongNameBank::stub_ram(BASE, 1)), stub_word(1));
        // and what following the redirect lands on
        assert_eq!(word(ptr(0)), 0x0041);
        assert_eq!(word(ptr(1)), 0x0043);
        // unclaimed indices, including the last, render as nothing
        assert_eq!(word(ptr(2)), TERMINATOR);
        assert_eq!(word(ptr(LONG_NAME_INDICES - 1)), TERMINATOR);
        assert_eq!(layout.ptrs % 4, 0);
    }
}
