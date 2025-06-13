#[framesize(0x7)]
_start:
    ;; Slot 0: PC
    ;; Slot 1: FP
    ;; Slot 2: Arg a
    ;; Slot 3: Arg b
    ;; Slot 4: Return quot value
    ;; Slot 5: Return rem value
    ;; Slot 6: Next FP
    
    ;; If b == 0, throw an exception
    BNZ div_entrypoint, @3
    ALLOCI! @6, #3
    CALLI trap_entrypoint, @6

div_entrypoint:
    ALLOCI! @6, #10
    MVV.W @6[2], @2
    MVV.W @6[3], @3
    CALLI div, @6
    MVV.W @6[4], @4
    MVV.W @6[5], @5
    RET

#[framesize(0x3)]
trap_entrypoint:
    TRAP #3