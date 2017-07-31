use super::*;

use std::io;
use std::io::Write;
use std::io::Cursor;

fn memset<T : Clone>(slice: &mut [T], val: T) {
    for x in slice { *x = val.clone() }
}

pub fn tdefl_radix_sort_syms_oxide<'a>(symbols0: &'a mut [tdefl_sym_freq],
                                       symbols1: &'a mut [tdefl_sym_freq]) -> &'a mut [tdefl_sym_freq]
{
    let mut hist = [[0; 256]; 2];

    for freq in symbols0.iter() {
        hist[0][(freq.m_key & 0xFF) as usize] += 1;
        hist[1][((freq.m_key >> 8) & 0xFF) as usize] += 1;
    }

    let mut n_passes = 2;
    if symbols0.len() == hist[1][0] {
        n_passes -= 1;
    }

    let mut current_symbols = symbols0;
    let mut new_symbols = symbols1;

    for pass in 0..n_passes {
        let mut offsets = [0; 256];
        let mut offset = 0;
        for i in 0..256 {
            offsets[i] = offset;
            offset += hist[pass][i];
        }

        for sym in current_symbols.iter() {
            let j = ((sym.m_key >> (pass * 8)) & 0xFF) as usize;
            new_symbols[offsets[j]] = *sym;
            offsets[j] += 1;
        }

        mem::swap(&mut current_symbols, &mut new_symbols);
    }

    current_symbols
}

// TODO change to iterators
pub fn tdefl_calculate_minimum_redundancy_oxide(symbols: &mut [tdefl_sym_freq]) {
    match symbols.len() {
        0 => (),
        1 => symbols[0].m_key = 1,
        n => {
            symbols[0].m_key += symbols[1].m_key;
            let mut root = 0;
            let mut leaf = 2;
            for next in 1..n - 1 {
                if (leaf >= n) || (symbols[root].m_key < symbols[leaf].m_key) {
                    symbols[next].m_key = symbols[root].m_key;
                    symbols[root].m_key = next as u16;
                    root += 1;
                } else {
                    symbols[next].m_key = symbols[leaf].m_key;
                    leaf += 1;
                }

                if (leaf >= n) || (root < next && symbols[root].m_key < symbols[leaf].m_key) {
                    symbols[next].m_key = symbols[next].m_key + symbols[root].m_key; // TODO why cast to u16 in C?
                    symbols[root].m_key = next as u16;
                    root += 1;
                } else {
                    symbols[next].m_key = symbols[next].m_key + symbols[leaf].m_key;
                    leaf += 1;
                }
            }

            symbols[n - 2].m_key = 0;
            for next in (0..n - 2).rev() {
                symbols[next].m_key = symbols[symbols[next].m_key as usize].m_key + 1;
            }

            let mut avbl = 1;
            let mut used = 0;
            let mut dpth = 0;
            let mut root = (n - 2) as i32;
            let mut next = (n - 1) as i32;
            while avbl > 0 {
                while (root >= 0) && (symbols[root as usize].m_key == dpth) {
                    used += 1;
                    root -= 1;
                }
                while avbl > used {
                    symbols[next as usize].m_key = dpth;
                    next -= 1;
                    avbl -= 1;
                }
                avbl = 2 * used;
                dpth += 1;
                used = 0;
            }
        }
    }
}

pub fn tdefl_huffman_enforce_max_code_size_oxide(num_codes: &mut [c_int],
                                                 code_list_len: usize,
                                                 max_code_size: usize)
{
    if code_list_len <= 1 { return; }

    num_codes[max_code_size] += num_codes[max_code_size + 1..].iter().sum();
    let total = num_codes[1..max_code_size + 1].iter().rev().enumerate().fold(0u32, |total, (i, &x)| {
        total + ((x as u32) << i)
    });

    for _ in (1 << max_code_size)..total {
        num_codes[max_code_size] -= 1;
        for i in (1..max_code_size).rev() {
            if num_codes[i] != 0 {
                num_codes[i] -= 1;
                num_codes[i + 1] += 2;
                break;
            }
        }
    }
}

pub fn tdefl_optimize_huffman_table_oxide(h: &mut HuffmanOxide,
                                          table_num: usize,
                                          table_len: usize,
                                          code_size_limit: usize,
                                          static_table: bool)
{
    let mut num_codes = [0 as c_int; TDEFL_MAX_SUPPORTED_HUFF_CODESIZE + 1];
    let mut next_code = [0 as c_uint; TDEFL_MAX_SUPPORTED_HUFF_CODESIZE + 1];

    if static_table {
        for &code_size in &h.code_sizes[table_num][..table_len] {
            num_codes[code_size as usize] += 1;
        }
    } else {
        let mut symbols0 = [tdefl_sym_freq { m_key: 0, m_sym_index: 0 }; TDEFL_MAX_HUFF_SYMBOLS];
        let mut symbols1 = [tdefl_sym_freq { m_key: 0, m_sym_index: 0 }; TDEFL_MAX_HUFF_SYMBOLS];

        let mut num_used_symbols = 0;
        for i in 0..table_len {
            if h.count[table_num][i] != 0 {
                symbols0[num_used_symbols] = tdefl_sym_freq {
                    m_key: h.count[table_num][i],
                    m_sym_index: i as u16
                };
                num_used_symbols += 1;
            }
        }

        let mut symbols = tdefl_radix_sort_syms_oxide(&mut symbols0[..num_used_symbols],
                                                      &mut symbols1[..num_used_symbols]);
        tdefl_calculate_minimum_redundancy_oxide(symbols);

        for symbol in symbols.iter() {
            num_codes[symbol.m_key as usize] += 1;
        }

        tdefl_huffman_enforce_max_code_size_oxide(&mut num_codes, num_used_symbols, code_size_limit);

        memset(&mut h.code_sizes[table_num][..], 0);
        memset(&mut h.codes[table_num][..], 0);

        let mut last = num_used_symbols;
        for i in 1..code_size_limit + 1 {
            let first = last - num_codes[i] as usize;
            for symbol in &symbols[first..last] {
                h.code_sizes[table_num][symbol.m_sym_index as usize] = i as u8;
            }
            last = first;
        }
    }

    let mut j = 0;
    next_code[1] = 0;
    for i in 2..code_size_limit + 1 {
        j = (j + num_codes[i - 1]) << 1;
        next_code[i] = j as c_uint;
    }

    for (&code_size, huff_code) in h.code_sizes[table_num].iter().take(table_len)
                                    .zip(h.codes[table_num].iter_mut().take(table_len))
    {
        if code_size == 0 { continue }

        let mut code = next_code[code_size as usize];
        next_code[code_size as usize] += 1;

        let mut rev_code = 0;
        for _ in 0..code_size { // TODO reverse u32 faster?
            rev_code = (rev_code << 1) | (code & 1);
            code >>= 1;
        }
        *huff_code = rev_code as u16;
    }
}

pub struct HuffmanOxide<'a> {
    pub count: &'a mut [[u16; TDEFL_MAX_HUFF_SYMBOLS]; TDEFL_MAX_HUFF_TABLES],
    pub codes: &'a mut [[u16; TDEFL_MAX_HUFF_SYMBOLS]; TDEFL_MAX_HUFF_TABLES],
    pub code_sizes: &'a mut [[u8; TDEFL_MAX_HUFF_SYMBOLS]; TDEFL_MAX_HUFF_TABLES]
}

pub struct OutputBufferOxide<'a> {
    pub inner: Cursor<&'a mut [u8]>,

    pub bit_buffer: &'a mut u32,
    pub bits_in: &'a mut u32
}

impl<'a> OutputBufferOxide<'a> {
    fn tdefl_put_bits(&mut self, bits: u32, len: u32) -> io::Result<()> {
        assert!(bits <= ((1u32 << len) - 1u32));
        *self.bit_buffer |= bits << *self.bits_in;
        *self.bits_in += len;
        while *self.bits_in >= 8 {
            self.inner.write(&[*self.bit_buffer as u8][..])?;
            *self.bit_buffer >>= 8;
            *self.bits_in -= 8;
        }
        Ok(())
    }
}

const TDEFL_PACKED_CODE_SIZE_SYMS_SWIZZLE: [u8; 19] =
    [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

pub fn tdefl_start_dynamic_block_oxide(h: &mut HuffmanOxide, output: &mut OutputBufferOxide) -> io::Result<()> {
    h.count[0][256] = 1;

    tdefl_optimize_huffman_table_oxide(h, 0, TDEFL_MAX_HUFF_SYMBOLS_0, 15, false);
    tdefl_optimize_huffman_table_oxide(h, 1, TDEFL_MAX_HUFF_SYMBOLS_1, 15, false);

    let num_lit_codes = 286 - &h.code_sizes[0][257..286]
        .iter().rev().take_while(|&x| *x == 0).count();

    let num_dist_codes = 30 - &h.code_sizes[1][1..30]
        .iter().rev().take_while(|&x| *x == 0).count();

    let mut code_sizes_to_pack = [0u8; TDEFL_MAX_HUFF_SYMBOLS_0 + TDEFL_MAX_HUFF_SYMBOLS_1];
    let mut packed_code_sizes = [0u8; TDEFL_MAX_HUFF_SYMBOLS_0 + TDEFL_MAX_HUFF_SYMBOLS_1];

    let total_code_sizes_to_pack = num_lit_codes + num_dist_codes;

    &code_sizes_to_pack[..num_lit_codes]
        .copy_from_slice(&h.code_sizes[0][..num_lit_codes]);

    &code_sizes_to_pack[num_lit_codes..total_code_sizes_to_pack]
        .copy_from_slice(&h.code_sizes[1][..num_dist_codes]);

    struct RLE {
        pub rle_z_count: u32,
        pub rle_repeat_count: u32,
        pub prev_code_size: u8
    }

    let mut rle = RLE {
        rle_z_count: 0,
        rle_repeat_count: 0,
        prev_code_size: 0xFF
    };

    let tdefl_rle_prev_code_size = |rle: &mut RLE,
                                    packed_code_sizes: &mut Cursor<&mut [u8]>,
                                    h: &mut HuffmanOxide| -> io::Result<()>
        {
            if rle.rle_repeat_count != 0 {
                if rle.rle_repeat_count < 3 {
                    h.count[2][rle.prev_code_size as usize] = (h.count[2][rle.prev_code_size as usize] as i32 + rle.rle_repeat_count as i32) as u16; // TODO
                    while rle.rle_repeat_count != 0 {
                        rle.rle_repeat_count -= 1;
                        packed_code_sizes.write(&[rle.prev_code_size][..])?;
                    }
                } else {
                    h.count[2][16] = (h.count[2][16] as i32 + 1) as u16;
                    packed_code_sizes.write(&[16, (rle.rle_repeat_count as i32 - 3) as u8][..])?;
                }
                rle.rle_repeat_count = 0;
            }

            Ok(())
        };

    let tdefl_rle_zero_code_size = |rle: &mut RLE,
                                    packed_code_sizes: &mut Cursor<&mut [u8]>,
                                    h: &mut HuffmanOxide| -> io::Result<()>
        {
            if rle.rle_z_count != 0 {
                if rle.rle_z_count < 3 {
                    h.count[2][0] = (h.count[2][0] as i32 + rle.rle_z_count as i32) as u16;
                    while rle.rle_z_count != 0 {
                        rle.rle_z_count -= 1;
                        packed_code_sizes.write(&[0][..])?;
                    }
                } else if rle.rle_z_count <= 10 {
                    h.count[2][17] = (h.count[2][17] as i32 + 1) as u16;
                    packed_code_sizes.write(&[17, (rle.rle_z_count as i32 - 3) as u8][..])?;
                } else {
                    h.count[2][18] = (h.count[2][18] as i32 + 1) as u16;
                    packed_code_sizes.write(&[18, (rle.rle_z_count as i32 - 11) as u8][..])?;
                }
                rle.rle_z_count = 0;
            }

            Ok(())
        };

    memset(&mut h.count[2][..TDEFL_MAX_HUFF_SYMBOLS_2], 0);

    let mut packed_code_sizes_cursor = Cursor::new(&mut packed_code_sizes[..]);
    for &code_size in &code_sizes_to_pack[..total_code_sizes_to_pack] {
        if code_size == 0 {
            tdefl_rle_prev_code_size(&mut rle, &mut packed_code_sizes_cursor, h)?;
            rle.rle_z_count += 1;
            if rle.rle_z_count == 138 {
                tdefl_rle_zero_code_size(&mut rle, &mut packed_code_sizes_cursor, h)?;
            }
        } else {
            tdefl_rle_zero_code_size(&mut rle, &mut packed_code_sizes_cursor, h)?;
            if code_size != rle.prev_code_size {
                tdefl_rle_prev_code_size(&mut rle, &mut packed_code_sizes_cursor, h)?;
                h.count[2][code_size as usize] = (h.count[2][code_size as usize] as i32 + 1) as u16; // TODO why as u16?
                packed_code_sizes_cursor.write(&[code_size][..])?;
            } else {
                rle.rle_repeat_count += 1;
                if rle.rle_repeat_count == 6 {
                    tdefl_rle_prev_code_size(&mut rle, &mut packed_code_sizes_cursor, h)?;
                }
            }
        }
        rle.prev_code_size = code_size;
    }

    if rle.rle_repeat_count != 0 {
        tdefl_rle_prev_code_size(&mut rle, &mut packed_code_sizes_cursor, h)?;
    } else {
        tdefl_rle_zero_code_size(&mut rle, &mut packed_code_sizes_cursor, h)?;
    }

    tdefl_optimize_huffman_table_oxide(h, 2, TDEFL_MAX_HUFF_SYMBOLS_2, 7, false);

    output.tdefl_put_bits(2, 2)?;

    output.tdefl_put_bits((num_lit_codes - 257) as u32, 5)?;
    output.tdefl_put_bits((num_dist_codes - 1) as u32, 5)?;

    let mut num_bit_lengths = 18 - TDEFL_PACKED_CODE_SIZE_SYMS_SWIZZLE
        .iter().rev().take_while(|&swizzle| h.code_sizes[2][*swizzle as usize] == 0).count();

    num_bit_lengths = cmp::max(4, num_bit_lengths + 1);
    output.tdefl_put_bits(num_bit_lengths as u32 - 4, 4)?;
    for &swizzle in &TDEFL_PACKED_CODE_SIZE_SYMS_SWIZZLE[..num_bit_lengths] {
        output.tdefl_put_bits(h.code_sizes[2][swizzle as usize] as u32, 3)?;
    }

    let mut packed_code_size_index = 0 as usize;
    let packed_code_sizes = packed_code_sizes_cursor.get_ref();
    while packed_code_size_index < packed_code_sizes_cursor.position() as usize {
        let code = packed_code_sizes[packed_code_size_index] as usize;
        packed_code_size_index += 1;
        assert!(code < TDEFL_MAX_HUFF_SYMBOLS_2);
        output.tdefl_put_bits(h.codes[2][code] as u32, h.code_sizes[2][code] as u32)?;
        if code >= 16 {
            output.tdefl_put_bits(packed_code_sizes[packed_code_size_index] as u32,
                                  [2, 3, 7][code - 16])?;
            packed_code_size_index += 1;
        }
    }

    Ok(())
}

pub fn tdefl_start_static_block_oxide(h: &mut HuffmanOxide, output: &mut OutputBufferOxide) -> io::Result<()> {
    memset(&mut h.code_sizes[0][0..144], 8);
    memset(&mut h.code_sizes[0][144..256], 9);
    memset(&mut h.code_sizes[0][256..280], 7);
    memset(&mut h.code_sizes[0][280..288], 8);

    memset(&mut h.code_sizes[1][..32], 5);

    tdefl_optimize_huffman_table_oxide(h, 0, 288, 15, true);
    tdefl_optimize_huffman_table_oxide(h, 1, 32, 15, true);

    output.tdefl_put_bits(1, 2)
}

// TODO: only slow version
pub fn tdefl_compress_lz_codes_oxide(h: &mut HuffmanOxide,
                                     output: &mut OutputBufferOxide,
                                     lz_code_buf: &[u8]) -> io::Result<bool>
{
    let mut flags = 1;

    let mut i = 0;
    while i < lz_code_buf.len() {
        if flags == 1 {
            flags = lz_code_buf[i] as u32 | 0x100;
            i += 1;
        }

        if flags & 1 == 1 {
            let sym;
            let num_extra_bits;

            let match_len = lz_code_buf[i] as usize;
            let match_dist = lz_code_buf[i + 1] as usize | ((lz_code_buf[i + 2] as usize) << 8);
            i += 3;

            assert!(h.code_sizes[0][TDEFL_LEN_SYM[match_len] as usize] != 0);
            output.tdefl_put_bits(h.codes[0][TDEFL_LEN_SYM[match_len] as usize] as u32,
                h.code_sizes[0][TDEFL_LEN_SYM[match_len] as usize] as u32)?;

            output.tdefl_put_bits(match_len as u32 & MZ_BITMASKS[TDEFL_LEN_EXTRA[match_len] as usize] as u32,
                TDEFL_LEN_EXTRA[match_len] as u32)?;

            if match_dist < 512 {
                sym = TDEFL_SMALL_DIST_SYM[match_dist] as usize;
                num_extra_bits = TDEFL_SMALL_DIST_EXTRA[match_dist] as usize;
            } else {
                sym = TDEFL_LARGE_DIST_SYM[match_dist >> 8] as usize;
                num_extra_bits = TDEFL_LARGE_DIST_EXTRA[match_dist >> 8] as usize;
            }

            assert!(h.code_sizes[1][sym] != 0);
            output.tdefl_put_bits(h.codes[1][sym] as u32, h.code_sizes[1][sym] as u32)?;
            output.tdefl_put_bits(match_dist as u32 & MZ_BITMASKS[num_extra_bits as usize] as u32, num_extra_bits as u32)?;
        } else {
            let lit = lz_code_buf[i];
            i += 1;

            assert!(h.code_sizes[0][lit as usize] != 0);
            output.tdefl_put_bits(h.codes[0][lit as usize] as u32, h.code_sizes[0][lit as usize] as u32)?;
        }

        flags >>= 1;
    }

    output.tdefl_put_bits(h.codes[0][256] as u32, h.code_sizes[0][256] as u32)?;

    Ok(true)
}



pub fn tdefl_get_adler32_oxide(d: &tdefl_compressor) -> c_uint {
    d.m_adler32
}

pub fn tdefl_create_comp_flags_from_zip_params_oxide(level: c_int,
                                                     window_bits: c_int,
                                                     strategy: c_int) -> c_uint
{
    let num_probes = (if level >= 0 {
        cmp::min(10, level)
    } else {
        ::CompressionLevel::DefaultLevel as c_int
    }) as usize;
    let greedy = if level <= 3 { TDEFL_GREEDY_PARSING_FLAG } else { 0 } as c_uint;
    let mut comp_flags = TDEFL_NUM_PROBES[num_probes] | greedy;

    if window_bits > 0 {
        comp_flags |= TDEFL_WRITE_ZLIB_HEADER as c_uint;
    }

    if level == 0 {
        comp_flags |= TDEFL_FORCE_ALL_RAW_BLOCKS as c_uint;
    } else if strategy == ::CompressionStrategy::Filtered as c_int {
        comp_flags |= TDEFL_FILTER_MATCHES as c_uint;
    } else if strategy == ::CompressionStrategy::HuffmanOnly as c_int {
        comp_flags &= !TDEFL_MAX_PROBES_MASK as c_uint;
    } else if strategy == ::CompressionStrategy::Fixed as c_int {
        comp_flags |= TDEFL_FORCE_ALL_STATIC_BLOCKS as c_uint;
    } else if strategy == ::CompressionStrategy::RLE as c_int {
        comp_flags |= TDEFL_RLE_MATCHES as c_uint;
    }

    comp_flags
}
