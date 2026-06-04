#![allow(clippy::missing_panics_doc, clippy::regex_creation_in_loops)]

use crate::common::{CODE_POINT_COUNT, PAGE_SIZE, VARIABLE_EMPTY_PAGE};
use crate::regex;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, hash::BuildHasher, ops::RangeInclusive, sync::LazyLock};
use unicode_canonical_combining_class::get_canonical_combining_class_u32 as get_ccc;

static UNI_DATA: LazyLock<String> =
    LazyLock::new(|| std::fs::read_to_string("unicode-data/cldr-46_1/UnicodeData.txt").unwrap());

// Ignored code point ranges for decompositions and FCD
const IGNORED_RANGES: [RangeInclusive<u32>; 15] = [
    0x3400..=0x4DBF,
    0x4E00..=0x9FFF,
    0xAC00..=0xD7A3,
    0xD800..=0xDFFF,
    0xE000..=0xF8FF,
    0x17000..=0x187F7,
    0x18D00..=0x18D08,
    0x20000..=0x2A6DF,
    0x2A700..=0x2B738,
    0x2B740..=0x2B81D,
    0x2B820..=0x2CEA1,
    0x2CEB0..=0x2EBE0,
    0x30000..=0x3134A,
    0xF0000..=0xFFFFD,
    0x10_0000..=0x10_FFFD,
];

// The output of map_decomps is needed for map_fcd
static DECOMP: LazyLock<DecompTable> = LazyLock::new(|| {
    let data = std::fs::read("bincode/cldr-46_1/decomp").unwrap();
    postcard::from_bytes(&data).unwrap()
});

#[derive(Serialize)]
pub struct FcdTable {
    pub page_index: Box<[u16]>,
    pub pages: Box<[u16]>,
}

#[derive(Deserialize, Serialize)]
pub struct DecompTable {
    pub page_index: Box<[u16]>,
    pub entries: Box<[u64]>,
    pub values: Box<[u32]>,
}

impl DecompTable {
    #[must_use]
    pub fn get(&self, code_point: u32) -> Option<&[u32]> {
        let page = self.page_index[usize::try_from(code_point >> 8).unwrap()];
        if page == VARIABLE_EMPTY_PAGE {
            return None;
        }

        let offset = usize::try_from(code_point & 0xFF).unwrap();
        let entry = self.entries[(usize::from(page) << 8) + offset];
        let len = usize::from((entry & 0xFFFF) as u16);
        if len == 0 {
            return None;
        }

        let start = usize::try_from(entry >> 16).unwrap();
        Some(&self.values[start..start + len])
    }
}

pub fn map_decomps() {
    let mut listed: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    let mut canonical: FxHashMap<u32, Box<[u32]>> = FxHashMap::default();

    // First pass: collect listed decompositions
    for line in UNI_DATA.lines() {
        if line.is_empty() {
            continue;
        }

        let fields: Vec<&str> = line.split(';').collect();

        let code_point = u32::from_str_radix(fields[0], 16).unwrap();
        if IGNORED_RANGES.iter().any(|r| r.contains(&code_point)) {
            continue;
        }

        let decomp_col = fields[5];
        if decomp_col.is_empty() {
            continue; // No decomposition; continue
        }

        if decomp_col.contains('<') {
            continue; // Non-canonical decomposition; continue
        }

        let re = regex!(r"[\dA-F]{4,5}");

        let mut decomp: Vec<u32> = Vec::new();
        for m in re.find_iter(decomp_col) {
            let code_point = u32::from_str_radix(m.as_str(), 16).unwrap();
            decomp.push(code_point);
        }

        assert!(!decomp.is_empty());

        listed.insert(code_point, decomp);
    }

    // Second pass: collect canonical decompositions
    for (code_point, decomp) in &listed {
        let final_decomp = if decomp.len() == 1 {
            // Single-code-point canonical decomposition; recurse simply
            get_canonical_decomp(&listed, decomp[0])
        } else {
            // Multi-code-point canonical decomposition; recurse badly
            decomp
                .iter()
                .flat_map(|c| get_canonical_decomp(&listed, *c))
                .collect()
        };

        canonical.insert(*code_point, final_decomp);
    }

    let mut sorted: Vec<(u32, Box<[u32]>)> = canonical
        .iter()
        .map(|(&code_point, decomp)| (code_point, decomp.clone()))
        .collect();
    sorted.sort_unstable_by_key(|&(code_point, _)| code_point);

    // Write to JSON for debugging
    let json_bytes = serde_json::to_vec(&sorted).unwrap();
    std::fs::write("json/cldr-46_1/decomp.json", json_bytes).unwrap();

    // Write to bincode; this is what we actually use
    let table = build_decomp_table(&canonical);
    let bytes = postcard::to_allocvec(&table).unwrap();
    std::fs::write("bincode/cldr-46_1/decomp", bytes).unwrap();
}

#[must_use]
pub fn build_decomp_table<S: BuildHasher>(map: &HashMap<u32, Box<[u32]>, S>) -> DecompTable {
    let mut row_ids: FxHashMap<Box<[u32]>, (u32, u16)> = FxHashMap::default();
    let mut values = Vec::new();
    let mut raw_pages = vec![[0u64; PAGE_SIZE]; CODE_POINT_COUNT / PAGE_SIZE];

    for (&code_point, decomp) in map {
        let (start, len) = row_ids.get(decomp.as_ref()).copied().unwrap_or_else(|| {
            let start = u32::try_from(values.len()).unwrap();
            let len = u16::try_from(decomp.len()).unwrap();
            values.extend_from_slice(decomp);
            row_ids.insert(decomp.clone(), (start, len));
            (start, len)
        });

        let page = usize::try_from(code_point >> 8).unwrap();
        let offset = usize::try_from(code_point & 0xFF).unwrap();
        raw_pages[page][offset] = u64::from(len) | (u64::from(start) << 16);
    }

    let mut page_index = Vec::with_capacity(raw_pages.len());
    let mut page_ids: FxHashMap<Box<[u64]>, u16> = FxHashMap::default();
    let mut entries = Vec::new();

    for page in raw_pages {
        if page == [0; PAGE_SIZE] {
            page_index.push(VARIABLE_EMPTY_PAGE);
            continue;
        }

        if let Some(page_id) = page_ids.get(page.as_slice()) {
            page_index.push(*page_id);
            continue;
        }

        let page_id = u16::try_from(page_ids.len()).unwrap();
        page_index.push(page_id);
        entries.extend_from_slice(&page);
        page_ids.insert(page.into(), page_id);
    }

    DecompTable {
        page_index: page_index.into_boxed_slice(),
        entries: entries.into_boxed_slice(),
        values: values.into_boxed_slice(),
    }
}

fn get_canonical_decomp(listed: &FxHashMap<u32, Vec<u32>>, code_point: u32) -> Box<[u32]> {
    listed.get(&code_point).map_or_else(
        || vec![code_point].into_boxed_slice(), // No further decomp; return code point itself
        |decomp| {
            if decomp.len() == 1 {
                // Single-code-point decomp; return it directly
                vec![decomp[0]].into_boxed_slice()
            } else {
                // Multi-code-point decomp; recurse
                decomp
                    .iter()
                    .flat_map(|c| get_canonical_decomp(listed, *c))
                    .collect::<Vec<u32>>()
                    .into_boxed_slice()
            }
        },
    )
}

pub fn map_fcd() {
    let mut map: FxHashMap<u32, u16> = FxHashMap::default();

    for line in UNI_DATA.lines() {
        if line.is_empty() {
            continue;
        }

        let left_of_semicolon = line.split(';').next().unwrap();

        let code_point = u32::from_str_radix(left_of_semicolon, 16).unwrap();
        if IGNORED_RANGES.iter().any(|r| r.contains(&code_point)) {
            continue;
        }

        let Some(canon_decomp) = DECOMP.get(code_point) else {
            continue;
        };

        let first_cc = get_ccc(canon_decomp[0]) as u8;
        let last_cc = get_ccc(canon_decomp[canon_decomp.len() - 1]) as u8;

        let packed = (u16::from(first_cc) << 8) | u16::from(last_cc);
        if packed == 0 {
            continue;
        }

        map.insert(code_point, packed);
    }

    let mut sorted: Vec<(u32, u16)> = map
        .iter()
        .map(|(&code_point, &value)| (code_point, value))
        .collect();
    sorted.sort_unstable_by_key(|&(code_point, _)| code_point);

    // Write to JSON for debugging
    let json_bytes = serde_json::to_vec(&sorted).unwrap();
    std::fs::write("json/cldr-46_1/fcd.json", json_bytes).unwrap();

    // Write to bincode; this is what we actually use
    let table = build_fcd_table(&map);
    let bytes = postcard::to_allocvec(&table).unwrap();
    std::fs::write("bincode/cldr-46_1/fcd", bytes).unwrap();
}

#[must_use]
pub fn build_fcd_table<S: BuildHasher>(map: &HashMap<u32, u16, S>) -> FcdTable {
    let mut raw_pages = vec![[0u16; PAGE_SIZE]; CODE_POINT_COUNT / PAGE_SIZE];
    for (&code_point, &value) in map {
        let page = usize::try_from(code_point >> 8).unwrap();
        let offset = usize::try_from(code_point & 0xFF).unwrap();
        raw_pages[page][offset] = value;
    }

    let mut page_index = Vec::with_capacity(raw_pages.len());
    let mut page_ids: FxHashMap<Box<[u16]>, u16> = FxHashMap::default();
    let mut pages = Vec::new();

    for page in raw_pages {
        if page == [0; PAGE_SIZE] {
            page_index.push(VARIABLE_EMPTY_PAGE);
            continue;
        }

        if let Some(page_id) = page_ids.get(page.as_slice()) {
            page_index.push(*page_id);
            continue;
        }

        let page_id = u16::try_from(page_ids.len()).unwrap();
        page_index.push(page_id);
        pages.extend_from_slice(&page);
        page_ids.insert(page.into(), page_id);
    }

    FcdTable {
        page_index: page_index.into_boxed_slice(),
        pages: pages.into_boxed_slice(),
    }
}
