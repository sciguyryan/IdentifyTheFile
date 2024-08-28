use rand::Rng;
use std::path::Path;
use walkdir::WalkDir;

/// The characters that may not appear in a NTFS file name.
const NTFS_INVALID_CHARS: &str = "\\/:*?\"<>|";
/// The characters that may not appear in a UNIX file name.
const UNIX_INVALID_CHARS: &str = "/";

/// Calculate the Shannon entropy for a block of bytes.
///
/// # Arguments
///
/// * `frequencies` - An array containing the byte frequencies.
///
/// # Returns
///
/// The Shannon entropy, expressed as a f32 value between 0 and 8.
#[inline(always)]
pub fn calculate_shannon_entropy(frequencies: &[usize; 256]) -> f32 {
    // Calculate the total number of bytes in our sample.
    let total_bytes = frequencies.iter().sum::<usize>() as f32;

    // Compute the entropy.
    let mut entropy = 0.0;
    for &count in frequencies {
        if count == 0 {
            continue;
        }

        let probability = count as f32 / total_bytes;
        entropy -= probability * probability.log2();
    }

    entropy
}

/// Check that a directory exist.
pub fn directory_exists<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().is_dir()
}

/// Check that a file exists.
pub fn file_exists<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().is_file()
}

/// Get the extension of a file.
///
/// # Arguments
///
/// * `path` - The path to the file.
///
/// # Returns
///
/// A string giving the file extension if one was extracted, an empty string will be returned otherwise.
pub fn get_file_extension<P: AsRef<Path>>(path: P) -> String {
    if let Some(extension) = path.as_ref().extension() {
        extension.to_string_lossy().to_uppercase()
    } else {
        "".to_string()
    }
}

/// List all of the files within a source directory that have a specific file extension.
///
/// # Arguments
///
/// * `source_directory` - The source directory containing all of the files.
/// * `target_extension` - The file extension that the files must possess.
///
/// # Returns
///
/// A vector of strings giving the paths to all of the matching files.
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

/// Generate a random UUID.
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

/// Round a f32 value to a certain number of decimal places.
///
/// # Arguments
///
/// * `value` - The value to be rounded.
/// * `decimal_places` - The number of digits to be retained after rounding.
pub fn round_to_dp(value: f32, decimal_places: usize) -> f32 {
    let multiplier = 10f32.powi(decimal_places as i32);
    (value * multiplier).round() / multiplier
}

/// Sanitize a file name by removing invalid characters from the file name string.
///
/// # Arguments
///
/// * `file_name` - The file name to be sanitized.
pub(crate) fn sanitize_file_name(file_name: &str) -> String {
    file_name
        .chars()
        .filter(|&c| !NTFS_INVALID_CHARS.contains(c) && !UNIX_INVALID_CHARS.contains(c))
        .collect()
}
