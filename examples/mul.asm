;; Rust equivalent:
;; ------------
;; return 3 * 7
;; ------------

#[framesize(0x4)]
mul:
    ;; Frame
    ;; Slot @0: Return PC
    ;; Slot @1: Return FP
    ;; Slot @2: Return value (3 * 7)
    ;; Slot @3: Local: 3

    MVI.H @3, #2    ;; x = 3
    MULI @2, @3, #7 ;; x * 7
    RET
