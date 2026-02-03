use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct DolHeader {
    pub text0_offset: u32,
    pub text1_offset: u32,
    pub text2_offset: u32,
    pub text3_offset: u32,
    pub text4_offset: u32,
    pub text5_offset: u32,
    pub text6_offset: u32,
    pub data0_offset: u32,
    pub data1_offset: u32,
    pub data2_offset: u32,
    pub data3_offset: u32,
    pub data4_offset: u32,
    pub data5_offset: u32,
    pub data6_offset: u32,
    pub data7_offset: u32,
    pub data8_offset: u32,
    pub data9_offset: u32,
    pub data10_offset: u32,

    pub text0_address: u32,
    pub text1_address: u32,
    pub text2_address: u32,
    pub text3_address: u32,
    pub text4_address: u32,
    pub text5_address: u32,
    pub text6_address: u32,
    pub data0_address: u32,
    pub data1_address: u32,
    pub data2_address: u32,
    pub data3_address: u32,
    pub data4_address: u32,
    pub data5_address: u32,
    pub data6_address: u32,
    pub data7_address: u32,
    pub data8_address: u32,
    pub data9_address: u32,
    pub data10_address: u32,

    pub text0_size: u32,
    pub text1_size: u32,
    pub text2_size: u32,
    pub text3_size: u32,
    pub text4_size: u32,
    pub text5_size: u32,
    pub text6_size: u32,
    pub data0_size: u32,
    pub data1_size: u32,
    pub data2_size: u32,
    pub data3_size: u32,
    pub data4_size: u32,
    pub data5_size: u32,
    pub data6_size: u32,
    pub data7_size: u32,
    pub data8_size: u32,
    pub data9_size: u32,
    pub data10_size: u32,
    // there's other stuff in a dol header but not relevant for text dumping
}

#[derive(Clone)]
pub struct DolSection {
    pub file: u32,
    pub ram: u32,
    pub size: u32,
}

impl DolHeader {
    pub fn section_iter(&self) -> impl Iterator<Item = DolSection> + '_ {
        [
            // text
            (self.text0_offset, self.text0_address, self.text0_size),
            (self.text1_offset, self.text1_address, self.text1_size),
            (self.text2_offset, self.text2_address, self.text2_size),
            (self.text3_offset, self.text3_address, self.text3_size),
            (self.text4_offset, self.text4_address, self.text4_size),
            (self.text5_offset, self.text5_address, self.text5_size),
            (self.text6_offset, self.text6_address, self.text6_size),
            // data
            (self.data0_offset, self.data0_address, self.data0_size),
            (self.data1_offset, self.data1_address, self.data1_size),
            (self.data2_offset, self.data2_address, self.data2_size),
            (self.data3_offset, self.data3_address, self.data3_size),
            (self.data4_offset, self.data4_address, self.data4_size),
            (self.data5_offset, self.data5_address, self.data5_size),
            (self.data6_offset, self.data6_address, self.data6_size),
            (self.data7_offset, self.data7_address, self.data7_size),
            (self.data8_offset, self.data8_address, self.data8_size),
            (self.data9_offset, self.data9_address, self.data9_size),
            (self.data10_offset, self.data10_address, self.data10_size),
        ]
        .into_iter()
        .filter(|(_, _, size)| *size != 0)
        .map(|(file, ram, size)| DolSection { file, ram, size })
    }
}

pub(crate) trait TextTableEntry: Sized {
    const SIZE: usize;

    fn from_bytes(bytes: &[u8]) -> Option<Self>;
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TextTableEntryA {
    pub ptr: u32,
}

impl TextTableEntry for TextTableEntryA {
    const SIZE: usize = 4;

    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }

        let ptr = u32::from_be_bytes(bytes.try_into().ok()?);
        Some(TextTableEntryA { ptr })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TextTableEntryB {
    pub text_1: u32,
    pub text_2: u32,
    pub text_3: u32,
    pub idk: u32,
}

impl TextTableEntry for TextTableEntryB {
    const SIZE: usize = 16;

    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 16 {
            return None;
        }

        let text_1 = u32::from_be_bytes(bytes[0..4].try_into().ok()?);
        let text_2 = u32::from_be_bytes(bytes[4..8].try_into().ok()?);
        let text_3 = u32::from_be_bytes(bytes[8..12].try_into().ok()?);
        let idk = u32::from_be_bytes(bytes[12..16].try_into().ok()?);
        Some(TextTableEntryB {
            text_1,
            text_2,
            text_3,
            idk,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TextTableEntryC {
    pub text_1: u32,
    pub text_2: u32,
    pub idk: u128,
}

impl TextTableEntry for TextTableEntryC {
    const SIZE: usize = 24;

    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 24 {
            return None;
        }

        let text_1 = u32::from_be_bytes(bytes[0..4].try_into().ok()?);
        let text_2 = u32::from_be_bytes(bytes[4..8].try_into().ok()?);
        let idk = u128::from_be_bytes(bytes[8..24].try_into().ok()?);
        Some(TextTableEntryC {
            text_1,
            text_2,
            idk,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TextTableEntryD {
    pub text_1: u32,
    pub text_2: u32,
    pub idk: u64,
}

impl TextTableEntry for TextTableEntryD {
    const SIZE: usize = 16;

    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 16 {
            return None;
        }

        let text_1 = u32::from_be_bytes(bytes[0..4].try_into().ok()?);
        let text_2 = u32::from_be_bytes(bytes[4..8].try_into().ok()?);
        let idk = u64::from_be_bytes(bytes[8..16].try_into().ok()?);
        Some(TextTableEntryD {
            text_1,
            text_2,
            idk,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TextTableEntryE {
    pub text_1: u32,
    pub text_2: u32,
    pub idk: u32,
}

impl TextTableEntry for TextTableEntryE {
    const SIZE: usize = 12;

    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 12 {
            return None;
        }

        let text_1 = u32::from_be_bytes(bytes[0..4].try_into().ok()?);
        let text_2 = u32::from_be_bytes(bytes[4..8].try_into().ok()?);
        let idk = u32::from_be_bytes(bytes[8..12].try_into().ok()?);
        Some(TextTableEntryE {
            text_1,
            text_2,
            idk,
        })
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct TextTableEntryF {
    pub text_1: u32,
    pub maybe_text_2: u32,
    pub idk_1: u32,
    pub idk_2: u32,
    pub idk_3: u32,
}

impl TextTableEntry for TextTableEntryF {
    const SIZE: usize = 20;

    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 20 {
            return None;
        }

        let text_1 = u32::from_be_bytes(bytes[0..4].try_into().ok()?);
        let maybe_text_2 = u32::from_be_bytes(bytes[4..8].try_into().ok()?);
        Some(TextTableEntryF {
            text_1,
            maybe_text_2,
            ..Default::default()
        })
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct TextTableEntryG {
    pub text_1: u32,
    pub idk_1: u32,
    pub idk_2: u32,
    pub idk_3: u32,
}

impl TextTableEntry for TextTableEntryG {
    const SIZE: usize = 16;

    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 16 {
            return None;
        }

        let text_1 = u32::from_be_bytes(bytes[0..4].try_into().ok()?);
        Some(TextTableEntryG {
            text_1,
            ..Default::default()
        })
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct TextTableEntryH {
    pub text_1: u32,
    pub idk_1: u32,
    pub idk_2: u32,
}

impl TextTableEntry for TextTableEntryH {
    const SIZE: usize = 12;

    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 12 {
            return None;
        }

        let text_1 = u32::from_be_bytes(bytes[0..4].try_into().ok()?);
        Some(TextTableEntryH {
            text_1,
            ..Default::default()
        })
    }
}
