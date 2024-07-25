use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{self, BufReader, Read},
    path::Path,
    time::Instant,
};

use rayon::prelude::*;
use walkdir::{DirEntry, WalkDir};

const FILE_CHUNK_SIZE: usize = 10 * 1024 * 1024; // 10 MB

const STRING_CHARS: [u8; 74] =
    *b" $+,-./0123456789<=>?ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";
const MIN_STRING_LENGTH: usize = 5;
const MAX_STRING_LENGTH: usize = 128;

const VERBOSE: bool = false;

fn main() {
    let file_dir = "D:\\GitHub\\IdentifyTheFile\\samples";
    let target_extension = "mkv";

    let ref_chars: HashSet<u8> = STRING_CHARS.iter().copied().collect();

    let mut initial_file_temp: Option<Vec<u8>> = None;
    let mut common_byte_sequences = HashMap::new();
    let mut common_strings = Vec::new();
    let mut valid_sample_files = 0;

    for entry in WalkDir::new(file_dir) {
        let entry = entry.unwrap();
        if !entry.file_type().is_file() {
            continue;
        }

        if !path_has_correct_extension(&entry, target_extension) {
            if VERBOSE {
                println!("Skipping file - {}", entry.path().to_string_lossy());
            }
            continue;
        }

        if VERBOSE {
            println!("Candidate file - {}", entry.path().to_string_lossy());
        }
        valid_sample_files += 1;

        // If we made it here then we have a valid file.
        let chunk = read_file_header_chunk(entry.path()).expect("failed to read file");

        let new_hashset = generate_file_string_hashset(&chunk, &ref_chars);
        common_strings.push(new_hashset);

        // We want to avoid holding the header blocks of every file we scan,
        // because that could have significant memory implications.
        // Instead, we store the first file's header and then build the
        // initial common sequences list from the first and second file headers.
        // Files that follow those will be used to refine the sequences already
        // identified.
        if common_byte_sequences.is_empty() {
            if let Some(file_1) = &initial_file_temp {
                // Run the initial byte sequence scan.
                common_byte_sequences = initial_common_byte_sequences_v1(file_1, &chunk);

                // Clear the superfluous from memory.
                initial_file_temp = None;
            } else {
                initial_file_temp = Some(chunk);
            }
        } else {
            refine_common_byte_sequences_v2(&chunk, &mut common_byte_sequences);
        }
    }

    println!("-------------------------------------------------------");
    println!("Valid sample files scanned {valid_sample_files}");
    println!("-------------------------------------------------------");
    println!("Matching positional byte sequences");
    print_byte_sequence_matches(&common_byte_sequences);

    if common_strings.is_empty() {
        println!("No common strings were found!");
    }

    println!("-------------------------------------------------------");
    println!("Starting string sieve (v2a)...");
    let before_v2a = Instant::now();
    let common_strings_hashset_v2a = common_string_identification_v2a(&mut common_strings);
    println!("Elapsed time (v2a): {:.2?}", before_v2a.elapsed());
    println!("{}", common_strings_hashset_v2a.len());
    println!("------------------------------------------");
    let common_strings_hashset = common_strings_hashset_v2a;
    println!("Final common strings = {common_strings_hashset:?}");
    println!("-------------------------------------------------------");

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

fn path_has_correct_extension(entry: &DirEntry, ext: &str) -> bool {
    match entry.path().extension() {
        Some(str) => str == ext,
        None => true,
    }
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
    for entry in WalkDir::new(path) {
        let entry = entry.unwrap();
        if !entry.file_type().is_file() {
            continue;
        }

        if !path_has_correct_extension(&entry, target_extension) {
            continue;
        }

        let chunk = read_file_header_chunk(entry.path()).expect("failed to read file");

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

        if VERBOSE {
            println!("--------------------------------------");
            println!("{}", entry.path().to_string_lossy());
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
    for entry in WalkDir::new(path) {
        let entry = entry.unwrap();
        if !entry.file_type().is_file() {
            continue;
        }

        if !path_has_correct_extension(&entry, target_extension) {
            continue;
        }

        let chunk = read_file_header_chunk(entry.path()).expect("failed to read file");
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
            println!("{}", entry.path().to_string_lossy());
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

fn refine_common_byte_sequences_v2(
    file_bytes: &[u8],
    common_byte_sequences: &mut HashMap<usize, Vec<u8>>,
) {
    let mut refined_sequence = HashMap::with_capacity(common_byte_sequences.len());

    for (index, sequence) in common_byte_sequences.iter() {
        // If the final index would fall outside the bounds of the
        // chunk then read to the end of the chunk instead.
        let segment_read_length = *index + sequence.len().min(file_bytes.len());

        // We can be certain that this will always fall within bounds.
        let new_segment = &file_bytes[*index..segment_read_length];

        // Fast path exit for a complete match.
        if sequence == new_segment {
            refined_sequence.insert(*index, sequence.clone());
            continue;
        }

        // Check to see if we can find a valid sub-match. If so, we'll retain that instead.
        let mut new_length = segment_read_length;
        for i in 0..segment_read_length {
            if sequence[i] != new_segment[i] {
                new_length = i;
                break;
            }
        }

        if new_length > 0 {
            refined_sequence.insert(*index, sequence[..new_length].to_vec());
        }
    }

    *common_byte_sequences = refined_sequence;
}

fn initial_common_byte_sequences_v1(
    file_1_bytes: &[u8],
    file_2_bytes: &[u8],
) -> HashMap<usize, Vec<u8>> {
    let mut common_byte_sequences = HashMap::new();

    let mut sequence_start = 0;
    let mut sequence_end;
    let mut in_sequence = false;
    for (i, b) in file_1_bytes.iter().enumerate() {
        if i < file_2_bytes.len() && file_2_bytes[i] == *b {
            // Indicate the start of the matching sequence, if we aren't already
            // within a sequence.
            if !in_sequence {
                sequence_start = i;
                in_sequence = true;
            }

            continue;
        }

        // We have reached the end of the matching sequence.
        sequence_end = i;
        in_sequence = false;

        let sequence = file_1_bytes[sequence_start..sequence_end].to_vec();
        common_byte_sequences.insert(sequence_start, sequence);
    }

    common_byte_sequences
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

fn read_file_header_chunk(file_path: &Path) -> io::Result<Vec<u8>> {
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
