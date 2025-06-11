#[framesize(0x6)]
collatz_main:
    ;; Frame:
    ;; Slot @0: Return PC
    ;; Slot @1: Return FP
    ;; Slot @2: Arg: n
    ;; Slot @3: Return value absolute address
    ;; Slot @4: Return value
    ;; Slot @5: ND Local: Next FP

    ;; Get the absolute address of the return value: 
    ;; current_fp XOR 4
    FP @3, #4
    ALLOCI! @5, #10
    MVV.W @5[2], @2 ;; Move n
    MVV.W @5[3], @3 ;; Move the absolute address of the return value
    TAILI collatz, @5

#[framesize(0xa)]
collatz:
    ;; Frame:
    ;; Slot @0: Return PC
    ;; Slot @1: Return FP
    ;; Slot @2: Arg: n
    ;; Slot @3: Return value absolute address
    ;; Slot @4: ND Local: Next FP
    ;; Slot @5: Local: n == 1
    ;; Slot @6: Local: n % 2
    ;; Slot @7: Local: n >> 1 or 3*n + 1
    ;; Slot @8: Local: 3*n (lower bits)
    ;; Slot @9: Local: 3*n (higher bits, unused)

    ;; Branch to recursion label if value in slot 2 is not 1
    XORI @5, @2, #1
    BNZ case_recurse, @5 ;; branch if n != 1
    MVV.W @3[0], @2
    RET

case_recurse:
    ANDI @6, @2, #1  ;; n % 2 is & 0x00..01
    ;; In both branching cases, we make a call to a new frame.
    ALLOCI! @4, #10
    BNZ case_odd, @6 ;; branch if n % 2 == 1u32

    ;; case even
    ;; n >> 1
    SRLI @7, @2, #1
    MVV.W @4[2], @7
    MVV.W @4[3], @3 ;; Copy the absolute address of the return value
    TAILI collatz, @4

case_odd:
    MULI @8, @2, #3
    ADDI @7, @8, #1
    MVV.W @4[2], @7
    MVV.W @4[3], @3
    TAILI collatz, @4
