use std::{
    cmp::min,
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use hashbrown::{HashMap, HashSet};
use walkdir::WalkDir;

const FILE_CHUNK_SIZE: usize = 10 * 1024 * 1024; // 10 MB

const STRING_CHARS: [u8; 75] =
    *b" .,/abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-+_.$<>?=";
const MIN_STRING_LENGTH: usize = 5;
const MAX_STRING_LENGTH: usize = 64;

const HEADER_FRONT_SIZE: usize = 1024;

fn main() {
    let file_dir = "D:\\GitHub\\IdentifyTheFile\\samples";
    let target_extension = "config";

    let ref_chars: HashSet<u8> = STRING_CHARS.iter().copied().collect();

    let mut hashsets: Vec<HashSet<String>> = Vec::new();

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

        // If we made it here then we have a valid file.
        let chunk = read_file_header_chunk(entry.path());
        let new_hashset = generate_file_hashset(&chunk, &ref_chars);

        hashsets.push(new_hashset);

        println!("-------------------------------------------------------");
        println!("{hashsets:?}");
    }

    println!("-------------------------------------------------------");

    if hashsets.is_empty() {
        println!("No strings were found!");
        return;
    }

    // TODO - remove the smallest set, less searching.
    let mut reference_hashset = hashsets.remove(0);

    // Find the intersection of all sets.
    for set in &hashsets {
        let mut temp_set = reference_hashset.clone();

        for ref_string in &reference_hashset {
            //println!("ref_string = {ref_string}");

            if set.contains(ref_string) {
                //println!("exact match; skipping");
                // Nothing to do here, the reference set already contains the item.
                continue;
            }

            let mut matches = HashSet::new();
            for string in set {
                // Are we able to match a substring between the reference and new string?
                if let Some(s) = largest_common_substring(string, ref_string) {
                    // Don't add strings that are smaller than our minimum to reduce overhead.
                    if s.len() > MIN_STRING_LENGTH {
                        matches.insert(s);
                    }
                }
            }

            // Select the largest substring match.
            let largest_match = matches.iter().max_by_key(|s| s.len());
            if let Some(m) = largest_match {
                temp_set.remove(ref_string);
                temp_set.insert(m.clone());
            }
        }

        reference_hashset = temp_set;
    }

    println!("-------------------------------------------------------");
    println!("reference_hashset = {reference_hashset:?}");

    for entry in WalkDir::new(file_dir) {
        let entry = entry.unwrap();
        if !entry.file_type().is_file() {
            continue;
        }

        let chunk = read_file_header_chunk(entry.path());
        let strings = generate_file_hashset(&chunk, &ref_chars);

        let mut matches = 0;
        for el in &reference_hashset {
            for str in &strings {
                if str.contains(el) {
                    matches += 1;
                }
            }
        }

        println!("--------------------------------------");
        println!("{}", entry.path().to_string_lossy());
        println!("{} of {}", matches, reference_hashset.len());
    }
}

fn all_substrings(string: &str) -> HashSet<&str> {
    let mut substrings = HashSet::new();
    for start in 0..string.len() {
        for end in start + 1..=string.len() {
            substrings.insert(&string[start..end]);
        }
    }

    substrings
}

fn largest_common_substring(str_1: &str, str_2: &str) -> Option<String> {
    let substrings_str1 = all_substrings(str_1);
    let mut largest: Option<&str> = None;

    for substr in substrings_str1 {
        if str_2.contains(substr) {
            match largest {
                Some(s) => {
                    if substr.len() > s.len() {
                        largest = Some(substr);
                    }
                }
                None => largest = Some(substr),
            }
        }
    }

    largest.map(|s| s.to_string())
}

fn read_file_header_chunk(file_path: &Path) -> Vec<u8> {
    let file = File::open(file_path).expect("");
    let filesize = file.metadata().unwrap().len() as usize;
    let read_size = min(filesize, FILE_CHUNK_SIZE);
    let mut buf_reader = BufReader::new(file);
    let mut buffer = vec![0; read_size];
    buf_reader.read_exact(&mut buffer).expect("");

    buffer
}

fn generate_file_hashset(bytes: &[u8], reference: &HashSet<u8>) -> HashSet<String> {
    let mut string_map = HashSet::new();

    let mut push_next = false;
    let mut string_buffer = String::with_capacity(MAX_STRING_LENGTH);
    for byte in bytes {
        if push_next {
            if string_buffer.len() >= MIN_STRING_LENGTH {
                string_map.insert(string_buffer.to_uppercase());
            }

            string_buffer = String::with_capacity(MAX_STRING_LENGTH);
            push_next = false;
        }

        if !reference.contains(byte) {
            push_next = true;
            continue;
        }

        // Push the character onto the buffer.
        string_buffer.push(*byte as char);

        if string_buffer.len() == MAX_STRING_LENGTH {
            push_next = true;
        }
    }

    string_map
}
