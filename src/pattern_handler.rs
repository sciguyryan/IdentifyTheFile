use std::{fs::File, io::Read, path::Path};

use crate::{
    pattern::{self, Pattern},
    utils,
};

#[derive(Default)]
pub struct PatternHandler {
    patterns: Vec<Pattern>,
}

impl PatternHandler {
    pub fn read<P: AsRef<Path>>(&mut self, path: P, target_pattern: &str) {
        let files = utils::list_files_of_type(path, "json");

        // Load every pattern, or the specific pattern if a target has been specified.
        for f in &files {
            if target_pattern.is_empty() || f.contains(target_pattern) {
                self.read_parse_pattern(f);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    fn read_parse_pattern(&mut self, path: &str) {
        let mut file = File::open(path).expect("failed to read file");

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .expect("failed to read file");

        if let Ok(p) = pattern::from_simd_json_str(&contents) {
            self.patterns.push(p);
        }
    }
}
