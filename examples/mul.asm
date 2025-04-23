;; Rust equivalent:
;; ------------
;; return 3 * 7
;; ------------

#[framesize(0x8)]
mul:
    ;; Frame
    ;; Slot @0: Return PC
    ;; Slot @1: Return FP
    ;; Slot @2 & 3: Return value 1: (3 * 7)
    ;; Slot @4 & 5: Return value 2: (2,147,483,647 * 10)
    ;; Slot @6: Local: 3
    ;; Slot @7: Local: 2,147,483,647

    LDI.W @6, #3 ;; x = 3
    MULI @2, @6, #7 ;; ret1 = x * 7
    
    LDI.W @7, #2147483647    ;; y = 2,147,483,647
    MULI @4, @7, #10 ;; ret2 = y * 10
    RET
