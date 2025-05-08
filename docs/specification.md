# PetraVM Architecture

_This specification document is still in progress._

## 1. Introduction

This document specifies the architecture and behavior of the Petra Virtual Machine. Petra is a general-purpose virtual machine that is succinctly verifiable using the [Binius](https://www.binius.xyz/) proof system. The PetraVM execution model and instruction set are designed specifically for efficient proving with Binius.

### 1.1 Background

[Verifiable computing](https://en.wikipedia.org/wiki/Verifiable_computing) is a technique for running an expensive computation on a powerful but untrusted server and proving its correct execution to a resource-constrained machine. Verifiable computing can be realized using a cryptographic primitive called a _SNARK_.

Binius is a new binary-field–based SNARK scheme with strong CPU performance and potential for best-in-class performance on custom hardware. Its native programming model, [M3](https://www.binius.xyz/basics/arithmetization/), is a low-level representation of formal decision languages using polynomial constraints. While M3 is flexible, it can be cumbersome for larger or more complex programs.

Verifiable virtual machines, like PetraVM, provide a familiar and flexible programming model, which improves the developer experience and expands the use cases for verifiable computing.

### 1.2. Project Goals

- **Low-overhead recursion:** PetraVM enables efficient recursive verification of Binius proofs.
- **WebAssembly compilation:** PetraVM enables verifiable general-purpose computations, via compilation of [WebAssembly](https://webassembly.org/) programs to the PetraVM ISA.
- **Efficient co-processor:** PetraVM handles performance-critical computations offloaded from other compatible VMs (e.g., RISC-V zkVMs).

### 1.3. Prior Work & Inspiration

The design and architecture of the PetraVM draws inspiration from several existing works.

- [Cairo](https://eprint.iacr.org/2021/1063), a minimal, Turing-complete verifiable VM designed for STARK verification.
- [Valida](https://github.com/valida-xyz/valida), a VM based on RISC-V and adapted for efficient STARK verification.
- RISC-V verifiable VMs, including [RISC Zero](https://github.com/risc0/risc0) and [SP1](https://github.com/succinctlabs/sp1).

## 2. Architecture Overview

PetraVM inspired by RISC-V and makes the following notable modifications:

* **Harvard architecture**. The program is stored in a read-only memory, separate from the execution state and data. This is called the program ROM, or PROM. The program may be committed to independently from the witness data, and the commitment can be included in a pre-processed verification key.
* **Non-deterministic ROM**. The main memory model is _non-deterministic read-only memory_ (see [Cairo], Section 3.3). The intermediate computed values in the trace are placed the value ROM, or VROM. Instructions load values from VROM and assert constraint relations on them.
* **Multiplicative group PROM addressing**. The PROM address space is the multiplicative group of the 32-bit binary tower field. Incrementing a PROM address corresponds to multiplication by the group generator.
* **Variable-length instruction encoding**. Instructions are encoded as one or more 64-bit tower field elements.
* **Additive subspace VROM addressing**. The VROM address space is the 32-bit binary tower field, and pointer arithmetic is based on binary field addition.
* **Binary field VROM words**. VROM words are 32-bit binary tower field elements.
* **Non-deterministic VROM allocation**. Pointers to objects allocated in VROM, including pointers to function frames, are allocated non-deterministically by the prover.
* **Minimal state registers**. The VM uses only two state registers: the program counter (PC) and frame pointer (FP). Instructions do not read from a general-purpose register bank, instead they read from the VROM.
* **Extensible instruction set**. The ISA can be extended with more instructions. If an ISA extension is not used by a programe, the verifier will not incur a cost for them.

[Cairo]: <https://eprint.iacr.org/2021/1063>

### 2.1. Execution model

The program contains a sequence of instructions structured as multiple function blocks. A _supervisor_ M3 constraint system orchestrates the verification of a function call by pushing an initial program state and pulling a final program state from the state channel. The simplest supervisor would use channel boundary values to initialize and finalize the execution. The supervisor sets up the function frame of the entrypoint function by constraining the argument slots of the initial function frame.

## 3. Machine Specification

### 3.1. Memory Architecture

PetraVM uses a Harvard architecture with separate memory spaces:

-   **Instruction Memory (PROM):** Immutable, stores the program code.
-   **Data Memory:**
    -   **Value ROM (VROM):** Write-once, non-deterministically populated read-only memory.
    -   **Standard RAM:** Read-write memory for dynamic data.

Each memory space (PROM, VROM, RAM) has a separate 32-bit address space.

### 3.2. Program State Registers

-   **Program Counter (PC):** Points to the current instruction in PROM.
-   **Frame Pointer (FP):** Points into VROM for the local function frame.
-   **Timestamp (TS):** Global timestamp for RAM reads/writes (32-bit unsigned integer).

### 3.3. Memory Addressing

-   **PROM Addressing:** Uses the 32-bit field's multiplicative group. Pointer arithmetic is performed using multiplication by a fixed generator.
-   **VROM Addressing:** Uses 32-bit addresses with access at 32-bit (word) granularity.
-   **RAM Addressing:** Uses standard 32-bit integer addressing.

### 3.4. Function Frames and Calling Convention

-   Functions execute within a frame in VROM. Frame size is statically determined.
-   Frames are 16-byte aligned.
-   Return values are treated as non-deterministic arguments to a function.
-   Frame Layout:
    1.  (32-bit) Return PC
    2.  (32-bit) Return FP
    3.  Arguments + Return Values
    4.  Local values

### 3.5. Multiplicative Pointer Arithmetic

-   PC advances via multiplication by a fixed 32-bit multiplicative group generator.
-   Incrementing PC by 1 corresponds to multiplying by the generator.
-   Address 0 indicates program exit.

### 3.6. Exceptions

-   Traps set PC = 0 and FP to a non-zero value, pointing to an exception frame in VROM.
-   Exception Frame:
    1.  (32-bit) PC when the exception occurred
    2.  (32-bit) FP when the exception occurred
    3.  (8-bit) Exception reason

## 4. Instruction Set Architecture (ISA)

### 4.1. Instruction Formats

-   Variable length, multiple of 64 bits.
-   **VV (Value-value):** Opcode, Destination offset, Source1 offset, Source2 offset.
-   **VI (Value-immediate):** Opcode, Destination offset, Source1 offset, Immediate value.

### 4.2. Base Instructions

-   **XOR Instructions:** `XOR`, `XORI`
-   **Binary Field Instructions:** `B32_ADD`, `B32_MUL`, `B128_ADD`, `B128_MUL`
-   **Move Instructions:** `LDI.W`, `MVV.H`, `MVV.W`, `MVV.L`
-   **Jumps:** `JUMPI`, `JUMPV`, `CALLI`, `CALLV`, `TAILI`, `TAILV`, `RET`
-   **Branches:** `BNZ`
-   **Memory Access (RAM):** `LW`, `SW`, `LB`, `LBU`, `LH`, `LHU`, `SB`, `SH`
-   **Integer Instructions:** `ADDI`, `SLTI`, `SLTIU`, `SLEI`, `SLEIU`, `ANDI`, `ORI`, `SLLI`, `SRLI`, `SRAI`, `ADD`, `SUB`, `SLT`, `SLTU`, `SLE`, `SLEU`, `AND`, `OR`, `XOR`, `SLL`, `SRL`, `SRA`, `MUL`, `MULU`, `MULSU`

### 4.3. Instruction Specification Examples

-   **`XOR`:**
    -   Encoding: VV
    -   Opcode: `0x0000`
    -   Syntax: `XOR dst, src1, src2`
    -   Effect: `fp[dst] = fp[src1] ^ fp[src2]`
-   **`LDI.W`:**
    -   Description: Move 4-byte immediate value into VROM
    -   Encoding: (16-bit) Opcode: `0x0005`, (16-bit) Destination offset, (32-bit) Immediate value
    -   Syntax: `LDI.W dst, imm1`
    -   Effect: `fp[dst] = imm1`
-   **`JUMPI`:**
    -   Description: Jump to the target address given as an immediate.
    -   Encoding: (16-bit) Opcode: `0x000C`, (16-bit) Zero, (32-bit) Target address
    -   Syntax: `J target`
    -   Effect: `PC = target`

*(Refer to the original design document for a comprehensive list of all instructions and their specifications.)*

### 4.4. Proving Execution

-   Instruction handler tables transition and constrain program state.
-   Program state is a tuple of (PC, FP, TS).
-   Supervisor program initializes execution and handles finalization.

## 5. Memory Management

### 5.1. Populating VROM

-   VROM is populated with read values at the required granularities.
-   Verifier provides the full address space; the prover chooses which addresses to populate.

### 5.2. Read-Write Memory Argument (RAM)

-   RAM uses a channel with triples (address, value, timestamp).
-   Initialization and finalization involve managing 2^k addresses with value 0 and timestamp 0.
-   Read/write operations update or read the (address, value, timestamp) triples.
-   Caching optimization using a lookup table for timestamp inequality checks.
