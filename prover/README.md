# zCrayVM Prover

This crate implements the proving system for zCrayVM.

## Architecture

The proving system is built using an M3 arithmetic circuit with the following components:

### Tables

1. **PROM Table**
   - Stores program instructions
   - Format: [PC, Opcode, Arg1, Arg2, Arg3]
   - Connected to instruction tables through `prom_channel`

2. **VROM Tables**
   - `VromAddrSpaceTable`: Pushes the full address space into `vrom_addr_space_channel`
   - `VromWriteTable`: Handles writing values to VROM addresses
   - `VromSkipTable`: Handles skipping unused VROM addresses
   - Format: [Address, Value]
   - Connected through `vrom_channel` and `vrom_addr_space_channel`

3. **Instruction Tables**
   - `LdiTable`: Handles Load Immediate instructions
   - `RetTable`: Handles Return instructions

### Channels

1. **State Channel**
   - Format: [PC, FP]
   - Used for state transitions between instructions
   - Pulled by instruction tables for current state
   - Pushed by instruction tables for next state

2. **PROM Channel**
   - Format: [PC, Opcode, Arg1, Arg2, Arg3]
   - Used to connect PROM table with instruction tables
   - Pulled by instruction tables to get instruction details
   - Pushed by PROM table with instruction data

3. **VROM Channel**
   - Format: [Address, Value]
   - Used for memory operations
   - Pulled by VromWriteTable for address+value pairs
   - Pushed by instruction tables when writing values

4. **VROM Address Space Channel**
   - Format: [Address]
   - Used to push the full address space (0-31)
   - Pulled by instruction tables and VROM tables
   - Pushed by VromAddrSpaceTable

### Design Considerations

1. **VROM Memory Model**
   - The verifier pushes all possible addresses (power of two) into the VROM address space channel
   - Each address must be pulled exactly once to balance the channel
   - Two specialized tables handle address consumption:
     - VROM Write Table: Pulls an address and pushes a (address, value) pair when data needs to be written
     - VROM Skip Table: Pulls an address without pushing any value for unused addresses
   - This design ensures complete coverage of the address space while maintaining write-once semantics

2. **Channel Balancing**
   - All pushes must be matched by pulls

3. **Table Organization**
   - Separate tables for different operations
   - Tables communicate through channels

## Usage

The proving system is used to generate and verify proofs of zCrayVM execution:

```rust
use zcrayvm_prover::prover::{Prover, verify_proof};
use zcrayvm_prover::model::Trace;

// Create a prover
let prover = Prover::new();

// Generate a proof
let (proof, statement, compiled_cs) = prover.prove(&trace)?;

// Verify the proof
verify_proof(&statement, &compiled_cs, proof)?;
```

## Testing

The crate includes integration tests that verify the complete proving pipeline.

Run the tests with:
```bash
cargo test
```
