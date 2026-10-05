# feruca-mapper

Generates tables for [feruca](https://github.com/theodore-s-beers/feruca). Currently uses Unicode 18 and CLDR 49 beta2.

To generate, copy (assuming a feruca checkout at `../feruca`), and test:

```sh
cargo run --release --locked
cp bincode/18/{ducet,decomp,fcd,variable} ../feruca/src/data/
cp generated/18/{implicit_weights.rs,low_ducet.rs} ../feruca/src/data/
cp generated/18/ccc.rs ../feruca/src/
cp bincode/cldr-49-beta2/cldr_root ../feruca/src/data/
cp bincode/cldr-49-beta2/tailoring/{arabic_interleaved,arabic_script} ../feruca/src/data/tailoring/
cp generated/cldr-49-beta2/low_cldr.rs ../feruca/src/data/
cargo test --manifest-path ../feruca/Cargo.toml --locked --all-features
```

Unicode data fall under the [Unicode license](https://www.unicode.org/license.txt).
