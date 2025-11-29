// TextTableA
pub(crate) const TEXT_TABLES_A: [u32; 26] = [
    0x80260310, // navi / wallace dialog
    0x801d767c, // some movement
    0x801d3bac, // move / attack confirmations
    0x801c9efc, // TODO(wesl-ee) what is this
    0x801cd0e8, // health status effects
    0x801ccefc, // barometer status effects
    0x801cd198, // darkling
    0x801cd050, // status effects
    0x801cd1b8, // ebola
    0x801cd1e0, // status
    0x801d3c0c, // turn menu
    0x801d6228, // data menu
    0x801d3c6c, // options menu
    0x801d6048, // more confirmations
    0x801d62f8, // Field magic
    0x801ddb38, // Sponsors
    0x801d6040, // TODO(wesl-ee) what is this
    0x801d627c, // counters
    0x801d6d68, // EXP
    0x801d723c, // discarded items upon death (おとす) (defender)
    0x801d74f4, // dokapon / wallace eggs
    0x801dd4c4, // discard items upon death (attacker)
    0x801dd51c, // skill increase
    0x801dd234, // stat check (attacker)
    0x8024a774, // alphabet
    0x802742e0, // battle result
];

// TextTableC
pub(crate) const TEXT_TABLES_B: [u32; 2] = [
    0x801c9eec, // Spaces (maybe dupe of D 0x801c9ef0)
    0x801ca37c, // TODO(wesl-ee) what is this
];

// TextTableC
pub(crate) const TEXT_TABLES_C: [u32; 1] = [
    0x801d0648, // items and descriptions
];

// TextTableD
pub(crate) const TEXT_TABLES_D: [u32; 2] = [
    0x801d11a0, // D goods and descriptions
    0x801c9ef0, // Spaces (maybe dupe of C 0x801c9ee)
];

// TextTableE
pub(crate) const TEXT_TABLES_E: [u32; 1] = [
    0x801d5ec8, // modes and some d parts stuff
];

// TextTableF
pub(crate) const TEXT_TABLES_F: [u32; 5] = [
    0x80243f84, // weapons
    0x80244e74, // TODO(wesl-ee) what is this
    0x802448fc, // lots of stuff
    0x80244e74, // Magic (but in the above one too)
    0x8024507c, // Defense magic
];

// TextTableG
pub(crate) const TEXT_TABLES_G: [u32; 1] = [
    0x8024459c, // defensive equipment and some stores?
];

// TextTableH
pub(crate) const TEXT_TABLES_H: [u32; 1] = [
    0x801d6b10, // spinner values
];
