use rand::Rng;
use std::{collections::HashMap, path::Path};

use walkdir::WalkDir;

pub fn get_file_extension<P: AsRef<Path>>(path: P) -> String {
    // Get the file extension, if it exists.
    if let Some(extension) = path.as_ref().extension() {
        extension.to_string_lossy().to_uppercase()
    } else {
        "".to_string()
    }
}

pub fn list_files_of_type(source_directory: &str, target_extension: &str) -> Vec<String> {
    WalkDir::new(source_directory)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .filter(|e| get_file_extension(e.path()) == target_extension.to_uppercase())
        .filter_map(|e| e.path().to_str().map(|s| s.to_string()))
        .collect()
}

pub fn make_uuid() -> String {
    // Generate a random u128 value.
    let random: u128 = rand::thread_rng().gen();

    // Format the value as a hex string with zero padding to ensure it has 32 characters.
    let hex = format!("{:032x}", random);

    // Split the string into parts and insert dashes according to UUID format.
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

pub fn merge_hashmaps(maps: Vec<&HashMap<u8, usize>>) -> HashMap<u8, usize> {
    let mut result = HashMap::new();

    for map in maps {
        for (key, value) in map {
            *result.entry(*key).or_insert(0) += value;
        }
    }

    result
}

pub fn print_byte_sequence_matches(sequences: &HashMap<usize, Vec<u8>>) {
    let mut vec: Vec<(usize, Vec<u8>)> = sequences
        .iter()
        .map(|(index, m)| (*index, m.clone()))
        .collect();
    vec.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    println!("{vec:?}");
}

pub fn round_to_dp(value: f64, decimal_places: usize) -> f64 {
    let multiplier = 10f64.powi(decimal_places as i32);
    (value * multiplier).round() / multiplier
}
