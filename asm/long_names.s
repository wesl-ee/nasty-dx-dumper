    .section .text
    .balign 4

    # One redirect trampoline per text walker.
    #
    # Every walker's loop head is the same instruction, `lhz CODE,0(CURSOR)`,
    # and only the register assignment differs. The hook replaces that word
    # with a branch to here; the cave repeats the load, follows a 0xC000-family
    # code through longNamePtrs, and re-enters the walker one instruction past
    # the hook with CODE already loaded.
    #
    # CODE doubles as the pointer-table base register, which is what keeps the
    # register budget at one: the redirect path always branches back to the top
    # and reloads it. So only SCRATCH has to be dead at the loop head, and cr0
    # is recomputed by the walker's own terminator compare on the way out.
    #
    # A redirect never touches the reveal counter, the glyph counter or the
    # cursor arithmetic, because it never leaves the loop head. That is the
    # reason for hooking here rather than in the unknown-code arm.
    #
    # Rust rewrites words 5 and 6 (longNamePtrs, ppc::rewrite / ImmKind::Addi)
    # and word 9 (the resume branch, ppc::branch).
    .macro redirect name, code, cursor, scratch
    .global \name
    # the loop-back branch has to name a local label: gas leaves a branch to a
    # global symbol as a relocation, and nothing links this blob.
\name:
0:  lhz     \code, 0(\cursor)
    rlwinm  \scratch, \code, 0, 16, 20
    cmplwi  \scratch, 0xC000
    bne     9f
    rlwinm  \scratch, \code, 2, 19, 29      # (code & 0x7FF) * 4
    lis     \code, 0                        # -> longNamePtrs@ha
    addi    \code, \code, 0                 # -> longNamePtrs@l
    lwzx    \cursor, \code, \scratch
    b       0b
9:  b       .                               # -> the hooked loop head + 4
    .endm

    # showTextAsBoxWorker, lbl_800158CC. r30 = code, r29 = cursor.
    redirect long_name_show_box,       r30, r29, r0

    # measureTextWidth, FUN_8001705C loop head 0x800170B0.
    redirect long_name_measure_width,  r7,  r29, r0

    # measureTextExtent, FUN_80017500 loop head 0x8001775C.
    redirect long_name_measure_extent, r7,  r27, r0

    # showTextMeasurePass, FUN_80018AA0 loop head 0x80018B3C.
    redirect long_name_measure_pass,   r26, r31, r0

    # expandTextInline, FUN_800DD86C outer loop head 0x800DDB4C.
    redirect long_name_expand_outer,   r5,  r31, r0

    # expandTextInline's inner copy loop, 0x800DDB0C, which walks an inserted
    # string. r0 is live here -- it carries the copy bound compared at
    # 0x800DDB04 -- so this is the one cave that needs a different scratch.
    redirect long_name_expand_inner,   r3,  r4,  r6
