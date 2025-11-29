// TextTableA
pub(crate) const TEXT_TABLES_A: [u32; 26] = [
    // navi / wallace dialog
    0x80260310, // some movement
    0x801d767c, // move / attack confirmations
    0x801d3bac, // TODO(wesl-ee) what is this
    0x801c9efc, // health status effects
    0x801cd0e8, // barometer status effects
    0x801ccefc, // darkling
    0x801cd198, // status effects
    0x801cd050, // ebola
    0x801cd1b8, // status
    0x801cd1e0, // turn menu
    0x801d3c0c, // data menu
    0x801d6228, // options menu
    0x801d3c6c, // more confirmations
    0x801d6048, // Field magic
    0x801d62f8, // Sponsors
    0x801ddb38, // TODO(wesl-ee) what is this
    0x801d6040, // counters
    0x801d627c, // EXP
    0x801d6d68,
    // discarded items upon death (おとす) (defender)
    0x801d723c, // dokapon / wallace eggs
    0x801d74f4, // discard items upon death (attacker)
    0x801dd4c4, // skill increase
    0x801dd51c, // stat check (attacker)
    0x801dd234, // alphabet
    0x8024a774, // battle result
    0x802742e0,
];

// TextTableC
pub(crate) const TEXT_TABLES_B: [u32; 2] = [
    // Spaces (maybe dupe of D 0x801c9ef0)
    0x801c9eec, // TODO(wesl-ee) what is this
    0x801ca37c,
];

// TextTableC
pub(crate) const TEXT_TABLES_C: [u32; 1] = [
    // items and descriptions
    0x801d0648,
];

// TextTableD
pub(crate) const TEXT_TABLES_D: [u32; 2] = [
    // D goods and descriptions
    0x801d11a0, // Spaces (maybe dupe of C 0x801c9ee)
    0x801c9ef0,
];

// TextTableE
pub(crate) const TEXT_TABLES_E: [u32; 1] = [
    // modes and some d parts stuff
    0x801d5ec8,
];

// TextTableF
pub(crate) const TEXT_TABLES_F: [u32; 5] = [
    // weapons
    0x80243f84, // TODO(wesl-ee) what is this
    0x80244e74, // lots of stuff
    0x802448fc, // Magic (but in the above one too)
    0x80244e74, // Defense magic
    0x8024507c,
];

// TextTableG
pub(crate) const TEXT_TABLES_G: [u32; 1] = [
    // defensive equipment and some stores?
    0x8024459c,
];

// TextTableH
pub(crate) const TEXT_TABLES_H: [u32; 1] = [
    // spinner values
    0x801d6b10,
];
