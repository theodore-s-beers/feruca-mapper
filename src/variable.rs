#![allow(clippy::missing_panics_doc, clippy::regex_creation_in_loops)]

use crate::{
    collation::KEYS_DUCET,
    common::{CODE_POINT_COUNT, PAGE_SIZE, PAGE_WORDS, VARIABLE_EMPTY_PAGE},
    regex,
};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Serialize;
use std::{collections::HashSet, hash::BuildHasher};

#[derive(Serialize)]
pub struct VariableTable {
    pub page_index: Box<[u16]>,
    pub pages: Box<[u64]>,
}

pub fn map_variable() {
    let mut set: FxHashSet<u32> = FxHashSet::default();

    // We only need to use DUCET for this, since (as far as I can tell from testing) every code
    // point in the CLDR table that has a variable weight or a zero primary weight, also has that
    // in DUCET. But the inverse is not true.
    'outer: for line in KEYS_DUCET.lines() {
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

    let mut sorted: Vec<u32> = set.iter().copied().collect();
    sorted.sort_unstable();

    // Write to JSON for debugging
    let json_bytes = serde_json::to_vec(&sorted).unwrap();
    std::fs::write("json/cldr-46_1/variable.json", json_bytes).unwrap();

    let table = build_variable_table(&set);
    let bytes = postcard::to_allocvec(&table).unwrap();
    std::fs::write("bincode/cldr-46_1/variable", bytes).unwrap();
}

#[must_use]
pub fn build_variable_table<S: BuildHasher>(set: &HashSet<u32, S>) -> VariableTable {
    let mut raw_pages = vec![[0u64; PAGE_WORDS]; CODE_POINT_COUNT / PAGE_SIZE];
    for &code_point in set {
        let page = usize::try_from(code_point >> 8).unwrap();
        let offset = usize::try_from(code_point & 0xFF).unwrap();
        raw_pages[page][offset >> 6] |= 1u64 << (offset & 0x3F);
    }

    let mut page_index = Vec::with_capacity(raw_pages.len());
    let mut page_ids: FxHashMap<[u64; PAGE_WORDS], u16> = FxHashMap::default();
    let mut pages = Vec::new();

    for page in raw_pages {
        if page == [0; PAGE_WORDS] {
            page_index.push(VARIABLE_EMPTY_PAGE);
            continue;
        }

        if let Some(page_id) = page_ids.get(&page) {
            page_index.push(*page_id);
            continue;
        }

        let page_id = u16::try_from(page_ids.len()).unwrap();
        page_index.push(page_id);
        pages.extend_from_slice(&page);
        page_ids.insert(page, page_id);
    }

    VariableTable {
        page_index: page_index.into_boxed_slice(),
        pages: pages.into_boxed_slice(),
    }
}
