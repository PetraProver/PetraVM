# PetraVM Assembly

This crate implements the parser and tooling for PetraVM's custom assembly language, as well as program emulation to obtain an execution trace to be used by the Petra prover.

## Overview

PetraVM assembly provides a low-level language to define programs that execute inside the zkVM.
The language includes:

- Binary field, arithmetic and logic instructions
- Memory operation
- Control flow
- Function calls

This crate implements the core developer-facing interface to PetraVM programs, including:

- A parser for PetraVM's custom assembly language
- A definition of the PetraVM instruction set architecture (ISA)
- An executor (interpreter) for emulating program behavior

### Key Components

#### 1. Parser

The parser reads `.asm` files and converts them into an internal representation of a PetraVM program. Instructions usually take value-value or value-immediate argument formats.

For instance, the `ADD` instruction encoding is the following:

```bash
(16-bit) Opcode
(16-bit) Destination offset from FP
(16-bit) Source 1 offset from FP
(16-bit) Source 2 offset from FP
```

The *immediate* variant of the `ADD` instruction, namely `ADDI`, has the following encoding:

```bash
(16-bit) Opcode
(16-bit) Destination offset from FP
(16-bit) Source 1 offset from FP
(16-bit) Immediate value
```

#### 2. ISA Definition

The crate allows defining arbitrary instruction sets (ISA), each tailored for a specific use-case.
The main ISA implements:

- Binary field ops (e.g. `B32_MUL`, `B128_ADD`)
- Arithmetic and logic ops (e.g. `SUB`, `ANDI`)
- Memory operations (e.g. `MVVL`)
- Control flow: jumps, calls, returns
- Typed arguments (immediates, registers, labels)

Supported instructions are defined at compile time. The Petra prover will take an instance of the targeted ISA when building [circuits](../prover/src/circuit.rs).

#### 3. Interpreter

The executor provides full emulation of PetraVM programs. This enables:

- Ensuring correctness of assembly programs
- Debugging execution flow before proving
- Generating execution trace for proof generation

## License

Licensed under Apache 2.0. See [LICENSE](LICENSE).