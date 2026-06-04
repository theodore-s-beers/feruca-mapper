#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::cast_lossless,
    clippy::missing_panics_doc,
    clippy::regex_creation_in_loops
)]

use feruca::Tailoring;
use regex::Regex;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    hash::BuildHasher,
    io::{BufWriter, Write},
    ops::RangeInclusive,
    path::Path,
};
use unicode_canonical_combining_class::get_canonical_combining_class_u32 as get_ccc;

use std::sync::{LazyLock, OnceLock};

static KEYS_DUCET: LazyLock<String> =
    LazyLock::new(|| std::fs::read_to_string("unicode-data/cldr-46_1/allkeys.txt").unwrap());

pub static KEYS_CLDR: LazyLock<String> =
    LazyLock::new(|| std::fs::read_to_string("unicode-data/cldr-46_1/allkeys_CLDR.txt").unwrap());

static UNI_DATA: LazyLock<String> =
    LazyLock::new(|| std::fs::read_to_string("unicode-data/cldr-46_1/UnicodeData.txt").unwrap());

const SEC_MAX: u16 = 0x126; // Largest secondary weight that is actually used
const TER_MAX: u16 = 0x1E; // Largest tertiary weight that is actually used

// This adjustment affects only the low and singles maps
const BUMP_START: u16 = 0x2384; // Latin small capital A
const BUMP_END: u16 = 0x2454; // Small gap above this, before Latin H, that we can use
pub const BUMP: u16 = 1;

const SHIFT_START: u16 = 0x2380; // Latin script begins
const SHIFT_END: u16 = 0x72B6; // Large gap above this that we can use
pub const SHIFT: u16 = 0x400;

const NO_ROW: u32 = u32::MAX;
const CODE_POINT_COUNT: usize = 0x11_0000;
const PAGE_SIZE: usize = 256;
const PAGE_WORDS: usize = 4;
pub const VARIABLE_EMPTY_PAGE: u16 = u16::MAX;
pub const ENTRY_MISSING: u64 = 0;
pub const ENTRY_SIMPLE: u64 = 1;
pub const ENTRY_CONTRACTION: u64 = 2;
const ENTRY_TAG_BITS: u64 = 2;
const ENTRY_LEN_BITS: u64 = 16;
const ENTRY_START_BITS: u64 = 32;
const ENTRY_TAG_MASK: u64 = (1 << ENTRY_TAG_BITS) - 1;
const ENTRY_LEN_MASK: u64 = (1 << ENTRY_LEN_BITS) - 1;
const ENTRY_START_MASK: u64 = (1 << ENTRY_START_BITS) - 1;
const ENTRY_LEN_SHIFT: u64 = ENTRY_TAG_BITS;
const ENTRY_START_SHIFT: u64 = ENTRY_LEN_SHIFT + ENTRY_LEN_BITS;
const ENTRY_META_SHIFT: u64 = ENTRY_START_SHIFT + ENTRY_START_BITS;

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
// If we were to use the PHF map instead...
// include!("../phf/decomp.rs");

#[macro_export]
macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new($re).unwrap())
    }};
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

    // Generate PHF map; not currently used, but worth studying
    let path_out = Path::new("phf/decomp.rs");
    let file = File::create(path_out).unwrap();
    let mut writer = BufWriter::new(file);

    let mut builder = phf_codegen::Map::new();

    for (key, value) in canonical {
        let value_str = format!(
            "&[{}]",
            value
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );

        builder.entry(key, value_str);
    }

    let phf_map = builder.build();

    writeln!(writer, "#[allow(clippy::unreadable_literal)]").unwrap();
    writeln!(
        writer,
        "static DECOMP: phf::Map<u32, &'static [u32]> = {phf_map};"
    )
    .unwrap();
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

        let packed = ((first_cc as u16) << 8) | (last_cc as u16);
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

    // Generate PHF map; not currently used, but worth studying
    let path_out = Path::new("phf/fcd.rs");
    let file = File::create(path_out).unwrap();
    let mut writer = BufWriter::new(file);

    let mut builder = phf_codegen::Map::new();
    for (key, value) in map {
        builder.entry(key, value.to_string());
    }

    let phf_map = builder.build();
    writeln!(writer, "#[allow(clippy::unreadable_literal)]").unwrap();
    writeln!(writer, "static FCD: phf::Map<u32, u16> = {phf_map};").unwrap();
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

fn _map_multi(keys: Tailoring) {
    let cldr = keys != Tailoring::Ducet;

    let data = if cldr { &KEYS_CLDR } else { &KEYS_DUCET };

    let mut map: FxHashMap<u64, Box<[u32]>> = FxHashMap::default();

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

        map.insert(pack_code_points(&k), v.into_boxed_slice());
    }

    let path_out = if cldr {
        "bincode/cldr-46_1/multis_cldr"
    } else {
        "bincode/cldr-46_1/multis"
    };

    let bytes = postcard::to_allocvec(&map).unwrap();
    std::fs::write(path_out, bytes).unwrap();

    if !cldr {
        let json_bytes = serde_json::to_vec(&map).unwrap();
        std::fs::write("json/cldr-46_1/multis.json", json_bytes).unwrap();
    }
}

fn _map_sing(keys: Tailoring) {
    let cldr = keys != Tailoring::Ducet;

    let data = if cldr { &KEYS_CLDR } else { &KEYS_DUCET };

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

    let boxed: FxHashMap<u32, Box<[u32]>> = map
        .into_iter()
        .map(|(k, v)| (k, v.into_boxed_slice()))
        .collect();

    // Write DUCET version to JSON for debugging
    if !cldr {
        let json_bytes = serde_json::to_vec(&boxed).unwrap();
        std::fs::write("json/cldr-46_1/singles.json", json_bytes).unwrap();
    }

    let path_out = if cldr {
        "bincode/cldr-46_1/singles_cldr"
    } else {
        "bincode/cldr-46_1/singles"
    };

    let bytes = postcard::to_allocvec(&boxed).unwrap();
    std::fs::write(path_out, bytes).unwrap();
}

#[derive(Serialize)]
pub struct CollationTrieTable {
    pub page_index: Box<[u16]>,
    pub entries: Box<[u64]>,
    pub contraction_meta: Box<[ContractionMeta]>,
    pub edges: Box<[ContractionEdge]>,
    pub weights: Box<[u32]>,
}

#[derive(Serialize)]
pub struct ContractionMeta {
    pub first_edge: u32,
    pub edge_len: u16,
    pub max_len: u8,
}

#[derive(Serialize)]
pub struct ContractionEdge {
    pub code_point: u32,
    pub next_first_edge: u32,
    pub weight_start: u32,
    pub next_edge_len: u16,
    pub weight_len: u16,
}

#[derive(Serialize)]
pub struct WeightRow {
    pub start: u32,
    pub len: u16,
}

#[derive(Serialize)]
pub struct VariableTable {
    pub page_index: Box<[u16]>,
    pub pages: Box<[u64]>,
}

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

#[derive(Default)]
struct RowPool {
    rows_by_weights: FxHashMap<Box<[u32]>, u32>,
    rows: Vec<WeightRow>,
    weights: Vec<u32>,
}

impl RowPool {
    fn insert(&mut self, weights: &[u32]) -> u32 {
        if let Some(row) = self.rows_by_weights.get(weights) {
            return *row;
        }

        let row = u32::try_from(self.rows.len()).unwrap();
        let start = u32::try_from(self.weights.len()).unwrap();
        let len = u16::try_from(weights.len()).unwrap();
        self.weights.extend_from_slice(weights);
        self.rows.push(WeightRow { start, len });
        self.rows_by_weights.insert(weights.into(), row);
        row
    }

    fn get(&self, row: u32) -> &WeightRow {
        &self.rows[usize::try_from(row).unwrap()]
    }
}

struct EdgeNode {
    weight_row: u32,
    children: FxHashMap<u32, Self>,
}

impl Default for EdgeNode {
    fn default() -> Self {
        Self {
            weight_row: NO_ROW,
            children: FxHashMap::default(),
        }
    }
}

pub fn map_trie(keys: Tailoring) {
    let cldr = keys != Tailoring::Ducet;
    let singles = collect_singles(keys);
    let multis = collect_multis(keys);
    let table = build_trie_table(&singles, &multis);

    let path_out = if cldr {
        "bincode/cldr-46_1/cldr_root"
    } else {
        "bincode/cldr-46_1/ducet"
    };

    let bytes = postcard::to_allocvec(&table).unwrap();
    std::fs::write(path_out, bytes).unwrap();
}

#[must_use]
pub fn build_trie_table<S1: BuildHasher, S2: BuildHasher>(
    singles: &HashMap<u32, Box<[u32]>, S1>,
    multis: &HashMap<u64, Box<[u32]>, S2>,
) -> CollationTrieTable {
    let mut row_pool = RowPool::default();
    let mut contraction_roots: FxHashMap<u32, EdgeNode> = FxHashMap::default();
    let mut max_lens: FxHashMap<u32, u8> = FxHashMap::default();

    for (&packed, weights) in multis {
        let cps = unpack_code_points(packed);
        let row = row_pool.insert(weights);
        let root = contraction_roots.entry(cps[0]).or_default();
        insert_contraction(root, &cps[1..], row);
        let len = u8::try_from(cps.len()).unwrap();
        max_lens
            .entry(cps[0])
            .and_modify(|max_len| *max_len = (*max_len).max(len))
            .or_insert(len);
    }

    let mut entries = vec![ENTRY_MISSING; CODE_POINT_COUNT];
    let mut contraction_meta = Vec::new();
    let mut edges = Vec::new();
    let mut code_points: FxHashSet<u32> = singles.keys().copied().collect();
    code_points.extend(contraction_roots.keys().copied());

    let mut code_points: Vec<u32> = code_points
        .into_iter()
        .filter(|&code_point| !is_low_fast_path_code_point(code_point))
        .collect();
    code_points.sort_unstable();

    for code_point in code_points {
        if let Some(root) = contraction_roots.get(&code_point) {
            let simple_weights = singles.get(&code_point).unwrap_or_else(|| {
                panic!("missing single mapping for contraction root U+{code_point:04X}")
            });
            let simple_row = row_pool.insert(simple_weights);
            let simple_row = row_pool.get(simple_row);
            let first_edge = u32::try_from(edges.len()).unwrap();
            let edge_len = write_edges(root, &row_pool, &mut edges);
            let meta_index = u16::try_from(contraction_meta.len()).unwrap();
            contraction_meta.push(ContractionMeta {
                first_edge,
                edge_len,
                max_len: max_lens[&code_point],
            });
            entries[usize::try_from(code_point).unwrap()] = pack_entry(
                ENTRY_CONTRACTION,
                simple_row.start,
                simple_row.len,
                meta_index,
            );
        } else if let Some(weights) = singles.get(&code_point) {
            let row = row_pool.insert(weights);
            let row = row_pool.get(row);
            entries[usize::try_from(code_point).unwrap()] =
                pack_entry(ENTRY_SIMPLE, row.start, row.len, 0);
        }
    }

    let (page_index, entries) = dedupe_entry_pages(&entries);

    CollationTrieTable {
        page_index,
        entries,
        contraction_meta: contraction_meta.into_boxed_slice(),
        edges: edges.into_boxed_slice(),
        weights: row_pool.weights.into_boxed_slice(),
    }
}

const fn pack_entry(tag: u64, start: u32, len: u16, meta_index: u16) -> u64 {
    tag | ((len as u64) << ENTRY_LEN_SHIFT)
        | ((start as u64) << ENTRY_START_SHIFT)
        | ((meta_index as u64) << ENTRY_META_SHIFT)
}

#[must_use]
pub const fn entry_tag(entry: u64) -> u64 {
    entry & ENTRY_TAG_MASK
}

#[must_use]
pub const fn entry_len(entry: u64) -> u16 {
    ((entry >> ENTRY_LEN_SHIFT) & ENTRY_LEN_MASK) as u16
}

#[must_use]
pub const fn entry_start(entry: u64) -> u32 {
    ((entry >> ENTRY_START_SHIFT) & ENTRY_START_MASK) as u32
}

#[must_use]
pub const fn entry_meta_index(entry: u64) -> u16 {
    (entry >> ENTRY_META_SHIFT) as u16
}

fn dedupe_entry_pages(entries: &[u64]) -> (Box<[u16]>, Box<[u64]>) {
    let mut page_index = Vec::with_capacity(CODE_POINT_COUNT / PAGE_SIZE);
    let mut page_ids: FxHashMap<Box<[u64]>, u16> = FxHashMap::default();
    let mut deduped_entries = Vec::new();

    for page in entries.chunks_exact(PAGE_SIZE) {
        if let Some(page_id) = page_ids.get(page) {
            page_index.push(*page_id);
            continue;
        }

        let page_id = u16::try_from(page_ids.len()).unwrap();
        page_index.push(page_id);
        deduped_entries.extend_from_slice(page);
        page_ids.insert(page.into(), page_id);
    }

    (
        page_index.into_boxed_slice(),
        deduped_entries.into_boxed_slice(),
    )
}

fn insert_contraction(node: &mut EdgeNode, suffix: &[u32], row: u32) {
    let child = node.children.entry(suffix[0]).or_default();
    if suffix.len() == 1 {
        child.weight_row = row;
    } else {
        insert_contraction(child, &suffix[1..], row);
    }
}

fn write_edges(node: &EdgeNode, row_pool: &RowPool, edges: &mut Vec<ContractionEdge>) -> u16 {
    let mut children: Vec<(&u32, &EdgeNode)> = node.children.iter().collect();
    children.sort_unstable_by_key(|(code_point, _)| **code_point);
    let edge_len = u16::try_from(children.len()).unwrap();

    let start = edges.len();
    for (code_point, child) in &children {
        let (weight_start, weight_len) = if child.weight_row == NO_ROW {
            (0, 0)
        } else {
            let row = row_pool.get(child.weight_row);
            (row.start, row.len)
        };

        edges.push(ContractionEdge {
            code_point: **code_point,
            next_first_edge: 0,
            weight_start,
            next_edge_len: 0,
            weight_len,
        });
    }

    for (i, (_, child)) in children.into_iter().enumerate() {
        if child.children.is_empty() {
            continue;
        }
        let child_start = u32::try_from(edges.len()).unwrap();
        let child_len = write_edges(child, row_pool, edges);
        let edge = &mut edges[start + i];
        edge.next_first_edge = child_start;
        edge.next_edge_len = child_len;
    }

    edge_len
}

const fn is_low_fast_path_code_point(code_point: u32) -> bool {
    code_point <= 0xB6 && code_point != 0x4C && code_point != 0x6C
}

fn unpack_code_points(packed: u64) -> Vec<u32> {
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

pub fn collect_multis(keys: Tailoring) -> FxHashMap<u64, Box<[u32]>> {
    let cldr = keys != Tailoring::Ducet;
    let data = if cldr { &KEYS_CLDR } else { &KEYS_DUCET };
    let mut map: FxHashMap<u64, Box<[u32]>> = FxHashMap::default();

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
            k.push(u32::from_str_radix(m.as_str(), 16).unwrap());
        }

        if k.len() < 2 {
            continue;
        }

        map.insert(
            pack_code_points(&k),
            parse_weights(left_of_hash, cldr, false),
        );
    }

    map
}

pub fn collect_singles(keys: Tailoring) -> FxHashMap<u32, Box<[u32]>> {
    let cldr = keys != Tailoring::Ducet;
    let data = if cldr { &KEYS_CLDR } else { &KEYS_DUCET };
    let mut map: FxHashMap<u32, Box<[u32]>> = FxHashMap::default();

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

        if points.len() > 1 {
            continue;
        }

        map.insert(points[0], parse_weights(left_of_hash, cldr, true));
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
