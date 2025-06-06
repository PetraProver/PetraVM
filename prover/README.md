# PetraVM Prover

This crate implements the proving system for PetraVM.

## Architecture

The proving system is built using an M3 arithmetic circuit with the following components:

### Tables

1. **PROM Table**
   - Stores program instructions
   - Format: [PC, Opcode, Arg1, Arg2, Arg3]
   - Connected to instruction tables through the `prom_channel`
   - Uses multiplicity field to track how many times each instruction is executed

2. **VROM Table**
   - Stores the VROM
   - `VromTable`: Pushes the full address space into `vrom_addr_space_channel`
   - Format: [AddressSpace, Address, Value]
   - AddressSpace is the whole padded memory space from 0 to 2^n
   - Address is a permutation of AddressSpace
   - Connected to instruction tables through the `vrom_channel`
   - Multiplicity field tracks how many times each memory location is accessed

3. **Instruction Tables**
   - Tables for all supported instructions (LDI, ADD, AND, OR, XOR, etc.)
   - Each table implements the corresponding instruction's semantics

### Channels

1. **State Channel**
   - Format: [PC, FP]
   - Used for state transitions between instructions
   - Pulled by instruction tables for current state
   - Pushed by instruction tables for next state

2. **PROM Channel**
   - Format: [PC + Opcode + Arg1 + Arg2 + Arg3, Multiplicity]
   - Used to connect PROM table with instruction tables
   - Multiplicity field enables tracking of repeated instruction execution

3. **VROM Channel**
   - Format: [Address, Value, Multiplicity]
   - Used for memory operations with access counting
   - Pushed by VromTable for address+value+multiplicity tuples
   - Pulled by instruction tables when reading/writing values
   - Multiplicity field tracks how many times a memory location is accessed

4. **VROM Address Space Channel**
   - Format: [Address]
   - Pulled by VromTable's Address column
   - Pushed by VromTable's AddressSpace column

### Design Considerations

1. **VROM Memory Model**
   - The verifier pushes all possible addresses (power of two) into the VROM address space channel
   - Each address must be pulled exactly once to balance the channel

2. **Lookup Tables**
   - Both PROM and VROM channels are implemented as lookup tables with multiplicity
   - This approach enables efficient verification of repeated instruction execution and memory accesses

3. **Channel Balancing**
   - All pushes must be matched by pulls

## Usage

The proving system is used to generate and verify proofs of PetraVM execution:

```rust
use petravm_prover::prover::{Prover, verify_proof};
use petravm_prover::model::Trace;

// Create a prover
let prover = Prover::new(Box::new(GenericISA));

// Generate a proof
let (proof, statement, compiled_cs) = prover.prove(&trace)?;

// Verify the proof
verify_proof(&statement, &compiled_cs, proof)?;
```

## Testing

The crate includes integration tests that verify the complete proving pipeline, including:

- Basic operations with LDI, ADD, and RET
- All binary operations (AND, OR, XOR, etc.)
- Tail call optimizations
- Fibonacci number proving

Run the tests with:
```bash
cargo test
```

## License

Licensed under Apache 2.0. See [LICENSE](LICENSE).