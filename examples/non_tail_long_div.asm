#[framesize(0xf)]
div:
    ;; Frame:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Arg a
    ;; Slot 3: Arg b
    ;; Slot 4: Return value q
    ;; Slot 5: Return value r
    ;; Slot 6: Non-deterministic local: Next FP
    ;; Slot 7: Local: a >> 1
    ;; Slot 8: Local: q1
    ;; Slot 9: Local: r1
    ;; Slot 10: Local: a & 1
    ;; Slot 11: Local: 2*r1
    ;; Slot 12: Local: (2*r1) + (a&1)
    ;; Slot 13: Local: (2*r1) + (a&1) < b
    ;; Slot 14: Local: 2*q1
    BNZ div_else1, @2
    LDI.W @4, #0
    LDI.W @5, #0
    RET
div_else1:
    SRAI @7, @2, #1
    MVV.W @6[2], @7
    MVV.W @6[3], @3
    MVV.W @6[4], @8
    MVV.W @6[5], @9
    CALLI div, @6
    ANDI @10, @2, #1
    SLLI @11, @9, #1
    ADD @12, @11, @10
    SLTU @13, @12, @3
    SLLI @14, @8, #1
    BNZ div_else2, @13
    ADDI @4, @14, #1
    SUB @5, @12, @3
    RET
div_else2:
    XORI @4, @14, #0
    XORI @5, @12, #0
    RET
