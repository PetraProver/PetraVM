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
;;
;; ============================================================================

#[framesize(0x20)]
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
    
    ;; Call the branch and jump test
    MVV.W @9[2], @10
    CALLI test_jumps_branches, @9
    BNZ test_failed, @10
    
    ;; Call the function call operations test
    MVV.W @11[2], @12
    CALLI test_function_calls, @11
    BNZ test_failed, @12
    
    ;; Call the TAILI test
    MVV.W @13[2], @14
    CALLI test_taili, @13
    BNZ test_failed, @14

    LDI.W @2, #0    ;; overall success flag
    RET

#[framesize(0x5)]
test_failed:
    LDI.W @2, #1    ;; overall failure flag
    RET

;; ============================================================================
;; TARGET FUNCTIONS FOR CALLV/TAILV TESTS
;; These functions are placed early in the program so we can know their PC values
;; ============================================================================

;; PC = 23G (we know this exact value for CALLV tests)
#[framesize(0x5)]
callv_target_fn:
    LDI.W @2, #123      ;; Set special return value to identify CALLV worked
    RET

;; PC = 25G (we know this exact value for TAILV tests)
#[framesize(0x5)]
tail_target_fn:
    LDI.W @2, #0        ;; Set success flag (0 = success)
    RET

;; PC = 27G (we know this exact value for JUMPV tests)
jumpv_destination:
    LDI.W @15, #77       ;; Set special value to identify JUMPV worked
    J jumpv_done        ;; Jump to continue testing
    
jumpv_done:
    ;; Now, check if JUMPV worked correctly by checking the value set in jumpv_destination
    XORI @16, @15, #77    ;; @15 should be 77 if JUMPV worked correctly
    BNZ branch_fail, @16
    
    ;; All tests passed
    LDI.W @2, #0           ;; Set success flag (0 = success)
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

#[framesize(0x50)]
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
    LDI.W @6, #65535 ;; Max u16 immediate value
    LDI.W @7, #0     ;; Initialize a working register

    ;; Set up a value with all bits set (equivalent to -1 in two's complement)
    XORI @8, @7, #65535     ;; Low 16 bits are all 1s
    SLLI @9, @8, #16        ;; Shift left by 16
    ORI @10, @9, #65535     ;; OR with 65535 to set all 32 bits to 1

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
    ADD @11, @3, @4      ;; 42 + 7 = 49
    ADDI @12, @3, #7     ;; 42 + 7 = 49
    
    ;; Verify both give same result
    XOR @13, @11, @12    ;; Compare ADD and ADDI results
    BNZ int_fail, @13

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
    SUB @14, @3, @4      ;; 42 - 7 = 35
    XORI @15, @14, #35   ;; Check result
    BNZ int_fail, @15

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
    AND @16, @3, @4      ;; 42 & 7 = 2
    ANDI @17, @3, #7     ;; 42 & 7 = 2
    
    ;; Verify both give same result and value is correct
    XOR @18, @16, @17    ;; Compare AND and ANDI results
    BNZ int_fail, @18
    
    XORI @19, @16, #2    ;; Check the result value
    BNZ int_fail, @19

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
    OR @20, @3, @4       ;; 42 | 7 = 47
    ORI @21, @3, #7      ;; 42 | 7 = 47
    
    ;; Verify both give same result and value is correct
    XOR @22, @20, @21    ;; Compare OR and ORI results
    BNZ int_fail, @22
    
    XORI @23, @20, #47   ;; Check the result value
    BNZ int_fail, @23

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
    SLLI @24, @4, #2     ;; 7 << 2 = 28
    XORI @25, @24, #28   ;; Check result
    BNZ int_fail, @25
    
    SRLI @26, @3, #2     ;; 42 >> 2 = 10
    XORI @27, @26, #10   ;; Check result
    BNZ int_fail, @27
    
    ;; Simple test for SRAI with small positive value
    SRAI @28, @4, #1     ;; 7 >> 1 = 3
    XORI @29, @28, #3    ;; Check result
    BNZ int_fail, @29
    
    ;; Test register shift variants
    SLL @30, @4, @5      ;; 7 << 2 = 28
    XORI @31, @30, #28   ;; Check result
    BNZ int_fail, @31
    
    SRL @32, @3, @5      ;; 42 >> 2 = 10
    XORI @33, @32, #10   ;; Check result
    BNZ int_fail, @33
    
    ;; Simple test for SRA with small positive value
    SRA @34, @4, @5      ;; 7 >> 2 = 1
    XORI @35, @34, #1    ;; Check result
    BNZ int_fail, @35

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: MUL / MULI
    ;; 
    ;; FORMAT: 
    ;;   MUL dst, src1, src2    (Signed multiplication)
    ;;   MULI dst, src, imm     (Immediate multiplication)
    ;; 
    ;; DESCRIPTION:
    ;;   Multiply integer values (signed).
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
    
    ;; Verify the actual value of lower 32 bits
    XORI @41, @36, #294  ;; Check the result value
    BNZ int_fail, @41

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: MULU
    ;; 
    ;; FORMAT: MULU dst, src1, src2
    ;; 
    ;; DESCRIPTION:
    ;;   Unsigned multiplication of two 32-bit integers, returning
    ;;   an unsigned 64-bit output.
    ;;
    ;; EFFECT: fp[dst:dst+1] = (unsigned)fp[src1] * (unsigned)fp[src2]
    ;; ------------------------------------------------------------
    ;; Test with positive numbers first (should match MUL for positive values)
    MULU @42, @3, @4     ;; 42u * 7u = 294u (lower 32 bits in @42, upper 32 bits in @43)
    XORI @44, @42, #294  ;; Check lower 32 bits match expected value
    BNZ int_fail, @44
    
    ;; Test with a larger value
    LDI.W @45, #100
    MULU @46, @45, @45   ;; 100u * 100u = 10000u (lower 32 bits in @46, upper 32 bits in @47)
    ;; 10000 decimal = 0x2710, so we can test against 10000
    ;; (which is within u16 immediate range)
    XORI @48, @46, #10000  ;; Check lower 32 bits
    BNZ int_fail, @48
    
    ;; Test with the all-ones value (@10) that we created earlier
    ;; Since we're testing with 5 * all-ones, we need to create the expected result
    ;; Expected: 5 * 0xFFFFFFFF = 5 * (2^32 - 1) = 5 * 2^32 - 5
    ;; The lower 32 bits will be -5 & 0xFFFFFFFF = 0xFFFFFFFB = (2^32 - 5)
    ;; We'll create this value and compare directly
    LDI.W @49, #5
    MULU @50, @49, @10   ;; 5u * 0xFFFFFFFF (lower 32 bits in @50, upper 32 bits in @51)
    
    ;; Create the expected result: -5 in 32 bits
    ;; We first complement 5 to get 0xFFFFFFFA, then add 1 to get 0xFFFFFFFB
    XORI @52, @7, #5     ;; @7 is 0, so this gives us 5
    XOR @53, @10, @52    ;; complement of 5 (all ones XOR 5)
    ADDI @54, @53, #1    ;; Two's complement: add 1 to get -5
    
    ;; Now compare the MULU result with our constructed -5
    XOR @55, @50, @54    ;; Should be 0 if they match
    BNZ int_fail, @55

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: MULSU
    ;; 
    ;; FORMAT: MULSU dst, src1, src2
    ;; 
    ;; DESCRIPTION:
    ;;   Multiplication of a signed 32-bit integer with an unsigned 32-bit
    ;;   integer, returning a signed 64-bit output.
    ;;
    ;; EFFECT: fp[dst:dst+1] = (signed)fp[src1] * (unsigned)fp[src2]
    ;; ------------------------------------------------------------
    ;; Test with all positive numbers first
    MULSU @56, @3, @4    ;; 42 * 7u = 294 (lower 32 bits in @56, upper 32 bits in @57)
    XORI @58, @56, #294  ;; Check lower 32 bits
    BNZ int_fail, @58
    
    ;; Test with the all-ones value as a signed negative number (-1)
    ;; -1 * 5u = -5
    MULSU @60, @10, @49  ;; -1 * 5u (lower 32 bits in @60, upper 32 bits in @61)
    
    ;; Compare with the -5 value we created earlier
    XOR @62, @60, @54    ;; Should be 0 if they match
    BNZ int_fail, @62

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
    SLT @64, @4, @3      ;; 7 < 42? = 1 (true)
    XORI @65, @64, #1    ;; Check result
    BNZ int_fail, @65
    
    SLTI @66, @4, #42    ;; 7 < 42? = 1 (true)
    XORI @67, @66, #1    ;; Check result
    BNZ int_fail, @67
    
    SLTU @68, @4, @3     ;; 7 <u 42? = 1 (true)
    XORI @69, @68, #1    ;; Check result
    BNZ int_fail, @69
    
    SLTIU @70, @4, #42   ;; 7 <u 42? = 1 (true)
    XORI @71, @70, #1    ;; Check result
    BNZ int_fail, @71

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

    XORI @10, @5, #2222  ;; Check if second word is correct
    BNZ move_call_l_fail, @10
    
    XORI @11, @6, #3333  ;; Check if third word is correct
    BNZ move_call_l_fail, @11
    
    XORI @12, @7, #4444  ;; Check if fourth word is correct
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

;; ============================================================================
;; JUMPS AND BRANCHES
;; ============================================================================
;; Tests for jump and branch instructions, which control the flow of execution.
;; ============================================================================

#[framesize(0x20)]
test_jumps_branches:
    ;; Frame slots:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Return value (success flag)
    ;; Slots 3+: Local variables for tests

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: BNZ (Branch If Not Zero)
    ;; 
    ;; FORMAT: BNZ target, cond
    ;; 
    ;; DESCRIPTION:
    ;;   Branch to target address if condition register is not zero.
    ;;
    ;; EFFECT: 
    ;;   if fp[cond] != 0 then PC = target
    ;;   else PC = PC * G (next instruction)
    ;; ------------------------------------------------------------
    ;; 1. Test branch NOT taken (condition is zero)
    ;; Note: All VROM locations can only be written once, so we use separate locations
    ;; to record program flow
    LDI.W @3, #0           ;; Set condition to 0
    BNZ bnz_path_1, @3     ;; Should NOT branch since @3 is 0
    
    ;; When branch not taken, we set @4 to 1 to indicate this path was followed
    LDI.W @4, #1           ;; Record that branch was not taken
    J bnz_check_path_1     ;; Jump to verify branch was not taken
    
bnz_path_1:
    ;; When branch taken, we set @5 to 1 to indicate this path was followed
    LDI.W @5, #1           ;; Record that branch was taken incorrectly
    
bnz_check_path_1:
    ;; Verify @4 == 1 (branch was not taken) and @5 is undefined (branch path not taken)
    XORI @6, @4, #1        ;; @4 should be 1 if branch was not taken
    BNZ branch_fail, @6
    
    ;; 2. Test branch taken (condition is non-zero)
    LDI.W @7, #42          ;; Set non-zero condition
    BNZ bnz_path_2, @7     ;; Should branch since @7 is non-zero
    
    ;; When branch not taken, we set @8 to 1 to indicate this path was followed incorrectly
    LDI.W @8, #1           ;; Record that branch was not taken incorrectly
    J bnz_check_path_2
    
bnz_path_2:
    ;; When branch taken, we set @9 to 1 to indicate this path was followed
    LDI.W @9, #1           ;; Record that branch was taken correctly
    
bnz_check_path_2:
    ;; Verify @9 == 1 (branch was taken correctly) and @8 is undefined (other path not taken)
    XORI @10, @9, #1       ;; @9 should be 1 if branch was taken
    BNZ branch_fail, @10

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: J (Jump to label)
    ;; 
    ;; FORMAT: J target
    ;; 
    ;; DESCRIPTION:
    ;;   Jump to a target address unconditionally.
    ;;
    ;; EFFECT: PC = target
    ;; ------------------------------------------------------------
    ;; Test unconditional jump
    J jump_target           ;; Should always jump
    
    ;; We should NOT reach here
    LDI.W @11, #1          ;; This should not execute
    J branch_fail
    
jump_target:
    ;; We SHOULD reach here
    LDI.W @12, #1          ;; Record that we reached the jump target

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: J (Jump to VROM address)
    ;; 
    ;; FORMAT: J @register
    ;; 
    ;; DESCRIPTION:
    ;;   Jump to a target address stored in VROM.
    ;;
    ;; EFFECT: PC = fp[register]
    ;; ------------------------------------------------------------
    ;; First, load the destination address into a VROM slot
    ;; We use the PC value of jumpv_destination (at PC = 27G)
    LDI.W @13, #2983627541  ;; Actual field element value for 27G
    
    ;; Now jump to that address using J @register syntax
    J @13               ;; Jump to the address in @13
    
    ;; We should NOT reach here
    LDI.W @14, #1       ;; This should not execute
    J branch_fail
    
branch_fail:
    LDI.W @2, #1         ;; Set failure flag (1 = failure)
    RET

;; ============================================================================
;; FUNCTION CALL OPERATIONS
;; ============================================================================
;; Tests for function call instructions, which save and restore execution context.
;; ============================================================================

#[framesize(0x30)]
test_function_calls:
    ;; Frame slots:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Return value (success flag)
    ;; Slots 3+: Local variables for tests and temporary frames

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: CALLI (Call Immediate)
    ;; 
    ;; FORMAT: CALLI target, next_fp
    ;; 
    ;; DESCRIPTION:
    ;;   Function call to a target address.
    ;;   Sets up a new frame with the return address and old FP.
    ;;
    ;; EFFECT: 
    ;;   [FP[next_fp] + 0] = PC * G (return address)
    ;;   [FP[next_fp] + 1] = FP (old frame pointer)
    ;;   FP = FP[next_fp]
    ;;   PC = target
    ;; ------------------------------------------------------------
    ;; Test a regular function call
    MVV.W @10[2], @11    ;; Set up a slot to receive the return value
    CALLI test_simple_fn, @10
    
    ;; Check the return value from the function
    XORI @12, @11, #42   ;; Function should return 42
    BNZ call_fail, @12

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: CALLV (Call Variable)
    ;; 
    ;; FORMAT: CALLV target, next_fp
    ;; 
    ;; DESCRIPTION:
    ;;   Function call to a target address stored in a register.
    ;;   Sets up a new frame with the return address and old FP.
    ;;
    ;; EFFECT: 
    ;;   [FP[next_fp] + 0] = PC * G (return address)
    ;;   [FP[next_fp] + 1] = FP (old frame pointer)
    ;;   FP = FP[next_fp]
    ;;   PC = fp[target]
    ;; ------------------------------------------------------------
    ;; For CALLV, we need to use a known PC value
    ;; We placed callv_target_fn at PC = 23G (marked in comments above)
    LDI.W @13, #2803768080  ;; Actual field element value for 23G
    
    ;; Set up a call frame for CALLV
    MVV.W @14[2], @15    ;; Set up a slot to receive the return value
    CALLV @13, @14       ;; Call using the address in @13
    
    ;; Check if we got the special return value from callv_target_fn (123)
    XORI @16, @15, #123  ;; Function should return 123
    BNZ call_fail, @16

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: TAILV (Tail Call Variable)
    ;; 
    ;; FORMAT: TAILV target, next_fp
    ;; 
    ;; DESCRIPTION:
    ;;   Tail call to a target address stored in a register.
    ;;   Preserves the original return address and frame.
    ;;
    ;; EFFECT: 
    ;;   [FP[next_fp] + 0] = FP[0] (return address)
    ;;   [FP[next_fp] + 1] = FP[1] (old frame pointer)
    ;;   FP = FP[next_fp]
    ;;   PC = fp[target]
    ;; ------------------------------------------------------------
    ;; Test TAILV using a known PC value
    ;; We placed tailv_target_fn at PC = 25G (marked in comments above)
    LDI.W @17, #3069186472  ;; Actual field element value for 25G
    
    ;; Pass the final return value slot to the function
    MVV.W @18[2], @2     ;; Pass the final return value slot
    TAILV @17, @18       ;; Tail call using address in @17
    
    ;; We should not reach here - the tail call should return directly
    ;; to the caller of test_function_calls
    LDI.W @2, #1         ;; Set failure flag
    RET

call_fail:
    LDI.W @2, #1         ;; Set failure flag (1 = failure)
    RET

#[framesize(0x10)]
test_taili:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Return value slot
    
    ;; ------------------------------------------------------------
    ;; INSTRUCTION: TAILI (Tail Call Immediate)
    ;; 
    ;; FORMAT: TAILI target, next_fp
    ;; 
    ;; DESCRIPTION:
    ;;   Tail call to a target address given by an immediate.
    ;;   Preserves the original return address and frame.
    ;;
    ;; EFFECT: 
    ;;   [FP[next_fp] + 0] = FP[0] (return address)
    ;;   [FP[next_fp] + 1] = FP[1] (old frame pointer)
    ;;   FP = FP[next_fp]
    ;;   PC = target
    ;; ------------------------------------------------------------
    
    ;; Set up a new frame for the tail call
    MVV.W @3[2], @2     ;; Pass the return value slot to the target function
    TAILI tail_target_fn, @3  ;; Tail call to tail_target_fn
    
    ;; Should not reach here - the tail call should return directly to our caller
    LDI.W @2, #1        ;; Set failure flag (1 = failure)
    RET

;; Simple test function
#[framesize(0x5)]
test_simple_fn:
    ;; Slot 0: Return PC (set by CALL instruction)
    ;; Slot 1: Return FP (set by CALL instruction)
    ;; Slot 2: Return value slot
    
    LDI.W @2, #42       ;; Set a test return value
    RET                 ;; Return to caller
