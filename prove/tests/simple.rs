//! Test the simple proving system with LDI and RET instructions.

use anyhow::Result;
use zcrayvm_prove::model::ZkVMTrace;

#[test]
fn test_simple_zkvm_trace() -> Result<()> {
    // Create a simple trace with known values
    let trace = ZkVMTrace::generate_ldi_ret_example(42);
    
    // Verify the trace has the expected instructions
    assert_eq!(trace.program.len(), 2);
    assert_eq!(trace.ldi_events.len(), 1);
    assert_eq!(trace.ret_events.len(), 1);
    
    // Verify the LDI event loaded the correct value
    assert_eq!(trace.ldi_events[0].imm, 42);
    
    Ok(())
}