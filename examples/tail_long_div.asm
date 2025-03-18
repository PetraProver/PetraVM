#[framesize(0x8)]
div:
    ;; Frame:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Arg a
    ;; Slot 3: Arg b
    ;; Slot 4: Return value quot
    ;; Slot 5: Return value rem
    ;; Slot 6: Non-deterministic local: Next FP
    ;; Slot 7: 0x80000000
    MVV.W @6[2], @2
    MVV.W @6[3], @3
    LDI.W @7, #2147483648 ;; 0x80000000
    MVV.W @6[4], @7
    MVI.H @6[5], #0
    MVI.H @6[6], #0
    MVV.W @6[7], @4
    MVV.W @6[8], @5
    TAILI div_helper, @6

#[framesize(0x14)]
div_helper:
    ;; Frame:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Arg a
    ;; Slot 3: Arg b
    ;; Slot 4: Arg selector
    ;; Slot 5: Arg quot
    ;; Slot 6: Arg rem
    ;; Slot 7: Return value quot
    ;; Slot 8: Return value rem
    ;; Slot 9: Non-deterministic local: Next FP
    ;; Slot 10: a & selector
    ;; Slot 11: (a & selector) == 0
    ;; Slot 12: curr_bit
    ;; SLOT 13: rem << 1
    ;; SLOT 14: (rem << 1) + curr_bit
    ;; SLOT 15: b > rem
    ;; SLOT 16: 2*quot
    ;; SLOT 17: 2*quot + 1
    ;; SLOT 18: (rem << 1) + curr_bit - B
    ;; SLOT 19: selector >> 1
    BNZ div_helper_else1, @4
    XORI @7, @5, #0
    XORI @8, @6, #0
    RET
div_helper_else1:
    AND @10, @2, @4
    SLTIU @11, @10, #1
    XORI @12, @11, #1
    SLLI @13, @6, #1
    ADD @14, @12, @13
    SLTU @15, @14, @3
    SLLI @16, @5, #1
    BNZ div_helper_else2, @15
    ADDI @17, @16, #1
    SUB @18, @14, @3
    J div_helper_tail
div_helper_else2:
    XORI @17, @16, #0
    XORI @18, @14, #0
div_helper_tail:
    MVV.W @9[2], @2
    MVV.W @9[3], @3
    SRLI @19, @4, #1
    MVV.W @9[4], @19
    MVV.W @9[5], @17
    MVV.W @9[6], @18
    MVV.W @9[7], @7
    MVV.W @9[8], @8
    TAILI div_helper, @9
