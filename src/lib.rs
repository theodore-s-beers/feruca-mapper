use feruca::KeysSource;
use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use tinyvec::ArrayVec;
use unicode_canonical_combining_class::get_canonical_combining_class_u32 as get_ccc;

// We need this struct from feruca, but I don't want to make it public there
#[derive(
    Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct Weights {
    pub variable: bool,
    pub primary: u16,
    pub secondary: u16,
    pub tertiary: u16,
}

// The output of map_decomps is needed for map_fcd
static DECOMP: Lazy<FxHashMap<u32, Vec<u32>>> = Lazy::new(|| {
    let data = std::fs::read("bincode/decomp").unwrap();
    let decoded: FxHashMap<u32, Vec<u32>> = bincode::deserialize(&data).unwrap();
    decoded
});

macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: OnceCell<Regex> = OnceCell::new();
        RE.get_or_init(|| Regex::new($re).unwrap())
    }};
}

pub fn map_decomps() {
    let data = std::fs::read_to_string("unicode-data/UnicodeData.txt").unwrap();

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
            // Non-canonical decomposition; continue
            continue;
        } else if decomp.len() > 1 {
            // Multi-code-point canonical decomposition; recurse badly
            decomp
                .into_iter()
                .flat_map(|c| {
                    let as_str = format!("{:04X}", c);
                    get_canonical_decomp(&as_str)
                })
                .collect()
        } else if decomp.len() == 1 {
            // Single-code-point canonical decomposition; recurse simply
            get_canonical_decomp(splits[0])
        } else {
            // No decomposition; continue
            continue;
        };

        map.insert(code_point, final_decomp);
    }

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write("bincode/decomp", bytes).unwrap();
}

fn get_canonical_decomp(code_point: &str) -> Vec<u32> {
    let data = std::fs::read_to_string("unicode-data/UnicodeData.txt").unwrap();

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
                        let as_str = format!("{:04X}", c);
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
    let data = std::fs::read_to_string("unicode-data/UnicodeData.txt").unwrap();

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

        let canon_decomp = match DECOMP.get(&code_point) {
            Some(d) => d,
            None => continue,
        };

        let first_cc = get_ccc(canon_decomp[0]) as u8;
        let last_cc = get_ccc(canon_decomp[canon_decomp.len() - 1]) as u8;

        let packed = (u16::from(first_cc) << 8) | u16::from(last_cc);

        map.insert(code_point, packed);
    }

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write("bincode/fcd", bytes).unwrap();
}

pub fn map_low(keys: KeysSource) {
    let cldr = keys == KeysSource::Cldr;

    let path_in = if cldr {
        "unicode-data/allkeys_CLDR.txt"
    } else {
        "unicode-data/allkeys.txt"
    };

    let data = std::fs::read_to_string(path_in).unwrap();

    let re_set_of_weights = regex!(r"[*.\dA-F]{15}");
    let re_individual_weight = regex!(r"[\dA-F]{4}");

    let mut map: FxHashMap<u32, Weights> = FxHashMap::default();

    // This is for code points under 183 (decimal)
    for i in 0..183 {
        // Skip capital and lowercase L; it's problematic
        if i == 76 || i == 108 {
            continue;
        }

        let as_hex = format!("{:04X}", i);

        // Find the line. Yeah, this is slow, but whatever.
        for line in data.lines() {
            if line.starts_with(&as_hex) {
                let set = re_set_of_weights.find(line).unwrap().as_str();

                let variable = set.starts_with('*');

                let mut weights = re_individual_weight.find_iter(set);
                let primary = u16::from_str_radix(weights.next().unwrap().as_str(), 16).unwrap();
                let secondary = u16::from_str_radix(weights.next().unwrap().as_str(), 16).unwrap();
                let tertiary = u16::from_str_radix(weights.next().unwrap().as_str(), 16).unwrap();

                let together = Weights {
                    variable,
                    primary,
                    secondary,
                    tertiary,
                };

                map.insert(i, together);

                break;
            }
        }
    }

    let path_out = if cldr {
        "bincode/low_cldr"
    } else {
        "bincode/low"
    };

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write(path_out, bytes).unwrap();
}

pub fn map_multi(keys: KeysSource) {
    let cldr = keys == KeysSource::Cldr;

    let path_in = if cldr {
        "unicode-data/allkeys_CLDR.txt"
    } else {
        "unicode-data/allkeys.txt"
    };

    let data = std::fs::read_to_string(path_in).unwrap();

    let mut map: FxHashMap<ArrayVec<[u32; 3]>, Vec<Weights>> = FxHashMap::default();

    for line in data.lines() {
        if line.is_empty() || line.starts_with('@') || line.starts_with('#') {
            continue;
        }

        let mut split_at_semicolon = line.split(';');
        let left_of_semicolon = split_at_semicolon.next().unwrap();
        let right_of_semicolon = split_at_semicolon.next().unwrap();
        let left_of_hash = right_of_semicolon.split('#').next().unwrap();

        let mut k = ArrayVec::<[u32; 3]>::new();
        let re_key = regex!(r"[\dA-F]{4,5}");
        for m in re_key.find_iter(left_of_semicolon) {
            let as_u32 = u32::from_str_radix(m.as_str(), 16).unwrap();
            k.push(as_u32);
        }

        // Here we're only looking for multi-code-point lines
        if k.len() < 2 {
            continue;
        }

        let mut v: Vec<Weights> = Vec::new();
        let re_weights = regex!(r"[*.\dA-F]{15}");
        let re_value = regex!(r"[\dA-F]{4}");

        for m in re_weights.find_iter(left_of_hash) {
            let weights_str = m.as_str();

            let variable = weights_str.starts_with('*');

            let mut vals = re_value.find_iter(weights_str);
            let primary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
            let secondary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
            let tertiary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();

            let weights = Weights {
                variable,
                primary,
                secondary,
                tertiary,
            };

            v.push(weights);
        }

        map.insert(k, v);
    }

    let path_out = if cldr {
        "bincode/multis_cldr"
    } else {
        "bincode/multis"
    };

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write(path_out, bytes).unwrap();
}

pub fn map_sing(keys: KeysSource) {
    let cldr = keys == KeysSource::Cldr;

    let path_in = if cldr {
        "unicode-data/allkeys_CLDR.txt"
    } else {
        "unicode-data/allkeys.txt"
    };

    let data = std::fs::read_to_string(path_in).unwrap();

    let mut map: FxHashMap<u32, Vec<Weights>> = FxHashMap::default();

    for line in data.lines() {
        if line.is_empty() || line.starts_with('@') || line.starts_with('#') {
            continue;
        }

        let mut split_at_semicolon = line.split(';');
        let left_of_semicolon = split_at_semicolon.next().unwrap();
        let right_of_semicolon = split_at_semicolon.next().unwrap();
        let left_of_hash = right_of_semicolon.split('#').next().unwrap();

        let mut points = ArrayVec::<[u32; 3]>::new();
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

        let mut v: Vec<Weights> = Vec::new();
        let re_weights = regex!(r"[*.\dA-F]{15}");
        let re_value = regex!(r"[\dA-F]{4}");

        for m in re_weights.find_iter(left_of_hash) {
            let weights_str = m.as_str();

            let variable = weights_str.starts_with('*');

            let mut vals = re_value.find_iter(weights_str);
            let primary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
            let secondary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
            let tertiary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();

            let weights = Weights {
                variable,
                primary,
                secondary,
                tertiary,
            };

            v.push(weights);
        }

        map.insert(k, v);
    }

    let path_out = if cldr {
        "bincode/singles_cldr"
    } else {
        "bincode/singles"
    };

    let bytes = bincode::serialize(&map).unwrap();
    std::fs::write(path_out, bytes).unwrap();
}

pub fn map_variable() {
    let mut set: FxHashSet<u32> = FxHashSet::default();

    // We only need to use DUCET for this, since (as far as I can tell from testing) every code
    // point in the CLDR table that has a variable weight or a zero primary weight, also has that
    // in DUCET. But the inverse is not true.
    let data = std::fs::read_to_string("unicode-data/allkeys.txt").unwrap();

    'outer: for line in data.lines() {
        if line.is_empty() || line.starts_with('@') || line.starts_with('#') {
            continue;
        }

        let mut split_at_semicolon = line.split(';');
        let left_of_semicolon = split_at_semicolon.next().unwrap();
        let right_of_semicolon = split_at_semicolon.next().unwrap();
        let left_of_hash = right_of_semicolon.split('#').next().unwrap();

        let mut points = ArrayVec::<[u32; 3]>::new();
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
    std::fs::write("bincode/variable", bytes).unwrap();
}
