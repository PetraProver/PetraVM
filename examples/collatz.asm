collatz:
    ;; Frame:
	;; Slot @0: Return PC
	;; Slot @1: Return FP
	;; Slot @2: Arg: n
    ;; Slot @3: Return value
	;; Slot @4: Local: n == 1
	;; Slot @5: Local: n % 2
	;; Slot @6: Local: 3*n
    ;; Slot @7: Local: n >> 2 or 3*n + 1
	;; Slot @8: ND Local: Next FP

	;; Branch to recursion label if value in slot 2 is not 1
	XORI @4, @2, #1G
	BNZ case_recurse, @4 ;; branch if n == 1
	XORI @3, @2, #0G
	RET

case_recurse:
	ANDI @5, @2, #1 ;; n % 2 is & 0x00..01
    BNZ case_odd, @5 ;; branch if n % 2 == 0u32

	;; case even
    ;; n >> 1
	SRLI @7, @2, #1
    MVV.W @8[2], @7
    MVV.W @8[3], @3
    TAILI collatz, @8

case_odd:
	MULI @6, @2, #3
	ADDI @7, @6, #1
    MVV.W @8[2], @7
    MVV.W @8[3], @3
	TAILI collatz, @8
