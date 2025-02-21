#![allow(clippy::regex_creation_in_loops)]

use std::collections::HashMap;
use std::sync::{LazyLock, OnceLock};

use regex::Regex;
use rustc_hash::FxHashMap;

use feruca_mapper::{BUMP, SHIFT, pack_weights, unpack_weights};

macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new($re).unwrap())
    }};
}

static MAPPING: LazyLock<HashMap<u16, u16>> = LazyLock::new(|| {
    HashMap::from([
        (0x2A69, 0x2381 + SHIFT),        // Alif madda
        (0x2A6A, 0x2382 + SHIFT),        // Alif hamza above
        (0x2A6E, 0x2383 + SHIFT),        // Alif hamza below
        (0x2A76, 0x2384 + SHIFT),        // Alif
        (0x2A78, 0x239B + BUMP + SHIFT), // Ba
        (0x2AA9, 0x23CB + BUMP + SHIFT), // Dal
        (0x2AAA, 0x23CC + BUMP + SHIFT), // Dhal
        (0x2AD8, 0x23CD + BUMP + SHIFT), // Ḍ
        (0x2AED, 0x2423 + BUMP + SHIFT), // Fa
        (0x2AE5, 0x2432 + BUMP + SHIFT), // Gh
        (0x2A9E, 0x2459 + SHIFT),        // Ḥ
        (0x2B30, 0x245A + SHIFT),        // Ha
        (0x2A93, 0x2490 + SHIFT),        // Jim
        (0x2A9F, 0x24A9 + SHIFT),        // Kh
        (0x2B00, 0x24AA + SHIFT),        // Kaf
        (0x2B19, 0x24BD + SHIFT),        // Lam
        (0x2B21, 0x24F7 + SHIFT),        // Mim
        (0x2B25, 0x2506 + SHIFT),        // Nun
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
        (0x2ABA, 0x2683 + SHIFT),        // Za
        (0x2ADE, 0x2684 + SHIFT),        // Ẓ
    ])
});

pub fn map_arabic_interleaved_multi() {
    // This is based on the CLDR table, of course
    let data = std::fs::read_to_string("unicode-data/cldr-46_1/allkeys_CLDR.txt").unwrap();

    let mut map: FxHashMap<Vec<u32>, Vec<u32>> = FxHashMap::default();

    for line in data.lines() {
        if line.is_empty() || line.starts_with('@') || line.starts_with('#') {
            continue;
        }

        let mut split_at_semicolon = line.split(';');
        let left_of_semicolon = split_at_semicolon.next().unwrap();
        let right_of_semicolon = split_at_semicolon.next().unwrap();
        let left_of_hash = right_of_semicolon.split('#').next().unwrap();

        let mut k = Vec::new();
        let re_key = regex!(r"[\dA-F]{4,5}");
        for m in re_key.find_iter(left_of_semicolon) {
            let as_u32 = u32::from_str_radix(m.as_str(), 16).unwrap();
            k.push(as_u32);
        }

        // Here we're only looking for multi-code-point lines
        if k.len() < 2 {
            continue;
        }

        let mut v: Vec<u32> = Vec::new();
        let re_weights = regex!(r"[*.\dA-F]{15}");
        let re_value = regex!(r"[\dA-F]{4}");

        for m in re_weights.find_iter(left_of_hash) {
            let weights_str = m.as_str();

            let variable = weights_str.starts_with('*');

            let mut vals = re_value.find_iter(weights_str);

            let primary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
            let secondary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
            let tertiary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();

            let weights = pack_weights(variable, primary, secondary, tertiary);

            v.push(weights);
        }

        // Up to this point, we haven't been so selective. We've taken any multi-code-point
        // sequence and the corresponding Vec of Weights. But we need to check to make sure there
        // is at least one Arabic-block primary weight. Otherwise we continue.

        let mut arabic = false;

        for weights in &v {
            let (_, primary, _, _) = unpack_weights(*weights);

            if MAPPING.contains_key(&primary) {
                arabic = true;
                break;
            }
        }

        if !arabic {
            continue;
        }

        // Then we look again for any Arabic-block primary weight, and shift it down to fit in the
        // space before the Latin script.

        for weights in &mut v {
            let (variable, primary, secondary, tertiary) = unpack_weights(*weights);

            if let Some(new_primary) = MAPPING.get(&primary) {
                *weights = pack_weights(variable, *new_primary, secondary, tertiary);
            }
        }

        map.insert(k, v);
    }

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write(
        "bincode/cldr-46_1/tailoring/arabic_interleaved_multi",
        bytes,
    )
    .unwrap();
}

pub fn map_arabic_interleaved_sing() {
    // This is based on the CLDR table, of course
    let data = std::fs::read_to_string("unicode-data/cldr-46_1/allkeys_CLDR.txt").unwrap();

    let mut map: FxHashMap<u32, Vec<u32>> = FxHashMap::default();

    for line in data.lines() {
        if line.is_empty() || line.starts_with('@') || line.starts_with('#') {
            continue;
        }

        let mut split_at_semicolon = line.split(';');
        let left_of_semicolon = split_at_semicolon.next().unwrap();
        let right_of_semicolon = split_at_semicolon.next().unwrap();
        let left_of_hash = right_of_semicolon.split('#').next().unwrap();

        let mut points = Vec::new();
        let re_key = regex!(r"[\dA-F]{4,5}");
        for m in re_key.find_iter(left_of_semicolon) {
            let as_u32 = u32::from_str_radix(m.as_str(), 16).unwrap();
            points.push(as_u32);
        }

        // Here we're only looking for single-code-point lines
        if points.len() > 1 {
            continue;
        }

        let k = points[0];

        let mut v: Vec<u32> = Vec::new();
        let re_weights = regex!(r"[*.\dA-F]{15}");
        let re_value = regex!(r"[\dA-F]{4}");

        for m in re_weights.find_iter(left_of_hash) {
            let weights_str = m.as_str();

            let variable = weights_str.starts_with('*');

            let mut vals = re_value.find_iter(weights_str);

            let primary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
            let secondary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
            let tertiary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();

            let weights = pack_weights(variable, primary, secondary, tertiary);

            v.push(weights);
        }

        // Up to this point, we haven't been so selective. We've taken any single code point and
        // the corresponding Vec of Weights. But we need to check to make sure there is at least
        // one Arabic-block primary weight. Otherwise we continue.

        let mut arabic = false;

        for weights in &v {
            let (_, primary, _, _) = unpack_weights(*weights);

            if MAPPING.contains_key(&primary) {
                arabic = true;
                break;
            }
        }

        if !arabic {
            continue;
        }

        // Then we look again for any Arabic-block primary weight, and shift it down to fit in the
        // space before the Latin script.

        for weights in &mut v {
            let (variable, primary, secondary, tertiary) = unpack_weights(*weights);

            if let Some(new_primary) = MAPPING.get(&primary) {
                *weights = pack_weights(variable, *new_primary, secondary, tertiary);
            }
        }

        map.insert(k, v);
    }

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write("bincode/cldr-46_1/tailoring/arabic_interleaved_sing", bytes).unwrap();
}
