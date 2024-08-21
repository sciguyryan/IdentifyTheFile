#![crate_name = "itf_single"]

use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use itf_core::{file_point_calculator::FilePointCalculator, file_processor, pattern::Pattern};

// This pattern block will be patched to contain the JSON data.
const PLACEHOLDER: u8 = 32;
const PATTERN_BLOCK_SIZE: usize = 8 * 1024;
static PATTERN: [u8; PATTERN_BLOCK_SIZE] = [PLACEHOLDER; PATTERN_BLOCK_SIZE];

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
    println!("Block found at index = {index}... Patching...");

    // TODO - I could compress this file to save space, but I'm not sure if it's
    // TODO - worth the effort at the moment.
    let bytes = read_json_file(pattern_path);

    let patched = patch_file(&new_file_path, &bytes, index);
    if !patched {
        eprintln!("Unable to patch file.");
    }
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
fn get_pattern(raw_pattern: &[u8]) -> Option<Pattern> {
    // First, check to see if the data is entirely our placeholder.
    let mut is_unpatched = true;
    for b in raw_pattern {
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

    if let Ok(str) = String::from_utf8(raw_pattern.to_vec()) {
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

fn read_json_file<P: AsRef<Path>>(path: P) -> Vec<u8> {
    if !path.as_ref().exists() {
        return vec![];
    }

    let mut buffer = Vec::new();
    let mut file = if let Ok(f) = File::open(path) {
        f
    } else {
        return vec![];
    };

    if file.read_to_end(&mut buffer).is_err() {
        return vec![];
    }

    if buffer.len() > PATTERN_BLOCK_SIZE {
        eprintln!("The pattern file is too large to be embedded. Maximum size = {PATTERN_BLOCK_SIZE}, pattern size = {}", buffer.len());
        return vec![];
    }

    buffer.resize(PATTERN_BLOCK_SIZE, PLACEHOLDER);
    buffer
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
