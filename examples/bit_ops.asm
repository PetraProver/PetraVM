;; Rust equivalent:
;; ------------
;; let x = 0b0011;
;;
;; let a = x | 0; // (res: 0b0011)
;; let b = x & 0; // (res: 0b0000)
;; let c = x ^ 0; // (res: 0b0000)
;; let d = x | 0b0101; // (res: 0b0111)
;; let e = x & 0b0101; // (res: 0b0001)
;; let f = x ^ 0b0101; // (res: 0b0110)
;; ------------

#[framesize(0x9)]
bit_ops:
    ;; Frame
    ;; Slot @0: Return PC
    ;; Slot @1: Return FP
    ;; Slot @2: Local: x
    ;; Slot @3: Local: a
    ;; Slot @4: Local: b
    ;; Slot @5: Local: c
    ;; Slot @6: Local: d
    ;; Slot @7: Local: e
    ;; Slot @8: Local: f

    MVI.H @2, #3 ;; x = 0b0011
    
    XORI @3, @2, #0 ;; a = x | 0
    ANDI @4, @2, #0 ;; b = x & 0
    ORI @5, @2, #0 ;; c = x ^ 0

    ORI @6, @2, #5 ;; d = x | 0b0101
    ANDI @7, @2, #5 ;; e = x & 0b0101
    XORI @8, @2, #5 ;; f = x ^ 0b0101

    RET
