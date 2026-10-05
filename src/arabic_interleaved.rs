use crate::arabic_tailoring::collect_entries;
use feruca_mapper::{BUMP, SHIFT, pack_code_points, write_trie};
use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::sync::LazyLock;

static MAPPING: LazyLock<HashMap<u16, u16>> = LazyLock::new(|| {
    HashMap::from([
        (0x2BAE, 0x2486 + SHIFT),        // Alif madda
        (0x2BAF, 0x2487 + SHIFT),        // Alif hamza above
        (0x2BB3, 0x2488 + SHIFT),        // Alif hamza below
        (0x2BBB, 0x2489 + SHIFT),        // Alif
        (0x2BBD, 0x24A5 + BUMP + SHIFT), // Ba
        (0x2BDC, 0x24C0 + BUMP + SHIFT), // Che
        (0x2BEE, 0x24D9 + BUMP + SHIFT), // Dal
        (0x2BEF, 0x24DA + BUMP + SHIFT), // Dhal
        (0x2C1D, 0x24DB + BUMP + SHIFT), // Ḍ
        (0x2C32, 0x2538 + BUMP + SHIFT), // Fa
        (0x2C2A, 0x2547 + BUMP + SHIFT), // Gh
        (0x2C4F, 0x2548 + BUMP + SHIFT), // Gaf
        (0x2BE3, 0x2571 + SHIFT),        // Ḥ
        (0x2C76, 0x2572 + SHIFT),        // Ha
        (0x2BD8, 0x25AE + SHIFT),        // Jim
        (0x2BE4, 0x25C7 + SHIFT),        // Kh
        (0x2C45, 0x25C8 + SHIFT),        // Kaf
        (0x2C46, 0x25C9 + SHIFT),        // Kaf (Persian)
        (0x2C5E, 0x25DC + SHIFT),        // Lam
        (0x2C66, 0x2617 + SHIFT),        // Mim
        (0x2C6A, 0x2628 + SHIFT),        // Nun
        (0x2BBF, 0x2688 + SHIFT),        // Pe
        (0x2C3E, 0x269E + SHIFT),        // Qaf
        (0x2BFE, 0x26B2 + SHIFT),        // Ra
        (0x2C11, 0x26FB + SHIFT),        // Sin
        (0x2C12, 0x26FC + SHIFT),        // Shin
        (0x2C1C, 0x26FD + SHIFT),        // Ṣ
        (0x2BCE, 0x2727 + SHIFT),        // Ta
        (0x2BCF, 0x2728 + SHIFT),        // Tha
        (0x2C22, 0x2729 + SHIFT),        // Ṭ
        (0x2C7C, 0x2794 + SHIFT),        // Waw
        (0x2C8B, 0x27AD + SHIFT),        // Ya
        (0x2C8C, 0x27AE + SHIFT),        // Ya (Persian)
        (0x2BFF, 0x27C4 + SHIFT),        // Za
        (0x2C07, 0x27C5 + SHIFT),        // Zhe
        (0x2C23, 0x27C6 + SHIFT),        // Ẓ
    ])
});

pub fn map_arabic_interleaved_trie(
    cldr_singles: &FxHashMap<u32, Box<[u32]>>,
    cldr_multis: &FxHashMap<u64, Box<[u32]>>,
) {
    let mut singles = cldr_singles.clone();
    singles.extend(collect_arabic_interleaved_singles());

    let mut multis = cldr_multis.clone();
    multis.extend(collect_arabic_interleaved_multis());

    write_trie(
        "bincode/cldr-49-beta2/tailoring/arabic_interleaved",
        &singles,
        &multis,
    );
}

fn collect_arabic_interleaved_multis() -> FxHashMap<u64, Box<[u32]>> {
    collect_entries(
        |points| points.len() >= 2,
        pack_code_points,
        map_arabic_interleaved_primary,
    )
}

fn collect_arabic_interleaved_singles() -> FxHashMap<u32, Box<[u32]>> {
    collect_entries(
        |points| points.len() == 1,
        |points| points[0],
        map_arabic_interleaved_primary,
    )
}

fn map_arabic_interleaved_primary(primary: u16) -> Option<u16> {
    MAPPING.get(&primary).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use feruca::Tailoring;
    use feruca_mapper::{collect_singles, unpack_weights};
    use std::collections::HashSet;

    #[test]
    fn destinations_are_distinct_and_do_not_collide_with_root_weights() {
        let destinations: HashSet<_> = MAPPING.values().copied().collect();
        assert_eq!(destinations.len(), MAPPING.len());

        let singles = collect_singles(Tailoring::default());
        for row in singles.values() {
            for &packed in row {
                let primary = unpack_weights(packed).1;
                assert!(
                    !destinations.contains(&primary),
                    "Arabic interleaving destination collides with root primary {primary:04X}"
                );
            }
        }
    }
}
