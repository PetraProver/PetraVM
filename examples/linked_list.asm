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
    ;; Slot 5: Return value (We actually can't implement this yet, so until we have the instruction to store addresses, this function will just return `0`.)
    
    MVV.W @4[2], @2
    MVV.W @4[3], @3
    CALLI build_linked_list_of_ints_rec, @4

    BNZ @6, non_empty_list
    MVI.H @5, #0 ;; `0` means an empty list.
    RET

    non_empty_list:
    ;; TODO: Replace instruction with one that stores addresses once available...
    MVV.W @4[7], @5 ;; A non-zero value is always the address of the first node.
    RET

#[framesize(0xc)]
build_linked_list_of_ints_rec:
    ;; Frame:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Arg: curr_val (Also used as `node.node_val`)
    ;; Slot 3: Arg: list_size
    ;; Slot 4: Arg: cur_fp (this is the address of the current frame. We can keep track of it in the code.)
    ;; Slot 5: ND Local: Next FP.
    ;; Slot 6: Return value, which is 0 if this is the last node or 1 if there is another node.
    ;; Slot 7: Return value: cur_val_addr (address of the current value)
    ;; Slot 8: Local: curr_val < list_size
    ;; Slot 9: Local: node.node_val
    ;; Slot 10: Local node.next
    ;; Slot 11: next_val

    ADDI @7, @4, #7 ;; Store the address of the current value.
    
    SLTI @8, @2, @3 ;; curr_val < list_size
    BNZ add_new_node, @8

    MVI.H @6, #0 ;; This is the last node.
    RET    

add_new_node:
    ADDI @11, @2, #1 ;; curr_val + 1
    ;; Populate next frame.
    ;; Args:
    MVV.W @5[2], @11 ;; Next value
    MVV.W @5[3], @3 ;; List size
    MVV.W @5[4], @5 ;; Store the address of the next frame pointer.
    ;; Return values
    MVV.W @5[7] @10 ;; The address of the next value is populated in the next frame. (stores local.next_node = $next_node.val).

    MVV.W @9 @2 ;; node.node_val = curr_val

    MVI.H @5, #1 ;; Indicate to caller that there is another node.
    TAILI build_linked_list_of_ints_rec, @5
    RET
