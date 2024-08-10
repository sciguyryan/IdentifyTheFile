use chrono::{DateTime, Utc};
use serde_derive::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Write,
    path::PathBuf,
};

use crate::{file_processor, utils};

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Pattern {
    /// The basic pattern information.
    pub type_data: PatternTypeData,
    /// The pattern data to be used when performing a scan.
    pub data: PatternData,
    /// Any other data that may be associated with the pattern.
    pub other_data: PatternOtherData,
    /// The submitter information, if specified.
    pub submitter_data: PatternSubmitterData,
}

impl Pattern {
    pub fn new(
        name: &str,
        description: &str,
        known_extensions: Vec<String>,
        known_mimetypes: Vec<String>,
    ) -> Self {
        Self {
            type_data: PatternTypeData {
                name: name.to_string(),
                description: description.to_string(),
                known_extensions: known_extensions.iter().map(|s| s.to_uppercase()).collect(),
                known_mimetypes,
                uuid: utils::make_uuid(),
            },
            data: PatternData::default(),
            other_data: PatternOtherData::default(),
            submitter_data: PatternSubmitterData::default(),
        }
    }

    pub fn add_pattern_data(
        &mut self,
        scan_strings: bool,
        string_patterns: HashSet<String>,
        scan_byte_sequences: bool,
        byte_sequences: Vec<(usize, Vec<u8>)>,
        scan_entropy: bool,
    ) {
        self.data = PatternData {
            scan_strings,
            string_patterns,
            scan_byte_sequences,
            byte_sequences,
            scan_file_composition: scan_entropy,
            average_entropy: 0.0,
        };
    }

    pub fn add_other_data(&mut self, total_scanned_files: usize) {
        self.other_data = PatternOtherData {
            total_scanned_files,
        };
    }

    pub fn add_submitter_data(&mut self, scanned_by: &str, scanned_by_email: &str) {
        self.submitter_data = PatternSubmitterData {
            scanned_by: scanned_by.to_string(),
            scanned_by_email: scanned_by_email.to_string(),
            scanned_on: chrono::offset::Utc::now(),
            refined_by: vec![],
            refined_by_email: vec![],
        };
    }

    pub fn build_patterns_from_data(
        &mut self,
        source_directory: &str,
        target_extension: &str,
        scan_strings: bool,
        scan_byte_sequences: bool,
        scan_byte_distribution: bool,
    ) {
        let mut first_byte_sequence_pass = true;

        let mut common_byte_sequences = Vec::<(usize, Vec<u8>)>::new();
        let mut all_strings = Vec::new();
        let mut byte_distribution = HashMap::new();

        let files = utils::list_files_of_type(source_directory, target_extension);
        for file_path in &files {
            // If we made it here then we have a valid file.
            let chunk =
                file_processor::read_file_header_chunk(file_path).expect("failed to read file");

            if scan_byte_distribution {
                file_processor::count_byte_frequencies(&chunk, &mut byte_distribution);
            }

            if scan_strings {
                let string_hashset = file_processor::generate_file_string_hashset(&chunk);
                all_strings.push(string_hashset);
            }

            // On the first pass, we simply set the matching sequence as the entire byte block.
            // This will get trimmed down and split into sections over future loop iterations.
            if scan_byte_sequences && first_byte_sequence_pass {
                common_byte_sequences.push((0, chunk));
                first_byte_sequence_pass = false;
                continue;
            }

            if scan_byte_sequences {
                file_processor::refine_common_byte_sequences_v2(&chunk, &mut common_byte_sequences);
            }
        }

        if scan_byte_sequences {
            file_processor::strip_unwanted_sequences(&mut common_byte_sequences);

            /*
             * Sort by the start position of the sequence, descending first.
             * This is done because the testing loop will bail if the start index is
             * beyond the bounds of the array. This could be an asset when testing
             * lots of smaller files.
             */
            common_byte_sequences.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        }

        // Sieve the strings to retain only the common ones.
        let mut common_strings = HashSet::new();
        if scan_strings {
            common_strings = file_processor::common_string_sieve(&mut all_strings);
        }

        if scan_byte_distribution {
            self.data.average_entropy = utils::calculate_shannon_entropy(&byte_distribution);
        }

        // Add the computed information into the struct.
        self.data.scan_strings = scan_strings;
        self.data.string_patterns = common_strings;
        self.data.scan_byte_sequences = scan_byte_sequences;
        self.data.byte_sequences = common_byte_sequences;
        self.data.scan_file_composition = scan_byte_distribution;

        self.other_data.total_scanned_files += files.len();
    }

    fn get_pattern_file_name(&self) -> String {
        let file_name = utils::remove_invalid_file_name(&self.type_data.name);
        file_name.replace(" ", "-") + ".json"
    }

    pub fn write(&self, path: &str) -> std::io::Result<()> {
        let serialized = serde_json::to_string(self).unwrap();

        let mut path = PathBuf::from(path);
        path.push(self.get_pattern_file_name());

        let mut output = File::create(path)?;
        write!(output, "{serialized}")
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct PatternTypeData {
    /// The name of this file type.
    pub name: String,
    /// The description of this file type.
    pub description: String,
    /// Any known extensions for this file type.
    #[serde(rename(serialize = "extensions", deserialize = "extensions"))]
    pub known_extensions: Vec<String>,
    /// Any known mimetypes for this file type.
    #[serde(rename(serialize = "mimetypes", deserialize = "mimetypes"))]
    pub known_mimetypes: Vec<String>,
    /// The UUID of the pattern file.
    pub uuid: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PatternData {
    /// Should we scan for strings in this file type?
    pub scan_strings: bool,
    /// Any strings that may be associated with this file type.
    /// This field will be empty if string scanning is disabled.
    ///
    /// # Notes
    /// String matches are optional and a missing string will not render the match void.
    pub string_patterns: HashSet<String>,
    /// Should we scan for byte sequences?
    pub scan_byte_sequences: bool,
    /// Any positional byte sequences that may be associated with this file type.
    /// This field will be empty if byte sequence scanning is disabled.
    ///
    /// # Notes
    /// Byte sequence matches are not optional - a missing sequence will result in a no-match.
    pub byte_sequences: Vec<(usize, Vec<u8>)>,
    /// Should we scan various aspects of the file's composition?
    pub scan_file_composition: bool,
    /// The average entropy for this file type.
    /// This will be zero if byte distribution scanning is disabled.
    ///
    /// # Notes
    /// Entropy will be evaluated based by its percentage of deviation from the stored average.
    average_entropy: f64,
}

impl PatternData {
    pub fn get_entropy(&self) -> f64 {
        utils::round_to_dp(self.average_entropy, 3)
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct PatternOtherData {
    /// The total number of files that have been scanned to build this pattern.
    /// Refinements to the pattern will add to this total.
    pub total_scanned_files: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternSubmitterData {
    /// The name of the person who performed the initial scan. May be left blank.
    pub scanned_by: String,
    /// The email of the person who performed the initial scan. May be left blank.
    pub scanned_by_email: String,
    /// The timestamp for when the initial scan was performed.
    pub scanned_on: DateTime<Utc>,
    /// The list of names of the people that performed refinements on the scan. May be empty.
    pub refined_by: Vec<String>,
    /// The list of email addresses of the people that performed refinements on the scan. May be empty.
    pub refined_by_email: Vec<String>,
}

impl Default for PatternSubmitterData {
    fn default() -> Self {
        Self {
            scanned_by: Default::default(),
            scanned_by_email: Default::default(),
            scanned_on: chrono::offset::Utc::now(),
            refined_by: Default::default(),
            refined_by_email: Default::default(),
        }
    }
}

pub fn from_simd_json_str(input: &str) -> Result<Pattern, Box<dyn std::error::Error>> {
    let mut json_bytes = input.as_bytes().to_vec();
    let p: Pattern = simd_json::from_slice(&mut json_bytes[..])?;
    Ok(p)
}

#[cfg(test)]
mod tests_pattern {
    use core::str;
    use std::{collections::HashSet, path::Path};

    use crate::{file_processor::ASCII_CHARACTER_STRING, utils};

    use super::Pattern;

    #[test]
    fn test_string_1() {
        // Basic match, two files both completely matching.
        let pattern = build_test("strings", "1", true, false, false);

        let set = HashSet::from(["ABCDEFGHIJK".to_string()]);

        assert_eq!(pattern.data.string_patterns, set,);
    }

    #[test]
    fn test_string_2() {
        // Simple non-match, two files and none are matching.
        let pattern = build_test("strings", "2", true, false, false);

        assert!(pattern.data.string_patterns.is_empty());
    }

    #[test]
    fn test_string_3() {
        // Simple match, but only a substring is matching.
        let pattern = build_test("strings", "3", true, false, false);

        let set = HashSet::from(["ABCDE".to_string()]);

        assert_eq!(pattern.data.string_patterns, set,);
    }

    #[test]
    fn test_string_4() {
        // Split match, two substrings will be returned. Delimiter formed by a "non-string" character.
        let pattern = build_test("strings", "4", true, false, false);

        let set = HashSet::from(["ABCDE".to_string(), "GHIJK".to_string()]);

        assert_eq!(pattern.data.string_patterns, set,);
    }

    #[test]
    fn test_string_5() {
        // Split match, one substrings will be returned.
        let pattern = build_test("strings", "5", true, false, false);

        let set = HashSet::from(["GHIJK".to_string()]);

        assert_eq!(pattern.data.string_patterns, set,);
    }

    #[test]
    fn test_string_6() {
        // Split match, two substrings will be returned, one will be skipped due to length requirements.
        let pattern = build_test("strings", "6", true, false, false);

        let set = HashSet::from(["ABCDEFGHIJK".to_string(), "123456".to_string()]);

        assert_eq!(pattern.data.string_patterns, set,);
    }

    #[test]
    fn test_string_7() {
        // Split match, one substring will be returned, one will be skipped due to length requirements.
        let pattern = build_test("strings", "7", true, false, false);

        let set = HashSet::from(["123456".to_string()]);

        assert_eq!(pattern.data.string_patterns, set,);
    }

    #[test]
    fn test_string_8() {
        // Testing that all of the safe string characters are returned in a string.
        let pattern = build_test("strings", "8", true, false, false);

        let set = HashSet::from([ASCII_CHARACTER_STRING.to_ascii_uppercase()]);

        assert_eq!(pattern.data.string_patterns, set,);
    }

    #[test]
    fn test_byte_sequence_1() {
        // Basic match, two files both completely matching.
        let pattern = build_test("byte_sequences", "1", false, true, false);

        let expected_set = vec![(0, (*b"abcdefghijk").to_vec())];

        assert_eq!(pattern.data.byte_sequences, expected_set);
    }

    #[test]
    fn test_byte_sequence_2() {
        // Simple non-match, two files and none are matching.
        let pattern = build_test("byte_sequences", "2", false, true, false);

        assert_eq!(pattern.data.byte_sequences, vec![]);
    }

    #[test]
    fn test_byte_sequence_3() {
        // Simple match, two sub-sequences matching.
        let pattern = build_test("byte_sequences", "3", false, true, false);

        let expected_set = vec![(6, (*b"ghijk").to_vec()), (0, (*b"abcde").to_vec())];

        assert_eq!(pattern.data.byte_sequences, expected_set);
    }

    #[test]
    fn test_byte_sequence_4() {
        // Single match, the end of the sequence is offset and so won't match.
        let pattern = build_test("byte_sequences", "4", false, true, false);

        let expected_set = vec![(0, (*b"abcde").to_vec())];

        assert_eq!(pattern.data.byte_sequences, expected_set);
    }

    #[test]
    fn test_byte_sequence_5() {
        // No matches.
        let pattern = build_test("byte_sequences", "5", false, true, false);

        assert_eq!(pattern.data.byte_sequences, vec![]);
    }

    #[test]
    fn test_byte_sequence_6() {
        // The entire sequence matches but since the sequence length would
        // exceed the maximum then it will get split into two segments.
        let pattern = build_test("byte_sequences", "6", false, true, false);

        let expected_set = vec![
            (16, "123456".as_bytes().to_vec()),
            (0, "abcdefghijkŠaŠ".as_bytes().to_vec()),
        ];

        assert_eq!(pattern.data.byte_sequences, expected_set);
    }

    #[test]
    fn test_byte_sequence_7() {
        // Split match, two substrings will be returned.
        let pattern = build_test("byte_sequences", "7", false, true, false);

        let expected_set = vec![
            (16, "123456".as_bytes().to_vec()),
            (13, "a".as_bytes().to_vec()),
        ];

        assert_eq!(pattern.data.byte_sequences, expected_set);
    }

    #[test]
    fn test_byte_sequence_8() {
        // Single match at the very end.
        let pattern = build_test("byte_sequences", "8", false, true, false);

        let expected_set = vec![(10, "k".as_bytes().to_vec())];

        assert_eq!(pattern.data.byte_sequences, expected_set);
    }

    #[test]
    fn test_byte_sequence_9() {
        // Single match at the beginning. The null byte sequence should be stripped.
        let pattern = build_test("byte_sequences", "9", false, true, false);

        let expected_set = vec![(0, "abcdefghijk".as_bytes().to_vec())];

        assert_eq!(pattern.data.byte_sequences, expected_set);
    }

    #[test]
    fn test_entropy_1() {
        let pattern = build_test("entropy", "1", false, false, true);

        // Floats are tricky, we need a little bit of fuzziness to properly check them.
        if !approx_equal(pattern.data.average_entropy, 4.3, 1) {
            panic!(
                "expected = 4.3, actual = {}",
                utils::round_to_dp(pattern.data.average_entropy, 1)
            );
        }
    }

    #[test]
    fn test_entropy_2() {
        let pattern = build_test("entropy", "2", false, false, true);

        // Floats are tricky, we need a little bit of fuzziness to properly check them.
        if !approx_equal(pattern.data.average_entropy, 8.0, 1) {
            panic!(
                "expected = 8.0, actual = {}",
                utils::round_to_dp(pattern.data.average_entropy, 1)
            );
        }
    }

    #[test]
    fn test_entropy_3() {
        let pattern = build_test("entropy", "3", false, false, true);

        // Floats are tricky, we need a little bit of fuzziness to properly check them.
        if !approx_equal(pattern.data.average_entropy, 0f64, 1) {
            panic!("expected = 0, actual = {}", pattern.data.average_entropy);
        }
    }

    fn approx_equal(a: f64, b: f64, decimal_places: usize) -> bool {
        utils::round_to_dp(a, decimal_places) == utils::round_to_dp(b, decimal_places)
    }

    fn test_path_builder(test_type: &str, test_id: &str) -> String {
        let test_dir = std::fs::canonicalize(format!("./tests/{test_type}/{test_id}"))
            .expect("failed to find test directory");
        let resolved_dir = test_dir.to_string_lossy().to_string();

        if !Path::new(&resolved_dir).exists() {
            panic!("failed to find test directory at '{resolved_dir}'");
        }

        resolved_dir
    }

    fn build_test(
        test_type: &str,
        test_id: &str,
        strings: bool,
        bytes: bool,
        entropy: bool,
    ) -> Pattern {
        let test_dir = test_path_builder(test_type, test_id);

        let mut pattern = Pattern::new(
            "test",
            "test",
            vec!["test".to_string()],
            vec!["text/plain".to_string()],
        );
        pattern.build_patterns_from_data(&test_dir, "test", strings, bytes, entropy);

        pattern
    }
}
