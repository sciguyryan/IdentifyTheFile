use chrono::{self, TimeZone, Utc};
use hashbrown::HashSet;
use serde::Deserialize;
use serde_derive::Serialize;
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    file_point_calculator::{CONFIDENCE_SCALE_FACTOR, FILE_EXTENSION_POINTS, MAX_ENTROPY_POINTS},
    file_processor, utils,
};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Pattern {
    /// The basic pattern information.
    #[serde(rename = "td")]
    pub type_data: PatternTypeData,
    /// The pattern data to be used when performing a scan.
    #[serde(rename = "pd")]
    pub data: PatternData,
    /// Any other data that may be associated with the pattern.
    #[serde(rename = "od")]
    pub other_data: PatternOtherData,
    /// The submitter information, if specified.
    #[serde(rename = "sd")]
    pub submitter_data: PatternSubmitterData,
    // The maximum number of points that can be given by a match against this pattern.
    #[serde(skip)]
    pub max_points: usize,
    /// The confidence factor, used in match point calculations.
    #[serde(skip)]
    pub confidence_factor: f32,
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
            max_points: 0,
            confidence_factor: 0.0,
        }
    }

    /// Add the relevant submitter data to the [`Pattern`].
    ///
    /// # Arguments
    ///
    /// * `scanned_by` - The name of the person that scanned the files. May be empty.
    /// * `scanned_by_email` - The email of the person that scanned the files. May be empty.
    pub fn add_submitter_data(&mut self, scanned_by: &str, scanned_by_email: &str) {
        self.submitter_data = PatternSubmitterData {
            scanned_by: scanned_by.to_string(),
            scanned_by_email: scanned_by_email.to_string(),
            scanned_on: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            refined_by: vec![],
            refined_by_email: vec![],
        };
    }

    /// Build a [`Pattern`] from a target directory and for a specific file type.
    ///
    /// # Arguments
    ///
    /// * `source_directory` - The target directory containing the sample files.
    /// * `target_extension` - The target extension for the sample files.
    /// * `scan_strings` - Should the sample files be scanned for viable strings? This can be performance intensive with a large number of files.
    /// * `scan_strings` - Should the sample files be scanned for matching byte sequences?
    /// * `scan_byte_distribution` - Should the sample files have their byte distribution scanned?
    pub fn build_patterns_from_data(
        &mut self,
        source_directory: &str,
        target_extension: &str,
        scan_strings: bool,
        scan_byte_sequences: bool,
        scan_byte_distribution: bool,
    ) {
        let mut first_byte_sequence_pass = true;

        let files = utils::list_files_of_type(source_directory, target_extension);

        let mut common_byte_sequences = Vec::<(usize, Vec<u8>)>::new();
        let mut all_strings = Vec::with_capacity(files.len());
        let mut byte_distribution: [usize; 256] = [0; 256];

        #[cfg(debug_assertions)]
        let mut no_strings = vec![];

        for file_path in &files {
            let chunk =
                file_processor::read_file_header_chunk(file_path).expect("failed to read file");

            if scan_byte_distribution {
                file_processor::count_byte_frequencies(&chunk, &mut byte_distribution);
            }

            if scan_strings {
                let strings = file_processor::extract_file_strings(&chunk);

                #[cfg(debug_assertions)]
                {
                    if strings.is_empty() {
                        no_strings.push(file_path);
                    }
                }

                all_strings.push(strings);
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
            common_byte_sequences.sort_unstable_by_key(|b| std::cmp::Reverse(b.0));
        }

        // Sieve the strings to retain only the common ones.
        let mut common_strings = Vec::new();
        if scan_strings {
            let mut all_string_sets: Vec<Vec<&str>> = Vec::with_capacity(all_strings.len());
            for string_set in &all_strings {
                all_string_sets.push(string_set.iter().map(AsRef::as_ref).collect());
            }

            common_strings = file_processor::common_string_sieve(&mut all_string_sets);
        }

        if scan_byte_distribution {
            self.data.average_entropy = utils::calculate_shannon_entropy(&byte_distribution);
        }

        #[cfg(debug_assertions)]
        {
            if scan_strings && no_strings.is_empty() {
                eprintln!("The following files had no string: {no_strings:#?}");
            }
        }

        // Add the computed information into the struct.
        self.data.strings = HashSet::from_iter(common_strings);
        self.data.sequences = common_byte_sequences;

        self.other_data.total_scanned_files = files.len();
    }

    /// Compute various attributes once the main object data has been initialized.
    pub fn compute_attributes(&mut self) {
        self.compute_confidence_factor();
        self.compute_max_points();
    }

    /// Compute the confidence scale factor based on the number of files scanned to build this pattern.
    fn compute_confidence_factor(&mut self) {
        self.confidence_factor =
            (self.other_data.total_scanned_files as f32).powf(CONFIDENCE_SCALE_FACTOR);
    }

    /// Computer the maximum number of points that can be awarded for a perfect match against this pattern.
    /// The more detailed the pattern, the higher the total points available.
    fn compute_max_points(&mut self) {
        let mut points = 0.0;

        if self.data.should_scan_sequences() {
            for (_, sequence) in &self.data.sequences {
                points += sequence.len() as f32;
            }
        }

        if self.data.should_scan_strings() {
            for string in &self.data.strings {
                points += string.len() as f32;
            }
        }

        if self.data.should_scan_composition() {
            points += MAX_ENTROPY_POINTS;
        }

        // Scale the relevant points by the confidence factor derived from the total files scanned.
        points *= self.confidence_factor;

        // The file extension is considered a separate factor and doesn't scale with the number
        // of scanned files.
        points += FILE_EXTENSION_POINTS;

        self.max_points = points.ceil() as usize;
    }

    /// Attempt to build a [`Pattern`] from a JSON string.
    ///
    /// # Arguments
    ///
    /// * `input` - The input JSON string.
    ///
    /// # Returns
    ///
    /// An error if the deserialization failed, otherwise the build [`Patten`] will be returned.
    pub fn from_json_str(input: &str) -> Result<Pattern, Box<dyn std::error::Error>> {
        let json_bytes = input.as_bytes().to_vec();
        let p: Pattern = serde_json::from_slice::<Pattern>(&json_bytes[..])?;
        Ok(p)
    }

    /// Attempt to build a [`Pattern`] from a JSON string, using SIMD.
    ///
    /// # Arguments
    ///
    /// * `input` - The input JSON string.
    ///
    /// # Returns
    ///
    /// An error if the deserialization failed, otherwise the build [`Patten`] will be returned.
    pub fn from_simd_json_str(input: &str) -> Result<Pattern, Box<dyn std::error::Error>> {
        let mut json_bytes = input.as_bytes().to_vec();
        let p: Pattern = simd_json::from_slice(&mut json_bytes[..])?;
        Ok(p)
    }

    /// Derive the name of a pattern based on the stored pattern data.
    fn get_pattern_file_name(&self) -> String {
        let file_name = utils::sanitize_file_name(&self.type_data.name);
        file_name.replace(" ", "-") + ".json"
    }

    /// Derive the name of a refined pattern based on the stored pattern data.
    pub fn get_refined_pattern_file_name(&self) -> String {
        let file_name = utils::sanitize_file_name(&self.type_data.name);
        file_name.replace(" ", "-") + "-refined.json"
    }

    /// Attempt to write a JSON file for the data contained within the pattern.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the file containing the pattern's JSON data.
    ///
    /// # Returns
    ///
    /// An error if the writing failed, otherwise a [`PathBuf`] to the written file will be returned.
    pub fn write<P: AsRef<Path>>(&self, path: P) -> std::io::Result<PathBuf> {
        let serialized = serde_json::to_string(self).unwrap();

        let mut path = PathBuf::from(path.as_ref());
        path.push(self.get_pattern_file_name());

        let mut output = File::create(&path)?;
        match write!(output, "{serialized}") {
            Ok(_) => Ok(path),
            Err(e) => Err(e),
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct PatternTypeData {
    /// The name of this file type.
    pub name: String,
    /// The description of this file type.
    #[serde(default = "default_description")]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Any known extensions for this file type.
    #[serde(rename = "extensions", default = "default_extensions")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub known_extensions: Vec<String>,
    /// Any known mimetypes for this file type.
    #[serde(rename = "mimetypes", default = "default_mimetypes")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub known_mimetypes: Vec<String>,
    /// The UUID of the pattern file.
    pub uuid: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct PatternData {
    /// Any positional byte sequences that may be associated with this file type.
    /// This field will be empty if byte sequence scanning was disabled.
    ///
    /// # Notes
    /// Byte sequence matches are -not- optional - a missing sequence will result in an immediate no-match.
    #[serde(default = "default_sequences")]
    pub sequences: Vec<(usize, Vec<u8>)>,
    /// Any strings that may be associated with this file type.
    /// This field will be empty if string scanning was disabled.
    ///
    /// # Notes
    /// String matches are optional and a missing string will not render the match void.
    #[serde(default = "default_strings")]
    pub strings: HashSet<String>,
    /// The average entropy for this file type.
    /// This will be zero if byte distribution scanning was disabled.
    ///
    /// # Notes
    /// Entropy will be evaluated based by its percentage of deviation from the stored average.
    #[serde(default = "default_average_entropy")]
    pub average_entropy: f32,
}

impl PatternData {
    /// Should we scan for strings when using this pattern?
    #[inline(always)]
    pub fn should_scan_strings(&self) -> bool {
        !self.strings.is_empty()
    }

    /// Should we scan for byte sequences when using this pattern?
    #[inline(always)]
    pub fn should_scan_sequences(&self) -> bool {
        !self.sequences.is_empty()
    }

    /// Should we scan the file's composition when using this pattern?
    #[inline(always)]
    pub fn should_scan_composition(&self) -> bool {
        self.average_entropy != 0.0
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct PatternOtherData {
    /// The total number of files that have been scanned to build this pattern.
    /// Refinements to the pattern will add to this total.
    pub total_scanned_files: usize,
    /// A URL documenting the file format.
    #[serde(default = "default_file_format_url")]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub file_format_url: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PatternSubmitterData {
    /// The name of the person who performed the initial scan. May be left blank.
    pub scanned_by: String,
    /// The email of the person who performed the initial scan. May be left blank.
    pub scanned_by_email: String,
    /// The timestamp for when the initial scan was performed.
    pub scanned_on: u64,
    /// The list of names of the people that performed refinements on the scan. May be empty.
    #[serde(default = "default_refined_by")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub refined_by: Vec<String>,
    /// The list of email addresses of the people that performed refinements on the scan. May be empty.
    #[serde(default = "default_refined_by_email")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub refined_by_email: Vec<String>,
}

impl Default for PatternSubmitterData {
    fn default() -> Self {
        Self {
            scanned_by: Default::default(),
            scanned_by_email: Default::default(),
            scanned_on: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            refined_by: Default::default(),
            refined_by_email: Default::default(),
        }
    }
}

impl PatternSubmitterData {
    pub fn get_localised_date(&self) -> String {
        let dt = Utc
            .timestamp_opt(self.scanned_on as i64, 0)
            .single()
            .ok_or_else(Utc::now)
            .unwrap();
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

fn default_description() -> String {
    String::new()
}

fn default_extensions() -> Vec<String> {
    vec![]
}

fn default_mimetypes() -> Vec<String> {
    vec![]
}

fn default_strings() -> HashSet<String> {
    HashSet::new()
}

fn default_sequences() -> Vec<(usize, Vec<u8>)> {
    vec![]
}

fn default_average_entropy() -> f32 {
    0.0
}

fn default_file_format_url() -> String {
    String::new()
}

fn default_refined_by() -> Vec<String> {
    vec![]
}

fn default_refined_by_email() -> Vec<String> {
    vec![]
}

#[cfg(test)]
mod tests_pattern {
    use core::str;

    use hashbrown::HashSet;

    use crate::{test_utils, utils};

    use super::Pattern;

    #[test]
    fn test_string_1() {
        // Basic match, two files both completely matching.
        let pattern = build_test("strings", "1", true, false, false);

        let set = HashSet::from(["ABCDEFGHIJK".to_string()]);

        assert_eq!(pattern.data.strings, set);
    }

    #[test]
    fn test_string_2() {
        // Simple non-match, two files and none are matching.
        let pattern = build_test("strings", "2", true, false, false);

        assert!(pattern.data.strings.is_empty());
    }

    #[test]
    fn test_string_3() {
        // Simple match, but only a substring is matching.
        let pattern = build_test("strings", "3", true, false, false);

        let set = HashSet::from(["ABCDE".to_string()]);

        assert_eq!(pattern.data.strings, set,);
    }

    #[test]
    fn test_string_4() {
        // Split match, two substrings will be returned. Delimiter formed by a "non-string" character.
        let pattern = build_test("strings", "4", true, false, false);

        let set = HashSet::from(["ABCDE".to_string(), "GHIJK".to_string()]);

        assert_eq!(pattern.data.strings, set,);
    }

    #[test]
    fn test_string_5() {
        // Split match, one substrings will be returned.
        let pattern = build_test("strings", "5", true, false, false);

        let set = HashSet::from(["GHIJK".to_string()]);

        assert_eq!(pattern.data.strings, set,);
    }

    #[test]
    fn test_string_6() {
        // Split match, two substrings will be returned, one will be skipped due to length requirements.
        let pattern = build_test("strings", "6", true, false, false);

        let set = HashSet::from(["ABCDEFGHIJK".to_string(), "123456".to_string()]);

        assert_eq!(pattern.data.strings, set,);
    }

    #[test]
    fn test_string_7() {
        // Split match, one substring will be returned, one will be skipped due to length requirements.
        let pattern = build_test("strings", "7", true, false, false);

        let set = HashSet::from(["123456".to_string()]);

        assert_eq!(pattern.data.strings, set,);
    }

    #[test]
    fn test_string_8() {
        // Testing that all of the safe string characters are returned in a string.
        let pattern = build_test("strings", "8", true, false, false);

        let set = HashSet::from([
            " !#$+,-./0123456789<=>?ABCDEFGHIJKLMNOPQRSTUVWXYZ_ABCDEFGHIJKLMN".to_string(),
        ]);

        assert_eq!(pattern.data.strings, set,);
    }

    #[test]
    fn test_byte_sequence_1() {
        // Basic match, two files both completely matching.
        let pattern = build_test("byte_sequences", "1", false, true, false);

        let expected_set = vec![(0, (*b"abcdefghijk").to_vec())];

        assert_eq!(pattern.data.sequences, expected_set);
    }

    #[test]
    fn test_byte_sequence_2() {
        // Simple non-match, two files and none are matching.
        let pattern = build_test("byte_sequences", "2", false, true, false);

        assert_eq!(pattern.data.sequences, vec![]);
    }

    #[test]
    fn test_byte_sequence_3() {
        // Simple match, two sub-sequences matching.
        let pattern = build_test("byte_sequences", "3", false, true, false);

        let expected_set = vec![(6, (*b"ghijk").to_vec()), (0, (*b"abcde").to_vec())];

        assert_eq!(pattern.data.sequences, expected_set);
    }

    #[test]
    fn test_byte_sequence_4() {
        // Single match, the end of the sequence is offset and so won't match.
        let pattern = build_test("byte_sequences", "4", false, true, false);

        let expected_set = vec![(0, (*b"abcde").to_vec())];

        assert_eq!(pattern.data.sequences, expected_set);
    }

    #[test]
    fn test_byte_sequence_5() {
        // No matches.
        let pattern = build_test("byte_sequences", "5", false, true, false);

        assert_eq!(pattern.data.sequences, vec![]);
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

        assert_eq!(pattern.data.sequences, expected_set);
    }

    #[test]
    fn test_byte_sequence_7() {
        // Split match, two substrings will be returned.
        let pattern = build_test("byte_sequences", "7", false, true, false);

        let expected_set = vec![
            (16, "123456".as_bytes().to_vec()),
            (13, "a".as_bytes().to_vec()),
        ];

        assert_eq!(pattern.data.sequences, expected_set);
    }

    #[test]
    fn test_byte_sequence_8() {
        // Single match at the very end.
        let pattern = build_test("byte_sequences", "8", false, true, false);

        let expected_set = vec![(10, "k".as_bytes().to_vec())];

        assert_eq!(pattern.data.sequences, expected_set);
    }

    #[test]
    fn test_byte_sequence_9() {
        // Single match at the beginning. The null byte sequence should be stripped.
        let pattern = build_test("byte_sequences", "9", false, true, false);

        let expected_set = vec![(0, "abcdefghijk".as_bytes().to_vec())];

        assert_eq!(pattern.data.sequences, expected_set);
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
    fn test_no_strings_observed() {
        let pattern = build_test("strings", "8", false, true, false);

        assert!(
            pattern.data.strings.is_empty(),
            "strings were set when no strings was specified"
        );
    }

    #[test]
    fn test_no_sequences_observed() {
        let pattern = build_test("strings", "8", true, false, false);

        assert!(
            pattern.data.sequences.is_empty(),
            "sequences were set when no sequences was specified"
        );
    }

    #[test]
    fn test_no_composition_observed() {
        let pattern = build_test("strings", "8", true, false, false);

        assert_eq!(
            pattern.data.average_entropy, 0.0,
            "average entropy were set when no average entropy was specified"
        );
    }

    #[test]
    fn test_entropy_3() {
        let pattern = build_test("entropy", "3", false, false, true);

        // Floats are tricky, we need a little bit of fuzziness to properly check them.
        if !approx_equal(pattern.data.average_entropy, 0f32, 1) {
            panic!("expected = 0, actual = {}", pattern.data.average_entropy);
        }
    }

    fn approx_equal(a: f32, b: f32, decimal_places: usize) -> bool {
        utils::round_to_dp(a, decimal_places) == utils::round_to_dp(b, decimal_places)
    }

    fn build_test(
        test_type: &str,
        test_id: &str,
        strings: bool,
        bytes: bool,
        entropy: bool,
    ) -> Pattern {
        let test_dir = test_utils::test_path_builder(test_type, test_id);

        let mut pattern = Pattern::new("test", "test", vec!["test".to_string()], vec![]);
        pattern.build_patterns_from_data(&test_dir, "test", strings, bytes, entropy);

        pattern
    }
}
