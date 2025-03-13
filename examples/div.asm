div:
    ;; Frame:
    ;; Slot 0: Return PC
    ;; Slot 4: Return FP
    ;; Slot 8: Arg a
    ;; Slot 12: Arg b
    ;; Slot 16: Return value q
    ;; Slot 20: Return value r
    ;; Slot 24: Non-deterministic local: Next FP
    ;; Slot 28: Local: a < b
    ;; Slot 32: Local: a-b
    ;; Slot 36: Local: q1
    SLTU @28, @8, @12
    BNZ div_consequent, @28
    SUB @32, @8, @12
    MVV.W @24[8], @32
    MVV.W @24[12], @12
    MVV.W @24[16], @36
    MVV.W @24[20], @20
    CALLI div, @24
    ADDI @16, @36, #1
    RET
div_consequent:
    LDI.W @16, #0
    XORI @20, @8, #0
    RET