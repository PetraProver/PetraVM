;; slot layout mimics the original non-deterministic bezout example
#[framesize(0x3)]
entrypoint:
    ;; here we assume that entrypoint has dummy zero FP & PC frame
    ALLOCI! @2, #15
    MVI.H @2[2], #274
    MVI.H @2[3], #685
    MVI.H @2[4], #137
    MVI.H @2[5], #-2
    MVI.H @2[6], #1
    CALLI bezout, @2
    RET
bezout:
    BNZ bezout_else, @2
    XORI @4, @3, #0
    LDI.W @5, #0
    LDI.W @6, #1
    RET
bezout_else:
    ;; contrived, @13/@14 are not in VROM assignment
    LDI.W! @13, #7
    ADDI! @14, @13, #3
    ALLOCV! @7, @14
    MVV.W @7[2], @3
    MVV.W @7[3], @2
    MVV.W @7[4], @9
    MVV.W @7[5], @10
    CALLI div, @7
    ALLOCI! @8, #15
    MVV.W @8[2], @10
    MVV.W @8[3], @2
    MVV.W @8[4], @4
    MVV.W @8[5], @6
    MVV.W @8[6], @11
    CALLI bezout, @8
    MUL @12, @6, @9
    SUB @5, @11, @12
    RET
