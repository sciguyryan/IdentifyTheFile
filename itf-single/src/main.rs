#![crate_name = "itf_single"]

use flate2::Compression;
use flate2::{read::DeflateDecoder, write::DeflateEncoder};
use itf_core::{file_point_calculator::FilePointCalculator, file_processor, pattern::Pattern};
use std::io::Cursor;
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

// This pattern block will be patched to contain the JSON data.
const PLACEHOLDER: u8 = 32;
const PATTERN_BLOCK_SIZE: usize = 8 * 1024;
const PATTERN: [u8; PATTERN_BLOCK_SIZE] = [PLACEHOLDER; PATTERN_BLOCK_SIZE];

fn main() {
    let pattern = include_bytes!("placeholder.bin");

    let args: Vec<String> = env::args().collect();

    if let Some(p) = get_pattern(&pattern[..]) {
        if args.len() < 2 {
            eprintln!("No file path to test was specified. Unable to continue.");
            return;
        }

        let target = &args[1];
        if !Path::new(target).is_file() {
            eprintln!("No valid file target was specified. Unable to continue.");
            return;
        }

        println!("{}", compute_match(&p, target));

        // We don't want to attempt to patch an already patched file.
        return;
    }

    if args.len() < 2 {
        eprintln!("No path to a JSON pattern file was specified. Unable to continue.");
        return;
    }

    build_patched_file(&args[1]);
}

fn build_patched_file(pattern_path: &str) {
    let data_bytes = compress_string(&read_json_file(pattern_path));
    if data_bytes.len() + 8 > PATTERN_BLOCK_SIZE {
        eprintln!("The pattern file is too large to be embedded. Maximum size = {PATTERN_BLOCK_SIZE}, pattern size = {}", data_bytes.len());
        return;
    }

    let mut final_bytes = data_bytes.len().to_le_bytes().to_vec();
    final_bytes.extend_from_slice(&data_bytes);
    final_bytes.resize(PATTERN_BLOCK_SIZE, PLACEHOLDER);

    // Clone the EXE.
    let new_file_path = if let Some(p) = copy_exe() {
        p
    } else {
        eprintln!("Unable to clone the existing executable file.");
        return;
    };

    // Find the index of the patch location in the cloned EXE.
    let index = if let Some(i) = find_patch_index(&new_file_path) {
        i
    } else {
        eprintln!("Unable to identify start of pattern block in executable file.");
        return;
    };
    println!("Pattern placeholder block found at index = {index}... Patching...");

    let patched = patch_file(&new_file_path, &final_bytes, index);
    if !patched {
        eprintln!("Unable to patch file.");
    }
}

fn compress_string(input: &str) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(input.as_bytes()).unwrap();
    encoder.finish().unwrap()
}

fn compute_match(pattern: &Pattern, target: &str) -> usize {
    let chunk = if let Ok(b) = file_processor::read_file_header_chunk(target) {
        b
    } else {
        return 0;
    };

    let mut frequencies = [0; 256];

    if pattern.data.scan_sequences || pattern.data.scan_composition {
        file_processor::count_byte_frequencies(&chunk, &mut frequencies);
    }

    FilePointCalculator::compute(pattern, &chunk, target, false)
}

fn copy_exe() -> Option<PathBuf> {
    let exe_path = if let Ok(str) = env::current_exe() {
        str
    } else {
        return None;
    };

    let file_name = exe_path.file_name()?;
    let new_file_name = file_name.to_string_lossy().replace(".exe", "-patched.exe");

    let mut new_file_path = exe_path.clone();
    new_file_path.set_file_name(new_file_name);

    if fs::copy(exe_path, &new_file_path).is_err() {
        return None;
    }

    Some(new_file_path)
}

fn decompress_string(compressed: &[u8]) -> String {
    let mut decoder = DeflateDecoder::new(Cursor::new(compressed));
    let mut decompressed = String::new();
    decoder.read_to_string(&mut decompressed).unwrap();
    decompressed
}

fn find_patch_index<P: AsRef<Path>>(path: P) -> Option<usize> {
    if !path.as_ref().exists() {
        return None;
    }

    let mut contents = Vec::new();
    if let Ok(f) = fs::File::open(path) {
        let mut buf_reader = BufReader::new(f);
        if buf_reader.read_to_end(&mut contents).is_err() {
            return None;
        }
    } else {
        return None;
    };

    find_sequence(&contents, &PATTERN)
}

fn find_sequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

// Since we are patching the EXE, we don't want any form of manipulation to this function as it could
// mess up the data retrieval later.
#[no_mangle]
fn get_pattern(raw_data: &[u8]) -> Option<Pattern> {
    // First, check to see if the data is entirely our placeholder.
    let mut is_unpatched = true;
    for b in raw_data {
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

    let data_length_bytes: [u8; 8] = raw_data[0..8].try_into().unwrap();
    let data_length = usize::from_le_bytes(data_length_bytes);
    let decompressed = decompress_string(&raw_data[8..data_length + 8]);
    if let Ok(p) = Pattern::from_simd_json_str(&decompressed) {
        pattern = Some(p);
    }

    // This should never happen since it would mean that the internal data had been incorrect adjusted.
    if pattern.is_none() {
        eprintln!("Pattern data has been included, but the data is corrupted.");
    }

    pattern
}

fn patch_file<P: AsRef<Path>>(path: P, bytes: &[u8], index: usize) -> bool {
    if !path.as_ref().is_file() {
        return false;
    }

    let mut file = if let Ok(f) = OpenOptions::new().write(true).open(path) {
        f
    } else {
        return false;
    };

    if file.seek(SeekFrom::Start(index as u64)).is_err() {
        return false;
    }

    file.write_all(bytes).is_ok()
}

fn read_json_file<P: AsRef<Path>>(path: P) -> String {
    if !path.as_ref().exists() {
        return String::new();
    }

    let mut string = String::new();
    let mut file = if let Ok(f) = File::open(path) {
        f
    } else {
        return String::new();
    };

    if file.read_to_string(&mut string).is_err() {
        return String::new();
    }

    string
}

#[allow(unused)]
fn write_pattern_placeholder<P: AsRef<Path>>(path: P) {
    let mut file = if let Ok(f) = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
    {
        f
    } else {
        return;
    };

    file.write_all(&PATTERN);
}
