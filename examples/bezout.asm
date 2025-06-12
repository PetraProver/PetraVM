;; slot layout mimics the original non-deterministic bezout example
;; NOTE: `div` is defined in `div.asm` and is included when `bezout.asm` is run.
#[framesize(0x10)]
bezout:
    ;; Frame:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Arg a
    ;; Slot 3: Arg b
    ;; Slot 4: Return value gcd
    ;; Slot 5: Return value a's coefficient
    ;; Slot 6: Return value b's coefficient
    ;; Slot 7: Non-deterministic local: Next FP
    ;; Slot 8: Non-deterministic local: Next FP
    ;; Slot 9: Local: c
    ;; Slot 10: Local: d
    ;; Slot 11: Local: g
    ;; Slot 12: Local: f*c
    BNZ bezout_else, @2
    XORI @4, @3, #0
    LDI.W @5, #0
    LDI.W @6, #1
    RET
bezout_else:
    ;; contrived, @14/@15 are not in VROM assignment
    LDI.W! @14, #7
    ADDI! @15, @14, #3
    ALLOCV! @7, @15
    MVV.W @7[2], @3
    MVV.W @7[3], @2
    CALLI div, @7
    ;; Set return values
    MVV.W @7[5], @10
    MVV.W @7[4], @9
    ALLOCI! @8, #15
    MVV.W @8[2], @10
    MVV.W @8[3], @2
    CALLI bezout, @8
    ;; Set return values
    MVV.W @8[4], @4
    MVV.W @8[5], @6
    MVV.W @8[6], @11
    MUL @12, @6, @9
    SUB @5, @11, @12
    RET
