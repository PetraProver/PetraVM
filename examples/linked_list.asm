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
;; ------------


#[framesize(0x6)]
build_linked_list_of_ints:
    ;; Frame:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Arg: start_val
    ;; Slot 3: Arg: list_size
    ;; Slot 4: ND Local: Next FP
    ;; Slot 5: Return value (We actually can't implement this yet, so until we have the instruction to store addresses, this function will just return `0`.)
    
    MVI.H @4[2], @2
    MVI.H @4[3], @3
    CALLI build_linked_list_of_ints_rec, @4

    MVI.H @5, #0
    RET


#[framesize(0x7)]
build_linked_list_of_ints_rec:
    ;; Frame:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Arg: curr_val (Also used as `node.node_val`)
    ;; Slot 3: Arg: list_size
    ;; Slot 4: ND Local: Next FP (Also used for `node.next_node`).
    ;; Slot 5: Return value, which is 0 if this is the last node or 1 if there is another node.
    ;; Slot 6: Local: curr_val < list_size

    SLTI @6, @2, @3 ;; curr_val < list_size
    BNZ @6, add_new_node

    MVI.H @5, #0 ;; This is the last node.
    RET    

add_new_node:
    ADDI @4[2], @2, #1
    MVI.H @4[3], @3

    MVI.W @5, #1 ;; Indicate to caller that there is another node.
    TAILI build_linked_list_of_ints_rec, @4
    RET
