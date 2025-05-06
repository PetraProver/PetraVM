# PetraVM
A verifiable supercomputer

## Overview
PetraVM is a new virtual machine (zkVM) designed specifically for efficient execution within Zero-Knowledge (ZK) proof systems, leveraging the [Binius](https://www.binius.xyz/) SNARK scheme's strengths. The primary goals are to improve performance of recursive proof verification and WebAssembly execution within ZK environments.

## Instruction Set
PetraVM's full instruction set is divided into five categories—Binary field, Arithmetic & Logic, Memory, Control Flow, and Function Calls—and the prover's current support is noted below.

### Prover Support (Work in Progress)

> **Note:** In PetraVM, variables refer to addresses in VROM (Value ROM, a write-once memory region). Instructions operate on values at these addresses unless specified as "immediate" operations.

> **Note:** Check out our [instruction set test suite](examples/opcodes.asm) for a complete overview of supported instructions and their usage.

#### Binary Field Operations
- [x] `B32_MUL` - 32-bit binary field multiplication
- [x] `B32_MULI` - 32-bit binary field multiplication with immediate
- [x] `B128_ADD` - 128-bit binary field addition
- [x] `B128_MUL` - 128-bit binary field multiplication

#### Arithmetic Operations
- [x] `ADD` - Integer addition
- [x] `ADDI` - Integer addition with immediate
- [x] `SUB` - Integer subtraction
- [ ] `MUL` - Signed multiplication
- [ ] `MULI` - Signed multiplication with immediate
- [ ] `MULU` - Unsigned multiplication
- [ ] `MULSU` - Signed × unsigned multiplication

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
- [x] `SRA` - Shift right arithmetic
- [x] `SRAI` - Shift right arithmetic with immediate

#### Comparison Operations
- [ ] `SLT` - Set if less than (signed)
- [ ] `SLTI` - Set if less than immediate (signed)
- [x] `SLTU` - Set if less than (unsigned)
- [ ] `SLTIU` - Set if less than immediate (unsigned)
- [ ] `SLE` - Set if less than or equal (signed)
- [ ] `SLEI` - Set if less than or equal immediate (signed)
- [ ] `SLEU` - Set if less than or equal (unsigned)
- [ ] `SLEIU` - Set if less than or equal immediate (unsigned)

#### VROM Operations
- [x] `LDI.W` - Load immediate word
- [x] `MVV.W` - Move word between addresses
- [x] `MVV.L` - Move 128-bit value between addresses
- [x] `MVI.H` - Move immediate half-word

#### Control Flow
- [x] `J` - Jump to label or address
    - [x] `JUMPI` - Jump to immediate address
    - [x] `JUMPV` - Jump to address in variable
- [x] `BNZ` - Branch if not zero

#### Function Calls
- [x] `CALLI` - Call function at immediate address
- [x] `CALLV` - Call function at variable address
- [x] `TAILI` - Tail call to immediate address
- [x] `TAILV` - Tail call to variable address
- [x] `RET` - Return from function

#### Future Memory Extensions
- [ ] `LW`/`SW` - Load/Store word
- [ ] `LB`/`SB` - Load/Store byte
- [ ] `LBU` - Load byte unsigned
- [ ] `LH`/`SH` - Load/Store halfword
- [ ] `LHU` - Load halfword unsigned

## Example Programs
The project includes several example programs that demonstrate the capabilities of PetraVM:

- [Fibonacci](prover/tests/fibonacci.rs): Prove a Fibonacci number
