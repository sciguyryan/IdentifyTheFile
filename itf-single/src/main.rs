#![crate_name = "itf_single"]

// This pattern block will be patched to contain the JSON data.
const PLACEHOLDER: u8 = 35;
const PATTERN_BLOCK_SIZE: usize = 64 * 1024;
static PATTERN: [u8; PATTERN_BLOCK_SIZE] = [PLACEHOLDER; PATTERN_BLOCK_SIZE];

fn main() {
    if let Ok(str) = String::from_utf8(PATTERN.to_vec()) {
        println!("{}", &str[0..16]);
    } else {
        eprintln!("The included pattern data was corrupted.");
    }

    //from_simd_json_str
}
