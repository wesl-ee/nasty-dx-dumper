use anyhow::Result;
use std::io::{Read, Write};

/// text0..text6 then data0..data10, the order they appear on disk
pub const SECTIONS: usize = 18;
/// the first data section: everything below it is instructions
pub const DATA0: usize = 7;
/// the last section the retail DOL ships; data8 is appended right after it
pub const DATA7: usize = 14;
/// the translation bank, empty on the original ROM
pub const DATA8: usize = 15;
/// the redirect caves. Empty on retail, and a text section rather than a
/// corner of data8 because the DOL loader invalidates icache per text section.
pub const TEXT2: usize = 2;

#[derive(Clone, Copy, Default)]
pub struct Section {
    pub file: u32,
    pub ram: u32,
    pub size: u32,
}

#[derive(Default)]
pub struct DolHeader {
    pub sections: [Section; SECTIONS],
    pub bss_address: u32,
    pub bss_size: u32,
    pub entry_point: u32,
}

impl DolHeader {
    /// 18 offsets, 18 addresses, 18 sizes, then bss and the entry point
    pub fn read(fh: &mut impl Read) -> Result<DolHeader> {
        let mut raw = [0u8; 4 * (3 * SECTIONS + 3)];
        fh.read_exact(&mut raw)?;
        let word = |i: usize| {
            u32::from_be_bytes(raw[i * 4..i * 4 + 4].try_into().unwrap())
        };

        let mut header = DolHeader::default();
        for (i, s) in header.sections.iter_mut().enumerate() {
            s.file = word(i);
            s.ram = word(SECTIONS + i);
            s.size = word(2 * SECTIONS + i);
        }
        header.bss_address = word(3 * SECTIONS);
        header.bss_size = word(3 * SECTIONS + 1);
        header.entry_point = word(3 * SECTIONS + 2);
        Ok(header)
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        for s in &self.sections {
            w.write_all(&s.file.to_be_bytes())?;
        }
        for s in &self.sections {
            w.write_all(&s.ram.to_be_bytes())?;
        }
        for s in &self.sections {
            w.write_all(&s.size.to_be_bytes())?;
        }
        w.write_all(&self.bss_address.to_be_bytes())?;
        w.write_all(&self.bss_size.to_be_bytes())?;
        w.write_all(&self.entry_point.to_be_bytes())?;
        // pad to 0x100
        w.write_all(&[0u8; 28])?;
        Ok(())
    }

    /// Sections that hold bytes. An empty one has no file offset worth trusting.
    pub fn live(&self) -> impl Iterator<Item = Section> + '_ {
        self.sections.iter().copied().filter(|s| s.size != 0)
    }

    /// Where the data sections start, and so whether an offset is instructions.
    pub fn data_start(&self) -> u32 {
        self.sections[DATA0].file
    }

    pub fn file_offset(&self, ram: u32) -> Option<u32> {
        self.live().find_map(|s| {
            ram.checked_sub(s.ram)
                .and_then(|d| (d < s.size).then_some(s.file + d))
        })
    }

    pub fn ram_addr(&self, off: u32) -> Option<u32> {
        self.live().find_map(|s| {
            off.checked_sub(s.file)
                .and_then(|d| (d < s.size).then_some(s.ram + d))
        })
    }
}
