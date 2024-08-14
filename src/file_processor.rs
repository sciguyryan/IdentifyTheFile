use hashbrown::HashSet;
use rayon::prelude::*;
use std::{
    fs::File,
    io::{self, BufReader, Read},
    sync::OnceLock,
};

pub const ASCII_CHARACTER_STRING: &str =
    " !#$+,-./0123456789<=>?ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";
pub static ASCII_READABLE_CHARACTERS: OnceLock<Vec<u8>> = OnceLock::new();
pub static ASCII_READABLE_CHARACTERS_SET: OnceLock<HashSet<u8>> = OnceLock::new();

pub fn get_ascii_readable_characters() -> &'static Vec<u8> {
    ASCII_READABLE_CHARACTERS.get_or_init(|| ASCII_CHARACTER_STRING.as_bytes().to_vec())
}

pub fn get_ascii_readable_characters_set() -> &'static HashSet<u8> {
    ASCII_READABLE_CHARACTERS_SET
        .get_or_init(|| get_ascii_readable_characters().iter().copied().collect())
}

/// The size of a file chunk to read. Larger is more accurate but slower.
const FILE_CHUNK_SIZE: usize = 10 * 1024 * 1024; // 10 MB

/// The minimum length of a string that will be retained.
const MIN_STRING_LENGTH: usize = 5;
/// The maximum length of a string that will be retained.
const MAX_STRING_LENGTH: usize = 128;
/// The minimum length of a byte sequence.
const MIN_BYTE_SEQUENCE_LENGTH: usize = 1;
/// The maximum length of a byte sequence.
const MAX_BYTE_SEQUENCE_LENGTH: usize = 16;

/// The number of characters that need to be present in a string before parallel processing
/// should be used for the substring generator.
const PARALLEL_STRING_THRESHOLD: usize = 50;

#[inline]
fn all_substrings_over_min_size(string: &str) -> Vec<&str> {
    let len = string.len();
    if len < MIN_STRING_LENGTH {
        return Vec::new();
    }

    if len < PARALLEL_STRING_THRESHOLD {
        all_substrings_over_min_size_sequential(string)
    } else {
        all_substrings_over_min_size_parallel(string)
    }
}

#[inline]
fn all_substrings_over_min_size_parallel(string: &str) -> Vec<&str> {
    let len = string.len();
    (0..len)
        .into_par_iter()
        .flat_map(|start| {
            let start_min_len = start + MIN_STRING_LENGTH;
            if start_min_len > len {
                return Vec::new();
            }
            let mut local_substrings = Vec::with_capacity(len - start_min_len + 1);
            for end in start_min_len..=len {
                unsafe {
                    local_substrings.push(string.get_unchecked(start..end));
                }
            }
            local_substrings
        })
        .collect()
}

#[inline]
fn all_substrings_over_min_size_sequential(string: &str) -> Vec<&str> {
    let len = string.len();

    let num_substrings = len - MIN_STRING_LENGTH;
    let max_capacity = (num_substrings * (num_substrings + 1)) / 2;
    let mut substrings = Vec::with_capacity(max_capacity);

    let end_bound = len + 1;
    for start in 0..len {
        let start_min_len = start + MIN_STRING_LENGTH;
        for end in start_min_len..end_bound {
            unsafe {
                substrings.push(string.get_unchecked(start..end));
            }
        }
    }

    substrings
}

#[inline]
pub fn common_string_sieve(sets: Vec<HashSet<String>>) -> HashSet<String> {
    let mut sets = sets.to_owned();
    if sets.is_empty() {
        return HashSet::new();
    }

    // Find largest set to maximize the matching potential.
    let largest_set_index = sets
        .iter()
        .enumerate()
        .max_by_key(|(_, set)| set.len())
        .map(|(index, _)| index)
        .unwrap_or(0);
    let mut common_strings_hashset = sets.swap_remove(largest_set_index);

    for mut set in sets.drain(..) {
        // Extract the common elements.
        let intersections: HashSet<_> =
            common_strings_hashset.intersection(&set).cloned().collect();

        // Retain items -not- present in the common set.
        set.retain(|s| !intersections.contains(s));

        // Update the common set by finding the longest substring match common to
        // the reference and target strings, if applicable.
        common_strings_hashset = common_strings_hashset
            .par_iter()
            .flat_map(|common_string| {
                set.par_iter()
                    .filter_map(|string| {
                        largest_common_substring(string, common_string)
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                    })
                    .max_by_key(|s| s.len())
            })
            // Add the intersections back to the new common set.
            .chain(intersections.into_par_iter())
            .collect();
    }

    // Filter out substrings of larger items.
    let final_hashset: HashSet<_> = common_strings_hashset
        .iter()
        .filter(|&item| {
            !common_strings_hashset
                .iter()
                .any(|other| other != item && other.contains(item))
        })
        .cloned()
        .collect();

    final_hashset
}

#[inline]
pub fn count_byte_frequencies(data: &[u8], frequencies: &mut [usize; 256]) {
    // Process data in parallel chunks and aggregate the results using
    // a reduce operation.
    // This should be especially effective on larger files.
    *frequencies = data
        .par_chunks(1024)
        .map(|chunk| {
            let mut local_frequencies = [0; 256];
            for &b in chunk {
                local_frequencies[b as usize] += 1;
            }
            local_frequencies
        })
        .reduce(
            || [0; 256],
            |acc, local| {
                let mut result = acc;
                for (i, &count) in local.iter().enumerate() {
                    result[i] += count;
                }
                result
            },
        );
}

#[inline]
fn extract_matching_sequences(
    start_at: &usize,
    seq_1: &[u8],
    seq_2: &[u8],
) -> Vec<(usize, Vec<u8>)> {
    let max_len = seq_1.len().min(seq_2.len());

    let mut subsequences = Vec::with_capacity(100);
    let mut subsequence_start = None;
    let mut subsequence = Vec::with_capacity(max_len);

    for (i, (&a, &b)) in seq_1.iter().zip(seq_2.iter()).enumerate() {
        if a == b {
            if subsequence_start.is_none() {
                subsequence_start = Some(i);
            }
            subsequence.push(a);

            if subsequence.len() >= MAX_BYTE_SEQUENCE_LENGTH {
                subsequences.push((
                    *start_at + subsequence_start.take().unwrap(),
                    subsequence.clone(),
                ));
                subsequence.clear();
            }
        } else if let Some(start) = subsequence_start.take() {
            subsequences.push((*start_at + start, std::mem::take(&mut subsequence)));
            subsequence.clear();
        }
    }

    // Check for any remaining sequence at the end.
    if let Some(start) = subsequence_start {
        if !subsequence.is_empty() {
            subsequences.push((*start_at + start, subsequence));
        }
    }

    subsequences
}

#[inline]
pub fn extract_file_strings(bytes: &[u8]) -> HashSet<String> {
    let readable_subset = get_ascii_readable_characters_set().clone();

    let mut strings = HashSet::with_capacity(256);
    let mut string_buffer = String::with_capacity(MAX_STRING_LENGTH);
    for (i, byte) in bytes.iter().enumerate() {
        let mut valid_readable = false;

        if readable_subset.contains(byte) {
            // Push the character onto the buffer.
            string_buffer.push(*byte as char);
            valid_readable = true;
        }

        // We should push the string buffer for any of the following conditions:
        // 1. There is a "non-readable" byte, so we consider the string terminated.
        // 2. The string is of the maximum length.
        // 3. This is the final byte.
        if !valid_readable || string_buffer.len() == MAX_STRING_LENGTH || i == bytes.len() - 1 {
            // Only retain strings that conform with the minimum length requirements.
            if string_buffer.len() >= MIN_STRING_LENGTH {
                strings.insert(string_buffer.to_uppercase());
            }

            string_buffer.clear();
        }
    }

    strings
}

fn largest_common_substring<'a>(str_1: &'a str, str_2: &str) -> Option<&'a str> {
    let mut str_1_substrings = all_substrings_over_min_size(str_1);
    str_1_substrings.sort_unstable_by_key(|b| std::cmp::Reverse(b.len()));

    str_1_substrings
        .into_iter()
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

pub fn refine_common_byte_sequences_v2(
    file_bytes: &[u8],
    common_byte_sequences: &mut Vec<(usize, Vec<u8>)>,
) {
    let len = file_bytes.len();
    let mut final_sequences = Vec::with_capacity(common_byte_sequences.len());
    for (index, test_sequence) in common_byte_sequences.iter().filter(|(i, _)| *i <= len) {
        if *index > len {
            continue;
        }

        // If the final index would fall outside the bounds of the
        // chunk then read to the end of the chunk instead.
        // If this still fall outside of the range (such as if index would be past the end of the file)
        // then this can't be a match for our candidate file.
        let segment_read_length = index.saturating_add(test_sequence.len().min(len));
        if segment_read_length > len {
            continue;
        }

        let subsequences = extract_matching_sequences(
            index,
            test_sequence,
            &file_bytes[*index..segment_read_length],
        );

        // Note - remember that the index in the sequence list is absolute
        // over the entire file, not the substring. This means we need
        // to add the overall index to the sub index!
        final_sequences.extend_from_slice(&subsequences);
    }

    *common_byte_sequences = final_sequences;
}

pub fn strip_unwanted_sequences(sequences: &mut Vec<(usize, Vec<u8>)>) {
    // Strip any sequences that don't meet the requirements.
    // 1. Any sequences that are below the minimum length requirement. Maximum length enforcement is done elsewhere.
    // 2. Any sequences that are purely null bytes. These are unlikely to be helpful.
    sequences.retain(|(_, b)| b.iter().all(|&x| x != 0) && b.len() >= MIN_BYTE_SEQUENCE_LENGTH);
}
