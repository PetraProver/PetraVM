collatz:
    ;; Frame:
	;; Slot @0: Return PC
	;; Slot @4: Return FP
	;; Slot @8: Arg: n
    ;; Slot @12: Return value
	;; Slot @16: ND Local: Next FP
	;; Slot @20: Local: n == 1
	;; Slot @24: Local: n % 2
	;; Slot @28: Local: 3*n
    ;; Slot @32: Local: n >> 2 or 3*n + 1

	;; Branch to recursion label if value in slot 2 is not 1
	XORI @20, @8, #1
	BNZ case_recurse, @20 ;; branch if n != 1
	XORI @12, @8, #0
	RET

case_recurse:
	ANDI @24, @8, #1 ;; n % 2 is & 0x00..01
    BNZ case_odd, @24 ;; branch if n % 2 == 0u32

	;; case even
    ;; n >> 1
	SRLI @32, @8, #1
    MVV.W @16[8], @32
    MVV.W @16[12], @12
    TAILI collatz, @16

case_odd:
	MULI @28, @8, #3
	ADDI @32, @28, #1
    MVV.W @16[8], @32
    MVV.W @16[12], @12
	TAILI collatz, @16
