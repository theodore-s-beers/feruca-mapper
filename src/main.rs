#![warn(clippy::pedantic)]
#![allow(clippy::missing_panics_doc)]

use feruca::Tailoring;
use feruca_mapper::{map_decomps, map_fcd, map_low, map_multi, map_sing, map_variable};

mod arabic_script;
use arabic_script::{map_arabic_script_multi, map_arabic_script_sing};

fn main() {
    map_decomps();
    map_fcd();
    map_variable();

    map_low(Tailoring::Ducet);
    map_low(Tailoring::default());

    map_sing(Tailoring::Ducet);
    map_sing(Tailoring::default());

    map_multi(Tailoring::Ducet);
    map_multi(Tailoring::default());

    map_arabic_script_sing();
    map_arabic_script_multi();
}
