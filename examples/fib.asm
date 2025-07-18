#[framesize(0x6)]
fib:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Arg: n
    ;; Slot 3: Return value absolute address
    ;; Slot 4: Return value
    ;; Slot 5: ND Local: Next FP

    ALLOCI! @5, #11
    ;; Construct the absolute address of the return value.
    FP @3, #4
    
    ;; Populate the next frame.
    MVI.H @5[2], #0      ;; Move 0 into a argument
    MVI.H @5[3], #1      ;; Move 1 into b argument
    MVV.W @5[4], @2      ;; Move n into n argument
    MVV.W @5[5], @3      ;; Move return value's absolute address
    TAILI fib_helper, @5 ;; Tail call to fib_helper (Slot 4 is the next FP)

#[framesize(0xb)]
fib_helper:
    ;; Slot @0: Return PC
    ;; Slot @1: Return FP
    ;; Slot @2: Arg: a
    ;; Slot @3: Arg: b
    ;; Slot @4: Arg: n
    ;; Slot @5: Return value absolute address
    ;; Slot @6: ND Local: Next FP
    ;; Slot @7: Local: a + b
    ;; Slot @8: Local: n - 1
    ;; Slot @9: Local: n == 0G
    ;; Slot @10: Local: 0G constant

    ;; Branch to recursion label if value in slot 4 is not equal to G^0
    LDI.W @10, #0G
    XOR @9, @4, @10     ;; XOR will put 0 in slot 9 if n == 0G
    BNZ case_recurse, @9 ;; branch if n != 0G

    ;; Constraint return value equals a
    MVV.W @5[0], @2
    RET
case_recurse:
    ADD @7, @2, @3
    B32_MULI @8, @4, #-1G

    ALLOCI! @6, #11
    MVV.W @6[2], @3       ;; Move b into a argument
    MVV.W @6[3], @7       ;; Move a + b into b argument
    MVV.W @6[4], @8       ;; Move n - 1 into n argument
    MVV.W @6[5], @5       ;; Move return value absolute address
    TAILI fib_helper, @6
