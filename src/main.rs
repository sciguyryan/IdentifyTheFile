use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{self, BufReader, Read},
    time::Instant,
};

use rayon::prelude::*;
use walkdir::WalkDir;

const FILE_CHUNK_SIZE: usize = 10 * 1024 * 1024; // 10 MB

const STRING_CHARS: [u8; 74] =
    *b" $+,-./0123456789<=>?ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";
const MIN_STRING_LENGTH: usize = 5;
const MAX_STRING_LENGTH: usize = 128;

// TODO - use these!
const MIN_SEQUENCE_LENGTH: usize = 1;
const MAX_SEQUENCE_LENGTH: usize = 16;

const VERBOSE: bool = false;

fn main() {
    let splitter = "-".repeat(54);
    let half_splitter = "-".repeat(27);

    let file_dir = "D:\\GitHub\\IdentifyTheFile\\samples\\mkv";
    //let file_dir = "D:\\Downloads\\YouTube";
    let target_extension = "mkv";
    let files = list_files_of_type(file_dir, target_extension);

    let ref_chars: HashSet<u8> = STRING_CHARS.iter().copied().collect();

    let mut common_byte_sequences = HashMap::new();
    let mut first_byte_sequence_pass = true;

    let mut common_strings = Vec::new();

    let mut entropy = Vec::new();

    for file_path in &files {
        if VERBOSE {
            println!("Analyzing candidate file - {file_path}");
        }

        // If we made it here then we have a valid file.
        let chunk = read_file_header_chunk(file_path).expect("failed to read file");

        entropy.push((file_path, calculate_shannon_entropy(&chunk)));

        let new_hashset = generate_file_string_hashset(&chunk, &ref_chars);
        common_strings.push(new_hashset);

        // On the first pass, we simply set the matching sequence as the entire byte block.
        // This will get trimmed down and split into sections over future loop iterations.
        if first_byte_sequence_pass {
            common_byte_sequences.insert(0, chunk);
            first_byte_sequence_pass = false;
            continue;
        }

        refine_common_byte_sequences_v2(&chunk, &mut common_byte_sequences);
    }

    /*println!("{splitter}");
    let max_entropy = shannon_entropy
        .iter()
        .cloned()
        .fold(None, |max, x| match max {
            None => Some(x),
            Some(y) => Some(y.max(x)),
        })
        .unwrap();

    let min_entropy = shannon_entropy
        .iter()
        .cloned()
        .fold(None, |max, x| match max {
            None => Some(x),
            Some(y) => Some(y.min(x)),
        })
        .unwrap();*/

    let sum_entropy: f64 = entropy.iter().map(|(_, (b, _))| b).sum();
    let average_entropy = sum_entropy / (entropy.len() as f64);
    //let variation = ((max_entropy - min_entropy) / min_entropy) * 100f64;

    println!("{splitter}");
    println!("Valid sample files scanned: {}", files.len());
    println!("{splitter}");
    //println!("Maximum Entropy = {max_entropy}");
    //println!("Minimum Entropy = {min_entropy}");
    println!("Average Entropy = {average_entropy}");
    //println!("Entropy Variation = {variation}%");
    println!("{half_splitter}");
    println!("Entry deviations");
    /*let deviations: Vec<f64> = entropy
        .iter()
        .map(|(_, value)| ((value - average_entropy).abs() / average_entropy) * 100.0)
        .collect();
    println!("{deviations:?}");*/

    println!("{splitter}");
    println!("Matching positional byte sequences");
    print_byte_sequence_matches(&common_byte_sequences);

    if common_strings.is_empty() {
        println!("No common strings were found!");
    }

    println!("{splitter}");
    println!("Starting string sieve (v2a)...");
    let before_v2a = Instant::now();
    let common_strings_hashset_v2a = common_string_identification_v2a(&mut common_strings);
    println!("Elapsed time (v2a): {:.2?}", before_v2a.elapsed());
    println!("{}", common_strings_hashset_v2a.len());
    println!("{splitter}");
    let common_strings_hashset = common_strings_hashset_v2a;
    println!("Final common strings = {common_strings_hashset:?}");
    println!("{splitter}");

    println!("Testing common string matches...");
    test_matching_file_strings(
        file_dir,
        target_extension,
        &ref_chars,
        &common_strings_hashset,
    );

    println!("Testing common byte sequence matches...");
    test_matching_file_byte_sequences(file_dir, target_extension, &common_byte_sequences);
}

fn list_files_of_type(dir: &str, target_ext: &str) -> Vec<String> {
    let mut mkv_files = Vec::new();

    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
    {
        if let Some(ext) = entry.path().extension() {
            if ext == target_ext {
                if let Some(path_str) = entry.path().to_str() {
                    mkv_files.push(path_str.to_string());
                }
            }
        }
    }

    mkv_files
}

fn print_byte_sequence_matches(sequences: &HashMap<usize, Vec<u8>>) {
    let mut vec: Vec<(usize, Vec<u8>)> = sequences
        .iter()
        .map(|(index, m)| (*index, m.clone()))
        .collect();
    vec.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    println!("{vec:?}");
}

fn test_matching_file_byte_sequences(
    path: &str,
    target_extension: &str,
    sequences: &HashMap<usize, Vec<u8>>,
) {
    let mut all_success = true;
    let files = list_files_of_type(path, target_extension);
    for file_path in &files {
        // No sequences, we can skip the scan completely.
        if sequences.is_empty() {
            break;
        }

        let chunk = read_file_header_chunk(file_path).expect("failed to read file");

        if VERBOSE {
            println!("--------------------------------------");
        }

        let mut matches = 0;
        for (start, sequence) in sequences {
            let end = *start + sequence.len();
            if end > chunk.len() {
                continue;
            }

            if sequence == &chunk[*start..end] {
                matches += 1;
            } else {
                if VERBOSE {
                    println!("start = {start}");
                    println!("{sequence:?} != {:?}", &chunk[*start..end]);
                }
            }
        }

        if VERBOSE {
            println!("--------------------------------------");
            println!("{file_path}");
            println!("{} of {}", matches, sequences.len());
        }

        if matches == sequences.len() {
            //println!("\x1b[92mSuccessful byte sequence matching!\x1b[0m");
        } else {
            //println!("\x1b[91mFailed byte sequence matching!\x1b[0m");
            all_success = false;
        }

        all_success &= matches == sequences.len();
    }

    if all_success {
        println!("\x1b[92mSuccessfully matched all applicable byte sequences!\x1b[0m");
    } else {
        println!("\x1b[91mFailed to match one or more byte sequences!\x1b[0m");
    }
}

fn test_matching_file_strings(
    path: &str,
    target_extension: &str,
    ref_chars: &HashSet<u8>,
    common_strings: &HashSet<String>,
) {
    let mut all_success = true;

    let files = list_files_of_type(path, target_extension);
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

        if VERBOSE {
            println!("--------------------------------------");
            println!("{file_path}");
            println!("{} of {}", matches, common_strings.len());
        }

        if matches == common_strings.len() {
            //println!("\x1b[92mSuccessful string matching!\x1b[0m");
        } else {
            //println!("\x1b[91mFailed string matching!\x1b[0m");
            all_success = false;
        }
    }

    if all_success {
        println!("\x1b[92mSuccessfully matched all common strings!\x1b[0m");
    } else {
        println!("\x1b[91mFailed to match one or more common strings!\x1b[0m");
    }
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

fn refine_common_byte_sequences_v2(
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

fn common_string_identification_v2a(hashsets: &mut Vec<HashSet<String>>) -> HashSet<String> {
    // Find the smallest set to minimize the search space.
    let smallest_hashset_index = hashsets
        .iter()
        .enumerate()
        .min_by_key(|(_, set)| set.len())
        .map(|(index, _)| index)
        .unwrap_or(0);
    let mut common_strings_hashset = hashsets.swap_remove(smallest_hashset_index);

    //println!("Original common strings hashset = {common_strings_hashset:?}");

    //let total = hashsets.len();
    //let mut i = 0;

    // Find the common strings between all of the hashsets.
    while !hashsets.is_empty() {
        //println!("String processing iteration {} of {}", i, total);
        //i += 1;

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

fn calculate_shannon_entropy(data: &[u8]) -> (f64, HashMap<u8, u8>) {
    // Count the frequency of each bute in the input data.
    let mut frequencies = HashMap::new();

    // Count and classify the bytes.
    for b in data {
        *frequencies.entry(*b).or_insert(0) += 1;
    }

    // Calculate the total range of bytes.
    let total_bytes = data.len() as f64;

    // Compute the entropy
    let mut entropy = 0.0;
    for &count in frequencies.values() {
        let probability = count as f64 / total_bytes;
        entropy -= probability * probability.log2();
    }

    (entropy, frequencies)
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

fn read_file_header_chunk(file_path: &str) -> io::Result<Vec<u8>> {
    let file = File::open(file_path)?;
    let filesize = file.metadata()?.len() as usize;
    let read_size = filesize.min(FILE_CHUNK_SIZE);
    let mut buf_reader = BufReader::new(file);
    let mut buffer = vec![0; read_size];
    buf_reader.read_exact(&mut buffer)?;

    Ok(buffer)
}

fn generate_file_string_hashset(bytes: &[u8], reference: &HashSet<u8>) -> HashSet<String> {
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
