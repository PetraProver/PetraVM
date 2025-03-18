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
    ;; Call the binary field test and check result
    CALLI test_binary_field, @3
    
    ;; Create temporary register to hold result from test
    LDI.W @5, #0
    
    ;; Copy test result (success flag) from return frame
    MVV.W @5[0], @3
    
    ;; Check if test passed (expecting @5 to be 1)
    XORI @6, @5, #1
    BNZ test_failed, @6

test_passed:
    LDI.W @2, #1    ;; overall success flag
    RET

test_failed:
    LDI.W @2, #0    ;; overall failure flag
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
    ;; B32_ADD @12, @3, @4
    ;; XOR @13, @10, @12     ;; Compare with previous XOR result
    ;; BNZ bf_fail, @13      ;; Should be 0 if B32_ADD is alias for XOR

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: B32_MUL / B32_MULI
    ;; 
    ;; FORMAT: 
    ;;   B32_MUL dst, src1, src2   (Register variant)
    ;;   B32_MULI dst, src1, imm   (Immediate variant)
    ;; 
    ;; DESCRIPTION:
    ;;   Multiply two 32-bit binary field elements.
    ;;   Performs multiplication in the binary field GF(2^32).
    ;;
    ;; EFFECT: 
    ;;   fp[dst] = fp[src1] * fp[src2] (in GF(2^32))
    ;;   fp[dst] = fp[src1] * imm (in GF(2^32))
    ;; ------------------------------------------------------------
    B32_MUL @14, @3, @4
    B32_MULI @15, @3, #7
    XOR @16, @14, @15
    BNZ bf_fail, @16

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: B128_ADD
    ;; 
    ;; FORMAT: B128_ADD dst, src1, src2
    ;; 
    ;; DESCRIPTION:
    ;;   Add two 128-bit binary field elements.
    ;;   This is a component-wise XOR of four 32-bit words.
    ;;
    ;; EFFECT: fp[dst] = fp[src1] ⊕ fp[src2] (128-bit operation)
    ;; ------------------------------------------------------------
    LDI.W @17, #1
    LDI.W @18, #2
    B128_ADD @19, @17, @18
    XORI @20, @19, #3    ;; expecting 1 XOR 2 = 3 (for binary addition)
    BNZ bf_fail, @20

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
    ;; Set up 128-bit values by initializing words
    LDI.W @21, #1
    LDI.W @22, #0
    LDI.W @23, #0
    LDI.W @24, #0
    
    LDI.W @25, #3
    LDI.W @26, #0
    LDI.W @27, #0
    LDI.W @28, #0
    
    ;; Perform 128-bit multiplication
    B128_MUL @29, @21, @25
    XORI @30, @29, #3    ;; if 1 is multiplicative identity then 1 * 3 = 3
    BNZ bf_fail, @30

    LDI.W @2, #1         ;; Set success flag
    RET
bf_fail:
    LDI.W @2, #0         ;; Set failure flag
    RET
