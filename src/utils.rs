use std::collections::HashMap;

use walkdir::WalkDir;

pub fn list_files_of_type(source_directory: &str, target_extension: &str) -> Vec<String> {
    let mut mkv_files = Vec::new();

    for entry in WalkDir::new(source_directory)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
    {
        if let Some(ext) = entry.path().extension() {
            if ext != target_extension {
                continue;
            }

            if let Some(path_str) = entry.path().to_str() {
                mkv_files.push(path_str.to_string());
            }
        }
    }

    mkv_files
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
