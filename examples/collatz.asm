#[framesize(0xa)]
collatz:
    ;; Frame:
    ;; Slot @0: Return PC
    ;; Slot @1: Return FP
    ;; Slot @2: Arg: n
    ;; Slot @3: Return value
    ;; Slot @4: ND Local: Next FP
    ;; Slot @5: Local: n == 1
    ;; Slot @6: Local: n % 2
    ;; Slot @7: Local: n >> 1 or 3*n + 1
    ;; Slot @8: Local: 3*n (lower bits)
    ;; Slot @9: Local: 3*n (higher bits, unused)

    ;; Branch to recursion label if value in slot 2 is not 1
    XORI @5, @2, #1
    BNZ case_recurse, @5 ;; branch if n != 1
    XORI @3, @2, #0
    RET

case_recurse:
    ANDI @6, @2, #1  ;; n % 2 is & 0x00..01
    BNZ case_odd, @6 ;; branch if n % 2 == 1u32

    ;; case even
    ;; n >> 1
    SRLI @7, @2, #1
    MVV.W @4[2], @7
    MVV.W @4[3], @3
    TAILI collatz, @4

case_odd:
    MULI @8, @2, #3
    ADDI @7, @8, #1
    MVV.W @4[2], @7
    MVV.W @4[3], @3
    TAILI collatz, @4
