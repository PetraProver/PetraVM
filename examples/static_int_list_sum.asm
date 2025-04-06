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
    ;; Slot 5: ND Local: Next FP
    ;; Slot 6: Local: new_list_size
    ;; Slot 7: Local: new_curr_sum

    BNZ @3, list_size_gt_0
    MVV.W @5, @4 ;; return curr_sum
    RET

list_size_gt_0:
    ;; Recursively call this function again
    MVV.W @5[2], @2[1] ;; list[1..]

    SUBI @6, @3, #1 ;; list_size - 1
    ADD @7, @4, @2[0] ;; curr_sum + list[0]

    MVV.W @5[3], @6
    MVV.W @5[4], @7

    TAILI static_int_list_sum, @5 ;; static_int_list_sum(&list[1..], list_size - 1, curr_sum + list[0])
