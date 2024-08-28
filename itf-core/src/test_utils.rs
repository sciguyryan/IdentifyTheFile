use std::path::Path;

pub(crate) fn test_path_builder(test_type: &str, test_id: &str) -> String {
    let test_dir = std::fs::canonicalize(format!("./tests/{test_type}/{test_id}"))
        .expect("failed to find test directory");
    let resolved_dir = test_dir.to_string_lossy().to_string();

    if !Path::new(&resolved_dir).exists() {
        panic!("failed to find test directory at '{resolved_dir}'");
    }

    resolved_dir
}
