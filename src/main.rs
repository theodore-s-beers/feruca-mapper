#![warn(clippy::pedantic, clippy::nursery)]

use feruca::Tailoring;
use feruca_mapper::{map_decomps, map_fcd, map_low, map_multi, map_sing, map_variable};

mod arabic_script;
use arabic_script::{map_arabic_script_multi, map_arabic_script_sing};

mod arabic_interleaved;
use arabic_interleaved::{map_arabic_interleaved_multi, map_arabic_interleaved_sing};

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
    println!("Variable hash set took: {} ms", elapsed.as_millis());

    now = std::time::Instant::now();
    map_low(Tailoring::Ducet);
    elapsed = now.elapsed();
    println!("Low mappings (DUCET) took: {} ms", elapsed.as_millis());

    now = std::time::Instant::now();
    map_low(Tailoring::default());
    elapsed = now.elapsed();
    println!("Low mappings (CLDR) took: {} ms", elapsed.as_millis());

    now = std::time::Instant::now();
    map_sing(Tailoring::Ducet);
    elapsed = now.elapsed();
    println!("Single mappings (DUCET) took: {} ms", elapsed.as_millis());

    now = std::time::Instant::now();
    map_sing(Tailoring::default());
    elapsed = now.elapsed();
    println!("Single mappings (CLDR) took: {} ms", elapsed.as_millis());

    now = std::time::Instant::now();
    map_multi(Tailoring::Ducet);
    elapsed = now.elapsed();
    println!("Multi mappings (DUCET) took: {} ms", elapsed.as_millis());

    now = std::time::Instant::now();
    map_multi(Tailoring::default());
    elapsed = now.elapsed();
    println!("Multi mappings (CLDR) took: {} ms", elapsed.as_millis());

    now = std::time::Instant::now();
    map_arabic_script_sing();
    elapsed = now.elapsed();
    println!(
        "Single mappings (ArabicScript) took: {} ms",
        elapsed.as_millis()
    );

    now = std::time::Instant::now();
    map_arabic_script_multi();
    elapsed = now.elapsed();
    println!(
        "Multi mappings (ArabicScript) took: {} ms",
        elapsed.as_millis()
    );

    now = std::time::Instant::now();
    map_arabic_interleaved_sing();
    elapsed = now.elapsed();
    println!(
        "Single mappings (ArabicInterleaved) took: {} ms",
        elapsed.as_millis()
    );

    now = std::time::Instant::now();
    map_arabic_interleaved_multi();
    elapsed = now.elapsed();
    println!(
        "Multi mappings (ArabicInterleaved) took: {} ms",
        elapsed.as_millis()
    );
}
