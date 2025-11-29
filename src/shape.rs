use serde::{Deserialize, Serialize};

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
