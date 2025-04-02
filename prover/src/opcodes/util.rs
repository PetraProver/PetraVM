use binius_field::ExtensionField;
use binius_m3::builder::{upcast_expr, Expr, B128, B16, B32, B64};

// TODO: Maybe this functions should be replaced by impl From<Expr<T>, N> for
// Expr<S, M> for the right S, M s
pub(crate) fn pack_b16_into_b32(limbs: [Expr<B16, 1>; 2]) -> Expr<B32, 1> {
    limbs
        .into_iter()
        .enumerate()
        .map(|(i, limb)| upcast_expr(limb) * <B32 as ExtensionField<B16>>::basis(i))
        .reduce(|a, b| a + b)
        .expect("limbs has length 2")
}

pub(crate) fn pack_b16_into_b64(limbs: [Expr<B16, 1>; 4]) -> Expr<B64, 1> {
    let instruction = limbs.into_iter().map(upcast_expr).collect::<Vec<_>>();
    instruction
        .into_iter()
        .enumerate()
        .map(|(i, limb)| limb * <B64 as ExtensionField<B16>>::basis(i))
        .reduce(|a, b| a + b)
        .expect("instruction has length 4")
}

pub(crate) fn pack_b32_into_b64(limbs: [Expr<B32, 1>; 2]) -> Expr<B64, 1> {
    limbs
        .into_iter()
        .enumerate()
        .map(|(i, limb)| upcast_expr(limb) * <B64 as ExtensionField<B32>>::basis(i))
        .reduce(|a, b| a + b)
        .expect("limbs has length 2")
}

pub(crate) fn pack_b64_into_b128(limbs: [Expr<B64, 1>; 2]) -> Expr<B128, 1> {
    let instruction = limbs.into_iter().map(upcast_expr).collect::<Vec<_>>();
    instruction
        .into_iter()
        .enumerate()
        .map(|(i, limb)| limb * <B128 as ExtensionField<B64>>::basis(i))
        .reduce(|a, b| a + b)
        .expect("instruction has length 2")
}
