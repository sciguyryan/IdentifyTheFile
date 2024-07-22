use std::{fs::File, io::{BufReader, Read}};

use hashbrown::HashSet;

const FILE_CHUNK_SIZE: usize = 10 * 1024 * 1024; // 10 MB

const STRING_CHARS: &str = " abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-+_.$<>?=\"\'/";
const MIN_STRING_LENGTH: usize = 5;
const MAX_STRING_LENGTH: usize = 150;


fn main() {
    let file_path = "D:\\Documents\\Visual Studio 2010\\Projects\\ITF\\ITF\\bin\\Release\\ITF.vshost.exe.config";

    let file = File::open(file_path).expect("");
    let mut buf_reader = BufReader::new(file);
    let mut buffer = vec![0; FILE_CHUNK_SIZE];
    let bytes_read = buf_reader.read(&mut buffer).expect("");
    buffer.truncate(bytes_read);

    println!("{}", buffer.len());

    let ref_chars: HashSet<u8> = STRING_CHARS.chars().map(|c| c as u8).collect();

    let mut string_map = HashSet::new();

    let mut string_buffer = String::with_capacity(MAX_STRING_LENGTH);
    for byte in buffer {
        if !ref_chars.contains(&byte) {
            // Start a new string sequence.
            if string_buffer.len() >= MIN_STRING_LENGTH {
                string_map.insert(string_buffer);
                string_buffer = String::with_capacity(MAX_STRING_LENGTH);
            }

            // Skip the non-string character.
            continue;
        }

        // Push the string character into the buffer.
        string_buffer.push(byte as char);

        // Is the string large enough that we must force a termination?
        if string_buffer.len() == MAX_STRING_LENGTH {
            string_map.insert(string_buffer);
            string_buffer = String::with_capacity(MAX_STRING_LENGTH);
        }
    }

    println!("{string_map:?}");
}
