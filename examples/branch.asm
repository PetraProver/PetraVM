;; Rust equivalent:
;; ------------
;; fn branches(n: u32) -> u32 {
;; if n < 3 {
;;     return 4;
;; } else {
;;     return 2;
;; }
;; ------------

#[framesize(0x5)]
branches:
    ;; Frame:
    ;; Slot @0: Return PC
    ;; Slot @1: Return FP
    ;; Slot @2: Arg: n
    ;; Slot @3: Return value
    ;; Slot @4: Local: n < 3

    SLTI @4, @2, #3 ;; n < 3
    BNZ less_than_3, @4 ;; if n >= 3

    LDI.W @3, #2 ;; return 2
    RET

less_than_3:
    LDI.W @3, #4 ;; return 4
    RET
