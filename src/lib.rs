#![warn(clippy::pedantic, clippy::nursery)]

mod collation;
pub use collation::{
    BUMP, KEYS_CLDR, SHIFT, collect_multis, collect_singles, map_low, pack_code_points,
    pack_weights, unpack_weights,
};

mod common;
pub use common::VARIABLE_EMPTY_PAGE;

mod normalization;
pub use normalization::{
    DecompTable, FcdTable, build_decomp_table, build_fcd_table, map_decomps, map_fcd,
};

mod trie;
pub use trie::{
    CollationTrieTable, ContractionEdge, ContractionMeta, ENTRY_CONTRACTION, ENTRY_MISSING,
    ENTRY_SIMPLE, build_trie_table, entry_len, entry_meta_index, entry_start, entry_tag,
    map_cldr_trie, map_trie, write_trie,
};

mod variable;
pub use variable::{VariableTable, build_variable_table, map_variable};

#[macro_export]
macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: ::std::sync::OnceLock<::regex::Regex> = ::std::sync::OnceLock::new();
        RE.get_or_init(|| ::regex::Regex::new($re).unwrap())
    }};
}
