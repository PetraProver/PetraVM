#[derive(Debug)]
pub(crate) enum MemoryError {
    VromRewrite(u32),
    VromMisaligned(u8, u32),
    VromMissingValue(u32),
}
