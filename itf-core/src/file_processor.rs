use hashbrown::HashSet;
use rayon::prelude::*;
use std::{
    fs::File,
    io::{self, BufReader, Read},
};

pub(crate) const ASCII_CHARACTER_STRING: &str =
    " !#$+,-./0123456789<=>?ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";
const ASCII_READABLE_CHARACTERS: &[u8] = ASCII_CHARACTER_STRING.as_bytes();
const ASCII_READABLE_CHARACTERS_SET: [bool; 256] =
    get_ascii_readable_characters_set(ASCII_READABLE_CHARACTERS);
const ASCII_UPPERCASE_MAP: [char; 256] = generate_uppercase_map();

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

/// Sieve a set of strings to retain only those that are present in all of the sets.
///
/// # Arguments
///
/// * `sets` - A slice of vectors of strings.
///
/// # Returns
///
/// A vector containing only the strings (or substrings) that are present in every set.
#[inline]
pub(crate) fn common_string_sieve(sets: &mut [Vec<&str>]) -> Vec<String> {
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
            if let Some(max_string) = set
                .par_iter()
                .filter_map(|string| largest_common_substring(string, common_string))
                .max_by_key(|s| s.len())
            {
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

/// Count the number of instances of each byte within a slice of u8 values.
///
/// # Arguments
///
/// * `data` - A slice of bytes.
/// * `frequencies` - A mutable reference to the array of byte counts.
#[inline(always)]
pub fn count_byte_frequencies(data: &[u8], frequencies: &mut [usize; 256]) {
    let mut accumulator = data
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

    // Add the original counts back into the overall total.
    for (i, &v) in frequencies.iter().enumerate() {
        accumulator[i] += v;
    }

    *frequencies = accumulator
}

/// Extract a list of common byte sequences between two slices of u8 values.
///
/// # Arguments
///
/// * `start_at` - The position within the slice to start the scan.
/// * `seq_1` - The first u8 slice.
/// * `seq_2` - The second u8 slice.
///
/// # Returns
///
/// A vector of tuples containing the position of the match, and the bytes that match.
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
        let inc = *ptr1.add(i);
        if inc == *ptr2.add(i) {
            if subsequence_start == usize::MAX {
                subsequence_start = i;
            }

            buffer.push(inc);

            if buffer.len() == MAX_BYTE_SEQUENCE_LENGTH {
                subsequences.push((*start_at + subsequence_start, std::mem::take(&mut buffer)));

                // Immediately begin a new sequence, since we are still within a match.
                // We need to start a new sequence due to the sequence length limitations.
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

/// Extract valid strings from a slice of u8 values.
///
/// # Arguments
///
/// * `bytes` - The slice of u8 values.
///
/// # Returns
///
/// A [`HashSet`] containing the extracted files.
#[inline(always)]
pub(crate) fn extract_file_strings(bytes: &[u8]) -> HashSet<String> {
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

/// Attempt to find a u8 sequence within a slice of u8 values.
///
/// # Arguments
///
/// * `haystack` - The slice of u8 values within which the sequence should be found.
/// * `needle` - The slice of u8 values to be located.
///
/// # Returns
///
/// An option - none if the needle wasn't located or the position of the first match.
#[inline(always)]
fn find_slice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    use std::ptr;

    if needle.is_empty() {
        return Some(0);
    }

    let needle_len = needle.len();
    let haystack_len = haystack.len();

    if needle_len > haystack_len {
        return None;
    }

    let end = haystack_len - needle_len + 1;

    unsafe {
        for i in 0..end {
            if ptr::read_unaligned(haystack.as_ptr().add(i)) == *needle.as_ptr()
                && haystack.get_unchecked(i..i + needle_len) == needle
            {
                return Some(i);
            }
        }
    }

    None
}

/// Generate an array that indicates whether a byte corresponds to a readable character from our permitted character subset.
///
/// # Arguments
///
/// * `chars` - A slice of u8 values corresponding to the our permitted characters.
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

/// Generate an uppercase map for the ASCII characters.
///
/// # Returns
///
/// An array giving the uppercase variants of each byte within the ASCII character set.
const fn generate_uppercase_map() -> [char; 256] {
    let mut map = ['\0'; 256];
    let mut i = 0;

    while i < 256 {
        map[i] = ((i as u8) as char).to_ascii_uppercase();

        i += 1;
    }

    map
}

/// Check whether two sequences have one or more values in common.
///
/// # Arguments
///
/// * `seq_1` - The first slice of u8 values.
/// * `seq_2` - The second slice of u8 values.
#[inline(always)]
fn has_common_elements(seq_1: &[u8], seq_2: &[u8]) -> bool {
    let mut instances: [bool; 256] = [false; 256];

    for &b in seq_1 {
        instances[b as usize] = true;
    }

    for &b in seq_2 {
        if instances[b as usize] {
            return true;
        }
    }

    false
}

/// Attempt to find the largest common substring between two string slices.
///
/// # Arguments
///
/// * `str_1` - The first string slice.
/// * `str_2` - The second string slice.
///
/// # Returns
///
/// An option - none if there was no common substring available, or the largest common substring.
#[inline(always)]
fn largest_common_substring<'a>(str_1: &'a str, str_2: &str) -> Option<&'a str> {
    if str_1 == str_2 {
        return Some(str_1);
    }

    let str_1_bytes = str_1.as_bytes();
    let str_2_bytes = str_2.as_bytes();
    if !has_common_elements(str_1_bytes, str_2_bytes) {
        return None;
    }

    (MIN_STRING_LENGTH..=str_1_bytes.len())
        .rev()
        .flat_map(|size| str_1_bytes.windows(size))
        .find(|seq| find_slice(str_2_bytes, seq).is_some())
        .map(|window| unsafe { std::str::from_utf8_unchecked(window) })
}

/// Attempt to read the header chunk of a file.
///
/// # Arguments
///
/// * `file_path` - The path to the file.
///
/// # Returns
///
/// A vector containing the u8 values if the data was successfully read, otherwise an error.
pub fn read_file_header_chunk(file_path: &str) -> io::Result<Vec<u8>> {
    let file = File::open(file_path)?;
    let filesize = file.metadata()?.len() as usize;
    let read_size = filesize.min(FILE_CHUNK_SIZE);
    let mut buf_reader = BufReader::new(file);
    let mut buffer = vec![0; read_size];
    buf_reader.read_exact(&mut buffer)?;

    Ok(buffer)
}

/// Refine a common byte sequence set, based on a new u8 slice.
///
/// # Arguments
///
/// * `file_bytes` - A slice of u8 values.
/// * `common_byte_sequences` - A mutable reference to the vector of tuples giving the position of the sequence and the byte sequence.
#[inline]
pub fn refine_common_byte_sequences_v2(file_bytes: &[u8], sequences: &mut Vec<(usize, Vec<u8>)>) {
    let len = file_bytes.len();
    let mut final_sequences = Vec::with_capacity(sequences.len());
    for (index, test_sequence) in sequences.iter().filter(|(i, _)| *i <= len) {
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

    *sequences = final_sequences;
}

/// Strip sequences that don't conform to our requirements.
///
/// # Arguments
///
/// * `sequences` - A mutable reference to the vector of tuples giving the position of the sequence and the byte sequence.
pub(crate) fn strip_unwanted_sequences(sequences: &mut Vec<(usize, Vec<u8>)>) {
    // Strip any sequences that don't meet the requirements.
    // 1. Any sequences that are below the minimum length requirement. Maximum length enforcement is done elsewhere.
    // 2. Any sequences that are purely null bytes. These are unlikely to be helpful.
    sequences.retain(|(_, b)| b.iter().all(|&x| x != 0) && b.len() >= MIN_BYTE_SEQUENCE_LENGTH);
}
