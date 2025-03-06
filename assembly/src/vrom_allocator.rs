use std::collections::HashMap;

// TODO: use a more accurate number for MIN_FRAME_SIZE
const MIN_FRAME_SIZE: u32 = 8;

/// VromAllocator allocates VROM addresses for objects, ensuring that:
/// - The object's size is padded to the next power-of-two (with a minimum of MIN_FRAME_SIZE),
/// - Available slack regions are reused when possible,
/// - The allocation pointer is aligned, and any alignment gap is recorded as slack,
/// - And any internal slack between (addr + requested_size) and (addr + padded size) is recorded.
pub struct VromAllocator {
    /// The next free allocation pointer.
    pos: u32,
    /// Slack blocks available for reuse, organized by the exponent (i.e. block size = 2^exponent).
    slack: HashMap<u32, Vec<u32>>,
}

impl VromAllocator {
    /// Creates a new VromAllocator.
    pub fn new() -> Self {
        Self {
            pos: 0,
            slack: HashMap::new(),
        }
    }

    /// Allocates a VROM address for an object with the given `requested_size`.
    ///
    /// The allocation process:
    /// 1. Compute `p`, the padded size (power-of-two ≥ MIN_FRAME_SIZE).
    /// 2. Attempt to reuse a slack block of size ≥ `p`.
    /// 3. If found, split off any leftover external slack.
    /// 4. Otherwise, align the allocation pointer (recording any gap as external slack),
    ///    and allocate a fresh block.
    /// 5. In either case, record any internal slack between (allocated_addr + requested_size)
    ///    and (allocated_addr + p) if it is at least MIN_FRAME_SIZE.
    pub fn alloc(&mut self, requested_size: u32) -> u32 {
        // p: the padded size (a power-of-two, at least MIN_FRAME_SIZE).
        let p = requested_size.next_power_of_two().max(MIN_FRAME_SIZE);
        // k: exponent such that p == 2^k.
        let k = p.trailing_zeros();

        // Attempt to find a slack block with size >= p.
        for exp in k..=(u32::BITS - 1) {
            if let Some(mut blocks) = self.slack.remove(&exp) {
                if let Some(addr) = blocks.pop() {
                    let block_size = 1 << exp;
                    // Reinsert remaining blocks for this exponent.
                    if !blocks.is_empty() {
                        self.slack.insert(exp, blocks);
                    }
                    let allocated_addr = addr;
                    let external_leftover = block_size - p;
                    // Record leftover external slack if large enough.
                    if external_leftover >= MIN_FRAME_SIZE {
                        self.add_slack(allocated_addr + p, external_leftover);
                    }
                    // Record internal slack: the unused portion of the padded block.
                    if p > requested_size {
                        let internal_slack = p - requested_size;
                        if internal_slack >= MIN_FRAME_SIZE {
                            self.add_slack(allocated_addr + requested_size, internal_slack);
                        }
                    }
                    return allocated_addr;
                }
            }
        }

        // No suitable slack block found: perform a fresh allocation.
        let old_pos = self.pos;
        let aligned_pos = align_to(self.pos, p);
        let gap = aligned_pos - old_pos;
        // Record the alignment gap as external slack if it is large enough.
        if gap >= MIN_FRAME_SIZE {
            self.add_slack(old_pos, gap);
        }
        let allocated_addr = aligned_pos;
        self.pos = aligned_pos + p;
        // Record internal slack if p > requested_size.
        if p > requested_size {
            let internal_slack = p - requested_size;
            if internal_slack >= MIN_FRAME_SIZE {
                self.add_slack(allocated_addr + requested_size, internal_slack);
            }
        }
        allocated_addr
    }

    /// Records a free (slack) region starting at `addr` with length `size`
    /// by splitting it into power-of-two blocks.
    ///
    /// Only blocks with size ≥ MIN_FRAME_SIZE are retained.
    fn add_slack(&mut self, addr: u32, size: u32) {
        if size < MIN_FRAME_SIZE {
            return;
        }
        for (block_addr, block_size) in split_into_power_of_two_blocks(addr, size) {
            self.slack.entry(block_size.trailing_zeros()).or_default().push(block_addr);
        }
    }
}

/// Aligns `pos` to the next multiple of `alignment` (which must be a power-of-two).
#[inline]
fn align_to(pos: u32, alignment: u32) -> u32 {
    (pos + alignment - 1) & !(alignment - 1)
}

/// Splits the interval [addr, addr + size) into power-of-two blocks with proper alignment.
///
/// Blocks smaller than MIN_FRAME_SIZE are dropped.
///
/// # Examples
///
/// - `split_into_power_of_two_blocks(0, 12)` yields `[(0,8)]` because the remaining 4 bytes are dropped.
/// - `split_into_power_of_two_blocks(4, 12)` initially produces `[(4,4), (8,8)]`, but the 4-byte block is dropped,
///   resulting in `[(8,8)]`.
fn split_into_power_of_two_blocks(addr: u32, size: u32) -> Vec<(u32, u32)> {
    let mut blocks = Vec::new();
    let mut current_addr = addr;
    let mut remaining = size;
    while remaining > 0 {
        // Maximum block size allowed by the current address's alignment.
        let alignment_constraint = if current_addr == 0 {
            remaining
        } else {
            current_addr & ((!current_addr).wrapping_add(1))
        };
        // Largest power-of-two not exceeding `remaining`.
        let largest_possible = 1 << (31 - remaining.leading_zeros());
        let mut block_size = if alignment_constraint < largest_possible {
            alignment_constraint
        } else {
            largest_possible
        };
        // Ensure block_size does not exceed remaining.
        while block_size > remaining {
            block_size /= 2;
        }
        // Skip blocks that are smaller than MIN_FRAME_SIZE.
        if block_size < MIN_FRAME_SIZE {
            current_addr += block_size;
            remaining -= block_size;
            continue;
        }
        blocks.push((current_addr, block_size));
        current_addr += block_size;
        remaining -= block_size;
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_to() {
        assert_eq!(align_to(0, MIN_FRAME_SIZE), 0);
        assert_eq!(align_to(3, MIN_FRAME_SIZE), 8);
        assert_eq!(align_to(8, MIN_FRAME_SIZE), 8);
        assert_eq!(align_to(9, MIN_FRAME_SIZE), 16);
    }

    #[test]
    fn test_split_into_power_of_two_blocks() {
        // Region exactly a power-of-two.
        assert_eq!(split_into_power_of_two_blocks(0, 8), vec![(0, 8)]);
        // 12 bytes splits into (0,8) and (8,4) but the 4-byte block is dropped.
        assert_eq!(split_into_power_of_two_blocks(0, 12), vec![(0, 8)]);
        // Region starting at a nonzero address:
        // (4,12) initially produces (4,4) and (8,8) but the 4-byte block is dropped.
        assert_eq!(split_into_power_of_two_blocks(4, 12), vec![(8, 8)]);
    }

    #[test]
    fn test_alloc_minimal_frame_size() {
        let mut allocator = VromAllocator::new();
        // A request smaller than MIN_FRAME_SIZE is bumped to MIN_FRAME_SIZE.
        let addr1 = allocator.alloc(1); // next_power_of_two(1)=1, but max(1,8)=8.
        assert_eq!(addr1, 0);
        assert_eq!(allocator.pos, 8);
        // A subsequent request bumps to 8.
        let addr2 = allocator.alloc(4);
        // Allocation occurs at pos = 8.
        assert_eq!(addr2, 8);
        assert_eq!(allocator.pos, 16);
        // No external slack should have been generated from alignment gaps.
        assert!(allocator.slack.is_empty());
    }

    #[test]
    fn test_alloc_no_slack() {
        let mut allocator = VromAllocator::new();
        // Two allocations that fit exactly without producing an alignment gap.
        let addr1 = allocator.alloc(9);  // p = 16, allocated at 0; pos becomes 16.
        assert_eq!(addr1, 0);
        let addr2 = allocator.alloc(10); // p = 16, allocated at 16; pos becomes 32.
        assert_eq!(addr2, 16);
        // pos should be updated correctly.
        assert_eq!(allocator.pos, 32);
    }

    #[test]
    fn test_alloc_with_slack_various() {
        let mut allocator = VromAllocator::new();
        // Step 1: alloc(17)
        // p = 32, allocated at 0, pos becomes 32.
        // Internal slack from (0+17, 0+32) is added.
        let addr1 = allocator.alloc(17);
        assert_eq!(addr1, 0);
        assert_eq!(allocator.pos, 32);
        // Internal slack from alloc(17) splits (17,15) to yield a block (24,8) (key 3).
        assert_eq!(allocator.slack.get(&3), Some(&vec![24]));

        // Step 2: alloc(33)
        // p = 64, pos=32 is aligned to 64, gap = 32 is recorded as external slack.
        // Allocation occurs at 64, pos becomes 128.
        // Internal slack from (64+33, 64+64) is recorded.
        let addr2 = allocator.alloc(33);
        assert_eq!(addr2, 64);
        assert_eq!(allocator.pos, 128);
        // External slack from alignment: (32,32) yields a block (32,32) under key 5.
        assert_eq!(allocator.slack.get(&5), Some(&vec![32]));
        // Internal slack from alloc(33) splits (97,31) to yield blocks (104,8) [key 3] and (112,16) [key 4].
        {
            let key3 = allocator.slack.get(&3).unwrap();
            // key 3 should now contain both 24 (from step 1) and 104 (from step 2).
            assert!(key3.contains(&24));
            assert!(key3.contains(&104));
        }
        assert_eq!(allocator.slack.get(&4), Some(&vec![112]));

        // Step 3: alloc(16)
        // p = 16, slack lookup (starting at key 4) finds block (112,16).
        // Allocation reuses that slack block, so addr becomes 112.
        let addr3 = allocator.alloc(16);
        assert_eq!(addr3, 112);
        assert_eq!(allocator.pos, 128);
        // Key 4 should now be removed.
        assert!(allocator.slack.get(&4).is_none());
        // Keys 3 and 5 remain.
        {
            let key3 = allocator.slack.get(&3).unwrap();
            assert!(key3.contains(&24) || key3.contains(&104));
        }
        assert_eq!(allocator.slack.get(&5), Some(&vec![32]));

        // Step 4: alloc(8)
        // p = 8, slack lookup (key 3) returns one block.
        // pop() removes the last element from the vector under key 3.
        let addr4 = allocator.alloc(8);
        // Depending on the vector order, this should be 104.
        assert_eq!(addr4, 104);
        assert_eq!(allocator.pos, 128);
        // Now key 3 should have the remaining element [24].
        assert_eq!(allocator.slack.get(&3), Some(&vec![24]));

        // Step 5: alloc(8)
        // p = 8, slack lookup (key 3) returns block at address 24.
        let addr5 = allocator.alloc(8);
        assert_eq!(addr5, 24);
        assert_eq!(allocator.pos, 128);
        // Now key 3 is empty and removed, leaving only key 5.
        assert_eq!(allocator.slack.len(), 1);
        assert_eq!(allocator.slack.get(&5), Some(&vec![32]));
    }
}
