/// A pointer table whose extent we do not know. The walk reads entries until
/// one stops looking like a table of text pointers.
pub(crate) struct WalkTable {
    /// RAM address of entry 0.
    pub base: u32,
    /// bytes per entry.
    pub stride: u32,
    /// byte offsets of the pointer words every entry must have. The first that
    /// does not resolve into a data section ends the walk.
    pub fields: &'static [u32],
    /// words that hold a pointer only in some entries; emitted when they do.
    pub optional: &'static [u32],
    /// the A tables sit directly against one another, so reaching another
    /// abutting table's base is the end of this one.
    pub abuts: bool,
    /// tag written into the .patch reference comments.
    pub tag: &'static str,
}

impl WalkTable {
    const fn new(
        tag: &'static str,
        stride: u32,
        fields: &'static [u32],
        base: u32,
    ) -> WalkTable {
        WalkTable {
            base,
            stride,
            fields,
            optional: &[],
            abuts: false,
            tag,
        }
    }

    const fn abutting(base: u32) -> WalkTable {
        WalkTable {
            abuts: true,
            ..WalkTable::new("A", 4, &[0], base)
        }
    }
}

pub(crate) const WALKING_TABLES: &[WalkTable] = &[
    WalkTable::abutting(0x80260310), // navi / wallace dialog
    WalkTable::abutting(0x801d767c), // some movement
    WalkTable::abutting(0x801d3bac), // move / attack confirmations
    WalkTable::abutting(0x801c9efc), // TODO(wesl-ee) what is this
    WalkTable::abutting(0x801cd0e8), // health status effects
    WalkTable::abutting(0x801ccefc), // barometer status effects
    WalkTable::abutting(0x801cd198), // darkling
    WalkTable::abutting(0x801cd050), // status effects
    WalkTable::abutting(0x801cd1b8), // ebola
    WalkTable::abutting(0x801cd1e0), // status
    WalkTable::abutting(0x801d3c0c), // turn menu
    WalkTable::abutting(0x801d6228), // data menu
    WalkTable::abutting(0x801d3c6c), // options menu
    WalkTable::abutting(0x801d6048), // more confirmations
    WalkTable::abutting(0x801d62f8), // Field magic
    WalkTable::abutting(0x801ddb38), // Sponsors
    WalkTable::abutting(0x801d6040), // TODO(wesl-ee) what is this
    WalkTable::abutting(0x801d627c), // counters
    WalkTable::abutting(0x801d6d68), // EXP
    // discarded items upon death (おとす) (defender)
    WalkTable::abutting(0x801d723c),
    WalkTable::abutting(0x801d74f4), // dokapon / wallace eggs
    WalkTable::abutting(0x801dd4c4), // discard items upon death (attacker)
    WalkTable::abutting(0x801dd51c), // skill increase
    WalkTable::abutting(0x801dd234), // stat check (attacker)
    WalkTable::abutting(0x8024a774), // alphabet
    WalkTable::abutting(0x802742e0), // battle result
    // Spaces (maybe dupe of D 0x801c9ef0)
    WalkTable::new("B", 16, &[0, 4, 8], 0x801c9eec),
    WalkTable::new("B", 16, &[0, 4, 8], 0x801ca37c), // TODO(wesl-ee) what is this
    WalkTable::new("C", 24, &[0, 4], 0x801d0648),    // items and descriptions
    WalkTable::new("D", 16, &[0, 4], 0x801d11a0),    // D goods and descriptions
    // Spaces (maybe dupe of C 0x801c9ee)
    WalkTable::new("D", 16, &[0, 4], 0x801c9ef0),
    // modes and some d parts stuff
    WalkTable::new("E", 12, &[0, 4], 0x801d5ec8),
    // F entries carry a second pointer only sometimes; nobody knows why
    WalkTable {
        optional: &[4],
        ..WalkTable::new("F", 20, &[0], 0x80243f84) // weapons
    },
    WalkTable {
        optional: &[4],
        ..WalkTable::new("F", 20, &[0], 0x80244e74) // TODO(wesl-ee) what is this
    },
    WalkTable {
        optional: &[4],
        ..WalkTable::new("F", 20, &[0], 0x802448fc) // lots of stuff
    },
    WalkTable {
        optional: &[4],
        // Magic (but in the above one too)
        ..WalkTable::new("F", 20, &[0], 0x80244e74)
    },
    WalkTable {
        optional: &[4],
        ..WalkTable::new("F", 20, &[0], 0x8024507c) // Defense magic
    },
    // defensive equipment and some stores?
    WalkTable::new("G", 16, &[0], 0x8024459c),
    WalkTable::new("H", 12, &[0], 0x801d6b10), // spinner values
];

/// A text table whose extent we know exactly, rather than one we walk until it
/// stops looking like pointers. Needed because several DOL tables have NULL
/// holes in the middle (the walking tables stop dead at the first one) or sit
/// immediately in front of non-pointer data that a walk would run into.
pub(crate) struct TextTable {
    /// RAM address of entry 0.
    pub base: u32,
    /// bytes per entry.
    pub stride: u32,
    /// number of entries. derived from the code that indexes the table; see
    /// `evidence`.
    pub count: u32,
    /// byte offsets within an entry that hold a text pointer.
    pub fields: &'static [u32],
    /// hard glyph limit on this string, where the machine code enforces one.
    pub cap: Option<u32>,
    /// tag written into the .patch reference comments.
    pub tag: &'static str,
}

/// Tables the walking extractors above cannot reach. Every `count` here comes
/// from the instruction that indexes the table or from the sentinel its reader
/// tests for; every entry has been decoded out of `main.dol` and terminates
/// inside its section.
pub(crate) const TEXT_TABLES_EXPLICIT: &[TextTable] = &[
    // status / event message pointers. one table, not the six the walking A
    // list splits it into: 20 of the 189 slots are NULL and a walk stops at the
    // first one. ends at 0x801cd1ec; 0x801cd1f0 is glyph data.
    // indexed by FUN_8002735C (800273b4), FUN_8002E7C8, FUN_8002F5BC,
    // FUN_800F22F0, FUN_8002F2B4 — all `lwzx` off the 0x801ccefc base.
    TextTable {
        base: 0x801ccefc,
        stride: 4,
        count: 189,
        fields: &[0],
        cap: None,
        tag: "event",
    },
    // weekday names. FUN_8003F524: 8003f5c4 `lbz r0,0x9(r7)` (day counter) /
    // 8003f5d0 `addi r7,r7,0x3c50` / 8003f5d4 `lwzx r7,r7,r0`. slot 7 is
    // 0x801d3c6c, the options-menu table already in TEXT_TABLES_A.
    TextTable {
        base: 0x801d3c50,
        stride: 4,
        count: 7,
        fields: &[0],
        cap: None,
        tag: "weekday",
    },
    // move / attack confirmations, and the death messages after them. same
    // table as the TEXT_TABLES_A entry, but that walk stops at the NULL in slot
    // 13 and never reaches slots 14-20. FUN_8002E684 (8002e6c4, 8002e6f4),
    // FUN_8002F3F0, FUN_8002E738 all `lwzx` off 0x801d3bac. 0x801d3c00 is glyph
    // data.
    TextTable {
        base: 0x801d3bac,
        stride: 4,
        count: 21,
        fields: &[0],
        cap: None,
        tag: "confirm",
    },
    // "the effect wore off" messages. FUN_8004CDA4: 8004cf5c
    // `addi r6,r6,0x641c` / 8004cf60 `lwzx`. 0x801d6418 is a code pointer and
    // 0x801d6428 is not a pointer.
    TextTable {
        base: 0x801d641c,
        stride: 4,
        count: 3,
        fields: &[0],
        cap: None,
        tag: "expired",
    },
    // data-menu page titles. FUN_800449A8 (80044a44/80044a48) and FUN_80045150
    // (80045320/8004534c), both `lha r,0x94(obj)` scaled by 4. stops at
    // 0x801d60ec, where the words stop being text pointers and become pointers
    // to the pointer lists at 0x802cd164.
    TextTable {
        base: 0x801d60a4,
        stride: 4,
        count: 18,
        fields: &[0],
        cap: None,
        tag: "datamenu",
    },
    // sponsor slogans. FUN_80078520: 800785d0 `subi r6,r5,0x2cb4` = 0x801dd34c,
    // 800785e8 `lwz r5,-0x4(r5)` so the index is 1-based off 0x801dd348.
    // 0x801dd364 is not a pointer.
    TextTable {
        base: 0x801dd34c,
        stride: 4,
        count: 6,
        fields: &[0],
        cap: None,
        tag: "sponsor",
    },
    // monster / character definition names. FUN_800A70F8 renders it as
    // `&PTR_DAT_80241d8c + idx * 0xf` (0xf words = 0x3c bytes); also indexed by
    // FUN_80063C98 (80063d18), FUN_800635D0, FUN_800C18BC (800c1ad8).
    // 142 entries: 0x80243e98 is the last whose field 0 is a text pointer.
    // 8-glyph cap: FUN_80063C98 copies field 0 with `li r5,0x12` and stores a
    // halfword at +0x12 of the destination.
    TextTable {
        base: 0x80241d8c,
        stride: 0x3c,
        count: 142,
        fields: &[0],
        cap: Some(8),
        tag: "monster",
    },
    // weapon / shield special-effect descriptions. FUN_8012432C indexes it with
    // the u8 at +9 of the 0x14-byte weapon record (801245d0) and of the 0x10-byte
    // shield record (801246d0). those two byte fields together take exactly the
    // values 0..30 across all 79 weapons and 55 shields, and 0x80245260 is not a
    // pointer.
    TextTable {
        base: 0x802451e4,
        stride: 4,
        count: 31,
        fields: &[0],
        cap: None,
        tag: "effect",
    },
    // scenario goals. FUN_80091CD8 loads exactly 0x0, 0x4, 0x8, 0xc and 0x10 off
    // this base (80091d2c, 80091d50, 80091e08, 80091e48/80091e70, 80091e2c).
    TextTable {
        base: 0x802456cc,
        stride: 4,
        count: 5,
        fields: &[0],
        cap: None,
        tag: "goal",
    },
    // per-character "in trouble" lines. FUN_80094000: `lhz r0,0x1e(r3)` masked
    // to a byte character id, scaled by 4, `lwzx` off 0x8024583c. 7 playable
    // characters.
    TextTable {
        base: 0x8024583c,
        stride: 4,
        count: 7,
        fields: &[0],
        cap: None,
        tag: "panic",
    },
    // status-effect apply / cure message pairs. FUN_800BEC34 walks it with
    // `addi r31,r31,0xc` and stops on `lha r4,0x8(r31)` == -1 (800beca4-800becac);
    // that sentinel record is at 0x8024aad4, so 53 records precede it. fields 0
    // and 4 are the two messages, field 8 is the effect id.
    TextTable {
        base: 0x8024a858,
        stride: 0xc,
        count: 53,
        fields: &[0, 4],
        cap: None,
        tag: "status",
    },
    // boss / NPC names, indexed by character id. FUN_800F64A8:
    // `lbz r0,0x1d(r6)` / 800f6518 `addi r6,r6,0x2e0` / `lwzx`. also
    // FUN_80091CD8 at 80091e5c. slot 11 (0x8026030c) is NULL and slot 12 is
    // the start of the navi/wallace table already in TEXT_TABLES_A.
    TextTable {
        base: 0x802602e0,
        stride: 4,
        count: 11,
        fields: &[0],
        cap: None,
        tag: "charname",
    },
    // per-character battle / status dialogue. FUN_80110EA0 returns
    // `(&PTR_DAT_802719e0)[line + charId * 0x33]` (801110c0 / 8011104c), so the
    // table is 51 lines per character. 357 = 7 characters * 51, and 0x80271f74
    // is the first word that is not a text pointer.
    TextTable {
        base: 0x802719e0,
        stride: 4,
        count: 357,
        fields: &[0],
        cap: None,
        tag: "dialogue",
    },
    // disc / drive error messages. FUN_8011E1C0: 8011e2e0 `addi r5,r5,0x4188` /
    // 8011e2e4 `lwzx`. 0x8027419c is a code pointer.
    TextTable {
        base: 0x80274188,
        stride: 4,
        count: 5,
        fields: &[0],
        cap: None,
        tag: "discerr",
    },
    // the label pool the data-menu sub-lists point into. reached through the
    // pointer-of-pointer-lists table at 0x801d60ec; 0x802cd184 is not a pointer.
    TextTable {
        base: 0x802cd164,
        stride: 4,
        count: 8,
        fields: &[0],
        cap: None,
        tag: "datamenu",
    },
];

/// Text in `main.dol` that we deliberately do not emit, and why. Written into
/// the top of `main.dol.patch` so the omission is visible to whoever is
/// translating rather than being silently absent.
///
/// The instruction scan also refuses to treat any of these addresses as a
/// string, which stops one of them being dragged in as a `lis`/`addi` target.
pub(crate) const EXCLUDED_NOTES: &[(u32, &[&str])] = &[(
    0x8027_6370,
    &[
        "not text: 12-word OS interrupt-mask table used by the loop at \
             0x80145470",
        "its third word begins with 0xF800, which fooled the terminated \
             glyph checking",
    ],
)];

/// A run of fixed-width text records stored inline: no pointer word anywhere,
/// and no terminator. `dump-all` emits them, but `patch-all` leaves them alone:
/// translating one means overwriting the record where it lies and padding back
/// out to `glyphs`, and no build has playtested that.
pub(crate) struct InlineField {
    /// RAM address of record 0.
    pub base: u32,
    /// bytes per record.
    pub stride: u32,
    /// number of records.
    pub count: u32,
    /// glyphs per record. also the hard cap on a translation, because the
    /// record's width is what this reader assumes
    pub glyphs: u32,
    /// tag written into the .patch comments.
    pub tag: &'static str,
}

impl InlineField {
    /// Record index, if `ram` is exactly the start of one. Interior addresses
    /// deliberately do not match: half a record is not a translatable unit.
    pub fn record(&self, ram: u32) -> Option<u32> {
        let delta = ram.checked_sub(self.base)?;
        let index = delta / self.stride;
        (delta % self.stride == 0 && index < self.count).then_some(index)
    }

    pub fn contains(&self, ram: u32) -> bool {
        ram >= self.base && ram - self.base < self.count * self.stride
    }
}

/// The instruction scan must not treat any address inside one of these as a
/// string: the table's only 0xF800 sits past the last record, so a terminator
/// walk from anywhere inside reads every remaining record as one long run.
pub(crate) const INLINE_FIELDS: &[InlineField] = &[
    // preset CPU opponent names, all evidence from FUN_8007DFD4:
    //   8007e11c  addi r29,r3,0x7bd4     base 0x80247bd4
    //   8007e120  li   r3,0x64           100 records
    //   8007e128  rlwinm r0,r3,0x4,...   index << 4, stride 0x10
    //   8007e158  cmpwi r0,0x8           exactly 8 words compared
    // all eight words are compared against the player's name and a full match
    // re-rolls, so the record width is 8 glyphs whatever the padding says. the
    // 0xF800 at 0x80248214 follows record 99 rather than belonging to it.
    InlineField {
        base: 0x80247bd4,
        stride: 0x10,
        count: 100,
        glyphs: 8,
        tag: "cpuname",
    },
];
