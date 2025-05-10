use std::collections::BTreeMap;

// We need at least two slots for return pc and return fp.
const MIN_FRAME_SIZE: u32 = 2;

/// VromAllocator allocates VROM addresses for objects, ensuring that:
/// - The object's size is padded to the next power-of-two (with a minimum of
///   MIN_FRAME_SIZE),
/// - Available slack regions are reused when possible,
/// - The allocation pointer is aligned (least significant log₂(padded size)
///   bits are cleared).
#[derive(Clone, Debug, Default)]
pub struct VromAllocator {
    /// The next free allocation pointer.
    pos: u32,
    /// Slack blocks available for reuse, organized by the exponent
    /// (i.e. block size = 2^exponent).
    slack: BTreeMap<u32, Vec<u32>>,
}

impl VromAllocator {
    /// Get the size of the VROM.
    pub const fn size(&self) -> usize {
        self.pos as usize
    }

    /// Set the current position of the allocator.
    pub fn set_pos(&mut self, pos: u32) {
        self.pos = pos;
    }

    /// Allocates a VROM address for an object with the given `requested_size`.
    ///
    /// The allocation process:
    /// 1. Compute `p`, the padded size (power-of-two ≥ MIN_FRAME_SIZE).
    /// 2. Attempt to reuse a slack block of size ≥ `p`.
    /// 3. If found, split off any leftover external slack.
    /// 4. Otherwise, align the allocation pointer (by clearing the least
    ///    significant log₂(padded size) bits) and allocate a fresh block.
    /// 5. In either case, record any internal slack between (allocated_addr +
    ///    requested_size) and (allocated_addr + p).
    pub fn alloc(&mut self, requested_size: u32) -> u32 {
        // p: padded size (power-of-two, at least MIN_FRAME_SIZE).
        let p = requested_size.next_power_of_two().max(MIN_FRAME_SIZE);
        // k: exponent such that p == 2^k.
        let k = p.trailing_zeros();

        // Attempt to find a slack block with size ≥ p.
        if let Some((&exp, blocks)) = self.slack.range_mut(k..).next() {
            if let Some(addr) = blocks.pop() {
                let block_size = 1 << exp;
                // Remove empty vectors to keep the map clean
                if blocks.is_empty() {
                    self.slack.remove(&exp);
                }
                let allocated_addr = addr;
                let external_leftover = block_size - p;
                // Record leftover external slack.
                self.add_slack(allocated_addr + p, external_leftover);
                self.record_internal_slack(allocated_addr, requested_size, p);
                return allocated_addr;
            }
        }

        // No suitable slack block found: perform a fresh allocation.
        let old_pos = self.pos;
        let aligned_pos = align_to(self.pos, p);
        let gap = aligned_pos - old_pos;
        // Record alignment gap as external slack.
        self.add_slack(old_pos, gap);
        let allocated_addr = aligned_pos;
        self.pos = aligned_pos + p;
        self.record_internal_slack(allocated_addr, requested_size, p);
        allocated_addr
    }

    /// Helper to record internal slack (unused portion within the padded
    /// block).
    fn record_internal_slack(
        &mut self,
        allocated_addr: u32,
        requested_size: u32,
        padded_size: u32,
    ) {
        let internal_slack = padded_size.saturating_sub(requested_size);
        if internal_slack >= MIN_FRAME_SIZE {
            self.add_slack(allocated_addr + requested_size, internal_slack);
        }
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
            self.slack
                .entry(block_size.trailing_zeros())
                .or_default()
                .push(block_addr);
        }
    }
}

/// Aligns `pos` to the next multiple of `alignment` (which must be a
/// power-of-two).
#[inline(always)]
const fn align_to(pos: u32, alignment: u32) -> u32 {
    (pos + alignment - 1) & !(alignment - 1)
}

/// Splits the interval [addr, addr + size) into power-of-two blocks with proper
/// alignment.
///
/// Blocks smaller than MIN_FRAME_SIZE are dropped.
///
/// # Examples
///
/// - `split_into_power_of_two_blocks(0, 12)` yields `[(0,8)]` because the
///   remaining 4 slots are dropped.
/// - `split_into_power_of_two_blocks(4, 12)` initially produces `[(4,4),
///   (8,8)]`, but the 4-slot block is dropped, resulting in `[(8,8)]`.
fn split_into_power_of_two_blocks(addr: u32, size: u32) -> Vec<(u32, u32)> {
    let mut blocks = Vec::new();
    let mut current_addr = addr;
    let mut remaining = size;
    while remaining > 0 {
        // Determine the maximum block size allowed by the current address's alignment.
        let alignment_constraint = if current_addr == 0 {
            remaining
        } else {
            1 << current_addr.trailing_zeros()
        };
        // Largest power-of-two not exceeding the remaining size.
        let largest_possible = 1 << (31 - remaining.leading_zeros());
        let block_size = alignment_constraint.min(largest_possible);
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
    use rand::Rng;

    use super::*;

    #[test]
    fn test_align_to() {
        assert_eq!(align_to(0, MIN_FRAME_SIZE), 0);
        assert_eq!(align_to(MIN_FRAME_SIZE - 1, MIN_FRAME_SIZE), MIN_FRAME_SIZE);
        assert_eq!(align_to(MIN_FRAME_SIZE, MIN_FRAME_SIZE), MIN_FRAME_SIZE);
        assert_eq!(
            align_to(MIN_FRAME_SIZE + 1, MIN_FRAME_SIZE),
            MIN_FRAME_SIZE * 2
        );
    }

    #[test]
    fn test_split_into_power_of_two_blocks() {
        // Region exactly a power-of-two.
        assert_eq!(split_into_power_of_two_blocks(0, 8), vec![(0, 8)]);
        // 12 slots splits into (0,8) and (8,4) but the 4-slot block is dropped.
        assert_eq!(split_into_power_of_two_blocks(0, 12), vec![(0, 8), (8, 4)]);
        // Region starting at a non power-of-two address.
        assert_eq!(
            split_into_power_of_two_blocks(5, 32),
            vec![(6, 2), (8, 8), (16, 16), (32, 4)]
        );
    }

    #[test]
    fn test_alloc_minimal_frame_size() {
        let mut allocator = VromAllocator::default();
        // A request smaller than MIN_FRAME_SIZE is bumped to MIN_FRAME_SIZE.
        let addr1 = allocator.alloc(1); // next_power_of_two(1)=1, but max(1,2)=2.
        assert_eq!(addr1, 0);
        assert_eq!(allocator.pos, 2);
        // A subsequent request bumps to 4.
        let addr2 = allocator.alloc(2);
        // Allocation occurs at pos = 4.
        assert_eq!(addr2, 2);
        assert_eq!(allocator.pos, 4);
        // No external slack should be generated from alignment gaps.
        assert!(allocator.slack.is_empty());
    }

    #[test]
    fn test_alloc_with_slack_various() {
        let mut allocator = VromAllocator::default();

        // --- Step 1: alloc(17) ---
        // p = 32, allocated at 0, pos becomes 32.
        // Internal slack from (0+17, 0+32) is recorded.
        let addr1 = allocator.alloc(17);
        assert_eq!(addr1, 0);
        assert_eq!(allocator.pos, 32);
        // Expected internal slack: split_into_power_of_two_blocks(17,15) yields
        // [(24,8)]. Thus, key 3 (2^3 = 8) should hold [24].
        assert_eq!(allocator.slack.get(&3), Some(&vec![24]));

        // --- Step 2: alloc(33) ---
        // p = 64, current pos=32 is aligned to 64 producing a gap of 32.
        // External slack from gap: split_into_power_of_two_blocks(32,32) yields
        // [(32,32)] → key 5. Allocation occurs at 64, pos becomes 128.
        // Internal slack from (64+33, 64+64) = (97,31) splits into [(104,8), (112,16)]
        // → key 3 gets 104 and key 4 gets 112.
        let addr2 = allocator.alloc(33);
        assert_eq!(addr2, 64);
        assert_eq!(allocator.pos, 128);
        // Check external slack: key 5 should be [32].
        assert_eq!(allocator.slack.get(&5), Some(&vec![32]));
        // Check internal slack: key 3 now should contain [24,104] (order sorted for
        // comparison)
        if let Some(mut key3) = allocator.slack.get(&3).cloned() {
            key3.sort();
            assert_eq!(key3, vec![24, 104]);
        } else {
            panic!("Expected key 3 to be present");
        }
        // And key 4 should be [112].
        assert_eq!(allocator.slack.get(&4), Some(&vec![112]));

        // --- Step 3: alloc(16) ---
        // p = 16, slack lookup from key 4 finds block [112].
        // Allocation reuses that block; no external or internal slack is recorded
        // (16-16=0).
        let addr3 = allocator.alloc(16);
        assert_eq!(addr3, 112);
        assert_eq!(allocator.pos, 128);
        // Key 4 should now be removed.
        assert!(!allocator.slack.contains_key(&4));
        // Remaining slack: key 3: [24,104] and key 5: [32].
        if let Some(mut key3) = allocator.slack.get(&3).cloned() {
            key3.sort();
            assert_eq!(key3, vec![24, 104]);
        }
        assert_eq!(allocator.slack.get(&5), Some(&vec![32]));

        // --- Step 4: alloc(16) ---
        // p = 16, slack lookup will now skip key 3 (size 8) and use key 5 (size 32).
        // It pops from key 5: block 32 is reused.
        // External leftover: 32 - 16 = 16, so add_slack(32+16,16) = add_slack(48,16)
        // yields [(48,16)] under key 4.
        let addr4 = allocator.alloc(16);
        assert_eq!(addr4, 32);
        assert_eq!(allocator.pos, 128);
        // After this, key 5 should be removed.
        assert!(!allocator.slack.contains_key(&5));
        // And key 4 should now contain [48].
        assert_eq!(allocator.slack.get(&4), Some(&vec![48]));
        // Key 3 remains unchanged.
        if let Some(mut key3) = allocator.slack.get(&3).cloned() {
            key3.sort();
            assert_eq!(key3, vec![24, 104]);
        }
    }

    #[test]
    fn test_random_allocations_space_efficiency() {
        let mut allocator = VromAllocator::default();
        let mut allocations = Vec::new();
        let mut total_requested = 0u32;
        let mut rng = rand::rng();

        // Generate 1000 random allocations.
        for _ in 0..1000 {
            // Random requested size between 1 and 1024.
            let requested: u32 = rng.random_range(1..=1024);
            total_requested += requested;
            let addr = allocator.alloc(requested);
            allocations.push((addr, requested));
            let padded = requested.next_power_of_two().max(MIN_FRAME_SIZE);
            // Check alignment: allocated address must be a multiple of padded size.
            assert_eq!(
                addr % padded,
                0,
                "Address {} is not aligned to {}",
                addr,
                padded
            );
        }

        // Sort allocations by starting address.
        allocations.sort_by_key(|x| x.0);
        // Check for overlap between allocated ranges.
        for window in allocations.windows(2) {
            let (addr1, requested) = window[0];
            let (addr2, _) = window[1];
            assert!(
                addr1 + requested <= addr2,
                "Allocation {}+{} overlaps with {}",
                addr1,
                requested,
                addr2
            );
        }

        // Check space efficiency: compute ratio of total allocated space vs. total
        // requested. (Note: some overhead is expected due to padding and
        // slack.)
        let ratio = allocator.pos as f64 / total_requested as f64;
        println!(
            "Total allocated: {}, total requested: {}, ratio: {:.3}",
            allocator.pos, total_requested, ratio
        );
    }
}
