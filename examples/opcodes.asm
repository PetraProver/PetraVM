;; ============================================================================
;; zCrayVM INSTRUCTION SET SPECIFICATION AND TEST SUITE
;; ============================================================================
;; This document serves as both a specification for the zCrayVM instruction set
;; and a test suite to verify correct implementation of each instruction.
;;
;; INSTRUCTION CATEGORIES:
;; 1. Binary Field Operations - Field-specific arithmetic (B32_ADD, B32_MUL, etc.)
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
;; ============================================================================

#[framesize(0x20)]
_start:
    ;; Initialize a register to count passed test groups
    LDI.W @20, #0    ;; success counter

    ;; Test values for integer operations
    LDI.W @2, #42    ;; test value A
    LDI.W @3, #7     ;; test value B
    LDI.W @4, #16    ;; shift amount

    ;; Call each test group (each returns success flag in @3)
    CALLI test_binary_field, @5
    ADDI @20, @20, @3

    CALLI test_integer_ops, @5
    ADDI @20, @20, @3

    CALLI test_move_ops, @5
    ADDI @20, @20, @3

    CALLI test_jumps_branches, @5
    ADDI @20, @20, @3

    CALLI test_jump_ops, @5
    ADDI @20, @20, @3

    ;; Expecting 5 successful test groups
    XORI @21, @20, #5
    BNZ test_failed, @21
test_passed:
    LDI.W @1, #1    ;; overall success flag
    RET

test_failed:
    LDI.W @1, #0    ;; overall failure flag
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
    ;; ------------------------------------------------------------
    ;; INSTRUCTION: XOR (Bitwise Exclusive OR)
    ;; 
    ;; FORMAT: XOR dst, src1, src2
    ;; 
    ;; DESCRIPTION:
    ;;   Perform bitwise XOR between two 32-bit values.
    ;;   This is also an alias for B32_ADD in binary field arithmetic.
    ;;
    ;; EFFECT: fp[dst] = fp[src1] ^ fp[src2]
    ;; 
    ;; ALIGNMENT: dst, src1, and src2 must be 4-byte aligned
    ;; ------------------------------------------------------------
test_binary_field:
    ;; ------------------------------------------------------------
    ;; INSTRUCTION: B32_ADD (alias for XOR)
    ;; 
    ;; FORMAT: B32_ADD dst, src1, src2
    ;; 
    ;; DESCRIPTION: 
    ;;   Add two 32-bit binary field elements.
    ;;   In binary fields, addition is equivalent to bitwise XOR.
    ;;
    ;; EFFECT: fp[dst] = fp[src1] ⊕ fp[src2]
    ;; 
    ;; ALIGNMENT: dst, src1, and src2 must be 4-byte aligned
    ;; ------------------------------------------------------------
    XOR @10, @2, @3      ;; 42 XOR 7 = 45
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
    ;; 
    ;; ALIGNMENT: dst, src1, and src2 must be 4-byte aligned
    ;; ------------------------------------------------------------
    B32_MUL @12, @2, @3
    B32_MULI @13, @2, #7   ;; Immediate variant
    XOR @14, @12, @13      ;; Should produce identical results
    BNZ bf_fail, @14

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
    ;; 
    ;; ALIGNMENT: dst, src1, and src2 must be 16-byte aligned
    ;; ------------------------------------------------------------
    LDI.W @15, #1
    LDI.W @16, #2
    MVV.W @6[0], @15
    MVV.W @7[0], @16
    B128_ADD @8, @6, @7
    MVV.W @17, @8[0]
    XORI @18, @17, #3    ;; expecting 1 XOR 2 = 3 (for binary addition)
    BNZ bf_fail, @18

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
    ;; 
    ;; ALIGNMENT: dst, src1, and src2 must be 16-byte aligned
    ;; ------------------------------------------------------------
    ;; Set up 128-bit value A = 1 and B = 3 (all other words are zero)
    LDI.W @30, #1
    LDI.W @31, #0
    LDI.W @32, #0
    LDI.W @33, #0
    LDI.W @34, #3
    LDI.W @35, #0
    LDI.W @36, #0
    LDI.W @37, #0
    MVV.W @10[0], @30
    MVV.W @10[1], @31
    MVV.W @10[2], @32
    MVV.W @10[3], @33
    MVV.W @11[0], @34
    MVV.W @11[1], @35
    MVV.W @11[2], @36
    MVV.W @11[3], @37
    B128_MUL @12, @10, @11
    MVV.W @38, @12[0]
    XORI @39, @38, #3    ;; if 1 is multiplicative identity then 1 * 3 = 3
    BNZ bf_fail, @39

    LDI.W @3, #1
    RET
bf_fail:
    LDI.W @3, #0
    RET

;; ============================================================================
;; INTEGER OPERATIONS
;; ============================================================================
;; These instructions perform operations on 32-bit integer values.
;; Includes arithmetic, logical, and comparison operations.
;; ============================================================================

#[framesize(0x40)]
test_integer_ops:
    ;; ------------------------------------------------------------
    ;; INSTRUCTION: ADD
    ;; 
    ;; FORMAT: ADD dst, src1, src2
    ;; 
    ;; DESCRIPTION:
    ;;   Add two 32-bit integer values.
    ;;
    ;; EFFECT: fp[dst] = fp[src1] + fp[src2]
    ;; 
    ;; ALIGNMENT: dst, src1, and src2 must be 4-byte aligned
    ;; ------------------------------------------------------------
    ADD @10, @2, @3      ;; 42 + 7 = 49
    ADDI @11, @2, #7     ;; Immediate variant: 42 + 7 = 49
    XOR @12, @10, @11    ;; 0 if equal
    BNZ int_fail, @12

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: AND
    ;; 
    ;; FORMAT: AND dst, src1, src2
    ;; 
    ;; DESCRIPTION:
    ;;   Bitwise AND of two 32-bit values.
    ;;
    ;; EFFECT: fp[dst] = fp[src1] & fp[src2]
    ;; 
    ;; ALIGNMENT: dst, src1, and src2 must be 4-byte aligned
    ;; ------------------------------------------------------------
    AND @13, @2, @3      ;; 42 & 7 = 2
    ANDI @14, @2, #7     ;; Immediate variant
    XORI @15, @13, #2
    BNZ int_fail, @15

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: OR
    ;; 
    ;; FORMAT: OR dst, src1, src2
    ;; 
    ;; DESCRIPTION:
    ;;   Bitwise OR of two 32-bit values.
    ;;
    ;; EFFECT: fp[dst] = fp[src1] | fp[src2]
    ;; 
    ;; ALIGNMENT: dst, src1, and src2 must be 4-byte aligned
    ;; ------------------------------------------------------------
    OR @16, @2, @3       ;; 42 | 7 = 47
    ORI @17, @2, #7      ;; Immediate variant
    XORI @18, @16, #47
    BNZ int_fail, @18

    ;; ------------------------------------------------------------
    ;; INSTRUCTIONS: SLLI, SRLI, SRAI (Immediate Shifts)
    ;; 
    ;; FORMAT: 
    ;;   SLLI dst, src, imm   (Shift Left Logical Immediate)
    ;;   SRLI dst, src, imm   (Shift Right Logical Immediate)
    ;;   SRAI dst, src, imm   (Shift Right Arithmetic Immediate)
    ;; 
    ;; DESCRIPTION:
    ;;   Perform shift operations with immediate shift amount.
    ;;   SLLI: Logical left shift (fill with zeros)
    ;;   SRLI: Logical right shift (fill with zeros)
    ;;   SRAI: Arithmetic right shift (sign-extend)
    ;;
    ;; EFFECT: 
    ;;   SLLI: fp[dst] = fp[src] << imm
    ;;   SRLI: fp[dst] = fp[src] >> imm  (zero-extended)
    ;;   SRAI: fp[dst] = fp[src] >> imm  (sign-extended)
    ;; 
    ;; ALIGNMENT: dst and src must be 4-byte aligned
    ;; ------------------------------------------------------------
    SLLI @19, @3, #2     ;; 7 << 2 = 28
    XORI @20, @19, #28
    BNZ int_fail, @20

    SRLI @21, @2, #2     ;; 42 >> 2 = 10
    XORI @22, @21, #10
    BNZ int_fail, @22

    LDI.W @23, #4294967168   ;; -128 (in two's complement)
    SRAI @24, @23, #1        ;; -128 >> 1 = -64 (4294967232)
    XORI @25, @24, #4294967232
    BNZ int_fail, @25

    ;; ------------------------------------------------------------
    ;; INSTRUCTIONS: SLL, SRL, SRA (Variable Shifts)
    ;; 
    ;; FORMAT: 
    ;;   SLL dst, src, shift   (Shift Left Logical)
    ;;   SRL dst, src, shift   (Shift Right Logical)
    ;;   SRA dst, src, shift   (Shift Right Arithmetic)
    ;; 
    ;; DESCRIPTION:
    ;;   Perform shift operations with shift amount in register.
    ;;   SLL: Logical left shift (fill with zeros)
    ;;   SRL: Logical right shift (fill with zeros)
    ;;   SRA: Arithmetic right shift (sign-extend)
    ;;
    ;; EFFECT: 
    ;;   SLL: fp[dst] = fp[src] << fp[shift]
    ;;   SRL: fp[dst] = fp[src] >> fp[shift]  (zero-extended)
    ;;   SRA: fp[dst] = fp[src] >> fp[shift]  (sign-extended)
    ;; 
    ;; ALIGNMENT: dst, src, and shift must be 4-byte aligned
    ;; ------------------------------------------------------------
    SLL @26, @3, @4      ;; 7 << 16 = 458752
    XORI @27, @26, #458752
    BNZ int_fail, @27

    SRL @28, @2, @3      ;; 42 >> 7 = 0
    XORI @29, @28, #0
    BNZ int_fail, @29

    SRA @30, @23, @3     ;; -128 >> 7 = -1 (4294967295)
    XORI @31, @30, #4294967295
    BNZ int_fail, @31

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: MULI (Multiply Immediate)
    ;; 
    ;; FORMAT: MULI dst, src, imm
    ;; 
    ;; DESCRIPTION:
    ;;   Multiply a 32-bit integer by an immediate value.
    ;;
    ;; EFFECT: fp[dst] = fp[src] * imm
    ;; 
    ;; ALIGNMENT: dst and src must be 4-byte aligned
    ;; ------------------------------------------------------------
    MULI @32, @2, #3     ;; 42 * 3 = 126
    XORI @33, @32, #126
    BNZ int_fail, @33

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: XORI (XOR Immediate)
    ;; 
    ;; FORMAT: XORI dst, src, imm
    ;; 
    ;; DESCRIPTION:
    ;;   Bitwise XOR with immediate value.
    ;;   Also an alias for B32_ADDI (binary field add immediate).
    ;;
    ;; EFFECT: fp[dst] = fp[src] ^ imm
    ;; 
    ;; ALIGNMENT: dst and src must be 4-byte aligned
    ;; ------------------------------------------------------------
    XORI @51, @2, #7     ;; 42 XOR 7 = 45
    XORI @52, @51, #45   ;; Should be 0 if correct
    BNZ int_fail, @52

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: SUB (Subtract)
    ;; 
    ;; FORMAT: SUB dst, src1, src2
    ;; 
    ;; DESCRIPTION:
    ;;   Subtract the second value from the first.
    ;;
    ;; EFFECT: fp[dst] = fp[src1] - fp[src2]
    ;; 
    ;; ALIGNMENT: dst, src1, and src2 must be 4-byte aligned
    ;; ------------------------------------------------------------
    SUB @34, @2, @3      ;; 42 - 7 = 35
    XORI @35, @34, #35
    BNZ int_fail, @35

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: SLT, SLTI (Set Less Than)
    ;; 
    ;; FORMAT: 
    ;;   SLT dst, src1, src2
    ;;   SLTI dst, src1, imm
    ;; 
    ;; DESCRIPTION:
    ;;   Set destination to 1 if first source is less than second (signed),
    ;;   otherwise set to 0.
    ;;
    ;; EFFECT: 
    ;;   fp[dst] = (fp[src1] < fp[src2]) ? 1 : 0
    ;;   fp[dst] = (fp[src1] < imm) ? 1 : 0
    ;; 
    ;; ALIGNMENT: dst, src1, and src2 must be 4-byte aligned
    ;; ------------------------------------------------------------
    SLT @36, @3, @2      ;; 7 < 42 should yield 1
    LDI.W @37, #1
    XOR @38, @36, @37
    BNZ int_fail, @38

    SLTI @39, @3, #42    ;; 7 < 42 yields 1
    XORI @40, @39, #1
    BNZ int_fail, @40

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: SLTU, SLTIU (Set Less Than Unsigned)
    ;; 
    ;; FORMAT: 
    ;;   SLTU dst, src1, src2
    ;;   SLTIU dst, src1, imm
    ;; 
    ;; DESCRIPTION:
    ;;   Set destination to 1 if first source is less than second (unsigned),
    ;;   otherwise set to 0.
    ;;
    ;; EFFECT: 
    ;;   fp[dst] = (fp[src1] <u fp[src2]) ? 1 : 0
    ;;   fp[dst] = (fp[src1] <u imm) ? 1 : 0
    ;; 
    ;; ALIGNMENT: dst, src1, and src2 must be 4-byte aligned
    ;; ------------------------------------------------------------
    SLTU @41, @3, @2     ;; 7 < 42 yields 1
    XORI @42, @41, #1
    BNZ int_fail, @42

    SLTIU @43, @3, #42   ;; 7 < 42 yields 1
    XORI @44, @43, #1
    BNZ int_fail, @44

    ;; ------------------------------------------------------------
    ;; INSTRUCTIONS: MUL, MULU, MULSU (Multiplication Variants)
    ;; 
    ;; FORMAT: 
    ;;   MUL dst, src1, src2    (Signed multiplication)
    ;;   MULU dst, src1, src2   (Unsigned multiplication)
    ;;   MULSU dst, src1, src2  (Signed*Unsigned multiplication)
    ;; 
    ;; DESCRIPTION:
    ;;   Multiply two 32-bit integers with different signedness.
    ;;   Returns a 64-bit result (unlike RISC-V which has separate 
    ;;   instructions for high and low parts).
    ;;
    ;; EFFECT: 
    ;;   MUL: fp[dst] = fp[src1] * fp[src2]    (signed)
    ;;   MULU: fp[dst] = fp[src1] * fp[src2]   (unsigned)
    ;;   MULSU: fp[dst] = fp[src1] * fp[src2]  (src1 signed, src2 unsigned)
    ;; 
    ;; ALIGNMENT: dst, src1, and src2 must be 4-byte aligned
    ;; ------------------------------------------------------------
    MUL @45, @2, @3      ;; 42 * 7 = 294
    XORI @46, @45, #294
    BNZ int_fail, @46

    MULU @47, @2, @3     ;; 42 * 7 = 294 (unsigned)
    XORI @48, @47, #294
    BNZ int_fail, @48

    MULSU @49, @2, @3    ;; 42 * 7 = 294 (mixed signedness)
    XORI @50, @49, #294
    BNZ int_fail, @50

    LDI.W @3, #1
    RET
int_fail:
    LDI.W @3, #0
    RET

;; ============================================================================
;; MOVE OPERATIONS
;; ============================================================================
;; These instructions move data between registers and memory.
;; Support different data widths and addressing modes.
;; ============================================================================

#[framesize(0x30)]
test_move_ops:
    ;; ------------------------------------------------------------
    ;; INSTRUCTION: LDI.W (Load Immediate Word)
    ;; 
    ;; FORMAT: LDI.W dst, imm
    ;; 
    ;; DESCRIPTION:
    ;;   Load a 32-bit immediate value into a destination.
    ;;
    ;; EFFECT: fp[dst] = imm
    ;; 
    ;; ALIGNMENT: dst must be 4-byte aligned
    ;; ------------------------------------------------------------
    LDI.W @10, #12345
    XORI @11, @10, #12345
    BNZ move_fail, @11

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
    ;; 
    ;; ALIGNMENT: dst must be 4-byte aligned
    ;; ------------------------------------------------------------
    MVI.H @12[0], #255
    XORI @13, @12, #255
    BNZ move_fail, @13

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: MVV.W (Move Value Word)
    ;; 
    ;; FORMAT: MVV.W dst[off], src
    ;; 
    ;; DESCRIPTION:
    ;;   Move a 32-bit value between VROM addresses.
    ;;
    ;; EFFECT: VROM[fp[dst] + off] = fp[src]
    ;; 
    ;; ALIGNMENT: dst and src must be 4-byte aligned
    ;; ------------------------------------------------------------
    LDI.W @14, #9876
    MVV.W @15[0], @14
    XORI @16, @15, #9876
    BNZ move_fail, @16

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: MVV.L (Move Value Long)
    ;; 
    ;; FORMAT: MVV.L dst[off], src
    ;; 
    ;; DESCRIPTION:
    ;;   Move a 128-bit value between VROM addresses.
    ;;
    ;; EFFECT: VROM128[fp[dst] + off] = fp128[src]
    ;; 
    ;; ALIGNMENT: dst and src must be 16-byte aligned
    ;; ------------------------------------------------------------
    LDI.W @17, #1111
    LDI.W @18, #2222
    LDI.W @19, #3333
    LDI.W @20, #4444
    MVV.W @21[0], @17
    MVV.W @21[1], @18
    MVV.W @21[2], @19
    MVV.W @21[3], @20
    MVV.L @25[0], @21
    MVV.W @26, @25[0]
    XORI @27, @26, #1111
    BNZ move_fail, @27
    MVV.W @26, @25[3]
    XORI @27, @26, #4444
    BNZ move_fail, @27

    LDI.W @3, #1
    RET
move_fail:
    LDI.W @3, #0
    RET

;; ============================================================================
;; CONTROL FLOW OPERATIONS
;; ============================================================================
;; These instructions control program execution flow.
;; Includes jumps, branches, and function calls.
;; Note: PC increment uses multiplication in the field's cyclic group.
;; ============================================================================

#[framesize(0x30)]
test_jumps_branches:
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
    ;; 
    ;; ALIGNMENT: cond must be 4-byte aligned
    ;; ------------------------------------------------------------
    LDI.W @10, #0
    BNZ branch_target, @10    ;; Should not branch since @10 is 0.
    ADDI @10, @10, #1         ;; Increment to 1.
    LDI.W @11, #1
    BNZ branch_target, @11    ;; Should branch since @11 is 1.
    JUMPI jump_fail           ;; If branch failed, jump to failure.
branch_target:
    XORI @12, @10, #1         ;; @10 should be 1.
    BNZ jump_fail, @12
    JUMPI jump_target
    JUMPI jump_fail           ;; Should not reach here.
jump_target:
    ;; ------------------------------------------------------------
    ;; INSTRUCTION: TAILI (Tail Call Immediate)
    ;; 
    ;; FORMAT: TAILI target, next_fp
    ;; 
    ;; DESCRIPTION:
    ;;   Tail call to the target address, with a new frame pointer.
    ;;   Preserves the original return address and frame.
    ;;
    ;; EFFECT: 
    ;;   [FP[next_fp] + 0] = FP[0] (return address)
    ;;   [FP[next_fp] + 1] = FP[1] (old frame pointer)
    ;;   FP = FP[next_fp]
    ;;   PC = target
    ;; ------------------------------------------------------------
    TAILI jump_return, @5
fail_return:
    JUMPI jump_fail
jump_return:
    LDI.W @3, #1
    RET
jump_fail:
    LDI.W @3, #0
    RET

;; ============================================================================
;; ADDITIONAL CONTROL FLOW OPERATIONS
;; ============================================================================

#[framesize(0x30)]
test_jump_ops:
    ;; ------------------------------------------------------------
    ;; INSTRUCTION: JUMPI (Jump Immediate)
    ;; 
    ;; FORMAT: JUMPI target
    ;; 
    ;; DESCRIPTION:
    ;;   Jump to the target address given as an immediate.
    ;;
    ;; EFFECT: PC = target
    ;; ------------------------------------------------------------
    JUMPI jumpi_target
    LDI.W @3, #0
    RET
jumpi_target:
    ;; ------------------------------------------------------------
    ;; INSTRUCTION: JUMPV (Jump Variable)
    ;; 
    ;; FORMAT: JUMPV target, unused
    ;; 
    ;; DESCRIPTION:
    ;;   Indirect jump to the target address read from VROM.
    ;;
    ;; EFFECT: PC = fp[target]
    ;; 
    ;; ALIGNMENT: target must be 4-byte aligned
    ;; ------------------------------------------------------------
    LDI.W @10, jumpv_target    ;; load immediate address for jump.
    JUMPV @10, @0
    LDI.W @3, #0
    RET
jumpv_target:
    ;; ------------------------------------------------------------
    ;; INSTRUCTION: CALLI (Call Immediate)
    ;; 
    ;; FORMAT: CALLI target, next_fp
    ;; 
    ;; DESCRIPTION:
    ;;   Function call to the target address given by an immediate.
    ;;   Sets up a new frame with the return address and old FP.
    ;;
    ;; EFFECT: 
    ;;   [FP[next_fp] + 0] = PC * G (return address)
    ;;   [FP[next_fp] + 1] = FP (old frame pointer)
    ;;   FP = FP[next_fp]
    ;;   PC = target
    ;; ------------------------------------------------------------
    CALLI calli_function, @20
    XORI @11, @3, #1
    BNZ jumpops_fail, @11

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: CALLV (Call Variable)
    ;; 
    ;; FORMAT: CALLV target, next_fp
    ;; 
    ;; DESCRIPTION:
    ;;   Function call to the indirect target address read from VROM.
    ;;   Sets up a new frame with the return address and old FP.
    ;;
    ;; EFFECT: 
    ;;   [FP[next_fp] + 0] = PC * G (return address)
    ;;   [FP[next_fp] + 1] = FP (old frame pointer)
    ;;   FP = FP[next_fp]
    ;;   PC = FP[target]
    ;; ------------------------------------------------------------
    LDI.W @12, callv_function
    CALLV @12, @20
    XORI @13, @3, #1
    BNZ jumpops_fail, @13

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: TAILV (Tail Call Variable)
    ;; 
    ;; FORMAT: TAILV target, next_fp
    ;; 
    ;; DESCRIPTION:
    ;;   Tail call to the indirect target address read from VROM.
    ;;   Preserves the original return address and frame.
    ;;
    ;; EFFECT: 
    ;;   [FP[next_fp] + 0] = FP[0] (return address)
    ;;   [FP[next_fp] + 1] = FP[1] (old frame pointer)
    ;;   FP = FP[next_fp]
    ;;   PC = FP[target]
    ;; ------------------------------------------------------------
    LDI.W @14, tailv_function
    TAILV @14, @20
    LDI.W @3, #0
    RET
calli_function:
    LDI.W @3, #1
    RET

callv_function:
    LDI.W @3, #1
    RET

tailv_function:
    LDI.W @3, #1
    RET

jumpops_fail:
    LDI.W @3, #0
    RET

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: RET (Return)
    ;; 
    ;; FORMAT: RET
    ;; 
    ;; DESCRIPTION:
    ;;   Return from a function call.
    ;;   Restores the program counter and frame pointer from the current frame.
    ;;
    ;; EFFECT: 
    ;;   PC = FP[0]  (Return to caller address)
    ;;   FP = FP[1]  (Restore caller's frame)
    ;; ------------------------------------------------------------

;; ============================================================================
;; END OF INSTRUCTION SET SPECIFICATION
;; ============================================================================
