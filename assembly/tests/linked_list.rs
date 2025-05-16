pub mod common;
use common::test_utils::{execute_test_asm, AsmToExecute};

fn run_test(cur_val: u32, list_size: u32) {
    let mut info = execute_test_asm(
        AsmToExecute::new(include_str!("../../examples/linked_list.asm"))
            .init_vals(vec![cur_val, list_size]),
    );
    let linked_list_frame = info.frames.add_frame("build_linked_list_of_ints");
    let mut current_node_ptr = linked_list_frame.get_vrom_expected::<u32>(5);
    let mut i = 0;
    // Traverse the whole list
    while current_node_ptr != 0 {
        // Assert that the current node value is correct
        let node_value = info
            .frames
            .trace
            .vrom()
            .read::<u32>(current_node_ptr)
            .expect("The node value must have been set");
        assert_eq!(
            node_value,
            cur_val + i,
            "Linked list mismatch for cur_val = {cur_val}, list_size = {list_size}"
        );
        current_node_ptr = info
            .frames
            .trace
            .vrom()
            .read::<u32>(current_node_ptr + 1)
            .expect("The next node ptr must have been set");
        i += 1;
    }

    assert_eq!(
        i,
        list_size - cur_val,
        "Linked list size mismatch for cur_val = {cur_val}, list_size =
    {list_size}"
    );
}

#[test]
fn test_linked_list() {
    petravm_asm::init_logger();
    // Test cases
    let list_size = 20;
    for cur_val in 0..list_size {
        run_test(cur_val, list_size);
    }
}
