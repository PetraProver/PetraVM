;; Rust equivalent:
;; ------------
;; return add_two_numbers(4, 8) + 10
;;
;; fn add_two_numbers(a: u32, b: u32) -> u32 { a + b }
;; ------------

#[framesize(0x8)]
func_call:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Return value
    ;; Slot 3: ND Local: Next FP
    ;; Slot 4: Local: add_two_numbers(4, 8)
    ;; Slot 5: Local: 

    ALLOCI! @3, #5
    MVI.H @3[2], #4  ;; a = 4
    MVI.H @3[3], #8  ;; b = 8
    CALLI add_two_numbers, @3
    MVV.W @3[4], @4  ;; x = add_two_numbers(4, 8)

    ADDI @2, @4, #10 ;; return x + 10
    RET

#[framesize(0x5)]
add_two_numbers:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Arg: a
    ;; Slot 3: Arg: b
    ;; Slot 4: Return value
    ADD @4, @2, @3
    RET
