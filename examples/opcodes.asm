;; ============================================================================
;; PetraVM INSTRUCTION SET TEST SUITE
;; ============================================================================
;; This file tests the support for PetraVM instructions to ensure the emulator
;; can correctly parse and execute all defined instructions.
;;
;; INSTRUCTION CATEGORIES:
;; 1. Binary Field Operations - Field-specific arithmetic
;; 2. Integer Operations - Standard integer arithmetic and logic
;; 3. Move Operations - Data movement between VROM locations
;; 4. Control Flow - Jumps, branches, and function calls
;;
;; MEMORY MODEL:
;; - Harvard architecture with separate instruction and data memory
;; - PC: Program Counter (multiplicative addressing using field's cyclic group)
;; - FP: Frame Pointer (points to current function frame in VROM)
;; - TS: Timestamp for RAM operations
;;
;; NOTATION:
;; - @N: Refers to frame offset N from the frame pointer (FP)
;; - All values in VROM are accessed via offsets from the frame pointer
;;
;; MEMORY MODEL:
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

#[framesize(0x12)]
_start: 
    ;; Call the binary field test
    ;; We also test ALLOCI with the test_binary_field frame
    ALLOCI! @3, #41
    MVV.W @3[2], @4
    CALLI test_binary_field, @3
    BNZ test_failed, @4
    
    ;; Call the integer operations test
    ;; We also test ALLOCV with the test_integer_ops frame
    LDI.W! @17, #78
    ALLOCV! @5, @17
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

    ;; Call the FP test
    MVV.W @15[2], @16
    CALLI test_fp, @15
    BNZ test_failed, @16

    LDI.W @2, #0    ;; overall success flag
    RET

#[framesize(0x3)]
test_failed:
    LDI.W @2, #1    ;; overall failure flag
    RET

;; ============================================================================
;; TARGET FUNCTIONS FOR CALLV/TAILV TESTS
;; These functions are placed early in the program so we can know their PC values
;; ============================================================================

;; PC = 26G (we know this exact value for CALLV tests)
#[framesize(0x3)]
callv_target_fn:
    LDI.W @2, #123      ;; Set special return value to identify CALLV worked
    RET

;; PC = 28G (we know this exact value for TAILV tests)
#[framesize(0x3)]
tail_target_fn:
    LDI.W @2, #0        ;; Set success flag (0 = success)
    RET

;; PC = 30G (we know this exact value for JUMPV tests)
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

#[framesize(0x29)]
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
    ;;   XOR dst, src1, src2     (VROM variant)
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
    ;; INSTRUCTION: B32_MUL/B32_MULI
    ;; 
    ;; FORMAT: 
    ;;   B32_MUL dst, src1, src2     (VROM variant)
    ;;   B32_MULI dst, src1, imm     (Immediate variant)
    ;; 
    ;; DESCRIPTION:
    ;;   Multiply two 32-bit binary field elements.
    ;;   Performs multiplication in the binary field GF(2^32).
    ;;
    ;; EFFECT: fp[dst] = fp[src1] * fp[src2] (in GF(2^32))
    ;; ------------------------------------------------------------
    B32_MUL @12, @3, @4
    
    ;; Test with multiplicative identity
    LDI.W @13, #1        ;; 1 is the multiplicative identity in binary fields
    B32_MUL @14, @13, @4
    B32_MULI @15, @4, #0G ;; 1 = G^0

    ;; Verify both give the same result
    XOR @16, @14, @15    ;; 1 * 7 should equal 7
    BNZ bf_fail, @16

    ;; Verify they give the correct result
    XORI @17, @14, #7
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
    LDI.W @26, #5
    LDI.W @27, #7
    
    B128_ADD @28, @20, @24
    
    ;; Check if fourth word is correct (0 XOR 7 = 7)
    XORI @32, @31, #7
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
    ;; Test multiplication (1 * x = x)
    B128_MUL @36, @20, @24

    ;; Check if third word is correct
    XORI @40, @38, #5
    BNZ bf_fail, @40

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

#[framesize(0x4e)]
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
    LDI.W @7, #0     ;; Initialize a working value

    ;; Set up a value with all bits set (equivalent to -1 in two's complement)
    XORI @8, @7, #65535     ;; Low 16 bits are all 1s
    SLLI @9, @8, #16        ;; Shift left by 16
    ORI @10, @9, #65535     ;; OR with 65535 to set all 32 bits to 1

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: ADD / ADDI
    ;; 
    ;; FORMAT: 
    ;;   ADD dst, src1, src2   (VROM variant)
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
    SUB @14, @11, @4      ;; 49 - 7 = 42
    XORI @15, @14, #42   ;; Check result
    BNZ int_fail, @15

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: AND / ANDI
    ;; 
    ;; FORMAT: 
    ;;   AND dst, src1, src2   (VROM variant)
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
    ;;   OR dst, src1, src2   (VROM variant)
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
    ;;   Perform shift operations. The effective shift amount is
    ;;   determined by the last 5 bits of the shift operand.
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
    
    ;; Test SRAI with negative value (-1)
    SRAI @30, @10, #2    ;; -1 >> 2 = -1 (sign bit preserved)
    XOR @31, @30, @10    ;; Check result (should match original -1 value)
    BNZ int_fail, @31
    
    ;; Test VROM-based shift variants
    SLL @32, @4, @5      ;; 7 << 2 = 28
    XORI @33, @32, #28   ;; Check result
    BNZ int_fail, @33
    
    SRL @34, @3, @5      ;; 42 >> 2 = 10
    XORI @35, @34, #10   ;; Check result
    BNZ int_fail, @35
    
    ;; Simple test for SRA with small positive value
    SRA @36, @4, @5      ;; 7 >> 2 = 1
    XORI @37, @36, #1    ;; Check result
    BNZ int_fail, @37
    
    ;; Test SRA with negative value (-1)
    SRA @38, @10, @5     ;; -1 >> 2 = -1 (sign bit preserved)
    XOR @39, @38, @10    ;; Check result (should match original -1 value)
    BNZ int_fail, @39

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: MUL / MULI
    ;; 
    ;; FORMAT: 
    ;;   MUL dst, src1, src2    (Signed multiplication)
    ;;   MULI dst, src, imm     (Immediate multiplication)
    ;; 
    ;; DESCRIPTION:
    ;;   Multiply integer values (signed).
    ;;   Note: Results in 64-bit output stored across two 32-bit slots.
    ;;   The destination slot must be aligned to an even address.
    ;;
    ;; EFFECT: 
    ;;   fp[dst:dst+1] = fp[src1] * fp[src2]  (64-bit result)
    ;;   fp[dst:dst+1] = fp[src] * imm        (64-bit result)
    ;; ------------------------------------------------------------
    ;; Using even slots for destination to ensure proper alignment
    MUL @40, @3, @4      ;; 42 * 7 = 294 (lower 32 bits in @40, upper 32 bits in @41)
    MULI @42, @3, #7     ;; 42 * 7 = 294 (lower 32 bits in @42, upper 32 bits in @43)
    
    ;; Verify both give same result (checking lower 32 bits, upper bits should be 0)
    XOR @44, @40, @42    ;; Compare low 32 bits of MUL and MULI results
    BNZ int_fail, @44
    
    ;; Verify the actual value of lower 32 bits
    XORI @45, @40, #294  ;; Check the result value
    BNZ int_fail, @45

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
    ;; Test 1: Simple positive case
    MULU @46, @3, @4     ;; 42u * 7u = 294u (lower 32 bits in @46, upper 32 bits in @47)
    XORI @48, @46, #294  ;; Check lower 32 bits match expected value
    BNZ int_fail, @48
    XORI @49, @47, #0    ;; Upper 32 bits should be 0
    BNZ int_fail, @49
    
    ;; Test 2: Using -1 (all bits set) * 2
    LDI.W @50, #2
    
    ;; Create expected lower 32 bits value (-2)
    ADD @51, @10, @10    ;; -1 + (-1) = -2 in two's complement
    
    MULU @52, @10, @50   ;; 0xFFFFFFFF * 2 = 0x1FFFFFFFE
    
    ;; Compare results
    XOR @54, @52, @51    ;; Lower bits should match -2
    BNZ int_fail, @54
    XORI @55, @53, #1    ;; Upper bits should be 1
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
    ;; Test 1: Simple positive case
    MULSU @56, @3, @4    ;; 42 * 7u = 294 (lower 32 bits in @56, upper 32 bits in @57)
    XORI @58, @56, #294  ;; Check lower 32 bits
    BNZ int_fail, @58
    XORI @59, @57, #0    ;; Upper 32 bits should be 0
    BNZ int_fail, @59
    
    ;; Test 2: Negative case (-1 * 2u)
    MULSU @60, @10, @50  ;; -1 * 2u = -2
    
    ;; We already computed -2 in @51, reuse it
    XOR @62, @60, @51    ;; Lower bits should match -2
    BNZ int_fail, @62
    
    ;; Upper 32 bits should be all 1s for negative number
    XOR @63, @61, @10    ;; Compare with all 1s
    BNZ int_fail, @63

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
    ;; Test SLT (signed comparison)
    SLT @64, @4, @3      ;; 7 < 42? = 1 (true)
    XORI @65, @64, #1    ;; Check result
    BNZ int_fail, @65
    
    ;; Test SLTI (signed immediate comparison)
    SLTI @66, @4, #42    ;; 7 < 42? = 1 (true)
    XORI @67, @66, #1    ;; Check result
    BNZ int_fail, @67
    
    ;; Test SLT with negative value
    SLT @68, @10, @3     ;; -1 < 42? = 1 (true)
    XORI @69, @68, #1    ;; Check result
    BNZ int_fail, @69
    
    ;; Test SLTU (unsigned comparison)
    SLTU @70, @4, @3     ;; 7 <u 42? = 1 (true)
    XORI @71, @70, #1    ;; Check result
    BNZ int_fail, @71
    
    ;; Test SLTIU (unsigned immediate comparison)
    SLTIU @72, @4, #42   ;; 7 <u 42? = 1 (true)
    XORI @73, @72, #1    ;; Check result
    BNZ int_fail, @73
    
    ;; Test signed vs unsigned difference
    SLTU @74, @3, @10    ;; 42 <u 0xFFFFFFFF? = 1 (true in unsigned)
    SLT @75, @3, @10     ;; 42 < -1? = 0 (false in signed)
    XORI @76, @74, #1    ;; SLTU should be 1
    BNZ int_fail, @76
    XORI @77, @75, #0    ;; SLT should be 0
    BNZ int_fail, @77

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: SLE / SLEI / SLEU / SLEIU
    ;; 
    ;; FORMAT:
    ;;   SLE dst, src1, src2  (Set if Less Than Or Equal, signed)
    ;;   SLEI dst, src, imm   (Set if Less Than Or Equal Immediate, signed)
    ;;   SLEU dst, src1, src2  (Set if Less Than Or Equal, unsigned)
    ;;   SLEIU dst, src, imm   (Set if Less Than Or Equal Immediate, unsigned)
    ;; 
    ;; DESCRIPTION:
    ;;   Set destination to 1 if first value is less than second,
    ;;   otherwise set to 0.
    ;;
    ;; EFFECT: 
    ;;   fp[dst] = (fp[src1] <= fp[src2]) ? 1 : 0
    ;;   fp[dst] = (fp[src] <= imm) ? 1 : 0
    ;; ------------------------------------------------------------
    ;; Test SLE (signed comparison)
    SLE @64, @4, @3      ;; 7 <= 42? = 1 (true)
    XORI @65, @64, #1    ;; Check result
    BNZ int_fail, @65
    
    ;; Test SLEI (signed immediate comparison)
    SLEI @66, @4, #42    ;; 7 <= 42? = 1 (true)
    XORI @67, @66, #1    ;; Check result
    BNZ int_fail, @67
    
    ;; Test SLE with negative value
    SLE @68, @10, @3     ;; -1 <= 42? = 1 (true)
    XORI @69, @68, #1    ;; Check result
    BNZ int_fail, @69
    
    ;; Test SLEU (unsigned comparison)
    SLEU @70, @4, @3     ;; 7 <=u 42? = 1 (true)
    XORI @71, @70, #1    ;; Check result
    BNZ int_fail, @71
    
    ;; Test SLEIU (unsigned immediate comparison)
    SLEIU @72, @4, #42   ;; 7 <=u 42? = 1 (true)
    XORI @73, @72, #1    ;; Check result
    BNZ int_fail, @73
    
    ;; Test signed vs unsigned difference
    SLEU @74, @3, @10    ;; 42 <=u 0xFFFFFFFF? = 1 (true in unsigned)
    SLE @75, @3, @10     ;; 42 <= -1? = 0 (false in signed)
    XORI @76, @74, #1    ;; SLEU should be 1
    BNZ int_fail, @76
    XORI @77, @75, #0    ;; SLE should be 0
    BNZ int_fail, @77

    LDI.W @2, #0         ;; Set success flag (0 = success)
    RET
int_fail:
    LDI.W @2, #1         ;; Set failure flag (1 = failure)
    RET

;; ============================================================================
;; MOVE OPERATIONS
;; ============================================================================
;; These instructions move data between different VROM locations.
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
    ;;   Load a 32-bit immediate value into a destination slot.
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

#[framesize(0x11)]
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
    ;;   Branch to target address if condition value is not zero.
    ;;
    ;; EFFECT: 
    ;;   if fp[cond] != 0 then PC = target
    ;;   else PC = PC * G (next instruction)
    ;; ------------------------------------------------------------
    ;; Test 1: Branch NOT taken (condition is zero)
    LDI.W @3, #0           ;; Set condition to 0
    BNZ bnz_path_1, @3     ;; Should NOT branch since @3 is 0
    
    ;; When branch not taken (correct), record that in @4
    LDI.W @4, #1           ;; Record that branch was not taken (success)
    J bnz_test_2_start
    
bnz_path_1:
    ;; If we get here, branch was incorrectly taken
    ;; We'll leave @4 unset, which indicates failure
    LDI.W @5, #0           ;; Dummy instruction to avoid adjacent labels
    
bnz_test_2_start:
    ;; Check if @4 was set to 1 (indicates branch was not taken)
    XORI @5, @4, #1        ;; @4 should be 1 if branch was not taken
    BNZ branch_fail, @5
    
    ;; Test 2: Branch taken (condition is non-zero)
    LDI.W @6, #42          ;; Set non-zero condition
    BNZ bnz_path_2, @6     ;; Should branch since @6 is non-zero
    
    ;; If we get here, branch was not taken (failure)
    J branch_fail
    
bnz_path_2:
    ;; Branch was correctly taken, continue testing
    LDI.W @7, #1           ;; Mark that branch was taken correctly

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
    ;; Test simple jump
    J jump_target           ;; Should always jump
    
    ;; We should NOT reach here
    J branch_fail
    
jump_target:
    ;; We SHOULD reach here after the jump
    LDI.W @8, #1           ;; Mark that we reached the jump target
    
    ;; ------------------------------------------------------------
    ;; INSTRUCTION: J (Jump to VROM address)
    ;; 
    ;; FORMAT: J @slot
    ;; 
    ;; DESCRIPTION:
    ;;   Jump to a target address stored in VROM.
    ;;
    ;; EFFECT: PC = fp[slot]
    ;; ------------------------------------------------------------
    ;; Load the destination address (PC = 30G from jumpv_destination)
    LDI.W @9, #815359857  ;; Field element value for 30G
    
    ;; Jump to that address
    J @9                    ;; Jump to the address in @9
    
    ;; We should NOT reach here
    J branch_fail
    
branch_fail:
    LDI.W @2, #1         ;; Set failure flag (1 = failure)
    RET

;; ============================================================================
;; FUNCTION CALL OPERATIONS
;; ============================================================================
;; Tests for function call instructions, which save and restore execution context.
;; ============================================================================

#[framesize(0xb)]
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
    MVV.W @3[2], @4    ;; Set up a slot to receive the return value
    CALLI test_simple_fn, @3
    
    ;; Check the return value from the function
    XORI @5, @4, #42   ;; Function should return 42
    BNZ call_fail, @5

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: CALLV (Call Variable)
    ;; 
    ;; FORMAT: CALLV target, next_fp
    ;; 
    ;; DESCRIPTION:
    ;;   Function call to a target address stored in a VROM slot.
    ;;   Sets up a new frame with the return address and old FP.
    ;;
    ;; EFFECT: 
    ;;   [FP[next_fp] + 0] = PC * G (return address)
    ;;   [FP[next_fp] + 1] = FP (old frame pointer)
    ;;   FP = FP[next_fp]
    ;;   PC = fp[target]
    ;; ------------------------------------------------------------
    ;; For CALLV, we need to use a known PC value
    ;; We placed callv_target_fn at PC = 26G (marked in comments above)
    LDI.W @6, #2118631418  ;; Actual field element value for 26G
    
    ;; Set up a call frame for CALLV
    MVV.W @7[2], @8    ;; Set up a slot to receive the return value
    CALLV @6, @7       ;; Call using the address in @13
    
    ;; Check if we got the special return value from callv_target_fn (123)
    XORI @9, @8, #123  ;; Function should return 123
    BNZ call_fail, @9

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: TAILV (Tail Call Variable)
    ;; 
    ;; FORMAT: TAILV target, next_fp
    ;; 
    ;; DESCRIPTION:
    ;;   Tail call to a target address stored in a VROM slot.
    ;;   Preserves the original return address and frame.
    ;;
    ;; EFFECT: 
    ;;   [FP[next_fp] + 0] = FP[0] (return address)
    ;;   [FP[next_fp] + 1] = FP[1] (old frame pointer)
    ;;   FP = FP[next_fp]
    ;;   PC = fp[target]
    ;; ------------------------------------------------------------
    ;; Test TAILV using a known PC value
    ;; We placed tailv_target_fn at PC = 28G (marked in comments above)
    LDI.W @10, #2552055959  ;; Actual field element value for 28G
    
    ;; Pass the final return value slot to the function
    MVV.W @11[2], @2     ;; Pass the final return value slot
    TAILV @10, @11       ;; Tail call using address in @17
    
    ;; We should not reach here - the tail call should return directly
    ;; to the caller of test_function_calls
    LDI.W @2, #1         ;; Set failure flag
    RET

call_fail:
    LDI.W @2, #1         ;; Set failure flag (1 = failure)
    RET

#[framesize(0x4)]
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
#[framesize(0x3)]
test_simple_fn:
    ;; Slot 0: Return PC (set by CALL instruction)
    ;; Slot 1: Return FP (set by CALL instruction)
    ;; Slot 2: Return value slot
    
    LDI.W @2, #42       ;; Set a test return value
    RET                 ;; Return to caller

;; ============================================================================
;; FP INSTRUCTION
;;
;; FORMAT:
;;   FP dst, imm
;;
;; DESCRIPTION:
;;   Set destination to FP + imm.
;;
;; EFFECT:
;;   fp[dst] = fp ^ imm
;; ============================================================================
#[framesize(0x5)]
test_fp:
    FP @3, #1       ;; Set to FP[1] = 24 + 1
    XORI @4, @3, #25
    BNZ fp_fail, @4
    LDI.W @2, #0    ;; Set success flag (0 = success)
    RET
fp_fail:
    LDI.W @2, #1    ;; Set failure flag (1 = failure)
    RET

