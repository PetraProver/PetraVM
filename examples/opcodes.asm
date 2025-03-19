;; ============================================================================
;; zCrayVM INSTRUCTION SET SPECIFICATION AND TEST SUITE
;; ============================================================================
;; This document serves as both a specification for the zCrayVM instruction set
;; and a test suite to verify correct implementation of each instruction.
;;
;; INSTRUCTION CATEGORIES:
;; 1. Binary Field Operations - Field-specific arithmetic
;; 2. Integer Operations - Standard integer arithmetic and logic
;; 3. Move Operations - Data movement between registers and memory
;; 4. Control Flow - Jumps, branches, and function calls
;;
;; REGISTER CONVENTIONS:
;; - FP: Frame Pointer (points to current function frame in VROM)
;; - PC: Program Counter (multiplicative addressing using field's cyclic group)
;; - TS: Timestamp for RAM operations
;;
;; NOTATION:
;; - @N: Refers to frame offset N from the frame pointer (FP)
;; - All values in VROM are accessed via offsets from the frame pointer
;;
;; MEMORY MODEL:
;; - Harvard architecture with separate instruction and data memory
;; - PROM: Program ROM (immutable instruction memory)
;; - VROM: Value ROM (write-once, non-deterministic allocation)
;;   * All temporary values and function frames are in VROM
;;   * Values are accessed via offsets from the frame pointer (FP)
;; - RAM: Read-write memory (byte-addressable, optional)
;;
;; FRAME SLOT CONVENTIONS:
;; - Slot 0: Return PC (set by CALL instructions)
;; - Slot 1: Return FP (set by CALL instructions)
;; - Slot 2+: Function-specific arguments, return values, and local variables
;; ============================================================================

#[framesize(0x10)]
_start: 
    ;; Copy test result (success flag) from return frame
    MVV.W @3[2], @4

    ;; Call the binary field test and check result
    CALLI test_binary_field, @3
    
    ;;BNZ test_failed, @4
    ;;LDI.W @2, #0    ;; overall success flag
    RET
test_failed:
    ;;LDI.W @2, #1    ;; overall failure flag
    RET

;; ============================================================================
;; BINARY FIELD OPERATIONS
;; ============================================================================
;; These instructions perform operations in binary field arithmetic (GF(2^n)).
;; Binary field addition is equivalent to bitwise XOR.
;; Binary field multiplication has special semantics for the field.
;; ============================================================================

#[framesize(0x30)]
test_binary_field:
    ;; Frame slots:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Return value (success flag)
    ;; Slots 3+: Local variables for tests

    ;; Initialize test values directly in this function
    LDI.W @3, #42    ;; test value A
    LDI.W @4, #7     ;; test value B

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: XOR / XORI / B32_ADD
    ;; 
    ;; FORMAT: 
    ;;   XOR dst, src1, src2     (Register variant)
    ;;   XORI dst, src1, imm     (Immediate variant)
    ;;   B32_ADD dst, src1, src2 (Binary field addition - alias for XOR)
    ;; 
    ;; DESCRIPTION:
    ;;   Perform bitwise XOR between values.
    ;;   In binary fields, XOR is equivalent to addition (B32_ADD).
    ;;
    ;; EFFECT: 
    ;;   fp[dst] = fp[src1] ^ fp[src2]
    ;;   fp[dst] = fp[src1] ^ imm
    ;; ------------------------------------------------------------
    XOR @10, @3, @4      ;; 42 XOR 7 = 45
    XORI @11, @10, #45   ;; result should be 0 if correct
    BNZ bf_fail, @11

    ;; B32_ADD (alias for XOR)
    B32_ADD @12, @3, @4
    XOR @13, @10, @12     ;; Compare with previous XOR result
    BNZ bf_fail, @13      ;; Should be 0 if B32_ADD is alias for XOR

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: B32_MUL
    ;; 
    ;; FORMAT: B32_MUL dst, src1, src2
    ;; 
    ;; DESCRIPTION:
    ;;   Multiply two 32-bit binary field elements.
    ;;   Performs multiplication in the binary field GF(2^32).
    ;;
    ;; EFFECT: fp[dst] = fp[src1] * fp[src2] (in GF(2^32))
    ;; ------------------------------------------------------------
    ;; Test B32_MUL with the test values
    B32_MUL @14, @3, @4
    
    ;; Test B32_MUL with the multiplicative identity
    ;; 1 is the multiplicative identity in binary fields
    LDI.W @15, #1
    B32_MUL @16, @15, @4
    XORI @17, @16, #7  ;; 1 * 7 should equal 7
    BNZ bf_fail, @17

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: B128_ADD
    ;; 
    ;; FORMAT: B128_ADD dst, src1, src2
    ;; 
    ;; DESCRIPTION:
    ;;   Add two 128-bit binary field elements.
    ;;   This is a component-wise XOR of four 32-bit words.
    ;;   Note: Requires proper 16-byte alignment for 128-bit operations
    ;;
    ;; EFFECT: fp[dst] = fp[src1] ⊕ fp[src2] (128-bit operation)
    ;; ------------------------------------------------------------
    ;; Ensure 16-byte alignment for 128-bit operations
    LDI.W @20, #1     ;; First 128-bit value starts at @20 (aligned)
    LDI.W @21, #0
    LDI.W @22, #0
    LDI.W @23, #0
    
    LDI.W @24, #2     ;; Second 128-bit value starts at @24 (aligned)
    LDI.W @25, #0
    LDI.W @26, #0
    LDI.W @27, #0
    
    B128_ADD @28, @20, @24   ;; Result stored at @28 (aligned)
    
    ;; Check if first word is correct (1 XOR 2 = 3)
    XORI @32, @28, #3
    BNZ bf_fail, @32

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: B128_MUL
    ;; 
    ;; FORMAT: B128_MUL dst, src1, src2
    ;; 
    ;; DESCRIPTION:
    ;;   Multiply two 128-bit binary field elements.
    ;;   Performs multiplication in the binary field GF(2^128).
    ;;
    ;; EFFECT: fp[dst] = fp[src1] * fp[src2] (in GF(2^128))
    ;; ------------------------------------------------------------
    ;; Perform 128-bit multiplication
    B128_MUL @36, @20, @24   ;; Result stored at @32 (aligned)
    
    XORI @37, @36, #2
    BNZ bf_fail, @37

    LDI.W @2, #0         ;; Set success flag
    RET
bf_fail:
    LDI.W @2, #1         ;; Set failure flag
    RET
