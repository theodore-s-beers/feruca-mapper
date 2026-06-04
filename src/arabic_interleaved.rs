use crate::arabic_tailoring::collect_entries;
use feruca_mapper::{BUMP, SHIFT, pack_code_points, write_trie};
use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::sync::LazyLock;

static MAPPING: LazyLock<HashMap<u16, u16>> = LazyLock::new(|| {
    HashMap::from([
        (0x2A69, 0x2381 + SHIFT),        // Alif madda
        (0x2A6A, 0x2382 + SHIFT),        // Alif hamza above
        (0x2A6E, 0x2383 + SHIFT),        // Alif hamza below
        (0x2A76, 0x2384 + SHIFT),        // Alif
        (0x2A78, 0x239B + BUMP + SHIFT), // Ba
        (0x2A97, 0x23B5 + BUMP + SHIFT), // Che
        (0x2AA9, 0x23CB + BUMP + SHIFT), // Dal
        (0x2AAA, 0x23CC + BUMP + SHIFT), // Dhal
        (0x2AD8, 0x23CD + BUMP + SHIFT), // Ḍ
        (0x2AED, 0x2423 + BUMP + SHIFT), // Fa
        (0x2AE5, 0x2432 + BUMP + SHIFT), // Gh
        (0x2B0A, 0x2433 + BUMP + SHIFT), // Gaf
        (0x2A9E, 0x2459 + SHIFT),        // Ḥ
        (0x2B30, 0x245A + SHIFT),        // Ha
        (0x2A93, 0x2490 + SHIFT),        // Jim
        (0x2A9F, 0x24A9 + SHIFT),        // Kh
        (0x2B00, 0x24AA + SHIFT),        // Kaf
        (0x2B01, 0x24AB + SHIFT),        // Kaf (Persian)
        (0x2B19, 0x24BD + SHIFT),        // Lam
        (0x2B21, 0x24F7 + SHIFT),        // Mim
        (0x2B25, 0x2506 + SHIFT),        // Nun
        (0x2A7A, 0x255D + SHIFT),        // Pe
        (0x2AF9, 0x2572 + SHIFT),        // Qaf
        (0x2AB9, 0x2585 + SHIFT),        // Ra
        (0x2ACC, 0x25C7 + SHIFT),        // Sin
        (0x2ACD, 0x25C8 + SHIFT),        // Shin
        (0x2AD7, 0x25C9 + SHIFT),        // Ṣ
        (0x2A89, 0x25F2 + SHIFT),        // Ta
        (0x2A8A, 0x25F3 + SHIFT),        // Tha
        (0x2ADD, 0x25F4 + SHIFT),        // Ṭ
        (0x2B36, 0x2657 + SHIFT),        // Waw
        (0x2B45, 0x266D + SHIFT),        // Ya
        (0x2B46, 0x266E + SHIFT),        // Ya (Persian)
        (0x2ABA, 0x2683 + SHIFT),        // Za
        (0x2AC2, 0x2684 + SHIFT),        // Zhe
        (0x2ADE, 0x2685 + SHIFT),        // Ẓ
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
        "bincode/cldr-46_1/tailoring/arabic_interleaved",
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
