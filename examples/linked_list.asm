;; Rust equivalent (sort of...):
;; ------------
;; struct Node {
;;     value: i32,
;;     
;;     // For us, `Option<Box<Node>> will be `0` for `None`.
;;     // Otherwise it will point to the next node relative to the next frame pointer.
;;     next: Option<Box<Node>>,
;; }
;;
;; // Note that the actual ASM is going to diverge a bit from this.'
;; // Because the only way to store addresses in memory is through frame pointers, we need to access the next node through the frame pointer.
;; /// Builds a LL of ascending integers and returns the node at the front of the linked list. 
;; build_linked_list_of_ints(curr_val: u32, list_size: usize) -> Node {
;;     match curr_val < list_size {
;;         true => Node {
;;             value: curr_val as i32,
;;             next: Some(Box::new(build_linked_list_of_ints(curr_val + 1, list_size)), // `1` in the actual ASM.
;;         },
;;         false => Node {
;;             value: curr_val as i32,
;;             next: None, // `0` in the actual ASM.
;;         },
;;     }
;; }
;;
;; // Not part of this example, but reading a linked list would look something like this pseudocode:
;; ll = build_linked_list_of_ints_rec(0, 5);
;; sum =-0;
;;
;; if ll != 0 {
;;     sum += ll.value;
;;     ll = ll.next;
;; }
;;
;; assert_eq!(sum, 10)
;; return sum
;; ------------

#[framesize(0x6)]
build_linked_list_of_ints:
    ;; Frame:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Arg: start_val
    ;; Slot 3: Arg: list_size
    ;; Slot 4: ND Local: Next FP
    ;; Slot 5: Return value: 0 (emtpy list), > 0 pointer to the first node value
    
    ALLOCI! @4, #12
    MVV.W @4[2], @2 ;; curr_val
    MVV.W @4[3], @3 ;; list_size
    CALLI build_linked_list_of_ints_rec, @4
    MVV.W @4[6], @5 ;; return value
    RET

#[framesize(0xb)]
build_linked_list_of_ints_rec:
    ;; Frame:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Arg: curr_val
    ;; Slot 3: Arg: list_size
    ;; Slot 4: ND Local: Next FP.
    ;; Slot 5: Address of node.node_val
    ;; Slot 6: Return value: 0 (emtpy list), > 0 pointer to the first node value
    ;; Slot 7: Local: curr_val < list_size
    ;; Slot 8: Local: node.node_val
    ;; Slot 9: Local node.next
    ;; Slot 10: next_val    
    FP @5, #8 ;; Store the address of the current value

    SLT @7, @2, @3 ;; curr_val < list_size
    BNZ add_new_node, @7
    LDI.W @6, #0 ;; This is the null node
    RET    

add_new_node:
    XORI @6 , @5, #0 ;; The address of the node is the address of node.node_val.
    XORI @8, @2, #0 ;; node.node_val = curr_val
    ADDI @10, @2, #1 ;; curr_val + 1
    ;; Populate next frame.
    ALLOCI! @4, #12
    ;; Args:
    MVV.W @4[2], @10 ;; Next value
    MVV.W @4[3], @3 ;; List size
    CALLI build_linked_list_of_ints_rec, @4
    MVV.W @4[6], @9 ;; Return value: next node address
    RET