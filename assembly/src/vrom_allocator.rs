use std::collections::HashMap;

/// A VROM allocator that allocates addresses using power‐of‐two padded sizes,
/// reusing slack regions when possible.
pub struct VromAllocator {
    /// The next free allocation pointer.
    pos: u32,
    /// Maps a slack block’s exponent (so that size = 2^exponent) to one or more free VROM addresses.
    slack: HashMap<u32, Vec<u32>>,
}

impl VromAllocator {
    /// Create a new allocator.
    pub fn new() -> Self {
        Self {
            pos: 0,
            slack: HashMap::new(),
        }
    }

    /// Allocate a VROM address for an object with the given requested size.
    ///
    /// The object's size is padded to the next power‐of‐two.
    pub fn alloc(&mut self, requested_size: u32) -> u32 {
        // p: the object's padded size (always a power-of-two).
        let p = requested_size.next_power_of_two();
        // k is the exponent such that p == 2^k.
        let k = p.trailing_zeros();
        
        // Search slack table for an available block whose size is at least p.
        // We iterate for exponents from k up to 31 (u32 addresses).
        for exp in k..=31 {
            // Remove the entire slack bucket for this exponent, if any,
            // to avoid holding a mutable reference while calling other methods.
            if let Some(mut blocks) = self.slack.remove(&exp) {
                if let Some(addr) = blocks.pop() {
                    let block_size = 1 << exp;
                    // If there are remaining blocks in the bucket, reinsert them.
                    if !blocks.is_empty() {
                        self.slack.insert(exp, blocks);
                    }
                    let allocated_addr = addr;
                    let leftover = block_size - p;
                    if leftover > 0 {
                        // Now that we no longer hold a mutable borrow on self.slack,
                        // we can safely record the leftover slack.
                        self.add_slack(allocated_addr + p, leftover);
                    }
                    return allocated_addr;
                }
            }
        }

        // No suitable slack block was found.
        // Align the current position to a multiple of p.
        let old_pos = self.pos;
        let aligned_pos = align_to(self.pos, p);
        // If alignment produced a gap, record that gap as slack.
        if aligned_pos > old_pos {
            let gap = aligned_pos - old_pos;
            self.add_slack(old_pos, gap);
        }
        let allocated_addr = aligned_pos;
        // Update the current position.
        self.pos = aligned_pos + p;
        allocated_addr
    }

    /// Record a free (slack) region starting at `addr` with length `size`
    /// by splitting it into power‐of‐two blocks and updating the slack table.
    fn add_slack(&mut self, addr: u32, size: u32) {
        for (block_addr, block_size) in split_into_power_of_two_blocks(addr, size) {
            let exp = block_size.trailing_zeros();
            self.slack.entry(exp).or_default().push(block_addr);
        }
    }
}

/// Align `pos` to the next multiple of `alignment` (which must be a power‐of‐two).
fn align_to(pos: u32, alignment: u32) -> u32 {
    (pos + alignment - 1) & !(alignment - 1)
}

/// Split an interval [addr, addr + size) into power‐of‐two blocks (with proper alignment).
///
/// For example:
/// - split_into_power_of_two_blocks(0, 12) yields [(0, 8), (8, 4)].
/// - split_into_power_of_two_blocks(4, 12) yields [(4, 4), (8, 8)].
fn split_into_power_of_two_blocks(addr: u32, size: u32) -> Vec<(u32, u32)> {
    let mut blocks = Vec::new();
    let mut current_addr = addr;
    let mut remaining = size;
    while remaining > 0 {
        // Determine the largest block allowed by the current address's alignment.
        let alignment_constraint = if current_addr == 0 {
            remaining
        } else {
            // Extract the least significant set bit (i.e. current_addr & (-current_addr))
            current_addr & ((!current_addr).wrapping_add(1))
        };
        // Largest power-of-two not exceeding `remaining`.
        let largest_possible = 1 << (31 - remaining.leading_zeros());
        // Choose the block size as the smaller of the alignment constraint and largest_possible.
        let mut block_size = if alignment_constraint < largest_possible {
            alignment_constraint
        } else {
            largest_possible
        };
        // Ensure block_size does not exceed remaining.
        while block_size > remaining {
            block_size /= 2;
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
        assert_eq!(align_to(0, 8), 0);
        assert_eq!(align_to(3, 8), 8);
        assert_eq!(align_to(8, 8), 8);
        assert_eq!(align_to(9, 8), 16);
    }

    #[test]
    fn test_split_into_power_of_two_blocks() {
        // When the region is already a power-of-two.
        assert_eq!(split_into_power_of_two_blocks(0, 8), vec![(0, 8)]);
        // Region not a power-of-two.
        assert_eq!(split_into_power_of_two_blocks(0, 12), vec![(0, 8), (8, 4)]);
        // Region with nonzero start.
        assert_eq!(split_into_power_of_two_blocks(4, 12), vec![(4, 4), (8, 8)]);
    }

    #[test]
    fn test_alloc_no_slack() {
        let mut allocator = VromAllocator::new();
        // First allocation: request size 3 → padded to 4.
        let addr1 = allocator.alloc(3);
        assert_eq!(addr1, 0);
        assert_eq!(allocator.pos, 4);
        // Second allocation: request size 5 → padded to 8.
        // Since current pos is 4, aligning to 8 gives 8 and creates a gap [4,8) of size 4.
        let addr2 = allocator.alloc(5);
        assert_eq!(addr2, 8);
        assert_eq!(allocator.pos, 16);
        // The gap [4,8) should be recorded as a slack block of size 4 (key = 2, because 4 == 2^2).
        let slack = allocator.slack.get(&2);
        assert!(slack.is_some());
        assert_eq!(slack.unwrap()[0], 4);
    }

    #[test]
    fn test_alloc_with_slack() {
        let mut allocator = VromAllocator::new();
        // Create slack first.
        let addr1 = allocator.alloc(3); // p = 4, allocated at 0.
        assert_eq!(addr1, 0);
        let addr2 = allocator.alloc(5); // p = 8, allocated at 8, gap [4,8) added.
        assert_eq!(addr2, 8);
        // Now, a third allocation: request size 2 → p = 2.
        // Should find slack in B. The slack block [4,8) of size 4 (key = 2) is used.
        let addr3 = allocator.alloc(2);
        assert_eq!(addr3, 4);
        // The leftover from the slack block is (4 - 2 = 2) bytes.
        // This leftover should be recorded in B as a block of size 2 (key = 1).
        let slack_entry = allocator.slack.get(&1);
        assert!(slack_entry.is_some());
        assert_eq!(slack_entry.unwrap()[0], 6); // leftover slack at address 6.
    }

    #[test]
    fn test_alloc_multiple() {
        let mut allocator = VromAllocator::new();
        let a1 = allocator.alloc(1);  // p = 1, allocated at 0, pos becomes 1.
        let a2 = allocator.alloc(2);  // p = 2, alignment from pos=1 yields addr=2 and creates gap [1,2) of size 1.
        let a3 = allocator.alloc(3);  // p = 4, allocated at pos=4, pos becomes 8.
        let a4 = allocator.alloc(1);  // p = 1, should reuse slack: gap [1,2) should be used.
        assert_eq!(a1, 0);
        assert_eq!(a2, 2);
        assert_eq!(a3, 4);
        assert_eq!(a4, 1);
    }
}
