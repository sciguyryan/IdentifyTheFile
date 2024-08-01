use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{self, BufReader, Read},
};

use rayon::prelude::*;

use crate::utils;

const FILE_CHUNK_SIZE: usize = 10 * 1024 * 1024; // 10 MB
pub const STRING_CHARS: [u8; 74] =
    *b" $+,-./0123456789<=>?ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";
const MIN_STRING_LENGTH: usize = 5;
const MAX_STRING_LENGTH: usize = 128;
const MIN_BYTE_SEQUENCE_LENGTH: usize = 1;
const MAX_BYTE_SEQUENCE_LENGTH: usize = 16;

pub fn strip_sequences_by_length(sequences: &mut HashMap<usize, Vec<u8>>) {
    // Strip any sequences that don't meet the requirements.
    sequences
        .retain(|_, b| b.len() >= MIN_BYTE_SEQUENCE_LENGTH && b.len() <= MAX_BYTE_SEQUENCE_LENGTH);
}

pub fn print_byte_sequence_matches(sequences: &HashMap<usize, Vec<u8>>) {
    let mut vec: Vec<(usize, Vec<u8>)> = sequences
        .iter()
        .map(|(index, m)| (*index, m.clone()))
        .collect();
    vec.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    println!("{vec:?}");
}

pub fn test_matching_file_byte_sequences(
    path: &str,
    target_extension: &str,
    sequences: &HashMap<usize, Vec<u8>>,
) -> bool {
    let mut all_success = true;

    let files = utils::list_files_of_type(path, target_extension);
    for file_path in &files {
        // No sequences, we can skip the scan completely.
        if sequences.is_empty() {
            break;
        }

        let chunk = read_file_header_chunk(file_path).expect("failed to read file");

        let mut matches = 0;
        for (start, sequence) in sequences {
            let end = *start + sequence.len();
            if end > chunk.len() {
                continue;
            }

            if sequence == &chunk[*start..end] {
                matches += 1;
            }
        }

        all_success &= matches == sequences.len();
    }

    all_success
}

pub fn test_matching_file_strings(
    path: &str,
    target_extension: &str,
    ref_chars: &HashSet<u8>,
    common_strings: &Vec<String>,
) -> bool {
    let mut all_success = true;

    let files = utils::list_files_of_type(path, target_extension);
    for file_path in &files {
        // No strings, we can skip the scan completely.
        if common_strings.is_empty() {
            break;
        }

        let chunk = read_file_header_chunk(file_path).expect("failed to read file");
        let strings = generate_file_string_hashset(&chunk, ref_chars);

        let mut matches = 0;
        for el in common_strings {
            for str in &strings {
                if str.contains(el) {
                    matches += 1;
                    break;
                }
            }
        }

        if matches != common_strings.len() {
            all_success = false;
            break;
        }
    }

    all_success
}

#[inline]
fn extract_matching_sequences(seq_1: &[u8], seq_2: &[u8]) -> HashMap<usize, Vec<u8>> {
    let mut subsequences = HashMap::new();
    let mut sequence_start = None;
    let mut subsequence = Vec::with_capacity(seq_1.len().min(seq_2.len()));

    for (i, (&a, &b)) in seq_1.iter().zip(seq_2.iter()).enumerate() {
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

pub fn common_string_sieve(hashsets: &mut Vec<HashSet<String>>) -> HashSet<String> {
    // Find the smallest set to minimize the search space.
    let smallest_hashset_index = hashsets
        .iter()
        .enumerate()
        .min_by_key(|(_, set)| set.len())
        .map(|(index, _)| index)
        .unwrap_or(0);
    let mut common_strings_hashset = hashsets.swap_remove(smallest_hashset_index);

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

pub fn calculate_shannon_entropy(frequencies: &HashMap<u8, usize>) -> f64 {
    // Calculate the total range of bytes.
    let total_bytes = frequencies.values().sum::<usize>() as f64;

    // Compute the entropy.
    let mut entropy = 0.0;
    for &count in frequencies.values() {
        let probability = count as f64 / total_bytes;
        entropy -= probability * probability.log2();
    }

    entropy
}

fn all_substrings_over_min_size(string: &str) -> Vec<&str> {
    let mut substrings = Vec::new();
    let len = string.len();
    for start in 0..len {
        for end in start + MIN_STRING_LENGTH..=len {
            substrings.push(&string[start..end]);
        }
    }

    substrings
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

pub fn generate_file_string_hashset(bytes: &[u8], reference: &HashSet<u8>) -> HashSet<String> {
    let mut string_map = HashSet::new();

    let mut push_string = false;
    let mut string_buffer = String::with_capacity(MAX_STRING_LENGTH);
    for (i, byte) in bytes.iter().enumerate() {
        // At the first non-valid string byte, we consider the string terminated.
        if !reference.contains(byte) {
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
