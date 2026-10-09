// Copyright 1995-2023 Mark Adler (zlib); port by The skia-rust Authors.
// Use of this source code is governed by the zlib licence in the LICENSE file.
// Port of: inftrees.c#L1-L299 (chromium zlib@646b7f56, zlib 1.3.0.1-motley)
// Ported from: inftrees.c, inftrees.h

//! Generate Huffman lookup tables for the inflate decoder.

/// Port of `MAXBITS`.
pub const MAXBITS: usize = 15;
/// Port of `ENOUGH_LENS`: the most table entries a literal/length table can take.
pub const ENOUGH_LENS: usize = 1332;
/// Port of `ENOUGH_DISTS`: the most table entries a distance table can take.
pub const ENOUGH_DISTS: usize = 592;
/// Port of `ENOUGH`.
pub const ENOUGH: usize = ENOUGH_LENS + ENOUGH_DISTS;

/// One entry of a decoding table. Port of `code` (inftrees.h#L24-L28).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[doc(alias = "code")]
pub struct Code {
    /// Operation: 0 literal, 16+ length/distance base with extra bits, 32 end of block, 64 invalid.
    pub op: u8,
    /// Number of bits this entry consumes.
    pub bits: u8,
    /// Literal value, base length/distance, or sub-table offset.
    pub val: u16,
}

/// Port of `codetype`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeType {
    Codes,
    Lens,
    Dists,
}

// Port of: inftrees.c#L54-L58 (the base and extra tables used by inflate_table)
const LBASE: [u16; 31] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258, 0, 0,
];
const LEXT: [u16; 31] = [
    16, 16, 16, 16, 16, 16, 16, 16, 17, 17, 17, 17, 18, 18, 18, 18, 19, 19, 19, 19, 20, 20, 20, 20,
    21, 21, 21, 21, 16, 70, 200,
];
const DBASE: [u16; 32] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577, 0, 0,
];
const DEXT: [u16; 32] = [
    16, 16, 16, 16, 17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22, 22, 23, 23, 24, 24, 25, 25, 26, 26,
    27, 27, 28, 28, 29, 29, 64, 64,
];

/// Port of `inflate_table`: builds the decoding table for `codes` code lengths in `lens`.
///
/// `table` is the whole decoding-table storage and `table_pos` the index where this table starts
/// (`*table` in C); on success `table_pos` is advanced past the table and `bits` is set to the
/// root table size actually used. `work` is scratch space of at least 288 entries. Returns 0 on
/// success, 1 if the table would not fit, and -1 for an over-subscribed or incomplete code.
// Port of: inftrees.c#L32-L299 (chromium zlib@646b7f56)
#[allow(clippy::too_many_lines)] // mirrors inflate_table as one function, as in zlib
#[allow(clippy::too_many_arguments)] // mirrors inflate_table's signature
#[allow(clippy::cast_possible_truncation)] // the C code narrows to unsigned char / short on purpose
#[allow(clippy::cast_sign_loss)] // counts and indices are non-negative here
#[allow(clippy::cast_possible_wrap)] // the C code compares `left` as a signed int
#[allow(clippy::similar_names)] // names follow zlib's variables (`len`, `lens`, `left`, `low`)
pub fn inflate_table(
    kind: CodeType,
    lens: &[u16],
    codes: usize,
    table: &mut [Code],
    table_pos: &mut usize,
    bits: &mut u32,
    work: &mut [u16],
) -> i32 {
    let mut count = [0u16; MAXBITS + 1];
    let mut offs = [0u16; MAXBITS + 1];

    // Port of: inftrees.c#L102-L105 (count the code lengths)
    for sym in 0..codes {
        count[lens[sym] as usize] += 1;
    }

    // Port of: inftrees.c#L108-L120 (root and maximum length)
    let mut root = *bits as usize;
    let mut max = MAXBITS;
    while max >= 1 && count[max] == 0 {
        max -= 1;
    }
    if root > max {
        root = max;
    }
    if max == 0 {
        // No symbols to code at all: make a table that always fails.
        let here = Code {
            op: 64,
            bits: 1,
            val: 0,
        };
        table[*table_pos] = here;
        *table_pos += 1;
        table[*table_pos] = here;
        *table_pos += 1;
        *bits = 1;
        return 0;
    }

    // Port of: inftrees.c#L121-L123 (minimum length)
    let mut min = 1;
    while min < max && count[min] == 0 {
        min += 1;
    }
    if root < min {
        root = min;
    }

    // Port of: inftrees.c#L126-L133 (check for an over-subscribed or incomplete set)
    let mut left: i32 = 1;
    for &c in &count[1..=MAXBITS] {
        left <<= 1;
        left -= i32::from(c);
        if left < 0 {
            return -1;
        }
    }
    if left > 0 && (kind == CodeType::Codes || max != 1) {
        return -1; // incomplete set
    }

    // Port of: inftrees.c#L136-L142 (offsets into the sorted symbol list)
    offs[1] = 0;
    for len in 1..MAXBITS {
        offs[len + 1] = offs[len] + count[len];
    }
    for sym in 0..codes {
        if lens[sym] != 0 {
            work[offs[lens[sym] as usize] as usize] = sym as u16;
            offs[lens[sym] as usize] += 1;
        }
    }

    // Port of: inftrees.c#L176-L190 (the value/extra tables for each kind)
    // For `Codes`, symbols are below `match` (20), so the base/extra lookups are never reached.
    let (base, extra, match_): (&[u16], &[u16], u16) = match kind {
        CodeType::Codes => (&[], &[], 20),
        CodeType::Lens => (&LBASE, &LEXT, 257),
        CodeType::Dists => (&DBASE, &DEXT, 0),
    };

    // Port of: inftrees.c#L193-L201 (set up the table-filling loop)
    let mut huff: u32 = 0;
    let mut sym: usize = 0;
    let mut len = min;
    let mut next = *table_pos;
    let mut curr = root;
    let mut drop: usize = 0;
    let mut low: u32 = u32::MAX;
    let mut used: usize = 1 << root;
    let mask: u32 = (used - 1) as u32;

    // Port of: inftrees.c#L204-L206 (check available table space)
    if (kind == CodeType::Lens && used > ENOUGH_LENS)
        || (kind == CodeType::Dists && used > ENOUGH_DISTS)
    {
        return 1;
    }

    // Port of: inftrees.c#L209-L283 (fill the table for each code, creating sub-tables as needed)
    loop {
        // Port of: inftrees.c#L210-L224 (the entry for symbol `sym`)
        let mut here = Code {
            bits: (len - drop) as u8,
            op: 0,
            val: 0,
        };
        let w = u32::from(work[sym]);
        if w + 1 < u32::from(match_) {
            here.op = 0;
            here.val = work[sym];
        } else if w >= u32::from(match_) {
            here.op = extra[(w - u32::from(match_)) as usize] as u8;
            here.val = base[(w - u32::from(match_)) as usize];
        } else {
            here.op = 32 + 64; // end of block
            here.val = 0;
        }

        // Port of: inftrees.c#L226-L232 (replicate the entry through the table)
        let mut incr = 1usize << (len - drop);
        let mut fill = 1usize << curr;
        min = fill;
        loop {
            fill -= incr;
            table[next + ((huff as usize) >> drop) + fill] = here;
            if fill == 0 {
                break;
            }
        }

        // Port of: inftrees.c#L235-L243 (backwards increment of the Huffman code)
        incr = 1usize << (len - 1);
        while (huff as usize) & incr != 0 {
            incr >>= 1;
        }
        if incr != 0 {
            huff &= (incr - 1) as u32;
            huff += incr as u32;
        } else {
            huff = 0;
        }

        // Port of: inftrees.c#L246-L250 (go to the next symbol, update the count and length)
        sym += 1;
        count[len] -= 1;
        if count[len] == 0 {
            if len == max {
                break;
            }
            len = lens[work[sym] as usize] as usize;
        }

        // Port of: inftrees.c#L253-L282 (create a new sub-table when a prefix is used up)
        if len > root && (huff & mask) != low {
            if drop == 0 {
                drop = root;
            }
            next += min;
            curr = len - drop;
            left = 1i32 << curr;
            while curr + drop < max {
                left -= i32::from(count[curr + drop]);
                if left <= 0 {
                    break;
                }
                curr += 1;
                left <<= 1;
            }
            used += 1usize << curr;
            if (kind == CodeType::Lens && used > ENOUGH_LENS)
                || (kind == CodeType::Dists && used > ENOUGH_DISTS)
            {
                return 1;
            }
            low = huff & mask;
            let root_entry = *table_pos + low as usize;
            table[root_entry] = Code {
                op: curr as u8,
                bits: root as u8,
                val: (next - *table_pos) as u16,
            };
        }
    }

    // Port of: inftrees.c#L288-L294 (an incomplete code gets an invalid end entry)
    if huff != 0 {
        table[next + huff as usize] = Code {
            op: 64,
            bits: (len - drop) as u8,
            val: 0,
        };
    }

    // Port of: inftrees.c#L296-L298
    *table_pos += used;
    *bits = root as u32;
    0
}
