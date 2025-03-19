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
    
    ;; Call the move operations test
    MVV.W @7[2], @8
    CALLI test_move_ops, @7
    BNZ test_failed, @8
    
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
    
    ;; Verify both give same result
    XOR @12, @10, @11    ;; Compare ADD and ADDI results
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
    XORI @14, @13, #35   ;; Check result
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
    
    ;; Verify both give same result and value is correct
    XOR @17, @15, @16    ;; Compare AND and ANDI results
    BNZ int_fail, @17
    
    XORI @18, @15, #2    ;; Check the result value
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
    
    ;; Verify both give same result and value is correct
    XOR @21, @19, @20    ;; Compare OR and ORI results
    BNZ int_fail, @21
    
    XORI @22, @19, #47   ;; Check the result value
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
    
    ;; Simple test for SRAI with small positive value
    SRAI @27, @4, #1     ;; 7 >> 1 = 3
    XORI @28, @27, #3    ;; Check result
    BNZ int_fail, @28
    
    ;; Test register shift variants
    SLL @29, @4, @5      ;; 7 << 2 = 28
    XORI @30, @29, #28   ;; Check result
    BNZ int_fail, @30
    
    SRL @31, @3, @5      ;; 42 >> 2 = 10
    XORI @32, @31, #10   ;; Check result
    BNZ int_fail, @32
    
    ;; Simple test for SRA with small positive value
    SRA @33, @4, @5      ;; 7 >> 2 = 1
    XORI @34, @33, #1    ;; Check result
    BNZ int_fail, @34

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: MUL / MULI
    ;; 
    ;; FORMAT: 
    ;;   MUL dst, src1, src2    (Signed multiplication)
    ;;   MULI dst, src, imm     (Immediate multiplication)
    ;; 
    ;; DESCRIPTION:
    ;;   Multiply integer values.
    ;;   Note: Results in 64-bit output stored across two 32-bit registers.
    ;;   The destination register must be aligned to an even address.
    ;;
    ;; EFFECT: 
    ;;   fp[dst:dst+1] = fp[src1] * fp[src2]  (64-bit result)
    ;;   fp[dst:dst+1] = fp[src] * imm        (64-bit result)
    ;; ------------------------------------------------------------
    ;; Using even registers for destination to ensure proper alignment
    MUL @36, @3, @4      ;; 42 * 7 = 294 (lower 32 bits in @36, upper 32 bits in @37)
    MULI @38, @3, #7     ;; 42 * 7 = 294 (lower 32 bits in @38, upper 32 bits in @39)
    
    ;; Verify both give same result (checking lower 32 bits, upper bits should be 0)
    XOR @40, @36, @38    ;; Compare low 32 bits of MUL and MULI results
    BNZ int_fail, @40
    
    ;; Verify the actual value of lower 32 bits (within u16 range)
    XORI @41, @36, #294  ;; Check the result value
    BNZ int_fail, @41

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
    SLT @42, @4, @3      ;; 7 < 42? = 1 (true)
    XORI @43, @42, #1    ;; Check result
    BNZ int_fail, @43
    
    SLTI @44, @4, #42    ;; 7 < 42? = 1 (true)
    XORI @45, @44, #1    ;; Check result
    BNZ int_fail, @45
    
    SLTU @46, @4, @3     ;; 7 <u 42? = 1 (true)
    XORI @47, @46, #1    ;; Check result
    BNZ int_fail, @47
    
    SLTIU @48, @4, #42   ;; 7 <u 42? = 1 (true)
    XORI @49, @48, #1    ;; Check result
    BNZ int_fail, @49

    LDI.W @2, #0         ;; Set success flag (0 = success)
    RET
int_fail:
    LDI.W @2, #1         ;; Set failure flag (1 = failure)
    RET

;; ============================================================================
;; MOVE OPERATIONS
;; ============================================================================
;; These instructions move data between registers and memory.
;; They support different data widths and addressing modes.
;; ============================================================================

#[framesize(0x30)]
test_move_ops:
    ;; Frame slots:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Return value (success flag)
    ;; Slots 3+: Local variables for tests

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: LDI.W (Load Immediate Word)
    ;; 
    ;; FORMAT: LDI.W dst, imm
    ;; 
    ;; DESCRIPTION:
    ;;   Load a 32-bit immediate value into a destination.
    ;;
    ;; EFFECT: fp[dst] = imm
    ;; ------------------------------------------------------------
    LDI.W @3, #12345     ;; Load immediate value
    XORI @4, @3, #12345  ;; Check if value loaded correctly
    BNZ move_fail, @4

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: MVV.W (Move Value Word)
    ;; 
    ;; FORMAT: MVV.W dst[off], src
    ;; 
    ;; DESCRIPTION:
    ;;   Move a 32-bit value between VROM addresses.
    ;;
    ;; EFFECT: VROM[fp[dst] + off] = fp[src]
    ;; ------------------------------------------------------------
    LDI.W @8, #9876      ;; Source value
    
    ;; Call a test function with MVV.W to verify it works
    LDI.W @8, #9876      ;; Source value
    MVV.W @9[2], @8      ;; Pass the value to the function
    MVV.W @9[3], @10     ;; Set up return value location
    CALLI test_move_call, @9
    BNZ move_fail, @10   ;; Check if test failed

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: MVV.L (Move Value Long)
    ;; 
    ;; FORMAT: MVV.L dst[off], src
    ;; 
    ;; DESCRIPTION:
    ;;   Move a 128-bit value between VROM addresses.
    ;;
    ;; EFFECT: VROM128[fp[dst] + off] = fp128[src]
    ;; ------------------------------------------------------------
    ;; First set up source 128-bit value (4 sequential 32-bit words)
    LDI.W @12, #1111     ;; 1st word of 128-bit value
    LDI.W @13, #2222     ;; 2nd word
    LDI.W @14, #3333     ;; 3rd word
    LDI.W @15, #4444     ;; 4th word
    
    ;; Call a test function with MVV.L to verify it works
    MVV.L @16[4], @12    ;; Pass the 128-bit value to the function (aligned at offset 4)
    MVV.W @16[2], @17    ;; Set up return value location (use slot 2 for return value)
    CALLI test_move_call_l, @16
    BNZ move_fail, @17   ;; Check if test failed

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: MVI.H (Move Immediate Half-word)
    ;; 
    ;; FORMAT: MVI.H dst[off], imm
    ;; 
    ;; DESCRIPTION:
    ;;   Move a 16-bit immediate value to a VROM address,
    ;;   zero-extending to 32 bits.
    ;;
    ;; EFFECT: VROM[fp[dst] + off] = ZeroExtend(imm)
    ;; ------------------------------------------------------------
    ;; Call a test function with MVI.H to verify it works
    MVI.H @18[2], #255   ;; Pass the immediate value to the function
    MVV.W @18[3], @19    ;; Set up return value location
    CALLI test_move_call_h, @18
    BNZ move_fail, @19   ;; Check if test failed

    LDI.W @2, #0         ;; Set success flag (0 = success)
    RET
move_fail:
    LDI.W @2, #1         ;; Set failure flag (1 = failure)
    RET

;; Helper function to test MVV.W
#[framesize(0x10)]
test_move_call:
    ;; Receive a value in @2 and check if it's what we expect
    XORI @4, @2, #9876   ;; Check if received value is correct
    BNZ move_call_fail, @4
    
    LDI.W @3, #0         ;; Set success flag in return value slot (slot 3, not 2)
    RET
move_call_fail:
    LDI.W @3, #1         ;; Set failure flag in return value slot (slot 3, not 2)
    RET

;; Helper function to test MVV.L
#[framesize(0x10)]
test_move_call_l:
    ;; Receive a 128-bit value in @4-@7 (slots 4-7) and check if it's what we expect
    XORI @9, @4, #1111   ;; Check if first word is correct
    BNZ move_call_l_fail, @9

    XORI @10, @5, #2222   ;; Check if second word is correct
    BNZ move_call_l_fail, @10
    
    XORI @11, @6, #3333   ;; Check if third word is correct
    BNZ move_call_l_fail, @11
    
    XORI @12, @7, #4444   ;; Check if fourth word is correct
    BNZ move_call_l_fail, @12
    
    LDI.W @2, #0         ;; Set success flag in return value slot (slot 2)
    RET
move_call_l_fail:
    LDI.W @2, #1         ;; Set failure flag in return value slot (slot 2)
    RET

;; Helper function to test MVI.H
#[framesize(0x10)]
test_move_call_h:
    ;; Receive a value in @2 and check if it's what we expect
    XORI @4, @2, #255    ;; Check if received value is correct (should be zero-extended)
    BNZ move_call_h_fail, @4
    
    LDI.W @3, #0         ;; Set success flag in return value slot (slot 3, not 2)
    RET
move_call_h_fail:
    LDI.W @3, #1         ;; Set failure flag in return value slot (slot 3, not 2)
    RET
