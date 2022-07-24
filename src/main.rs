use feruca::KeysSource;
use feruca_mapper::{map_decomps, map_fcd, map_low, map_multi, map_sing, map_variable};

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
}
