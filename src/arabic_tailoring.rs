#![allow(clippy::regex_creation_in_loops)]

use feruca_mapper::{KEYS_CLDR, pack_weights, regex, unpack_weights};
use rustc_hash::FxHashMap;
use std::hash::Hash;

pub fn collect_entries<K>(
    include_points: impl Fn(&[u32]) -> bool,
    pack_key: impl Fn(&[u32]) -> K,
    map_primary: impl Fn(u16) -> Option<u16>,
) -> FxHashMap<K, Box<[u32]>>
where
    K: Eq + Hash,
{
    // This is based on the CLDR table, of course
    let data = KEYS_CLDR.as_str();

    let mut map: FxHashMap<K, Box<[u32]>> = FxHashMap::default();

    for line in data.lines() {
        if line.is_empty() || line.starts_with('@') || line.starts_with('#') {
            continue;
        }

        let mut split_at_semicolon = line.split(';');
        let left_of_semicolon = split_at_semicolon.next().unwrap();
        let right_of_semicolon = split_at_semicolon.next().unwrap();
        let left_of_hash = right_of_semicolon.split('#').next().unwrap();

        let mut key = Vec::new();
        let re_key = regex!(r"[\dA-F]{4,5}");
        for m in re_key.find_iter(left_of_semicolon) {
            key.push(u32::from_str_radix(m.as_str(), 16).unwrap());
        }

        if !include_points(&key) {
            continue;
        }

        let mut weights = Vec::new();
        let re_weights = regex!(r"[*.\dA-F]{15}");
        let re_value = regex!(r"[\dA-F]{4}");

        for m in re_weights.find_iter(left_of_hash) {
            let weights_str = m.as_str();
            let variable = weights_str.starts_with('*');
            let mut vals = re_value.find_iter(weights_str);
            let primary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
            let secondary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();
            let tertiary = u16::from_str_radix(vals.next().unwrap().as_str(), 16).unwrap();

            weights.push(pack_weights(variable, primary, secondary, tertiary));
        }

        if !weights
            .iter()
            .any(|weights| map_primary(unpack_weights(*weights).1).is_some())
        {
            continue;
        }

        for weights in &mut weights {
            let (variable, primary, secondary, tertiary) = unpack_weights(*weights);
            if let Some(new_primary) = map_primary(primary) {
                *weights = pack_weights(variable, new_primary, secondary, tertiary);
            }
        }

        map.insert(pack_key(&key), weights.into_boxed_slice());
    }

    map
}
