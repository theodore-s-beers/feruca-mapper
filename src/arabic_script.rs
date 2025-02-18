#![allow(clippy::regex_creation_in_loops)]

use feruca_mapper::{pack_weights, unpack_weights};
use once_cell::sync::OnceCell;
use regex::Regex;
use rustc_hash::FxHashMap;

macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: OnceCell<Regex> = OnceCell::new();
        RE.get_or_init(|| Regex::new($re).unwrap())
    }};
}

const FIRST_ARABIC_PRIMARY: u16 = 0x2A68; // 0621, "ARABIC LETTER HAMZA"
const LAST_ARABIC_PRIMARY: u16 = 0x2B56; // 088E, "ARABIC VERTICAL TAIL"
const OFFSET: u16 = 2_010; // This is tested below

pub fn map_arabic_script_multi() {
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

            if (FIRST_ARABIC_PRIMARY..=LAST_ARABIC_PRIMARY).contains(&primary) {
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

            if (FIRST_ARABIC_PRIMARY..=LAST_ARABIC_PRIMARY).contains(&primary) {
                let new_primary = primary - OFFSET;
                *weights = pack_weights(variable, new_primary, secondary, tertiary);
            }
        }

        map.insert(k, v);
    }

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write("bincode/cldr-46_1/tailoring/arabic_script_multi", bytes).unwrap();
}

pub fn map_arabic_script_sing() {
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

            if (FIRST_ARABIC_PRIMARY..=LAST_ARABIC_PRIMARY).contains(&primary) {
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

            if (FIRST_ARABIC_PRIMARY..=LAST_ARABIC_PRIMARY).contains(&primary) {
                let new_primary = primary - OFFSET;
                *weights = pack_weights(variable, new_primary, secondary, tertiary);
            }
        }

        map.insert(k, v);
    }

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write("bincode/cldr-46_1/tailoring/arabic_script_sing", bytes).unwrap();
}

#[cfg(test)]
mod tests {
    #![allow(clippy::assertions_on_constants)]

    use super::*;

    const LAST_PRIMARY_BEFORE_LATIN: u16 = 0x2186;
    const FIRST_LATIN_PRIMARY: u16 = 0x2380; // 0061, "LATIN SMALL LETTER A"

    #[test]
    fn verify_offset() {
        assert!((FIRST_ARABIC_PRIMARY - OFFSET) > LAST_PRIMARY_BEFORE_LATIN);
        assert!((LAST_ARABIC_PRIMARY - OFFSET) < FIRST_LATIN_PRIMARY);
    }
}
