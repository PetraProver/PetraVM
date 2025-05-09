# PetraVM

PetraVM is a general-purpose virtual machine that is succinctly verifiable using the [Binius](https://www.binius.xyz/) proof system. The PetraVM execution model and instruction set are designed specifically for efficient proving with Binius. The VM is intended to handle several use cases simultaneously:

1. recursive proof verification,
2. general purpose computation, via compilation from [WebAssembly](https://webassembly.org/), and
3. high-performance verifiable computing use cases, using a custom high-level language called PetraML.

The VM consists of a basic instruction set and optional instruction set extensions. The arithmetization of the machine enables the prover and verifier to only handle the ISA extensions used by an agreed-upon program.

The full machine specification can be found [here](https://petraprover.github.io/PetraVM/specification.html).

## Documentation

Documentation is still incomplete and will be improved over time.
You can go through the [PetraVM book](https://petraprover.github.io/PetraVM/)
for explanations on the zkVM design and architecture.

## Features

- Binary field and arithmetic operations
- Memory and control flow primitives
- Efficient support for recursion
- Write-once memory model (VROM)

## Instruction Set

PetraVM's full instruction set is divided into five categories:
- Binary field operations
- Arithmetic & Logic operations
- Memory operations
- Control Flow instructions
- Function Calls

Prover support for the full instruction set is a work in progress, tracked below.

The VM will also define a minimal ISA tailored for efficient recursion.

Expansion to include RAM-related instructions is kept for future work.

### Prover Support (Work in Progress)

> **Note:** In PetraVM, variables refer to addresses in VROM (Value ROM, a write-once memory region). Instructions operate on values at these addresses unless specified as "immediate" operations.

> **Note:** Check out our [instruction set test suite](https://github.com/PetraProver/PetraVM/examples/opcodes.asm) for a complete overview of supported instructions and their usage.

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
- [x] `SLTIU` - Set if less than immediate (unsigned)
- [ ] `SLE` - Set if less than or equal (signed)
- [ ] `SLEI` - Set if less than or equal immediate (signed)
- [x] `SLEU` - Set if less than or equal (unsigned)
- [x] `SLEIU` - Set if less than or equal immediate (unsigned)

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

#### Future Random-Access Memory Extensions
- [ ] `LW`/`SW` - Load/Store word
- [ ] `LB`/`SB` - Load/Store byte
- [ ] `LBU` - Load byte unsigned
- [ ] `LH`/`SH` - Load/Store halfword
- [ ] `LHU` - Load halfword unsigned

## Example Programs
The project includes several example programs that demonstrate the capabilities of PetraVM:

- [Fibonacci](prover/tests/fibonacci.rs): Prove a Fibonacci number
- [Collatz](prover/test/collatz.rs): Prove the Collatz sequence for a given number

## Development Status

The project is actively developed. Many instructions are already supported by the prover, with new instructions and additional features added regularly.

## Crates

- [`assembly`](./assembly): zkVM assembly DSL, parser and program executor
- [`prover`](./prover): Circuit definition and proof generation

## License

Licensed under Apache 2.0. See [LICENSE](LICENSE).

## Contributing

The PetraVM project is a collaboration between several teams and welcomes community contributions. Please open issues or pull requests for bugs, features, or improvements. See the [CONTRIBUTING](CONTRIBUTING.md) document for guidelines.

The initial development is led by [Polygon](https://polygon.technology/) and [Irreducible](https://www.irreducible.com/).

We reserve the right to close issues and PRs deemed unnecessary or not bringing sufficient interest.
