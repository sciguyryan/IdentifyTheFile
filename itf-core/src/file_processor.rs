use hashbrown::HashSet;
use rayon::prelude::*;
use std::{
    fs::File,
    io::{self, BufReader, Read},
};

pub const ASCII_CHARACTER_STRING: &str =
    " !#$+,-./0123456789<=>?ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";
const ASCII_READABLE_CHARACTERS: &[u8] = ASCII_CHARACTER_STRING.as_bytes();
const ASCII_READABLE_CHARACTERS_SET: [bool; 256] =
    get_ascii_readable_characters_set(ASCII_READABLE_CHARACTERS);

#[inline(always)]
const fn get_ascii_readable_characters_set(chars: &[u8]) -> [bool; 256] {
    let mut is_readable = [false; 256];
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        is_readable[c as usize] = true;
        i += 1;
    }

    is_readable
}

const ASCII_UPPERCASE_MAP: [char; 256] = generate_uppercase_map();

#[inline(always)]
const fn generate_uppercase_map() -> [char; 256] {
    let mut map = ['\0'; 256];
    let mut i = 0;

    while i < 256 {
        map[i] = ((i as u8) as char).to_ascii_uppercase();

        i += 1;
    }

    map
}

/// The size of a file chunk to read. Larger is more accurate but slower.
const FILE_CHUNK_SIZE: usize = 5 * 1024 * 1024; // 5 MB
/// The size of a byte chunk to be processed in parallel when computing byte distributions.
const BYTE_COUNT_CHUNK_SIZE: usize = 512; // 512 B

/// The minimum length of a string that will be retained.
const MIN_STRING_LENGTH: usize = 5;
/// The maximum length of a string that will be retained.
pub const MAX_STRING_LENGTH: usize = 64;
/// The minimum length of a byte sequence.
const MIN_BYTE_SEQUENCE_LENGTH: usize = 1;
/// The maximum length of a byte sequence.
const MAX_BYTE_SEQUENCE_LENGTH: usize = 16;

#[inline]
pub fn common_string_sieve(sets: &mut [Vec<&str>]) -> Vec<String> {
    if sets.is_empty() {
        return Vec::new();
    }

    sets.sort_unstable_by_key(|b| b.len());

    let mut common_strings = unsafe { sets.get_unchecked(sets.len() - 1) }.to_vec();
    let mut new_common_strings = Vec::with_capacity(common_strings.len());

    let last_index = sets.len() - 1;
    for set in &sets[..last_index] {
        new_common_strings.clear();

        for common_string in &common_strings {
            let max_string = set
                .par_iter()
                .filter_map(|string| largest_common_substring(string, common_string))
                .max_by_key(|s| s.len());

            if let Some(max_string) = max_string {
                new_common_strings.push(max_string);
            }
        }

        if new_common_strings.is_empty() {
            return Vec::new();
        }

        unsafe {
            std::ptr::swap(&mut common_strings, &mut new_common_strings);
        }
    }

    // Filter out substrings of larger strings, we only want to keep the
    // largest possible match.
    let final_set: Vec<_> = common_strings
        .iter()
        .filter(|&&item| {
            !common_strings
                .iter()
                .any(|&other| other != item && other.contains(item))
        })
        .map(|s| s.to_string())
        .collect();

    final_set
}

#[inline(always)]
pub fn count_byte_frequencies(data: &[u8], frequencies: &mut [usize; 256]) {
    *frequencies = data
        .par_chunks(BYTE_COUNT_CHUNK_SIZE)
        .fold(
            || [0; 256],
            |mut local_frequencies, chunk| {
                for &b in chunk {
                    local_frequencies[b as usize] += 1;
                }
                local_frequencies
            },
        )
        .reduce(
            || [0; 256],
            |mut acc, local| {
                for (i, &count) in local.iter().enumerate() {
                    acc[i] += count;
                }
                acc
            },
        );
}

#[inline(always)]
unsafe fn extract_matching_sequences(
    start_at: &usize,
    seq_1: &[u8],
    seq_2: &[u8],
) -> Vec<(usize, Vec<u8>)> {
    let mut subsequences = Vec::with_capacity(100);
    let mut subsequence_start = usize::MAX;

    // The safe use of pointers, since we have guaranteed that our indices
    // will always be in bounds.
    let ptr1 = seq_1.as_ptr();
    let ptr2 = seq_2.as_ptr();

    let mut buffer = Vec::with_capacity(MAX_BYTE_SEQUENCE_LENGTH);

    let len = seq_1.len().min(seq_2.len());
    for i in 0..len {
        if *ptr1.add(i) == *ptr2.add(i) {
            if subsequence_start == usize::MAX {
                subsequence_start = i;
            }

            buffer.push(*ptr1.add(i));

            if buffer.len() == MAX_BYTE_SEQUENCE_LENGTH {
                subsequences.push((*start_at + subsequence_start, std::mem::take(&mut buffer)));

                // Immediately begin a new sequence, since we matched here, but we need to start a new sequence
                // due to the sequence length limitations.
                subsequence_start = i + 1;
            }
        } else if subsequence_start != usize::MAX {
            subsequences.push((*start_at + subsequence_start, std::mem::take(&mut buffer)));
            subsequence_start = usize::MAX;
        }
    }

    if !buffer.is_empty() {
        subsequences.push((*start_at + subsequence_start, buffer));
    }

    subsequences
}

#[inline(always)]
pub fn extract_file_strings(bytes: &[u8]) -> HashSet<String> {
    let mut strings = HashSet::with_capacity(128);
    let mut string_buffer = String::with_capacity(MAX_STRING_LENGTH);
    for byte in bytes {
        let is_readable = unsafe { *ASCII_READABLE_CHARACTERS_SET.get_unchecked(*byte as usize) };

        if is_readable {
            // The map is a fixed size and the call is safe since it can never go
            // beyond the bounds.
            unsafe {
                string_buffer.push(*ASCII_UPPERCASE_MAP.get_unchecked(*byte as usize));
            }

            if string_buffer.len() == MAX_STRING_LENGTH {
                strings.insert(std::mem::take(&mut string_buffer));
                string_buffer.clear();
            }
        } else {
            if string_buffer.len() >= MIN_STRING_LENGTH {
                strings.insert(std::mem::take(&mut string_buffer));
            }

            string_buffer.clear();
        }
    }

    if string_buffer.len() >= MIN_STRING_LENGTH {
        strings.insert(string_buffer);
    }

    strings
}

#[inline(always)]
fn largest_common_substring<'a>(str_1: &'a str, str_2: &str) -> Option<&'a str> {
    if str_1 == str_2 {
        return Some(str_1);
    }

    (MIN_STRING_LENGTH..=str_1.len())
        .rev()
        .flat_map(|size| str_1.as_bytes().windows(size))
        .map(|window| unsafe { std::str::from_utf8_unchecked(window) })
        .find(|&substr| str_2.contains(substr))
}

pub fn read_file_header_chunk(file_path: &str) -> io::Result<Vec<u8>> {
    let file = File::open(file_path)?;
    let filesize = file.metadata()?.len() as usize;
    let read_size = filesize.min(FILE_CHUNK_SIZE);
    let mut buf_reader = BufReader::new(file);
    let mut buffer = vec![0; read_size];
    buf_reader.read_exact(&mut buffer)?;

    Ok(buffer)
}

#[inline]
pub fn refine_common_byte_sequences_v2(
    file_bytes: &[u8],
    common_byte_sequences: &mut Vec<(usize, Vec<u8>)>,
) {
    let len = file_bytes.len();
    let mut final_sequences = Vec::with_capacity(common_byte_sequences.len());
    for (index, test_sequence) in common_byte_sequences.iter().filter(|(i, _)| *i <= len) {
        // Calculate the end index of the read. Should the end index fall outside
        // the bounds of the vector, then we'll read to the end of the vector instead.
        // This check -must- be maintained because, for smaller files, an end index
        // may fall beyond the bounds of the file.
        let segment_read_length = index.saturating_add(test_sequence.len()).min(len);

        // This is safe as we have validated the indices will always be within
        // the bounds of the vector.
        unsafe {
            let subsequences = extract_matching_sequences(
                index,
                test_sequence,
                file_bytes.get_unchecked(*index..segment_read_length),
            );

            // Note - remember that the index in the sequence list is absolute
            // over the entire file, not the substring. This means we need
            // to add the overall index to the sub index!
            final_sequences.extend_from_slice(&subsequences);
        }
    }

    *common_byte_sequences = final_sequences;
}

pub fn strip_unwanted_sequences(sequences: &mut Vec<(usize, Vec<u8>)>) {
    // Strip any sequences that don't meet the requirements.
    // 1. Any sequences that are below the minimum length requirement. Maximum length enforcement is done elsewhere.
    // 2. Any sequences that are purely null bytes. These are unlikely to be helpful.
    sequences.retain(|(_, b)| b.iter().all(|&x| x != 0) && b.len() >= MIN_BYTE_SEQUENCE_LENGTH);
}
