;; ============================================================================
;; zCrayVM INSTRUCTION SET TEST SUITE
;; ============================================================================
;; This file tests the support for zCrayVM instructions to ensure the emulator
;; can correctly parse and execute all defined instructions.
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
    ;; Call the binary field test
    MVV.W @3[2], @4
    CALLI test_binary_field, @3
    BNZ test_failed, @4
    
    ;; Call the integer operations test
    MVV.W @5[2], @6
    CALLI test_integer_ops, @5
    BNZ test_failed, @6
    
    LDI.W @2, #0    ;; overall success flag
    RET
test_failed:
    LDI.W @2, #1    ;; overall failure flag
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
    ;; INSTRUCTION: XOR / XORI
    ;; 
    ;; FORMAT: 
    ;;   XOR dst, src1, src2     (Register variant)
    ;;   XORI dst, src1, imm     (Immediate variant)
    ;; 
    ;; DESCRIPTION:
    ;;   Perform bitwise XOR between values.
    ;;   In binary fields, XOR is equivalent to addition.
    ;;
    ;; EFFECT: 
    ;;   fp[dst] = fp[src1] ^ fp[src2]
    ;;   fp[dst] = fp[src1] ^ imm
    ;; ------------------------------------------------------------
    XOR @10, @3, @4      ;; 42 XOR 7 = 45
    XORI @11, @10, #45   ;; result should be 0 if correct
    BNZ bf_fail, @11

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
    B32_MUL @12, @3, @4
    
    ;; Test with multiplicative identity
    LDI.W @15, #1        ;; 1 is the multiplicative identity in binary fields
    B32_MUL @16, @15, @4
    XORI @17, @16, #7    ;; 1 * 7 should equal 7
    BNZ bf_fail, @17

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
    ;; Set up 128-bit values (16-byte aligned)
    LDI.W @20, #1     ;; First 128-bit value starts at @20
    LDI.W @21, #0
    LDI.W @22, #0
    LDI.W @23, #0
    
    LDI.W @24, #2     ;; Second 128-bit value starts at @24
    LDI.W @25, #0
    LDI.W @26, #0
    LDI.W @27, #0
    
    B128_ADD @28, @20, @24
    
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
    ;; Test multiplication (1 * 2 = 2)
    B128_MUL @36, @20, @24
    XORI @37, @36, #2
    BNZ bf_fail, @37

    LDI.W @2, #0         ;; Set success flag (0 = success)
    RET
bf_fail:
    LDI.W @2, #1         ;; Set failure flag (1 = failure)
    RET

;; ============================================================================
;; INTEGER OPERATIONS
;; ============================================================================
;; These instructions perform operations on 32-bit integer values.
;; Includes arithmetic, logical, comparison, and shift operations.
;; ============================================================================

#[framesize(0x40)]
test_integer_ops:
    ;; Frame slots:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Return value (success flag)
    ;; Slots 3+: Local variables for tests

    ;; Initialize test values directly in this function
    LDI.W @3, #42    ;; test value A
    LDI.W @4, #7     ;; test value B
    LDI.W @5, #2     ;; shift amount

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: ADD / ADDI
    ;; 
    ;; FORMAT: 
    ;;   ADD dst, src1, src2   (Register variant)
    ;;   ADDI dst, src, imm    (Immediate variant)
    ;; 
    ;; DESCRIPTION:
    ;;   Add integer values.
    ;;
    ;; EFFECT: 
    ;;   fp[dst] = fp[src1] + fp[src2]
    ;;   fp[dst] = fp[src] + imm
    ;; ------------------------------------------------------------
    ADD @10, @3, @4      ;; 42 + 7 = 49
    ADDI @11, @3, #7     ;; 42 + 7 = 49
    XOR @12, @10, @11    ;; result should be 0 if equal
    BNZ int_fail, @12

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: SUB
    ;; 
    ;; FORMAT: SUB dst, src1, src2
    ;; 
    ;; DESCRIPTION:
    ;;   Subtract the second value from the first.
    ;;
    ;; EFFECT: fp[dst] = fp[src1] - fp[src2]
    ;; ------------------------------------------------------------
    SUB @13, @3, @4      ;; 42 - 7 = 35
    XORI @14, @13, #35   ;; should be 0 if correct
    BNZ int_fail, @14

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: AND / ANDI
    ;; 
    ;; FORMAT: 
    ;;   AND dst, src1, src2   (Register variant)
    ;;   ANDI dst, src, imm    (Immediate variant)
    ;; 
    ;; DESCRIPTION:
    ;;   Bitwise AND of values.
    ;;
    ;; EFFECT: 
    ;;   fp[dst] = fp[src1] & fp[src2]
    ;;   fp[dst] = fp[src] & imm
    ;; ------------------------------------------------------------
    AND @15, @3, @4      ;; 42 & 7 = 2
    ANDI @16, @3, #7     ;; 42 & 7 = 2
    XOR @17, @15, @16    ;; Should be 0 if equal
    BNZ int_fail, @17
    
    XORI @18, @15, #2    ;; Check actual value (2)
    BNZ int_fail, @18

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: OR / ORI
    ;; 
    ;; FORMAT: 
    ;;   OR dst, src1, src2   (Register variant)
    ;;   ORI dst, src, imm    (Immediate variant)
    ;; 
    ;; DESCRIPTION:
    ;;   Bitwise OR of values.
    ;;
    ;; EFFECT: 
    ;;   fp[dst] = fp[src1] | fp[src2]
    ;;   fp[dst] = fp[src] | imm
    ;; ------------------------------------------------------------
    OR @19, @3, @4       ;; 42 | 7 = 47
    ORI @20, @3, #7      ;; 42 | 7 = 47
    XOR @21, @19, @20    ;; Should be 0 if equal
    BNZ int_fail, @21
    
    XORI @22, @19, #47   ;; Check actual value (47)
    BNZ int_fail, @22

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: Shift Operations
    ;; 
    ;; FORMAT: 
    ;;   SLL dst, src1, src2   (Shift Left Logical)
    ;;   SRL dst, src1, src2   (Shift Right Logical)
    ;;   SRA dst, src1, src2   (Shift Right Arithmetic)
    ;;   SLLI dst, src, imm    (Shift Left Logical Immediate)
    ;;   SRLI dst, src, imm    (Shift Right Logical Immediate)
    ;;   SRAI dst, src, imm    (Shift Right Arithmetic Immediate)
    ;; 
    ;; DESCRIPTION:
    ;;   Perform shift operations.
    ;;   Logical shifts fill with zeros.
    ;;   Arithmetic right shift preserves the sign bit.
    ;;
    ;; EFFECT: 
    ;;   fp[dst] = fp[src1] << fp[src2]
    ;;   fp[dst] = fp[src1] >> fp[src2] (zero-extended)
    ;;   fp[dst] = fp[src1] >> fp[src2] (sign-extended)
    ;; ------------------------------------------------------------
    ;; Test immediate shift variants
    SLLI @23, @4, #2     ;; 7 << 2 = 28
    XORI @24, @23, #28   ;; Check result
    BNZ int_fail, @24
    
    SRLI @25, @3, #2     ;; 42 >> 2 = 10
    XORI @26, @25, #10   ;; Check result
    BNZ int_fail, @26
    
    ;; Set up a negative number to test SRAI
    LDI.W @27, #4294967290  ;; -6 in two's complement (0xFFFFFFFA)
    SRAI @28, @27, #1       ;; -6 >> 1 = -3 (0xFFFFFFFD)
    XORI @29, @28, #4294967293
    BNZ int_fail, @29
    
    ;; Test register shift variants
    SLL @30, @4, @5      ;; 7 << 2 = 28
    XORI @31, @30, #28   ;; Check result
    BNZ int_fail, @31
    
    SRL @32, @3, @5      ;; 42 >> 2 = 10
    XORI @33, @32, #10   ;; Check result
    BNZ int_fail, @33
    
    SRA @34, @27, @5     ;; -6 >> 2 = -2 (0xFFFFFFFE)
    XORI @35, @34, #4294967294
    BNZ int_fail, @35

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: MUL / MULU / MULSU / MULI
    ;; 
    ;; FORMAT: 
    ;;   MUL dst, src1, src2    (Signed multiplication)
    ;;   MULU dst, src1, src2   (Unsigned multiplication)
    ;;   MULSU dst, src1, src2  (Signed*Unsigned multiplication)
    ;;   MULI dst, src, imm     (Immediate multiplication)
    ;; 
    ;; DESCRIPTION:
    ;;   Multiply integer values.
    ;;
    ;; EFFECT: 
    ;;   fp[dst] = fp[src1] * fp[src2]
    ;;   fp[dst] = fp[src] * imm
    ;; ------------------------------------------------------------
    MUL @36, @3, @4      ;; 42 * 7 = 294
    XORI @37, @36, #294  ;; Check result
    BNZ int_fail, @37
    
    MULI @38, @3, #7     ;; 42 * 7 = 294
    XORI @39, @38, #294  ;; Check result
    BNZ int_fail, @39

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: SLT / SLTI / SLTU / SLTIU
    ;; 
    ;; FORMAT: 
    ;;   SLT dst, src1, src2   (Set if Less Than, signed)
    ;;   SLTI dst, src, imm    (Set if Less Than Immediate, signed)
    ;;   SLTU dst, src1, src2  (Set if Less Than, unsigned)
    ;;   SLTIU dst, src, imm   (Set if Less Than Immediate, unsigned)
    ;; 
    ;; DESCRIPTION:
    ;;   Set destination to 1 if first value is less than second,
    ;;   otherwise set to 0.
    ;;
    ;; EFFECT: 
    ;;   fp[dst] = (fp[src1] < fp[src2]) ? 1 : 0
    ;;   fp[dst] = (fp[src] < imm) ? 1 : 0
    ;; ------------------------------------------------------------
    SLT @60, @4, @3      ;; 7 < 42? = 1 (true)
    XORI @61, @60, #1    ;; Check result
    BNZ int_fail, @61
    
    SLTI @62, @4, #42    ;; 7 < 42? = 1 (true)
    XORI @63, @62, #1    ;; Check result
    BNZ int_fail, @63
    
    SLTU @64, @4, @3     ;; 7 <u 42? = 1 (true)
    XORI @65, @64, #1    ;; Check result
    BNZ int_fail, @65
    
    SLTIU @66, @4, #42   ;; 7 <u 42? = 1 (true)
    XORI @67, @66, #1    ;; Check result
    BNZ int_fail, @67

    LDI.W @2, #0         ;; Set success flag (0 = success)
    RET
int_fail:
    LDI.W @2, #1         ;; Set failure flag (1 = failure)
    RET
