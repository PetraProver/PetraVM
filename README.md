# zCrayVM
A verifiable supercomputer

## Overview
zCrayVM is a new virtual machine (zkVM) designed specifically for efficient execution within Zero-Knowledge (ZK) proof systems, leveraging the [Binius](https://www.binius.xyz/) SNARK scheme's strengths. The primary goals are to improve performance of recursive proof verification and WebAssembly execution within ZK environments.

## Instruction Set
zCrayVM's full instruction set is divided into five categories—Binary field, Arithmetic & Logic, Memory, Control Flow, and Function Calls—and the prover's current support is noted below.

### Prover Support (Work in Progress)

#### Binary Field Operations
- [x] `B32_MUL` - 32-bit binary field multiplication
- [x] `B32_MULI` - 32-bit binary field multiplication with immediate
- [x] `B128_ADD` - 128-bit binary field addition
- [x] `B128_MUL` - 128-bit binary field multiplication

#### Arithmetic Operations
- [x] `ADD` - Integer addition
- [ ] `ADDI` - Integer addition with immediate
- [x] `SUB` - Integer subtraction
- [ ] `SUBI` - Integer subtraction with immediate
- [ ] `MUL` - Integer multiplication (signed)
- [ ] `MULI` - Integer multiplication with immediate (signed)
- [ ] `MULU` - Integer multiplication (unsigned)
- [ ] `MULSU` - Integer multiplication (signed × unsigned)

#### Logic Operations
- [x] `AND` - Bitwise AND
- [x] `ANDI` - Bitwise AND with immediate
- [x] `OR` - Bitwise OR
- [x] `ORI` - Bitwise OR with immediate
- [x] `XOR` - Bitwise XOR
- [x] `XORI` - Bitwise XOR with immediate

#### Shift Operations
- [x] `SLL` - Shift left logical
- [x] `SLLI` - Shift left logical with immediate
- [x] `SRL` - Shift right logical
- [x] `SRLI` - Shift right logical with immediate
- [ ] `SRA` - Shift right arithmetic
- [ ] `SRAI` - Shift right arithmetic with immediate

#### Comparison Operations
- [ ] `SLT` - Set if less than (signed)
- [ ] `SLTI` - Set if less than immediate (signed)
- [ ] `SLTU` - Set if less than (unsigned)
- [ ] `SLTIU` - Set if less than immediate (unsigned)
- [ ] `SLE` - Set if less than or equal (signed)
- [ ] `SLEI` - Set if less than or equal immediate (signed)
- [ ] `SLEU` - Set if less than or equal (unsigned)
- [ ] `SLEIU` - Set if less than or equal immediate (unsigned)

#### Memory Operations
- [x] `LDI` (LDI.W) - Load immediate word
- [x] `MVV.W` - Move variable to variable (word)
- [x] `MVV.L` - Move variable to variable (long, 128-bit)
- [x] `MVI.H` - Move immediate to variable (half-word)

#### Control Flow
- [x] `J` - Jump (to label or variable)
- [x] `JUMPI` - Jump to immediate address
- [x] `JUMPV` - Jump to address in variable
- [x] `BNZ` - Branch if not zero

#### Function Calls
- [x] `CALLI` - Call function at immediate address
- [x] `CALLV` - Call function at address in variable
- [x] `TAILI` - Tail call to immediate address
- [x] `TAILV` - Tail call to address in variable
- [x] `RET` - Return from function

#### Future Memory Extensions
- [ ] `LW`/`SW` - Load/Store word (32-bit)
- [ ] `LB`/`SB` - Load/Store byte
- [ ] `LBU` - Load byte unsigned
- [ ] `LH`/`SH` - Load/Store halfword
- [ ] `LHU` - Load halfword unsigned

Check out our [instruction set test suite](examples/opcodes.asm) for a complete overview of supported instructions and their usage.

## Example Programs
The project includes several example programs that demonstrate the capabilities of zCrayVM:

- [Fibonacci](prover/tests/fibonacci.rs): Prove a Fibonacci number
