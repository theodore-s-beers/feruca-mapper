use crate::output::write_rust;
use std::{fmt::Write, sync::LazyLock};

pub static UNI_DATA: LazyLock<String> =
    LazyLock::new(|| std::fs::read_to_string("unicode-data/18/UnicodeData.txt").unwrap());

// UnicodeData represents large assigned ranges with First/Last records
static RECORDS: LazyLock<Vec<(u32, u32, u8)>> = LazyLock::new(|| {
    let mut records = Vec::new();
    let mut first = None;
    for line in UNI_DATA.lines() {
        let fields: Vec<_> = line.split(';').collect();
        let cp = u32::from_str_radix(fields[0], 16).unwrap();
        let ccc = fields[3].parse::<u8>().unwrap();
        if fields[1].ends_with(", First>") {
            assert!(first.replace((cp, ccc)).is_none());
        } else if fields[1].ends_with(", Last>") {
            let (start, first_ccc) = first.take().unwrap();
            assert_eq!(ccc, first_ccc);
            records.push((start, cp, ccc));
        } else {
            assert!(first.is_none());
            records.push((cp, cp, ccc));
        }
    }
    assert!(first.is_none());
    records
});

static CCC: LazyLock<Vec<u8>> = LazyLock::new(|| {
    let mut ccc = vec![0; crate::common::CODE_POINT_COUNT];
    for &(start, end, value) in RECORDS.iter() {
        ccc[usize::try_from(start).unwrap()..=usize::try_from(end).unwrap()].fill(value);
    }
    ccc
});

pub fn get_ccc(cp: u32) -> u8 {
    CCC[usize::try_from(cp).unwrap()]
}

fn hex_range(start: u32, end: u32) -> String {
    if start == end {
        format!("0x{start:X}")
    } else {
        format!("0x{start:X}..=0x{end:X}")
    }
}

fn parse_range(text: &str) -> (u32, u32) {
    let (start, end) = text
        .trim()
        .split_once("..")
        .unwrap_or_else(|| (text.trim(), text.trim()));
    (
        u32::from_str_radix(start, 16).unwrap(),
        u32::from_str_radix(end, 16).unwrap(),
    )
}

fn map_ccc() {
    const BLOCK_SIZE: usize = 256;
    let last = CCC.iter().rposition(|&ccc| ccc != 0).unwrap();
    let mut blocks: Vec<&[u8; BLOCK_SIZE]> = Vec::new();
    let mut offsets = Vec::new();

    // Keep one copy of each block, including the all-zero block
    for block in CCC
        .as_chunks::<BLOCK_SIZE>()
        .0
        .iter()
        .take(last / BLOCK_SIZE + 1)
    {
        let index = blocks
            .iter()
            .position(|&existing| existing == block)
            .unwrap_or_else(|| {
                blocks.push(block);
                blocks.len() - 1
            });
        offsets.push(u16::try_from(index * BLOCK_SIZE).unwrap());
    }
    let values: Vec<u8> = blocks.into_iter().flatten().copied().collect();

    let source = format!(
        "// Deduplicated 256-code-point blocks; values beyond the last nonzero class are zero.\n\
         pub const fn get_ccc(cp: u32) -> u8 {{\n\
         if cp <= {last} {{\n\
         BLOCKS[OFFSETS[(cp >> 8) as usize] as usize + (cp as usize & 255)]\n\
         }} else {{ 0 }}\n\
         }}\n\
         #[allow(clippy::unreadable_literal)]\n\
         static OFFSETS: [u16; {offset_count}] = {offsets:?};\n\
         #[allow(clippy::unreadable_literal)]\n\
         static BLOCKS: [u8; {value_count}] = {values:?};\n",
        offset_count = offsets.len(),
        value_count = values.len(),
    );
    write_rust("ccc.rs", &source);
}

pub fn map_unicode() {
    map_ccc();

    let mut rules = Vec::new();
    for line in crate::collation::KEYS_DUCET.lines() {
        if let Some(rule) = line.strip_prefix("@implicitweights ") {
            let (range, rest) = rule.split_once(';').unwrap();
            let (start, end) = parse_range(range);
            let base = u32::from_str_radix(rest.split_whitespace().next().unwrap(), 16).unwrap();
            rules.push((start, end, base));
        }
    }
    assert_ne!(rules, []);

    let mut assigned_ranges: Vec<(u32, u32)> = Vec::new();
    for &(start, end, _) in RECORDS.iter() {
        if let Some(last) = assigned_ranges.last_mut()
            && last.1 + 1 == start
        {
            last.1 = end;
        } else {
            assigned_ranges.push((start, end));
        }
    }

    let mut source = String::from(
        "// UCA 18 Table 16; unassigned code points use the fallback arm.\n\
         pub const fn implicit_weights(cp: u32) -> (u32, u32) {\n\
         let (a, b) = match cp {\n",
    );
    for &(start, end, base) in &rules {
        // Supplement blocks share their original block's offset and lead weight
        let offset = rules
            .iter()
            .filter(|r| r.2 == base)
            .map(|r| r.0)
            .min()
            .unwrap();
        for &(assigned_start, assigned_end) in &assigned_ranges {
            let first = start.max(assigned_start);
            let last = end.min(assigned_end);
            if first <= last {
                writeln!(
                    source,
                    "{} => (0x{base:X}, cp - 0x{offset:X}),",
                    hex_range(first, last)
                )
                .unwrap();
            }
        }
    }

    let properties = std::fs::read_to_string("unicode-data/18/PropList.txt").unwrap();
    for line in properties.lines() {
        let data = line.split('#').next().unwrap();
        let Some((range, property)) = data.split_once(';') else {
            continue;
        };
        if property.trim() != "Unified_Ideograph" {
            continue;
        }
        let (start, end) = parse_range(range);
        let base = if (0x4E00..=0x9FFF).contains(&start) || (0xF900..=0xFAFF).contains(&start) {
            0xFB40
        } else {
            0xFB80
        };
        writeln!(
            source,
            "{} => (0x{base:X} + (cp >> 15), cp & 0x7FFF),",
            hex_range(start, end)
        )
        .unwrap();
    }
    source.push_str("_ => (0xFBC0 + (cp >> 15), cp & 0x7FFF), }; (a, b | 0x8000) }\n");
    write_rust("implicit_weights.rs", &source);
}

#[cfg(test)]
mod tests {
    use super::*;

    mod generated {
        include!("../generated/18/ccc.rs");
    }

    #[test]
    fn generated_combining_classes_match_unicode_data() {
        for (cp, &expected) in CCC.iter().enumerate() {
            assert_eq!(
                generated::get_ccc(u32::try_from(cp).unwrap()),
                expected,
                "U+{cp:04X}"
            );
        }
        for cp in [0x11_0000, u32::MAX] {
            assert_eq!(generated::get_ccc(cp), 0);
        }
    }
}
