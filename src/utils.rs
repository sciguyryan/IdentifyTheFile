use rand::Rng;
use std::path::Path;
use walkdir::WalkDir;

#[inline(always)]
pub fn calculate_shannon_entropy(frequencies: &[usize; 256]) -> f64 {
    // Calculate the total number of bytes in our sample.
    let total_bytes = frequencies.iter().sum::<usize>() as f64;

    // Compute the entropy.
    let mut entropy = 0.0;
    for &count in frequencies {
        if count == 0 {
            continue;
        }

        let probability = count as f64 / total_bytes;
        entropy -= probability * probability.log2();
    }

    entropy
}

pub fn directory_exists<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().is_dir()
}

pub fn file_exists<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().is_file()
}

pub fn get_file_extension<P: AsRef<Path>>(path: P) -> String {
    if let Some(extension) = path.as_ref().extension() {
        extension.to_string_lossy().to_uppercase()
    } else {
        "".to_string()
    }
}

pub fn list_files_of_type<P: AsRef<Path>>(
    source_directory: P,
    target_extension: &str,
) -> Vec<String> {
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
    let hex = format!("{random:032x}");

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

#[allow(unused)]
pub fn print_byte_sequence_matches(sequences: &[(usize, Vec<u8>)]) {
    let mut vec = sequences.to_vec().clone();
    vec.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    println!("{vec:?}");
}

pub fn round_to_dp(value: f64, decimal_places: usize) -> f64 {
    let multiplier = 10f64.powi(decimal_places as i32);
    (value * multiplier).round() / multiplier
}

const NTFS_INVALID_CHARS: &str = "\\/:*?\"<>|";
const UNIX_INVALID_CHARS: &str = "/";

pub fn remove_invalid_file_name(file_name: &str) -> String {
    file_name
        .chars()
        .filter(|&c| !NTFS_INVALID_CHARS.contains(c) && !UNIX_INVALID_CHARS.contains(c))
        .collect()
}
