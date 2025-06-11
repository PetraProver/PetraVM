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
    ;; Slot 7: Return quot value absolute addr
    ;; Slot 8: Return rem value absolute addr
    ;; Slot 9: 0x80000000
    ALLOCI! @6, #20
    MVV.W @6[2], @2
    MVV.W @6[3], @3
    LDI.W @9, #2147483648 ;; 0x80000000
    MVV.W @6[4], @9
    MVI.H @6[5], #0
    MVI.H @6[6], #0
    FP @7, #4
    FP @8, #5
    ;; We only need to provide the pointer to the quot slot. 
    ;; `div_helper` can then fill both return values.
    MVV.W @6[7], @7
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
    ;; Slot 7: Return values pointer
    ;; Slot 8: Non-deterministic local: Next FP
    ;; Slot 9: a & selector
    ;; Slot 10: (a & selector) == 0
    ;; Slot 11: curr_bit
    ;; Slot 12: rem << 1
    ;; Slot 13: (rem << 1) + curr_bit
    ;; Slot 14: b > rem
    ;; Slot 15: 2*quot
    ;; Slot 16: 2*quot + 1
    ;; Slot 17: (rem << 1) + curr_bit - B
    ;; Slot 18: selector >> 1
    BNZ div_helper_else1, @4
    ;; Set return quot value.
    MVV.W @7[0], @5
    ;; Set return rem value.
    MVV.W @7[1], @6
    RET
div_helper_else1:
    AND @9, @2, @4
    SLTIU @10, @9, #1
    XORI @11, @10, #1
    SLLI @12, @6, #1
    ADD @13, @11, @12
    SLTU @14, @13, @3
    SLLI @15, @5, #1
    BNZ div_helper_else2, @14
    ADDI @16, @15, #1
    SUB @17, @13, @3
    J div_helper_tail
div_helper_else2:
    XORI @16, @15, #0
    XORI @17, @13, #0
div_helper_tail:
    ALLOCI! @8, #20
    MVV.W @8[2], @2
    MVV.W @8[3], @3
    SRLI @18, @4, #1
    MVV.W @8[4], @18
    MVV.W @8[5], @16
    MVV.W @8[6], @17
    MVV.W @8[7], @7
    TAILI div_helper, @8
