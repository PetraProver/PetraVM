;; Rust equivalent:
;; ------------
;; // Note that in our case that the pointer to the list is always going to be a static list.
;; fn static_int_list_sum(list: &[u32], list_size: usize, curr_sum: u32) -> u32 {
;;   match list_size > 0 {
;;   true => static_int_list_sum(&list[1..], list_size - 1, curr_sum + list[0]),
;;   false => curr_sum,
;; }
;; ------------

#[framesize(0x8)]
static_int_list_sum:
    ;; Frame:
    ;; Slot 0: Return PC
    ;; Slot 1: Return FP
    ;; Slot 2: Arg: list addr
    ;; Slot 3: Arg: list_size
    ;; Slot 4: Arg: curr_sum
    ;; Slot 5: Return value
    ;; Slot 6: ND Local: Next FP
    ;; Slot 7: Local: curr_node_val
    ;; Slot 8: Local: curr_node_next
    ;; Slot 9: Local: new_list_size
    ;; Slot 10: Local: new_curr_sum

    BNZ @3, list_size_gt_0
    MVV.W @5, @4 ;; return curr_sum
    RET

list_size_gt_0:
    MVV.W @2[0], @7 ;; curr_node_val = list[0]
    MVV.W @2[1], @8 ;; curr_node_next = list[1..]

    ;; Recursively call this function again
    MVV.W @6[2], @8 ;; list[1..]

    SUBI @9, @3, #1 ;; list_size - 1
    MVV.W @6[3], @9 ;; new_list_size

    ADD @10, @4, @7 ;; curr_sum + list[0]    
    MVV.W @6[4], @10 ;; new_curr_sum

    TAILI static_int_list_sum, @6 ;; static_int_list_sum(&list[1..], new_list_size, new_curr_sum)
