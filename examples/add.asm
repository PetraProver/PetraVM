#[framesize(0x4)]
add:
    ;; Frame
    ;; Slot @0: Return PC
    ;; Slot @1: Return FP
    ;; Slot @2: Return value (2 + 6)
    ;; Slot @3: Local: 2

    LDI.W @3, #2      ;; x = 2
    ADDI @2, @3, #6 ;; x + 6
    RET
