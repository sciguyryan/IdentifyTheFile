use rayon::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{self, BufReader, Read},
    sync::OnceLock,
};

pub static ASCII_READABLE_CHARACTERS: OnceLock<Vec<u8>> = OnceLock::new();
pub static ASCII_READABLE_CHARACTERS_SET: OnceLock<HashSet<u8>> = OnceLock::new();

pub fn get_ascii_readable_characters() -> &'static Vec<u8> {
    ASCII_READABLE_CHARACTERS.get_or_init(|| {
        b" !#$+,-./0123456789<=>?ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz".to_vec()
    })
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

fn all_substrings_over_min_size(string: &str) -> Vec<&str> {
    let mut substrings = Vec::new();
    let len = string.len();
    for start in 0..len {
        for end in (start + MIN_STRING_LENGTH)..=len {
            substrings.push(&string[start..end]);
        }
    }

    substrings
}

pub fn calculate_shannon_entropy(frequencies: &HashMap<u8, usize>) -> f64 {
    // Calculate the total frequency sum.
    let total_bytes = frequencies.values().sum::<usize>() as f64;

    // Compute the entropy.
    let mut entropy = 0.0;
    for &count in frequencies.values() {
        let probability = count as f64 / total_bytes;
        entropy -= probability * probability.log2();
    }

    entropy
}

pub fn common_string_sieve(hashsets: &mut Vec<HashSet<String>>) -> HashSet<String> {
    if hashsets.is_empty() {
        return HashSet::new();
    }

    // Find largest set to maximise the matching potential.
    let largest_hashset_index = hashsets
        .iter()
        .enumerate()
        .max_by_key(|(_, set)| set.len())
        .map(|(index, _)| index)
        .unwrap_or(0);
    let mut common_strings_hashset = hashsets.swap_remove(largest_hashset_index);

    // Find the common strings between all of the hashsets.
    while !hashsets.is_empty() {
        // Take the topmost hashset, allowing memory to be freed as we go.
        let mut set = hashsets.remove(0);

        // Extract the common elements between the new and common sets.
        let mut temp_set: HashSet<_> = common_strings_hashset.intersection(&set).cloned().collect();

        // Only retain items -not- present in the common set for analysis.
        set.retain(|s| !temp_set.contains(s));

        // Parallel iterate over the entries.
        temp_set.par_extend(
            common_strings_hashset
                .par_iter()
                .filter_map(|common_string| {
                    let mut largest_match = "";

                    for string in &set {
                        // Are we able to match a substring between the reference and new string?
                        if let Some(s) = largest_common_substring(string, common_string) {
                            // Check if this is the largest match we've seen so far.
                            if s.len() > largest_match.len() {
                                largest_match = s;
                            }
                        }
                    }

                    // Only insert if a match was found.
                    if !largest_match.is_empty() {
                        Some(largest_match.to_string())
                    } else {
                        None
                    }
                }),
        );

        // Update the common hashmap to reflect the new changes.
        common_strings_hashset = temp_set;
    }

    // There is one final step here, we do not want to retain any items that are
    // simply substrings of larger items.
    // They won't add anything unique.
    let mut final_hashset = HashSet::new();

    for item in &common_strings_hashset {
        if !common_strings_hashset
            .iter()
            .any(|other| other != item && other.contains(item))
        {
            final_hashset.insert(item.clone());
        }
    }

    final_hashset
}

pub fn count_byte_frequencies(data: &[u8], frequencies: &mut HashMap<u8, usize>) {
    for b in data {
        *frequencies.entry(*b).or_insert(0) += 1;
    }
}

#[inline]
fn extract_matching_sequences(seq_1: &[u8], seq_2: &[u8]) -> HashMap<usize, Vec<u8>> {
    let mut subsequences = HashMap::new();
    let mut sequence_start = None;
    let mut subsequence = Vec::with_capacity(seq_1.len().min(seq_2.len()));

    for (i, (&a, &b)) in seq_1.iter().zip(seq_2.iter()).enumerate() {
        if subsequence.len() >= MAX_BYTE_SEQUENCE_LENGTH {
            // End the current sequence if the length would exceed the maximum.
            if let Some(start) = sequence_start {
                if !subsequence.is_empty() {
                    subsequences.insert(start, std::mem::take(&mut subsequence));
                }

                sequence_start = None;
                subsequence.clear();
            }
        }

        if a == b {
            // Start a new sequence, if we aren't already in one.
            if sequence_start.is_none() {
                sequence_start = Some(i);
            }

            subsequence.push(a);

            continue;
        }

        if let Some(start) = sequence_start {
            // End the current sequence and store it if the sequence isn't empty.
            if !subsequence.is_empty() {
                subsequences.insert(start, std::mem::take(&mut subsequence));
            }

            sequence_start = None;
            subsequence.clear();
        }
    }

    // Check for any remaining sequence at the end
    if let Some(start) = sequence_start {
        if !subsequence.is_empty() {
            subsequences.insert(start, subsequence);
        }
    }

    subsequences
}

pub fn generate_file_string_hashset(bytes: &[u8]) -> HashSet<String> {
    let mut string_map = HashSet::new();

    let mut push_string = false;
    let mut string_buffer = String::with_capacity(MAX_STRING_LENGTH);
    for (i, byte) in bytes.iter().enumerate() {
        // At the first non-valid string byte, we consider the string terminated.
        if !get_ascii_readable_characters_set().contains(byte) {
            push_string = true;
        } else {
            // Push the character onto the buffer.
            string_buffer.push(*byte as char);
        }

        // If the string is of the maximum length then we want to
        // push it on the next iteration.
        // We also want to push the string if this is the final byte.
        if string_buffer.len() == MAX_STRING_LENGTH || i == bytes.len() - 1 {
            push_string = true;
        }

        if push_string {
            // Only retain strings that conform with the minimum length requirements.
            if string_buffer.len() >= MIN_STRING_LENGTH {
                string_map.insert(string_buffer.to_uppercase());
            }

            string_buffer = String::with_capacity(MAX_STRING_LENGTH);
            push_string = false;
        }
    }

    string_map
}

fn largest_common_substring<'a>(str_1: &'a str, str_2: &str) -> Option<&'a str> {
    let mut substrings_str1 = all_substrings_over_min_size(str_1);
    substrings_str1.sort_unstable_by_key(|b| std::cmp::Reverse(b.len()));

    substrings_str1
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
    common_byte_sequences: &mut HashMap<usize, Vec<u8>>,
) {
    let mut final_sequences = HashMap::with_capacity(common_byte_sequences.len());

    for (index, test_sequence) in common_byte_sequences.iter() {
        if *index > file_bytes.len() {
            continue;
        }

        // If the final index would fall outside the bounds of the
        // chunk then read to the end of the chunk instead.
        // If this still fall outside of the range (such as if index would be past the end of the file)
        // then this can't be a match for our candidate file.
        let segment_read_length = index.saturating_add(test_sequence.len().min(file_bytes.len()));
        if segment_read_length > file_bytes.len() {
            continue;
        }

        let subsequences =
            extract_matching_sequences(test_sequence, &file_bytes[*index..segment_read_length]);

        // Note - remember that the index in the sequence list is absolute
        // over the entire file, not the substring. This means we need
        // to add the overall index to the sub index!
        for (sub_index, seq) in subsequences {
            final_sequences.insert(*index + sub_index, seq);
        }
    }

    *common_byte_sequences = final_sequences;
}

pub fn strip_sequences_by_length(sequences: &mut HashMap<usize, Vec<u8>>) {
    // Strip any sequences that don't meet the requirements.
    // They should never be larger than the maximum length due to the way they are
    // processed, so we only need to worry about the minimum length requirements here.
    sequences.retain(|_, b| b.len() >= MIN_BYTE_SEQUENCE_LENGTH);
}
