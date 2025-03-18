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

#[framesize(0x20)]
_start: 
    ;; Call each test group and collect results in separate registers
    CALLI test_binary_field, @3
    MVV.W @5, @3[2]  ;; Get success flag (0 or 1)

    CALLI test_integer_ops, @4
    MVV.W @6, @4[2]  ;; Get success flag

    CALLI test_move_ops, @7
    MVV.W @8, @7[2]  ;; Get success flag

    CALLI test_jumps_branches, @9
    MVV.W @10, @9[2]  ;; Get success flag

    CALLI test_jump_ops, @11
    MVV.W @12, @11[2]  ;; Get success flag
    
    ;; Compute total successes in a sequence of new registers
    ADD @13, @5, @6       ;; Binary field + Integer ops
    ADD @14, @13, @8      ;; + Move ops
    ADD @15, @14, @10     ;; + Jumps & branches
    ADD @16, @15, @12     ;; + Jump ops - holds total success count

    ;; Expecting 5 successful test groups
    XORI @17, @16, #5
    BNZ test_failed, @17
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
    B32_MUL @12, @3, @4
    B32_MULI @13, @3, #7
    XOR @14, @12, @13
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
    ;; ------------------------------------------------------------
    LDI.W @15, #1
    LDI.W @16, #2
    B128_ADD @17, @15, @16
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
    ;; ------------------------------------------------------------
    ;; Set up 128-bit values by initializing words
    LDI.W @20, #1
    LDI.W @24, #3
    LDI.W @21, #0
    LDI.W @22, #0
    LDI.W @23, #0
    LDI.W @25, #0
    LDI.W @26, #0
    LDI.W @27, #0
    ;; Perform 128-bit multiplication
    B128_MUL @28, @20, @24
    XORI @29, @28, #3    ;; if 1 is multiplicative identity then 1 * 3 = 3
    BNZ bf_fail, @29

    LDI.W @2, #1         ;; Set success flag
    RET
bf_fail:
    LDI.W @2, #0         ;; Set failure flag
    RET

;; ============================================================================
;; INTEGER OPERATIONS
;; ============================================================================
;; These instructions perform operations on 32-bit integer values.
;; Includes arithmetic, logical, and comparison operations.
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
    LDI.W @5, #16    ;; shift amount

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
    XOR @12, @10, @11    ;; 0 if equal
    BNZ int_fail, @12

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
    AND @13, @3, @4
    ANDI @14, @3, #7
    XORI @15, @13, #2
    BNZ int_fail, @15

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
    OR @16, @3, @4
    ORI @17, @3, #7
    XORI @18, @16, #47
    BNZ int_fail, @18

    ;; ------------------------------------------------------------
    ;; INSTRUCTIONS: Shift Operations
    ;; 
    ;; FORMAT: 
    ;;   SLLI dst, src, imm   (Shift Left Logical Immediate)
    ;;   SRLI dst, src, imm   (Shift Right Logical Immediate)
    ;;   SRAI dst, src, imm   (Shift Right Arithmetic Immediate)
    ;;   SLL dst, src, shift  (Shift Left Logical - register)
    ;;   SRL dst, src, shift  (Shift Right Logical - register)
    ;;   SRA dst, src, shift  (Shift Right Arithmetic - register)
    ;; 
    ;; DESCRIPTION:
    ;;   Perform shift operations with immediate or register shift amount.
    ;;   - Logical shifts fill with zeros
    ;;   - Arithmetic right shifts preserve the sign bit
    ;;
    ;; EFFECT: 
    ;;   SLLI/SLL: fp[dst] = fp[src] << imm/fp[shift]
    ;;   SRLI/SRL: fp[dst] = fp[src] >> imm/fp[shift] (zero-extended)
    ;;   SRAI/SRA: fp[dst] = fp[src] >> imm/fp[shift] (sign-extended)
    ;; ------------------------------------------------------------
    SLLI @19, @4, #2        ;; 7 << 2 = 28
    XORI @20, @19, #28
    BNZ int_fail, @20

    SRLI @21, @3, #2        ;; 42 >> 2 = 10
    XORI @22, @21, #10
    BNZ int_fail, @22

    LDI.W @23, #4294967168   ;; -128 (in two's complement)
    SRAI @24, @23, #1        ;; -128 >> 1 = -64 (4294967232)
    XORI @25, @24, #4294967232
    BNZ int_fail, @25

    SLL @26, @4, @5         ;; 7 << 16 = 458752
    XORI @27, @26, #458752
    BNZ int_fail, @27

    SRL @28, @3, @4         ;; 42 >> 7 = 0
    XORI @29, @28, #0
    BNZ int_fail, @29

    SRA @30, @23, @4        ;; -128 >> 7 = -1 (4294967295)
    XORI @31, @30, #4294967295
    BNZ int_fail, @31

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: MULI (Integer Multiplication Immediate)
    ;; 
    ;; FORMAT: MULI dst, src, imm
    ;; 
    ;; DESCRIPTION:
    ;;   Multiply source value by an immediate value.
    ;;
    ;; EFFECT: fp[dst] = fp[src] * imm
    ;; ------------------------------------------------------------
    MULI @32, @3, #3
    XORI @33, @32, #126
    BNZ int_fail, @33

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
    SUB @36, @3, @4
    XORI @37, @36, #35
    BNZ int_fail, @37

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: SLT / SLTI / SLTU / SLTIU (Set Less Than)
    ;; 
    ;; FORMAT: 
    ;;   SLT dst, src1, src2     (Set if Less Than, signed)
    ;;   SLTI dst, src1, imm     (Set if Less Than Immediate, signed)
    ;;   SLTU dst, src1, src2    (Set if Less Than, unsigned)
    ;;   SLTIU dst, src1, imm    (Set if Less Than Immediate, unsigned)
    ;; 
    ;; DESCRIPTION:
    ;;   Set destination to 1 if first source is less than second,
    ;;   otherwise set to 0. Handles both signed and unsigned variants.
    ;;
    ;; EFFECT: 
    ;;   SLT/SLTI:   fp[dst] = (fp[src1] < src2/imm) ? 1 : 0   (signed)
    ;;   SLTU/SLTIU: fp[dst] = (fp[src1] < src2/imm) ? 1 : 0   (unsigned)
    ;; ------------------------------------------------------------
    SLT @38, @4, @3
    LDI.W @39, #1
    XOR @6, @38, @39
    BNZ int_fail, @6

    SLTI @7, @4, #42
    XORI @8, @7, #1
    BNZ int_fail, @8

    SLTU @9, @4, @3
    XORI @10, @9, #1
    BNZ int_fail, @10

    SLTIU @11, @4, #42
    XORI @12, @11, #1
    BNZ int_fail, @12

    ;; ------------------------------------------------------------
    ;; INSTRUCTIONS: MUL / MULU / MULSU (Multiplication)
    ;; 
    ;; FORMAT: 
    ;;   MUL dst, src1, src2    (Signed multiplication)
    ;;   MULU dst, src1, src2   (Unsigned multiplication)
    ;;   MULSU dst, src1, src2  (Signed*Unsigned multiplication)
    ;; 
    ;; DESCRIPTION:
    ;;   Multiply integer values with different signedness.
    ;;   Returns a 64-bit result (unlike RISC-V which has separate 
    ;;   instructions for high and low parts).
    ;;
    ;; EFFECT: 
    ;;   MUL:   fp[dst] = fp[src1] * fp[src2]    (signed)
    ;;   MULU:  fp[dst] = fp[src1] * fp[src2]    (unsigned)
    ;;   MULSU: fp[dst] = fp[src1] * fp[src2]    (src1 signed, src2 unsigned)
    ;; ------------------------------------------------------------
    MUL @13, @3, @4
    XORI @14, @13, #294
    BNZ int_fail, @14

    MULU @15, @3, @4
    XORI @16, @15, #294
    BNZ int_fail, @16

    MULSU @17, @3, @4
    XORI @18, @17, #294
    BNZ int_fail, @18

    LDI.W @2, #1            ;; Set success flag
    RET
int_fail:
    LDI.W @2, #0            ;; Set failure flag
    RET

;; ============================================================================
;; MOVE OPERATIONS
;; ============================================================================
;; These instructions move data between registers and memory.
;; Support different data widths and addressing modes.
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

    LDI.W @2, #1            ;; Set success flag
    RET
move_fail:
    LDI.W @2, #0            ;; Set failure flag
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
    LDI.W @10, #0
    BNZ branch_target, @10    ;; Should not branch since @10 is 0.
    ADDI @10, @10, #1         ;; Increment to 1.
    LDI.W @11, #1
    BNZ branch_target, @11    ;; Should branch since @11 is 1.
    JUMPI jump_fail           ;; If branch failed, jump to failure.
branch_target:
    XORI @12, @10, #1         ;; @10 should be 1.
    BNZ jump_fail, @12
    
    ;; ------------------------------------------------------------
    ;; INSTRUCTION: JUMPI (Jump Immediate)
    ;; 
    ;; FORMAT: JUMPI target
    ;; 
    ;; DESCRIPTION:
    ;;   Jump to a target address unconditionally.
    ;;
    ;; EFFECT: PC = target
    ;; ------------------------------------------------------------
    JUMPI jump_target
    JUMPI jump_fail           ;; Should not reach here.
jump_target:
    ;; ------------------------------------------------------------
    ;; INSTRUCTION: TAILI (Tail Call Immediate)
    ;; 
    ;; FORMAT: TAILI target, next_fp
    ;; 
    ;; DESCRIPTION:
    ;;   Tail call to a target address, with a new frame pointer.
    ;;   Preserves the original return address and frame.
    ;;
    ;; EFFECT: 
    ;;   [FP[next_fp] + 0] = FP[0] (return address)
    ;;   [FP[next_fp] + 1] = FP[1] (old frame pointer)
    ;;   FP = FP[next_fp]
    ;;   PC = target
    ;; ------------------------------------------------------------
    CALLI jump_return, @21
fail_return:
    JUMPI jump_fail
jump_return:
    LDI.W @2, #1              ;; Set success flag
    RET
jump_fail:
    LDI.W @2, #0              ;; Set failure flag
    RET

#[framesize(0x30)]
test_jump_ops:
    ;; Frame slots:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Return value (success flag)
    ;; Slots 3+: Local variables for tests and frames

    ;; ------------------------------------------------------------
    ;; INSTRUCTION: JUMPI (Jump Immediate)
    ;; 
    ;; FORMAT: JUMPI target
    ;; 
    ;; DESCRIPTION:
    ;;   Jump to a target address unconditionally.
    ;;
    ;; EFFECT: PC = target
    ;; ------------------------------------------------------------
    JUMPI jumpi_target
    LDI.W @2, #0
    RET
jumpi_target:
    ;; ------------------------------------------------------------
    ;; INSTRUCTION: JUMPV (Jump Variable)
    ;; 
    ;; FORMAT: JUMPV target, unused
    ;; 
    ;; DESCRIPTION:
    ;;   Jump to a target address stored in a register.
    ;;
    ;; EFFECT: PC = fp[target]
    ;; ------------------------------------------------------------
    LDI.W @10, jumpv_target    ;; load immediate address for jump.
    JUMPV @10, @0
    LDI.W @2, #0
    RET
jumpv_target:
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
    CALLI calli_function, @20
    MVV.W @11, @20[2]          ;; Get return value
    XORI @12, @11, #1
    BNZ jumpops_fail, @12

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
    LDI.W @13, callv_function
    CALLV @13, @20
    MVV.W @14, @20[2]          ;; Get return value
    XORI @15, @14, #1
    BNZ jumpops_fail, @15

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
    LDI.W @16, tailv_function
    TAILV @16, @20
    LDI.W @2, #0               ;; Should not reach here
    RET

#[framesize(0x3)]
calli_function:
    ;; Frame slots:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Return value
    
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
    LDI.W @2, #1               ;; Set success value
    RET

#[framesize(0x3)]
callv_function:
    ;; Frame slots:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Return value
    
    LDI.W @2, #1               ;; Set success value
    RET

#[framesize(0x3)]
tailv_function:
    ;; Frame slots:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Return value
    
    LDI.W @2, #1               ;; Set success value
    RET

jumpops_fail:
    LDI.W @2, #0               ;; Set failure flag
    RET

;; ============================================================================
;; END OF INSTRUCTION SET SPECIFICATION
;; ============================================================================
