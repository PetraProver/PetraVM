#[framesize(0x20)]
_start:
    ;; Initialize flags to track pass/fail results
    LDI.W @20, #0   ;; Register to accumulate success count

    ;; Test values
    LDI.W @2, #42   ;; Test value A
    LDI.W @3, #7    ;; Test value B
    LDI.W @4, #16   ;; Test shift amount

    ;; Call each test group
    CALLI test_binary_field, @5
    ADDI @20, @20, @3  ;; Add test result to success count

    CALLI test_integer_ops, @5
    ADDI @20, @20, @3  ;; Add test result to success count

    CALLI test_move_ops, @5
    ADDI @20, @20, @3  ;; Add test result to success count

    CALLI test_jumps_branches, @5
    ADDI @20, @20, @3  ;; Add test result to success count

    ;; If all tests passed, @20 should equal 4
    XORI @21, @20, #4
    BNZ test_failed, @21

test_passed:
    LDI.W @1, #1    ;; Success flag
    RET

test_failed:
    LDI.W @1, #0    ;; Failure flag
    RET

#[framesize(0x30)]
test_binary_field:
    ;; Test B32_ADD (alias for XOR)
    XOR @10, @2, @3       ;; 42 XOR 7 = 45
    XORI @11, @10, #45    ;; Should be 0 if correct
    BNZ bf_fail, @11

    ;; Test B32_MUL
    B32_MUL @12, @2, @3   ;; Binary field multiplication
    B32_MULI @13, @2, #7G ;; Immediate version
    XOR @14, @12, @13     ;; Should be 0 if both match
    BNZ bf_fail, @14

    ;; Test B128_ADD (if available)
    ;; Set up 128-bit values (would need 4 slots each)
    LDI.W @15, #1         ;; First byte of value 1
    LDI.W @16, #2         ;; First byte of value 2
    MVV.W @6[0], @15
    MVV.W @7[0], @16
    B128_ADD @8, @6, @7   ;; Should be values XORed
    MVV.W @17, @8[0]      ;; Get result
    XORI @18, @17, #3     ;; 1 XOR 2 = 3, should be 0 if correct
    BNZ bf_fail, @18

    ;; Test B128_MUL (if available)
    B128_MUL @9, @6, @7   ;; Binary field 128-bit multiplication

    LDI.W @3, #1  ;; Success
    RET

bf_fail:
    LDI.W @3, #0  ;; Failure
    RET

#[framesize(0x40)]
test_integer_ops:
    ;; Test ADD/ADDI
    ADD @10, @2, @3      ;; 42 + 7 = 49
    ADDI @11, @2, #7     ;; 42 + 7 = 49
    XOR @12, @10, @11    ;; Should be 0 if both match
    BNZ int_fail, @12

    ;; Test AND/ANDI
    AND @13, @2, @3      ;; 42 & 7 = 2
    ANDI @14, @2, #7     ;; 42 & 7 = 2
    XORI @15, @13, #2    ;; Should be 0 if correct
    BNZ int_fail, @15

    ;; Test OR/ORI
    OR @16, @2, @3       ;; 42 | 7 = 47
    ORI @17, @2, #7      ;; 42 | 7 = 47
    XORI @18, @16, #47   ;; Should be 0 if correct
    BNZ int_fail, @18

    ;; Test Shifts
    SLLI @19, @3, #2     ;; 7 << 2 = 28
    XORI @20, @19, #28   ;; Should be 0 if correct
    BNZ int_fail, @20

    SRLI @21, @2, #2     ;; 42 >> 2 = 10
    XORI @22, @21, #10   ;; Should be 0 if correct
    BNZ int_fail, @22

    LDI.W @23, #4294967168  ;; -128 in two's complement
    SRAI @24, @23, #1    ;; -128 >> 1 = -64 (with sign extension)
    XORI @25, @24, #4294967232 ;; -64 in two's complement
    BNZ int_fail, @25

    ;; Test dynamic shifts
    SLL @26, @3, @4      ;; 7 << 16 = 458752
    XORI @27, @26, #458752
    BNZ int_fail, @27

    SRL @28, @2, @3      ;; 42 >> 7 = 0
    XORI @29, @28, #0
    BNZ int_fail, @29

    SRA @30, @23, @3     ;; -128 >> 7 = -1
    XORI @31, @30, #4294967295 ;; -1 in two's complement
    BNZ int_fail, @31

    ;; Test MULI
    MULI @32, @2, #3     ;; 42 * 3 = 126
    XORI @33, @32, #126
    BNZ int_fail, @33

    ;; Include SUB, SLT, SLTU, SLTI, SLTIU, MUL, MULU, MULSU when implemented
    ;; SUB @34, @2, @3     ;; 42 - 7 = 35
    ;; MUL @35, @2, @3     ;; 42 * 7 = 294
    ;; MULU @36, @2, @3
    ;; MULSU @37, @2, @3
    ;; SLT @38, @3, @2     ;; 7 < 42 = 1
    ;; SLTU @39, @3, @2    ;; 7 < 42 = 1
    ;; SLTI @40, @3, #42   ;; 7 < 42 = 1
    ;; SLTIU @41, @3, #42  ;; 7 < 42 = 1

    LDI.W @3, #1  ;; Success
    RET

int_fail:
    LDI.W @3, #0  ;; Failure
    RET

#[framesize(0x30)]
test_move_ops:
    ;; Test LDI.W
    LDI.W @10, #12345
    XORI @11, @10, #12345
    BNZ move_fail, @11

    ;; Test MVI.H (move half-word)
    MVI.H @12[0], #255
    XORI @13, @12, #255
    BNZ move_fail, @13

    ;; Test MVV.W
    LDI.W @14, #9876
    MVV.W @15[0], @14
    XORI @16, @15, #9876
    BNZ move_fail, @16

    ;; Test MVV.L (128-bit move)
    LDI.W @17, #1111
    LDI.W @18, #2222
    LDI.W @19, #3333
    LDI.W @20, #4444
    
    ;; Set up a 128-bit value at @21
    MVV.W @21[0], @17
    MVV.W @21[1], @18
    MVV.W @21[2], @19
    MVV.W @21[3], @20
    
    ;; Copy 128-bit value from @21 to @25
    MVV.L @25[0], @21
    
    ;; Verify parts
    MVV.W @26, @25[0]
    XORI @27, @26, #1111
    BNZ move_fail, @27
    
    MVV.W @26, @25[3]
    XORI @27, @26, #4444
    BNZ move_fail, @27

    LDI.W @3, #1  ;; Success
    RET

move_fail:
    LDI.W @3, #0  ;; Failure
    RET

#[framesize(0x30)]
test_jumps_branches:
    ;; Initialize counter 
    LDI.W @10, #0
    
    ;; Test BNZ
    BNZ branch_target, @10  ;; Should not branch (counter is 0)
    ADDI @10, @10, #1      ;; Increment counter to 1
    
    LDI.W @11, #1
    BNZ branch_target, @11  ;; Should branch (value is 1)
    
    ;; If we get here, the branch failed
    J jump_fail

branch_target:
    ;; Test counter value - it should be 1 if branch logic works correctly
    XORI @12, @10, #1
    BNZ jump_fail, @12
    
    ;; Test jump
    J jump_target
    J jump_fail  ;; Should not reach here

jump_target:
    ;; Test TAILI (tail call)
    TAILI jump_return, @5

fail_return:
    J jump_fail

jump_return:
    ;; Test RET
    LDI.W @3, #1  ;; Success
    RET

jump_fail:
    LDI.W @3, #0  ;; Failure
    RET

;; Include JUMPI, JUMPV, CALLI, CALLV, TAILV when implemented
;; JUMPI jump_target
;; JUMPV @reg_with_target
;; CALLI another_function, @frame_pointer
;; CALLV @reg_with_function, @frame_pointer
;; TAILV @reg_with_function, @frame_pointer
