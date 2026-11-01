#![warn(clippy::pedantic, clippy::nursery)]

use feruca_mapper::{
    CollationSource, collect_multis, collect_singles, map_cldr_trie, map_decomps, map_fcd, map_low,
    map_trie, map_variable,
};

mod arabic_script;
use arabic_script::map_arabic_script_trie;

mod arabic_interleaved;
use arabic_interleaved::map_arabic_interleaved_trie;

mod arabic_tailoring;

fn main() {
    for path in [
        "bincode/18",
        "json/18",
        "generated/18",
        "generated/cldr-49",
        "bincode/cldr-49/tailoring",
        "json/cldr-49",
    ] {
        std::fs::create_dir_all(path).unwrap();
    }
    timed("Unicode properties", feruca_mapper::map_unicode);
    timed("Decompositions", map_decomps);
    timed("FCD", map_fcd);
    timed("Variable table", map_variable);
    timed("Low mappings (DUCET)", || map_low(CollationSource::Ducet));
    timed("Low mappings (CLDR)", || map_low(CollationSource::Cldr));
    timed("Trie mappings (DUCET)", || map_trie(CollationSource::Ducet));

    let cldr_singles = timed("Collect mappings (CLDR singles)", || {
        collect_singles(CollationSource::Cldr)
    });
    let cldr_multis = timed("Collect mappings (CLDR multis)", || {
        collect_multis(CollationSource::Cldr)
    });

    timed("Trie mappings (CLDR)", || {
        map_cldr_trie(&cldr_singles, &cldr_multis);
    });
    timed("Trie mappings (ArabicScript)", || {
        map_arabic_script_trie(&cldr_singles, &cldr_multis);
    });
    timed("Trie mappings (ArabicInterleaved)", || {
        map_arabic_interleaved_trie(&cldr_singles, &cldr_multis);
    });
}

fn timed<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let now = std::time::Instant::now();
    let output = f();
    println!("{label} took: {} ms", now.elapsed().as_millis());
    output
}
