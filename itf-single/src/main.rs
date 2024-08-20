#![crate_name = "itf_single"]

use std::{
    env,
    fs::{self, OpenOptions},
    io::{BufReader, Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

use itf_core::pattern::Pattern;

// This pattern block will be patched to contain the JSON data.
const PLACEHOLDER: u8 = 32;
const PATTERN_BLOCK_SIZE: usize = 16 * 1024;
static PATTERN: [u8; PATTERN_BLOCK_SIZE] = [PLACEHOLDER; PATTERN_BLOCK_SIZE];

fn find_sequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len()) // Create an iterator over all windows of the same length as `needle`
        .position(|window| window == needle) // Find the first window that matches `needle`
}

fn copy_exe() -> Option<PathBuf> {
    let exe_path = if let Ok(str) = env::current_exe() {
        str
    } else {
        return None;
    };

    let file_name = exe_path.file_name()?;
    let new_file_name = file_name.to_string_lossy().replace(".exe", ".single.exe");

    let mut new_file_path = exe_path.clone();
    new_file_path.set_file_name(new_file_name);

    if fs::copy(exe_path, &new_file_path).is_err() {
        return None;
    }

    Some(new_file_path)
}

fn main() {
    /*if let Some(p) = get_pattern() {
        println!("{:?}", p.type_data.uuid);
        println!("Run the pattern matcher.");
    } else {
        println!("Show the options to build a pattern matcher.");
    }*/

    // Clone the EXE.
    let new_file_path = if let Some(p) = copy_exe() {
        p
    } else {
        eprintln!("Unable to clone the existing executable file.");
        return;
    };

    // Path the EXE.
    let index: Option<usize>;
    {
        let mut contents = Vec::new();
        let file = fs::File::open(&new_file_path).unwrap();
        let mut buf_reader = BufReader::new(file);
        buf_reader.read_to_end(&mut contents).unwrap();

        // Attempt to find the placeholder block within the file.
        index = find_sequence(&contents, &PATTERN);
    }

    if index.is_none() {
        eprintln!("Unable to identify start of pattern block in executable file.");
        return;
    }
    let index = index.unwrap();
    println!("Block found at index = {index}");

    // TODO - modify the JSON file to strip down some of the bits we don't really need.
    // TODO - this will save space and mean we can fit larger patterns within the file.
    let json =
        fs::read_to_string("D:\\GitHub\\IdentifyTheFile\\target\\release\\matroska.json").unwrap();
    let mut bytes = json.as_bytes().to_vec();
    bytes.resize(PATTERN_BLOCK_SIZE, PLACEHOLDER);

    let mut file = if let Ok(f) = OpenOptions::new().write(true).open(&new_file_path) {
        f
    } else {
        eprintln!("Unable to open new executable file.");
        return;
    };

    if file.seek(SeekFrom::Start(index as u64)).is_err() {
        eprintln!("Unable to seek to file within copy executable file.");
        return;
    } else {
        file.write_all(&bytes).expect("failed to write data");
    }

    {
        let mut contents2 = Vec::new();
        let file = fs::File::open(&new_file_path).unwrap();
        let mut buf_reader = BufReader::new(file);
        buf_reader.read_to_end(&mut contents2).unwrap();

        println!("{:?}", &contents2[index..index + 16]);
    }
}

fn get_pattern() -> Option<Pattern> {
    // First, check to see if the data is entirely our placeholder.
    let mut is_unpatched = true;
    for b in &PATTERN {
        if *b != PLACEHOLDER {
            is_unpatched = false;
            break;
        }
    }

    if is_unpatched {
        return None;
    }

    // Attempt to build a pattern from the presumed JSON data.
    let mut pattern: Option<Pattern> = None;

    if let Ok(str) = String::from_utf8(PATTERN.to_vec()) {
        if let Ok(p) = Pattern::from_simd_json_str(&str) {
            pattern = Some(p);
        }
    }

    // This should never happen since it would mean that the internal data had been incorrect adjusted.
    if pattern.is_none() {
        eprintln!("Pattern data has been included, but the data is corrupted.");
    }

    pattern
}
