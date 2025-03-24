# zCrayVM
A verifiable supercomputer

## Overview
zCrayVM is a new virtual machine (zkVM) designed specifically for efficient execution within Zero-Knowledge (ZK) proof systems, leveraging the [Binius](https://www.binius.xyz/) SNARK scheme's strengths. The primary goals are to improve performance of recursive proof verification and WebAssembly execution within ZK environments.

## Instruction Set
The zCrayVM features a comprehensive instruction set including:
- Binary field operations
- Integer arithmetic and logic operations
- Memory operations
- Control flow instructions
- Function call mechanics

Check out our [instruction set test suite](examples/opcodes.asm) for a complete overview of supported instructions and their usage.
