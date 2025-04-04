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
    ;; Slot 3: Local: 4
    ;; Slot 4: ND Local: Next FP
    ;; Slot 5: Local: 4
    ;; Slot 6: Local: 8
    ;; Slot 7: Local: add_two_numbers(4, 8)

    MVI.H @4[3] #4 
    MVI.H @4[4] #8
    MVV.W @4[2] @7  // add_two_numbers(4, 8)
    CALLI add_two_numbers, @4
    ADDI @2, @7, #10 // return add_two_numbers(4, 8) + 10
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
