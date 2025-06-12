;; Rust equivalent:
;; ------------
;; fn div(a: u32, b: u32) -> (q, r) {
;;      if a < b {
;;          let (q, r) = div(a - b, b);
;;          (q + 1, r)
;;      } else {
;;          (0, a)
;;      }
;; }
;; ------------

#[framesize(0xa)]
div:
    ;; Frame:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Arg a
    ;; Slot 3: Arg b
    ;; Slot 4: Return value q
    ;; Slot 5: Return value r
    ;; Slot 6: Non-deterministic local: Next FP
    ;; Slot 7: Local: a<b
    ;; Slot 8: Local: a-b
    ;; Slot 9: Local: q1

    ALLOCI! @6, #10
    SLTU @7, @2, @3
    BNZ div_consequent, @7 ;; TODO: This logic is inverted but we're going to wait until SLE is implemented...
    SUB @8, @2, @3
    MVV.W @6[2], @8
    MVV.W @6[3], @3
    CALLI div, @6
    MVV.W @6[4], @9
    MVV.W @6[5], @5
    ADDI @4, @9, #1
    RET
div_consequent:
    LDI.W @4, #0
    XORI @5, @2, #0
    RET
