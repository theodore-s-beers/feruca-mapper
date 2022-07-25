#![warn(clippy::pedantic)]
#![allow(clippy::missing_panics_doc)]

use feruca::KeysSource;
use feruca_mapper::{map_decomps, map_fcd, map_low, map_multi, map_sing, map_variable};

mod arabic_script;
use arabic_script::{map_arabic_script_multi, map_arabic_script_sing};

fn main() {
    map_decomps();
    map_fcd();
    map_variable();

    map_low(KeysSource::Ducet);
    map_low(KeysSource::Cldr);

    map_sing(KeysSource::Ducet);
    map_sing(KeysSource::Cldr);

    map_multi(KeysSource::Ducet);
    map_multi(KeysSource::Cldr);

    map_arabic_script_sing();
    map_arabic_script_multi();
}
