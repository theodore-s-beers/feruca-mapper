use crate::arabic_tailoring::collect_entries;
use feruca_mapper::{pack_code_points, write_trie};
use rustc_hash::FxHashMap;

const FIRST_ARABIC_PRIMARY: u16 = 0x2A68; // 0621, "ARABIC LETTER HAMZA"
const LAST_ARABIC_PRIMARY: u16 = 0x2B56; // 088E, "ARABIC VERTICAL TAIL"
const OFFSET: u16 = 0x600; // This is tested below

pub fn map_arabic_script_trie(
    cldr_singles: &FxHashMap<u32, Box<[u32]>>,
    cldr_multis: &FxHashMap<u64, Box<[u32]>>,
) {
    let mut singles = cldr_singles.clone();
    singles.extend(collect_arabic_script_singles());

    let mut multis = cldr_multis.clone();
    multis.extend(collect_arabic_script_multis());

    write_trie(
        "bincode/cldr-46_1/tailoring/arabic_script",
        &singles,
        &multis,
    );
}

fn collect_arabic_script_multis() -> FxHashMap<u64, Box<[u32]>> {
    collect_entries(
        |points| points.len() >= 2,
        pack_code_points,
        map_arabic_script_primary,
    )
}

fn collect_arabic_script_singles() -> FxHashMap<u32, Box<[u32]>> {
    collect_entries(
        |points| points.len() == 1,
        |points| points[0],
        map_arabic_script_primary,
    )
}

fn map_arabic_script_primary(primary: u16) -> Option<u16> {
    (FIRST_ARABIC_PRIMARY..=LAST_ARABIC_PRIMARY)
        .contains(&primary)
        .then_some(primary - OFFSET)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::assertions_on_constants)]

    use super::*;
    use feruca_mapper::SHIFT;

    const LAST_PRIMARY_BEFORE_LATIN: u16 = 0x237F;
    const FIRST_LATIN_PRIMARY: u16 = 0x2380 + SHIFT; // 0061, "LATIN SMALL LETTER A"

    #[test]
    fn verify_offset() {
        assert!((FIRST_ARABIC_PRIMARY - OFFSET) > LAST_PRIMARY_BEFORE_LATIN);
        assert!((LAST_ARABIC_PRIMARY - OFFSET) < FIRST_LATIN_PRIMARY);
    }
}
