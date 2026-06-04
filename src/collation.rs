#![allow(clippy::missing_panics_doc, clippy::regex_creation_in_loops)]

use crate::regex;
use feruca::Tailoring;
use rustc_hash::FxHashMap;
use std::{hash::Hash, sync::LazyLock};

pub static KEYS_DUCET: LazyLock<String> =
    LazyLock::new(|| std::fs::read_to_string("unicode-data/cldr-46_1/allkeys.txt").unwrap());

pub static KEYS_CLDR: LazyLock<String> =
    LazyLock::new(|| std::fs::read_to_string("unicode-data/cldr-46_1/allkeys_CLDR.txt").unwrap());

// This adjustment affects only the low and singles maps
pub const BUMP: u16 = 1;
const BUMP_START: u16 = 0x2384; // Latin small capital A
const BUMP_END: u16 = 0x2454; // Small gap above this, before Latin H, that we can use

pub const SHIFT: u16 = 0x400;
const SHIFT_START: u16 = 0x2380; // Latin script begins
const SHIFT_END: u16 = 0x72B6; // Large gap above this that we can use

const SEC_MAX: u16 = 0x126; // Largest secondary weight that is actually used
const TER_MAX: u16 = 0x1E; // Largest tertiary weight that is actually used

pub fn map_low(keys: Tailoring) {
    let cldr = keys != Tailoring::Ducet;

    let data = if cldr { &KEYS_CLDR } else { &KEYS_DUCET };

    let re_set_of_weights = regex!(r"[*.\dA-F]{15}");
    let re_individual_weight = regex!(r"[\dA-F]{4}");

    let mut map: FxHashMap<u32, u32> = FxHashMap::default();

    for line in data.lines() {
        if line.is_empty() || line.starts_with('@') || line.starts_with('#') {
            continue;
        }

        let mut split_at_semicolon = line.split(';');
        let left_of_semicolon = split_at_semicolon.next().unwrap();
        let right_of_semicolon = split_at_semicolon.next().unwrap();
        let left_of_hash = right_of_semicolon.split('#').next().unwrap();

        let re_key = regex!(r"[\dA-F]{4,5}");

        let first_cp = re_key.find(left_of_semicolon).unwrap().as_str();
        let cp = u32::from_str_radix(first_cp, 16).unwrap();

        // Skip capital and lowercase L; it's problematic
        if cp > 0xB6 || cp == 0x4C || cp == 0x6C {
            continue;
        }

        let first_set = re_set_of_weights.find(left_of_hash).unwrap().as_str();

        let variable = first_set.starts_with('*');

        let mut weights = re_individual_weight.find_iter(first_set);

        let mut primary = u16::from_str_radix(weights.next().unwrap().as_str(), 16).unwrap();
        if cldr && (BUMP_START..=BUMP_END).contains(&primary) {
            primary += BUMP;
        }
        if cldr && (SHIFT_START..=SHIFT_END).contains(&primary) {
            primary += SHIFT;
        }

        let secondary = u16::from_str_radix(weights.next().unwrap().as_str(), 16).unwrap();
        assert!(secondary <= SEC_MAX);

        let tertiary = u16::from_str_radix(weights.next().unwrap().as_str(), 16).unwrap();
        assert!(tertiary <= TER_MAX);

        let packed = pack_weights(variable, primary, secondary, tertiary);

        map.insert(cp, packed);
    }

    // Since we have 181 code points with values in the range 0..183, we can put the associated
    // collation weights into an array such that the index is the code point value.
    let mut arr = [0u32; 183];
    for kv in &map {
        arr[*kv.0 as usize] = *kv.1;
    }

    for (i, &v) in arr.iter().enumerate() {
        let map_val = map.get(&u32::try_from(i).unwrap()).unwrap_or(&0);
        assert_eq!(v, *map_val);
    }

    // Write to JSON only in this case; we'll copy-paste the values into feruca source code
    let path_out = if cldr {
        "json/cldr-46_1/low_cldr.json"
    } else {
        "json/cldr-46_1/low.json"
    };

    let json_bytes = serde_json::to_vec(arr.as_slice()).unwrap();
    std::fs::write(path_out, json_bytes).unwrap();
}

#[must_use]
pub fn collect_multis(keys: Tailoring) -> FxHashMap<u64, Box<[u32]>> {
    collect_entries(keys, |points| points.len() >= 2, pack_code_points, false)
}

#[must_use]
pub fn collect_singles(keys: Tailoring) -> FxHashMap<u32, Box<[u32]>> {
    collect_entries(keys, |points| points.len() == 1, |points| points[0], true)
}

fn collect_entries<K>(
    keys: Tailoring,
    include_points: impl Fn(&[u32]) -> bool,
    pack_key: impl Fn(&[u32]) -> K,
    bump: bool,
) -> FxHashMap<K, Box<[u32]>>
where
    K: Eq + Hash,
{
    let cldr = keys != Tailoring::Ducet;
    let data = if cldr { &KEYS_CLDR } else { &KEYS_DUCET };
    let mut map: FxHashMap<K, Box<[u32]>> = FxHashMap::default();

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
            points.push(u32::from_str_radix(m.as_str(), 16).unwrap());
        }

        if !include_points(&points) {
            continue;
        }

        map.insert(pack_key(&points), parse_weights(left_of_hash, cldr, bump));
    }

    map
}

fn parse_weights(left_of_hash: &str, cldr: bool, bump: bool) -> Box<[u32]> {
    let mut v = Vec::new();
    let re_weights = regex!(r"[*.\dA-F]{15}");
    let re_value = regex!(r"[\dA-F]{4}");

    for m in re_weights.find_iter(left_of_hash) {
        let weights_str = m.as_str();
        let variable = weights_str.starts_with('*');
        let mut vals = re_value.find_iter(weights_str);

        let mut primary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
        if cldr && bump && (BUMP_START..=BUMP_END).contains(&primary) {
            primary += BUMP;
        }
        if cldr && (SHIFT_START..=SHIFT_END).contains(&primary) {
            primary += SHIFT;
        }

        let secondary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
        assert!(secondary <= SEC_MAX);

        let tertiary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
        assert!(tertiary <= TER_MAX);

        v.push(pack_weights(variable, primary, secondary, tertiary));
    }

    v.into_boxed_slice()
}

#[must_use]
pub fn pack_code_points(code_points: &[u32]) -> u64 {
    match code_points.len() {
        2 => (u64::from(code_points[0]) << 21) | u64::from(code_points[1]),
        3 => {
            (u64::from(code_points[0]) << 42)
                | (u64::from(code_points[1]) << 21)
                | u64::from(code_points[2])
        }
        _ => unreachable!(),
    }
}

pub fn unpack_code_points(packed: u64) -> Vec<u32> {
    if packed >> 42 == 0 {
        vec![
            u32::try_from(packed >> 21).unwrap(),
            u32::try_from(packed & 0x1F_FFFF).unwrap(),
        ]
    } else {
        vec![
            u32::try_from(packed >> 42).unwrap(),
            u32::try_from((packed >> 21) & 0x1F_FFFF).unwrap(),
            u32::try_from(packed & 0x1F_FFFF).unwrap(),
        ]
    }
}

#[must_use]
pub const fn pack_weights(variable: bool, primary: u16, secondary: u16, tertiary: u16) -> u32 {
    let upper = (primary as u32) << 16;

    let v_int = variable as u16;
    let lower = (v_int << 15) | (tertiary << 9) | secondary;

    upper | (lower as u32)
}

#[must_use]
pub const fn unpack_weights(packed: u32) -> (bool, u16, u16, u16) {
    let primary = (packed >> 16) as u16;

    let lower = (packed & 0xFFFF) as u16;
    let variable = lower >> 15 == 1;
    let secondary = lower & 0b1_1111_1111;
    let tertiary = (lower >> 9) & 0b11_1111;

    (variable, primary, secondary, tertiary)
}
