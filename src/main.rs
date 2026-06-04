#![warn(clippy::pedantic, clippy::nursery)]

use feruca::Tailoring;
use feruca_mapper::{map_decomps, map_fcd, map_low, map_trie, map_variable};

mod arabic_script;
use arabic_script::map_arabic_script_trie;

mod arabic_interleaved;
use arabic_interleaved::map_arabic_interleaved_trie;

fn main() {
    let mut now = std::time::Instant::now();
    map_decomps();
    let mut elapsed = now.elapsed();
    println!("Decompositions took: {} ms", elapsed.as_millis());

    now = std::time::Instant::now();
    map_fcd();
    elapsed = now.elapsed();
    println!("FCD took: {} ms", elapsed.as_millis());

    now = std::time::Instant::now();
    map_variable();
    elapsed = now.elapsed();
    println!("Variable table took: {} ms", elapsed.as_millis());

    now = std::time::Instant::now();
    map_low(Tailoring::Ducet);
    elapsed = now.elapsed();
    println!("Low mappings (DUCET) took: {} ms", elapsed.as_millis());

    now = std::time::Instant::now();
    map_low(Tailoring::default());
    elapsed = now.elapsed();
    println!("Low mappings (CLDR) took: {} ms", elapsed.as_millis());

    now = std::time::Instant::now();
    map_trie(Tailoring::Ducet);
    elapsed = now.elapsed();
    println!("Trie mappings (DUCET) took: {} ms", elapsed.as_millis());

    now = std::time::Instant::now();
    map_trie(Tailoring::default());
    elapsed = now.elapsed();
    println!("Trie mappings (CLDR) took: {} ms", elapsed.as_millis());

    now = std::time::Instant::now();
    map_arabic_script_trie();
    elapsed = now.elapsed();
    println!(
        "Trie mappings (ArabicScript) took: {} ms",
        elapsed.as_millis()
    );

    now = std::time::Instant::now();
    map_arabic_interleaved_trie();
    elapsed = now.elapsed();
    println!(
        "Trie mappings (ArabicInterleaved) took: {} ms",
        elapsed.as_millis()
    );
}
