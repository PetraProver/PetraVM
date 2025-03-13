bezout:
    ;; Frame:
    ;; Slot 0: Return PC
    ;; Slot 4: Return FP
    ;; Slot 8: Arg a
    ;; Slot 12: Arg b
    ;; Slot 16: Return value gcd
    ;; Slot 20: Return value a's coefficient
    ;; Slot 24: Return value b's coefficient
    ;; Slot 28: Non-deterministic local: Next FP
    ;; Slot 32: Non-deterministic local: Next FP
    ;; Slot 36: Local: c
    ;; Slot 40: Local: d
    ;; Slot 44: Local: g
    ;; Slot 48: Local: f*c
    BNZ bezout_else, @8
    XORI @16, @12, #0
    LDI.W @20, #0
    LDI.W @24, #1
    RET
bezout_else:
    MVV.W @28[8], @12
    MVV.W @28[12], @8
    MVV.W @28[16], @36
    MVV.W @28[20], @40
    CALLI div, @28
    MVV.W @32[8], @40
    MVV.W @32[12], @8
    MVV.W @32[16], @16
    MVV.W @32[20], @24
    MVV.W @32[24], @44
    CALLI bezout, @32
    MUL @48, @24, @36
    SUB @20, @44, @48
    RET