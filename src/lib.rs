#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::cast_lossless,
    clippy::missing_panics_doc,
    clippy::regex_creation_in_loops
)]

use feruca::Tailoring;
use regex::Regex;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::{LazyLock, OnceLock};
use unicode_canonical_combining_class::get_canonical_combining_class_u32 as get_ccc;

const KEYS_DUCET: &str = "unicode-data/cldr-46_1/allkeys.txt";
pub const KEYS_CLDR: &str = "unicode-data/cldr-46_1/allkeys_CLDR.txt";
const UNI_DATA: &str = "unicode-data/cldr-46_1/UnicodeData.txt";

const SEC_MAX: u16 = 0x126; // Largest secondary weight that is actually used
const TER_MAX: u16 = 0x1E; // Largest tertiary weight that is actually used

// This adjustment affects only the low and singles maps
const BUMP_START: u16 = 0x2384; // Latin small capital A
const BUMP_END: u16 = 0x2454; // Small gap above this, before Latin H, that we can use
pub const BUMP: u16 = 1;

const SHIFT_START: u16 = 0x2380; // Latin script begins
const SHIFT_END: u16 = 0x72B6; // Large gap above this that we can use
pub const SHIFT: u16 = 0x400;

// The output of map_decomps is needed for map_fcd
static DECOMP: LazyLock<FxHashMap<u32, Vec<u32>>> = LazyLock::new(|| {
    let data = std::fs::read("bincode/cldr-46_1/decomp").unwrap();
    let decoded: FxHashMap<u32, Vec<u32>> = bincode::deserialize(&data).unwrap();
    decoded
});

#[macro_export]
macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new($re).unwrap())
    }};
}

pub fn map_decomps() {
    let data = std::fs::read_to_string(UNI_DATA).unwrap();

    let mut map: FxHashMap<u32, Vec<u32>> = FxHashMap::default();

    for line in data.lines() {
        if line.is_empty() {
            continue;
        }

        let splits: Vec<&str> = line.split(';').collect();

        let code_point = u32::from_str_radix(splits[0], 16).unwrap();

        // Ignore these ranges
        if (0x3400..=0x4DBF).contains(&code_point) // CJK ext A
            || (0x4E00..=0x9FFF).contains(&code_point) // CJK
            || (0xAC00..=0xD7A3).contains(&code_point)  // Hangul
            || (0xD800..=0xDFFF).contains(&code_point) // Surrogates
            || (0xE000..=0xF8FF).contains(&code_point)  // Private use
            || (0x17000..=0x187F7).contains(&code_point) // Tangut
            || (0x18D00..=0x18D08).contains(&code_point) // Tangut suppl
            || (0x20000..=0x2A6DF).contains(&code_point) // CJK ext B
            || (0x2A700..=0x2B738).contains(&code_point) // CJK ext C
            || (0x2B740..=0x2B81D).contains(&code_point) // CJK ext D
            || (0x2B820..=0x2CEA1).contains(&code_point) // CJK ext E
            || (0x2CEB0..=0x2EBE0).contains(&code_point) // CJK ext F
            || (0x30000..=0x3134A).contains(&code_point) // CJK ext G
            || (0xF0000..=0xFFFFD).contains(&code_point) // Plane 15 private use
            // Plane 16 private use
            || (1_048_576..=1_114_109).contains(&code_point)
        {
            continue;
        }

        let decomp_col = splits[5];

        let re = regex!(r"[\dA-F]{4,5}");

        let mut decomp: Vec<u32> = Vec::new();

        for m in re.find_iter(decomp_col) {
            let code_point = u32::from_str_radix(m.as_str(), 16).unwrap();
            decomp.push(code_point);
        }

        let final_decomp = if decomp_col.contains('<') {
            continue; // Non-canonical decomposition; continue
        } else if decomp.len() > 1 {
            // Multi-code-point canonical decomposition; recurse badly
            decomp
                .into_iter()
                .flat_map(|c| {
                    let as_str = format!("{c:04X}");
                    get_canonical_decomp(&as_str)
                })
                .collect()
        } else if decomp.len() == 1 {
            // Single-code-point canonical decomposition; recurse simply
            get_canonical_decomp(splits[0])
        } else {
            continue; // No decomposition; continue
        };

        map.insert(code_point, final_decomp);
    }

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write("bincode/cldr-46_1/decomp", bytes).unwrap();
}

fn get_canonical_decomp(code_point: &str) -> Vec<u32> {
    let data = std::fs::read_to_string(UNI_DATA).unwrap();

    for line in data.lines() {
        if line.starts_with(code_point) {
            let decomp_col = line.split(';').nth(5).unwrap();

            // Further decomposition is non-canonical; return the code point itself
            if decomp_col.contains('<') {
                return vec![u32::from_str_radix(code_point, 16).unwrap()];
            }

            let re = regex!(r"[\dA-F]{4,5}");

            let mut decomp: Vec<u32> = Vec::new();

            for m in re.find_iter(decomp_col) {
                let cp_val = u32::from_str_radix(m.as_str(), 16).unwrap();
                decomp.push(cp_val);
            }

            // Further multiple-code-point decomposition; recurse badly
            if decomp.len() > 1 {
                return decomp
                    .into_iter()
                    .flat_map(|c| {
                        let as_str = format!("{c:04X}");
                        get_canonical_decomp(&as_str)
                    })
                    .collect();
            }

            // Further single-code-point decomposition; recurse simply
            if decomp.len() == 1 {
                let as_str = format!("{:04X}", decomp[0]);
                return get_canonical_decomp(&as_str);
            }

            // No further decomposition; return the code point itself
            return vec![u32::from_str_radix(code_point, 16).unwrap()];
        }
    }

    // This means we followed a canonical decomposition to a single code point that was then not
    // found in the first column of the table. Return it, I guess?
    vec![u32::from_str_radix(code_point, 16).unwrap()]
}

pub fn map_fcd() {
    let data = std::fs::read_to_string(UNI_DATA).unwrap();

    let mut map: FxHashMap<u32, u16> = FxHashMap::default();

    for line in data.lines() {
        if line.is_empty() {
            continue;
        }

        let left_of_semicolon = line.split(';').next().unwrap();

        let code_point = u32::from_str_radix(left_of_semicolon, 16).unwrap();

        // Ignore these ranges
        if (0x3400..=0x4DBF).contains(&code_point) // CJK ext A
            || (0x4E00..=0x9FFF).contains(&code_point) // CJK
            || (0xAC00..=0xD7A3).contains(&code_point)  // Hangul
            || (0xD800..=0xDFFF).contains(&code_point) // Surrogates
            || (0xE000..=0xF8FF).contains(&code_point)  // Private use
            || (0x17000..=0x187F7).contains(&code_point) // Tangut
            || (0x18D00..=0x18D08).contains(&code_point) // Tangut suppl
            || (0x20000..=0x2A6DF).contains(&code_point) // CJK ext B
            || (0x2A700..=0x2B738).contains(&code_point) // CJK ext C
            || (0x2B740..=0x2B81D).contains(&code_point) // CJK ext D
            || (0x2B820..=0x2CEA1).contains(&code_point) // CJK ext E
            || (0x2CEB0..=0x2EBE0).contains(&code_point) // CJK ext F
            || (0x30000..=0x3134A).contains(&code_point) // CJK ext G
            || (0xF0000..=0xFFFFD).contains(&code_point) // Plane 15 private use
            // Plane 16 private use
            || (1_048_576..=1_114_109).contains(&code_point)
        {
            continue;
        }

        let Some(canon_decomp) = DECOMP.get(&code_point) else {
            continue;
        };

        let first_cc = get_ccc(canon_decomp[0]) as u8;
        let last_cc = get_ccc(canon_decomp[canon_decomp.len() - 1]) as u8;

        let packed = ((first_cc as u16) << 8) | (last_cc as u16);

        map.insert(code_point, packed);
    }

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write("bincode/cldr-46_1/fcd", bytes).unwrap();
}

pub fn map_low(keys: Tailoring) {
    let cldr = keys != Tailoring::Ducet;

    let path_in = if cldr { KEYS_CLDR } else { KEYS_DUCET };
    let data = std::fs::read_to_string(path_in).unwrap();

    let re_set_of_weights = regex!(r"[*.\dA-F]{15}");
    let re_individual_weight = regex!(r"[\dA-F]{4}");

    let mut map: FxHashMap<u32, u32> = FxHashMap::default();

    // This is for code points under 183 (decimal)
    for i in 0..183 {
        // Skip capital and lowercase L; it's problematic
        if i == 76 || i == 108 {
            continue;
        }

        let as_hex = format!("{i:04X}");

        // Find the line. Yeah, this is slow, but whatever.
        for line in data.lines() {
            if line.starts_with(&as_hex) {
                let set = re_set_of_weights.find(line).unwrap().as_str();

                let variable = set.starts_with('*');

                let mut weights = re_individual_weight.find_iter(set);

                let mut primary =
                    u16::from_str_radix(weights.next().unwrap().as_str(), 16).unwrap();
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

                map.insert(i, packed);

                break;
            }
        }
    }

    let path_out = if cldr {
        "bincode/cldr-46_1/low_cldr"
    } else {
        "bincode/cldr-46_1/low"
    };

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write(path_out, bytes).unwrap();
}

pub fn map_multi(keys: Tailoring) {
    let cldr = keys != Tailoring::Ducet;

    let path_in = if cldr { KEYS_CLDR } else { KEYS_DUCET };
    let data = std::fs::read_to_string(path_in).unwrap();

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

            let mut primary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
            if cldr && (SHIFT_START..=SHIFT_END).contains(&primary) {
                primary += SHIFT;
            }

            let secondary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
            assert!(secondary <= SEC_MAX);

            let tertiary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
            assert!(tertiary <= TER_MAX);

            let weights = pack_weights(variable, primary, secondary, tertiary);
            assert_eq!(
                unpack_weights(weights),
                (variable, primary, secondary, tertiary)
            );

            v.push(weights);
        }

        map.insert(k, v);
    }

    let path_out = if cldr {
        "bincode/cldr-46_1/multis_cldr"
    } else {
        "bincode/cldr-46_1/multis"
    };

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write(path_out, bytes).unwrap();
}

pub fn map_sing(keys: Tailoring) {
    let cldr = keys != Tailoring::Ducet;

    let path_in = if cldr { KEYS_CLDR } else { KEYS_DUCET };
    let data = std::fs::read_to_string(path_in).unwrap();

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

            let mut primary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
            if cldr && (BUMP_START..=BUMP_END).contains(&primary) {
                primary += BUMP;
            }
            if cldr && (SHIFT_START..=SHIFT_END).contains(&primary) {
                primary += SHIFT;
            }

            let secondary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
            assert!(secondary <= SEC_MAX);

            let tertiary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
            assert!(tertiary <= TER_MAX);

            let weights = pack_weights(variable, primary, secondary, tertiary);

            v.push(weights);
        }

        map.insert(k, v);
    }

    let path_out = if cldr {
        "bincode/cldr-46_1/singles_cldr"
    } else {
        "bincode/cldr-46_1/singles"
    };

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write(path_out, bytes).unwrap();
}

pub fn map_variable() {
    let mut set: FxHashSet<u32> = FxHashSet::default();

    // We only need to use DUCET for this, since (as far as I can tell from testing) every code
    // point in the CLDR table that has a variable weight or a zero primary weight, also has that
    // in DUCET. But the inverse is not true.
    let data = std::fs::read_to_string(KEYS_DUCET).unwrap();

    'outer: for line in data.lines() {
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

        let re_weights = regex!(r"[*.\dA-F]{15}");
        let re_value = regex!(r"[\dA-F]{4}");

        for m in re_weights.find_iter(left_of_hash) {
            let weights_str = m.as_str();

            let variable = weights_str.starts_with('*');

            let mut vals = re_value.find_iter(weights_str);
            let primary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();

            // We're only interested in code points for which there is a variable weight or a zero
            // primary weight.
            if variable || primary == 0 {
                set.insert(k);
                continue 'outer;
            }
        }
    }

    let bytes = bincode::serialize(&set).unwrap();
    std::fs::write("bincode/cldr-46_1/variable", bytes).unwrap();
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
