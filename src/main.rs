use feruca::KeysSource;
use feruca_mapper::{map_decomps, map_fcd, map_keys_low, map_keys_multi, map_keys_sing};

fn main() {
    map_decomps();
    map_fcd();

    map_keys_low(KeysSource::Ducet);
    map_keys_low(KeysSource::Cldr);

    map_keys_sing(KeysSource::Ducet);
    map_keys_sing(KeysSource::Cldr);

    map_keys_multi(KeysSource::Ducet);
    map_keys_multi(KeysSource::Cldr);
}
