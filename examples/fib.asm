fib:
    ;; Slot 0: Return PC
    ;; Slot 4: Return FP
    ;; Slot 8: Arg: n
    ;; Slot 12: Return value
    ;; Slot 16: ND Local: Next FP

    MVI.H @16[8], #0      ;; Move 0 into a argument
    MVI.H @16[12], #1     ;; Move 1 into b argument
    MVV.W @16[16], @8     ;; Move n into n argument
    MVV.W @16[20], @12    ;; Move return value
    TAILI fib_helper, @16 ;; Tail call to fib_helper (Slot 16 is the next FP)

fib_helper:
    ;; Slot @0: Return PC
    ;; Slot @4: Return FP
    ;; Slot @8: Arg: a
    ;; Slot @12: Arg: b
    ;; Slot @16: Arg: n
    ;; Slot @20: Return value
    ;; Slot @24: ND Local: Next FP
    ;; Slot @28: Local: a + b
    ;; Slot @32: Local: n - 1
    ;; Slot @36: Local: n == 0G
    ;; Slot @40: Local: 0G constant

    ;; Branch to recursion label if value in slot 16 is not equal to G^0
    LDI.W @40, #0G
    XOR @36, @16, @40     ;; XOR will put 0 in slot 36 if n == 0G
    BNZ case_recurse, @36 ;; branch if n != 0G

    ;; Constraint return value equals a
    ;; Idea: assembly CPY is alias for XORI with 0 immediate
    XORI @20, @8, #0
    RET
case_recurse:
    ADD @28, @8, @12
    B32_MULI @32, @16, #-1G ;; TODO: B32_MULI is deprecated and will be removed

    MVV.W @24[8], @12       ;; Move b into a argument
    MVV.W @24[12], @28      ;; Move a + b into b argument
    MVV.W @24[16], @32      ;; Move n - 1 into n argument
    MVV.W @24[20], @20      ;; Move return value
    TAILI fib_helper, @24
