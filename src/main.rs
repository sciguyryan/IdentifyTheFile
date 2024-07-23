use std::{
    cmp::min,
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use hashbrown::HashSet;
use walkdir::WalkDir;

const FILE_CHUNK_SIZE: usize = 10 * 1024 * 1024; // 10 MB

const STRING_CHARS: &str =
    " abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-+_.$<>?=.,/";
const MIN_STRING_LENGTH: usize = 5;
const MAX_STRING_LENGTH: usize = 64;

const HEADER_FRONT_SIZE: usize = 1024;

fn main() {
    let file_dir = "D:\\GitHub\\IdentifyTheFile\\samples";
    let target_extension = "config";

    let ref_chars: HashSet<u8> = STRING_CHARS.chars().map(|c| c as u8).collect();

    let mut is_first = true;
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
    }

    if hashsets.is_empty() {
        println!("No strings were found!");
        return;
    }

    let reference_hashset = hashsets.remove(0);
    let mut common_elements = HashSet::new();

    // TODO - would this be more efficient using a trie data structure instead?

    // Iterate through the remaining sets
    for set in hashsets {
        for string in &set {
            if reference_hashset.contains(string) {
                // If the string is an exact match for one in the reference set.
                common_elements.insert(string.clone());
            } else {
                // Check if the string is a substring of any string in the reference set.
                for master_string in &reference_hashset {
                    if master_string.contains(string) {
                        common_elements.insert(string.clone());
                        break;
                    }
                }
            }
        }
    }

    println!(
        "{} elements = {reference_hashset:?}",
        reference_hashset.len()
    );
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

    let mut string_buffer = String::with_capacity(MAX_STRING_LENGTH);
    for byte in bytes {
        if !reference.contains(byte) {
            // Start a new string sequence.
            if string_buffer.len() >= MIN_STRING_LENGTH {
                string_map.insert(string_buffer.to_uppercase());
                string_buffer = String::with_capacity(MAX_STRING_LENGTH);
            }

            // Skip the non-string character.
            continue;
        }

        // Push the string character into the buffer.
        string_buffer.push(*byte as char);

        // Is the string large enough that we must force a termination?
        if string_buffer.len() == MAX_STRING_LENGTH {
            string_map.insert(string_buffer.to_uppercase());
            string_buffer = String::with_capacity(MAX_STRING_LENGTH);
        }
    }

    string_map
}
