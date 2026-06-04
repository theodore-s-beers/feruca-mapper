#![allow(clippy::missing_panics_doc)]

use crate::{
    collation::{collect_multis, collect_singles, unpack_code_points},
    common::{CODE_POINT_COUNT, PAGE_SIZE},
};
use feruca::Tailoring;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Serialize;
use std::{collections::HashMap, hash::BuildHasher};

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

const NO_ROW: u32 = u32::MAX;

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
struct WeightRow {
    start: u32,
    len: u16,
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

    let path_out = if cldr {
        "bincode/cldr-46_1/cldr_root"
    } else {
        "bincode/cldr-46_1/ducet"
    };

    write_trie(path_out, &singles, &multis);
}

pub fn map_cldr_trie<S1: BuildHasher, S2: BuildHasher>(
    singles: &HashMap<u32, Box<[u32]>, S1>,
    multis: &HashMap<u64, Box<[u32]>, S2>,
) {
    write_trie("bincode/cldr-46_1/cldr_root", singles, multis);
}

pub fn write_trie<S1: BuildHasher, S2: BuildHasher>(
    path: &str,
    singles: &HashMap<u32, Box<[u32]>, S1>,
    multis: &HashMap<u64, Box<[u32]>, S2>,
) {
    let table = build_trie_table(singles, multis);
    let bytes = postcard::to_allocvec(&table).unwrap();
    std::fs::write(path, bytes).unwrap();
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
