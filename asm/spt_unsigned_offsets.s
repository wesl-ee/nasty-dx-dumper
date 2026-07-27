    .section .text
    .balign 4

    # These are three independent replacement instructions
    #
    # FUN_800D2CCC: base + zero_extend_16(offset)
    .global spt_direct_offset_unsigned
spt_direct_offset_unsigned:
    clrlwi  r0, r3, 16

    # FUN_800D2CE8: load a directory entry without sign extension.
    .global spt_directory_offset_unsigned
spt_directory_offset_unsigned:
    lhzx    r0, r4, r0

    # FUN_800D2D0C: load a table entry without sign extension.
    .global spt_table_offset_unsigned
spt_table_offset_unsigned:
    lhzx    r3, r3, r0
