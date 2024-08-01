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
