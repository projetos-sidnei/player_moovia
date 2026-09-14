//! Ordenação natural (Ep2 < Ep10) — spec §7 regra 5. Delegada ao crate `natord`.

use std::cmp::Ordering;

pub fn compare(a: &str, b: &str) -> Ordering {
    natord::compare_ignore_case(a, b)
}
