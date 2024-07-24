use core::str;
use std::{
    cmp::min,
    fs::File,
    io::{self, BufReader, Read},
    path::Path,
    time::Instant,
};

use hashbrown::HashSet;
use walkdir::WalkDir;

const FILE_CHUNK_SIZE: usize = 10 * 1024 * 1024; // 10 MB

const STRING_CHARS: [u8; 74] =
    *b" $+,-./0123456789<=>?ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";
const MIN_STRING_LENGTH: usize = 5;
const MAX_STRING_LENGTH: usize = 128;

fn main() {
    let file_dir = "D:\\GitHub\\IdentifyTheFile\\samples";
    let target_extension = "mkv";

    let ref_chars: HashSet<u8> = STRING_CHARS.iter().copied().collect();

    let mut hashsets = Vec::new();
    for entry in WalkDir::new(file_dir) {
        let entry = entry.unwrap();
        if !entry.file_type().is_file() {
            continue;
        }

        let ext = entry.path().extension();
        let skip = match ext {
            Some(str) => str != target_extension,
            None => true,
        };

        if skip {
            println!("Skipping file - {}", entry.path().to_string_lossy());
            continue;
        }

        println!("Candidate file - {}", entry.path().to_string_lossy());

        // If we made it here then we have a valid file.
        let chunk = read_file_header_chunk(entry.path()).expect("failed to read file");
        let new_hashset = generate_file_string_hashset(&chunk, &ref_chars);

        hashsets.push(new_hashset);
    }

    let before = Instant::now();

    if hashsets.is_empty() {
        println!("No strings were found!");
        return;
    }

    println!("Starting string analysis...");

    // Find the smallest set to minimize the search space.
    let smallest_hashset_index = hashsets
        .iter()
        .enumerate()
        .min_by_key(|(_, set)| set.len())
        .map(|(index, _)| index)
        .unwrap_or(0);
    let mut common_strings_hashset = hashsets.swap_remove(smallest_hashset_index);

    println!("original common strings hashset = {common_strings_hashset:?}");

    let total = hashsets.len();
    let mut i = 0;

    // Find the common strings between all of the hashsets.
    while !hashsets.is_empty() {
        println!("String processing iteration {i} of {total}");
        i += 1;

        // Take the bottom hashset, allowing memory to be freed as we go.
        let set = hashsets.remove(0);

        // Iterate over each common string. In each iteration we prune
        // strings that didn't match, making the process more efficient as
        // with each passing cycle.
        let mut temp_set = HashSet::new();
        for common_string in &common_strings_hashset {
            let mut largest_match = "";

            for string in &set {
                // If the strings are identical, we don't need to check for substring matches
                // and we don't need to add the item to the hashmap since it's already there.
                if common_string == string {
                    continue;
                }

                // Are we able to find a substring match between the old and new strings?
                if let Some(s) = largest_common_substring(string, common_string) {
                    // Check if this is the largest match we've seen so far.
                    if s.len() > largest_match.len() {
                        largest_match = s;
                    }
                }
            }

            // Only insert if a match was found, otherwise we can retain
            // items that aren't matches at all.
            if !largest_match.is_empty() {
                temp_set.insert(largest_match.to_string());
            }
        }

        // Update the reference hashmap to reduce the search space in future
        // loop iterations.
        common_strings_hashset = temp_set;
    }

    println!("Elapsed time: {:.2?}", before.elapsed());

    println!("-------------------------------------------------------");
    println!("reference_hashset = {common_strings_hashset:?}");

    for entry in WalkDir::new(file_dir) {
        let entry = entry.unwrap();
        if !entry.file_type().is_file() {
            continue;
        }

        let ext = entry.path().extension();
        let skip = match ext {
            Some(str) => str != target_extension,
            None => true,
        };

        if skip {
            continue;
        }

        let chunk = read_file_header_chunk(entry.path()).expect("failed to read file");
        let strings = generate_file_string_hashset(&chunk, &ref_chars);

        let mut matches = 0;
        for el in &common_strings_hashset {
            for str in &strings {
                if str.contains(el) {
                    matches += 1;
                    break;
                }
            }
        }

        println!("--------------------------------------");
        println!("{}", entry.path().to_string_lossy());
        println!("{} of {}", matches, common_strings_hashset.len());
    }
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
    let read_size = min(filesize, FILE_CHUNK_SIZE);
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
