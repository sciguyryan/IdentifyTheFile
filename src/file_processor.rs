use hashbrown::HashSet;
use rayon::prelude::*;
use std::{
    fs::File,
    io::{self, BufReader, Read},
    sync::OnceLock,
};

pub const ASCII_CHARACTER_STRING: &str =
    " !#$+,-./0123456789<=>?ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";
pub static ASCII_READABLE_CHARACTERS: OnceLock<&[u8]> = OnceLock::new();
pub static ASCII_READABLE_CHARACTERS_SET: OnceLock<HashSet<u8>> = OnceLock::new();

pub fn get_ascii_readable_characters() -> &'static [u8] {
    ASCII_READABLE_CHARACTERS.get_or_init(|| ASCII_CHARACTER_STRING.as_bytes())
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
pub const MAX_STRING_LENGTH: usize = 32;
/// The minimum length of a byte sequence.
const MIN_BYTE_SEQUENCE_LENGTH: usize = 1;
/// The maximum length of a byte sequence.
const MAX_BYTE_SEQUENCE_LENGTH: usize = 16;

/// The number of characters that need to be present in a string before parallel processing
/// should be used for the substring generator.
const PARALLEL_STRING_THRESHOLD: usize = 16;
/// The number of string collections that need to be present before parallel processing
/// should be used for the string sieve.
const PARALLEL_STRING_SIEVE_THRESHOLD: usize = 16;

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

            (start_min_len..=len)
                .map(|end| unsafe { string.get_unchecked(start..end) })
                .collect()
        })
        .collect()
}

#[inline]
fn all_substrings_over_min_size_sequential(string: &str) -> Vec<&str> {
    let len = string.len();
    (0..len)
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
pub fn common_string_sieve(sets: &mut [HashSet<&str>]) -> HashSet<String> {
    if sets.is_empty() {
        return HashSet::new();
    }

    // Find the largest set to maximize the matching potential.
    sets.sort_unstable_by_key(|b| std::cmp::Reverse(b.len()));

    let common_strings_hashset = if sets.len() < PARALLEL_STRING_SIEVE_THRESHOLD {
        common_string_sieve_parallel(sets)
    } else {
        common_string_sieve_sequential(sets)
    };

    // Filter out substrings of larger strings, we only want to keep the
    // largest possible match.
    let final_set: HashSet<_> = common_strings_hashset
        .iter()
        .filter(|&&item| {
            !common_strings_hashset
                .iter()
                .any(|&other| other != item && other.contains(item))
        })
        .map(|s| s.to_string())
        .collect();

    final_set
}

#[inline]
pub fn common_string_sieve_parallel<'a>(sets: &mut [HashSet<&'a str>]) -> HashSet<&'a str> {
    // The largest set will always appear first.
    let mut common_strings_hashset = sets.first().cloned().unwrap();

    for set in sets.iter_mut().skip(1) {
        common_strings_hashset = common_strings_hashset
            .par_iter()
            .flat_map(|common_string| {
                set.par_iter()
                    .filter_map(|&string| {
                        if !common_strings_hashset.contains(&string) {
                            // Only consider strings not present in the intersection of the two sets.
                            largest_common_substring(string, common_string)
                                .filter(|s| !s.is_empty())
                        } else {
                            None
                        }
                    })
                    // Only retain the largest substring.
                    .max_by_key(|s| s.len())
            })
            .chain(
                // Add the common items (set intersections) back into the final set.
                common_strings_hashset
                    .intersection(set)
                    .copied()
                    .collect::<HashSet<_>>()
                    .into_par_iter(),
            )
            .collect();
    }

    common_strings_hashset
}

#[inline]
pub fn common_string_sieve_sequential<'a>(sets: &mut [HashSet<&'a str>]) -> HashSet<&'a str> {
    // The largest set will always appear first.
    let mut common_strings_hashset = sets.first().cloned().unwrap();

    for set in sets.iter_mut().skip(1) {
        common_strings_hashset = common_strings_hashset
            .iter()
            .flat_map(|common_string| {
                set.iter()
                    .filter_map(|&string| {
                        if !common_strings_hashset.contains(&string) {
                            // Only consider strings not present in the intersection of the two sets.
                            largest_common_substring(string, common_string)
                                .filter(|s| !s.is_empty())
                        } else {
                            None
                        }
                    })
                    // Only retain the largest substring.
                    .max_by_key(|s| s.len())
            })
            .chain(
                // Add the common items (set intersections) back into the final set.
                common_strings_hashset
                    .intersection(set)
                    .copied()
                    .collect::<HashSet<_>>()
                    .into_iter(),
            )
            .collect();
    }

    common_strings_hashset
}

#[inline]
pub fn count_byte_frequencies(data: &[u8], frequencies: &mut [usize; 256]) {
    *frequencies = data
        .par_chunks(512)
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

#[inline]
unsafe fn extract_matching_sequences(
    start_at: &usize,
    seq_1: &[u8],
    seq_2: &[u8],
) -> Vec<(usize, Vec<u8>)> {
    let mut subsequences = Vec::with_capacity(100);
    let mut subsequence_start = usize::MAX;

    // Use the length of the shorter slice to avoid out-of-bounds access.
    let len = seq_1.len().min(seq_2.len());

    for i in 0..len {
        if *seq_1.get_unchecked(i) == *seq_2.get_unchecked(i) {
            if subsequence_start == usize::MAX {
                subsequence_start = i;
            }

            if i - subsequence_start == MAX_BYTE_SEQUENCE_LENGTH {
                subsequences.push((
                    *start_at + subsequence_start,
                    seq_1.get_unchecked(subsequence_start..i).to_vec(),
                ));

                // Immediately begin a new sequence, since we matched here, but we need to start a new sequence
                // due to the sequence length limitations.
                subsequence_start = i;
            }
        } else if subsequence_start != usize::MAX {
            subsequences.push((
                *start_at + subsequence_start,
                seq_1.get_unchecked(subsequence_start..i).to_vec(),
            ));
            subsequence_start = usize::MAX;
        }
    }

    if subsequence_start != usize::MAX {
        subsequences.push((
            *start_at + subsequence_start,
            seq_1.get_unchecked(subsequence_start..len).to_vec(),
        ));
    }

    subsequences
}

#[inline]
pub fn extract_file_strings(bytes: &[u8], readable: &HashSet<u8>) -> HashSet<String> {
    let mut strings = HashSet::with_capacity(128);
    let mut string_buffer = String::with_capacity(MAX_STRING_LENGTH);
    for (i, byte) in bytes.iter().enumerate() {
        let valid_readable = readable.contains(byte);
        if valid_readable {
            // Push the character onto the buffer.
            string_buffer.push(*byte as char);
        }

        // We should push the string buffer for any of the following conditions:
        // 1. There is a "non-readable" byte, so we consider the string terminated.
        // 2. The string is of the maximum length.
        // 3. This is the final byte.
        if !valid_readable || string_buffer.len() == MAX_STRING_LENGTH || i == bytes.len() - 1 {
            // Only retain strings that conform with the minimum length requirements.
            if string_buffer.len() >= MIN_STRING_LENGTH {
                // Convert the string to uppercase and insert into the set.
                strings.insert(string_buffer.to_ascii_uppercase());
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
