#![crate_name = "itf_core"]

pub mod file_point_calculator;
pub mod file_processor;
pub mod pattern;
pub mod pattern_handler;
#[cfg(test)]
mod test_utils;
pub mod utils;

#[cfg(test)]
mod tests_pattern {
    use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
    use std::{env, fs, path::PathBuf};

    use crate::{
        file_point_calculator::FilePointCalculator, file_processor, pattern::Pattern,
        pattern_handler::PatternHandler, test_utils, utils,
    };

    struct PatternMatch<'a> {
        pub uuid: &'a str,
        pub percentage: f32,
    }

    impl<'a> PatternMatch<'a> {
        pub fn new(uuid: &'a str, points: usize, max_points: usize) -> Self {
            Self {
                uuid,
                percentage: utils::round_to_dp(points as f32 / max_points as f32 * 100.0, 1),
            }
        }
    }

    /// Automatically delete a file after a test has been completed.
    /// Use with care! Files go bye-bye!
    struct FileDropper {
        path: PathBuf,
    }

    impl Drop for FileDropper {
        fn drop(&mut self) {
            _ = fs::remove_file(&self.path);
        }
    }

    struct TestEntry {
        #[allow(unused)]
        path: FileDropper,
        pub new_pattern: Pattern,
        pub pattern_handler: PatternHandler,
        pub test_dir: String,
    }

    impl TestEntry {
        pub fn new(test_id: usize) -> Self {
            let id = test_id.to_string();

            // Build a pattern from the sample files.
            let (path, pattern) = Self::build_test("matching", &id);

            let test_dir = test_utils::test_path_builder("matching", &id);

            // Read all of the test patterns.
            let pattern_handler = built_pattern_handler(&test_dir, "");
            assert!(
                !pattern_handler.is_empty(),
                "pattern handler didn't load the pattern files"
            );

            Self {
                path,
                new_pattern: pattern,
                pattern_handler,
                test_dir,
            }
        }

        fn build_test(test_type: &str, test_id: &str) -> (FileDropper, Pattern) {
            let test_dir = test_utils::test_path_builder(test_type, test_id);

            let mut pattern = Pattern::new("valid", "test", vec!["test".to_string()], vec![]);
            pattern.build_patterns_from_data(&test_dir, "test", true, true, true);

            // Write the pattern file.
            let path = pattern.write(&test_dir).expect("failed to write test file");

            (FileDropper { path }, pattern)
        }

        fn get_test_file(&self, id: usize) -> String {
            self.get_test_file_of_type(id, "test")
        }

        fn get_test_file_of_type(&self, id: usize, extension: &str) -> String {
            let files = utils::list_files_of_type(&self.test_dir, extension);
            files.get(id).expect("failed to find test file").to_owned()
        }
    }

    #[test]
    fn test_matching_1() {
        let test = TestEntry::new(1);

        // There should only ever be a single result.
        let results = match_patterns(&test.pattern_handler, &test.get_test_file(0));
        assert_eq!(results.len(), 1);

        let first_result = results.first().unwrap();
        assert_eq!(first_result.uuid, test.new_pattern.type_data.uuid);
        assert_eq!(first_result.percentage, 100.0);
    }

    #[test]
    fn test_matching_2() {
        // The directory contains another pattern file that is not a match due to
        // a byte sequence mismatch.
        let test = TestEntry::new(2);

        // There should only ever be a single result.
        let results = match_patterns(&test.pattern_handler, &test.get_test_file(0));
        assert_eq!(results.len(), 1);

        let top_match = results.first().unwrap();
        assert_eq!(top_match.uuid, test.new_pattern.type_data.uuid);
        assert_eq!(top_match.percentage, 100.0);
    }

    #[test]
    fn test_matching_3() {
        // The directory contains another pattern file that is not a match due to
        // a byte sequence mismatch.
        let test = TestEntry::new(3);

        // There should be two results, the created pattern being the top one.
        // This is because the existing pattern is a "less perfect" match.
        let results = match_patterns(&test.pattern_handler, &test.get_test_file(0));
        assert_eq!(results.len(), 2);

        let top_match = results.first().unwrap();
        assert_eq!(top_match.uuid, test.new_pattern.type_data.uuid);
        assert_eq!(top_match.percentage, 100.0);
    }

    #[test]
    fn test_matching_4() {
        let test = TestEntry::new(4);

        // There should be no matches for the target file since it is
        // fundamentally different than the defined pattern file.
        let results = match_patterns(&test.pattern_handler, &test.get_test_file_of_type(0, "abc"));
        assert_eq!(results.len(), 0);
    }

    fn built_pattern_handler(source_directory: &str, target_pattern: &str) -> PatternHandler {
        let mut pattern_handler = PatternHandler::default();

        // By default we'll look at the path /patterns/ relative to the path of the executable.
        // If the source path is specified then we will attempt to load the patterns from there instead.
        let pattern_source = if source_directory.is_empty() {
            if let Ok(p) = env::current_dir() {
                let mut temp = p.clone();
                temp.push("patterns");
                temp
            } else {
                eprintln!("Unable to get the current working directory, and no definition source specified. Unable to continue.");
                return pattern_handler;
            }
        } else {
            PathBuf::from(source_directory)
        };

        if !utils::directory_exists(&pattern_source) {
            eprintln!("The specified pattern source directory doesn't exist. Unable to continue.");
            return pattern_handler;
        }

        pattern_handler.read(pattern_source, target_pattern);

        pattern_handler
    }

    #[inline]
    fn match_patterns<'a>(
        pattern_handler: &'a PatternHandler,
        path: &str,
    ) -> Vec<PatternMatch<'a>> {
        let chunk =
            file_processor::read_file_header_chunk(path).expect("failed to read sample file");

        let mut point_store: Vec<PatternMatch> = pattern_handler
            .patterns
            .par_iter()
            .filter_map(|pattern| {
                let points = FilePointCalculator::compute(pattern, &chunk, path, true);
                if points > 0 {
                    Some(PatternMatch::new(
                        &pattern.type_data.uuid,
                        points,
                        pattern.max_points,
                    ))
                } else {
                    None
                }
            })
            .collect();

        // Sort the results by percentage match score, descending.
        point_store.sort_unstable_by(|a, b| b.percentage.partial_cmp(&a.percentage).unwrap());

        point_store
    }
}
