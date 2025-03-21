;; Rust equivalent:
;; ------------
;; return 2 + 6
;; ------------
add:
    ;; Frame
    ;; Slot @0: Return PC
    ;; Slot @1: Return FP
    ;; Slot @2: Return value
    ;; Slot @3: Local: 2
    ;; Slot @4: Local: 2 + 6

    MVI.H @3, #2    ;; x = 2
    ADDI @4, @3, #6 ;; x + 6
    RET
