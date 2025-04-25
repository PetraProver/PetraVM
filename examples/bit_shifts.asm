;; Rust equivalent:
;; ------------
;; let mut x = 5;
;; x <<= 1;
;; x >>= 1;
;; x >>= 3;
;; x <<= 3;
;;
;; let mut y = 2;
;; let shift_amt = 2;
;; y <<= shift_amt;
;;
;; return
;; ------------

#[framesize(0xb)]
bit_shifts:
    ;; Frame
    ;; Slot @0: Return PC
    ;; Slot @1: Return FP
    ;; Slot @2: Return value
    ;; Slot @3: Local: 5
    ;; Slot @4: Local: x <<= 1
    ;; Slot @5: Local: x >>= 1
    ;; Slot @6: Local: x >>= 3
    ;; Slot @7: Local: x <<= 3
    ;; Slot @8: Local: 2
    ;; Slot @9: Local: shift_amt = 6;
    ;; Slot @10: Local: y <<= shift_amt

    LDI.W @3, #5 ;; x = 5
    SLLI @4, @3, #1 ;; x <<= 1
    SRLI @5, @4, #1 ;; x >>= 1
    SRLI @6, @5, #3 ;; x >>= 3
    SLLI @7, @6, #3 ;; x <<= 3

    LDI.W @8, #2 ;; y = 2
    LDI.W @9, #6 ;; shift_amt = 6
    SLL @10, @8, @9 ;; y <<= shift_amt

    RET
