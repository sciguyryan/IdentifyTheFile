use chrono::{DateTime, Utc};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{file_processor, utils};

const VERBOSE: bool = false;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Pattern {
    pub type_data: PatternTypeData,
    pub data: PatternData,
    pub other_data: PatternOtherData,
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
                known_extensions,
                known_mimetypes,
                uuid: Uuid::now_v7(),
            },
            data: PatternData::default(),
            other_data: PatternOtherData::default(),
            submitter_data: PatternSubmitterData::default(),
        }
    }

    pub fn add_pattern_data(
        &mut self,
        scan_strings: bool,
        string_patterns: Vec<String>,
        scan_byte_sequences: bool,
        byte_sequences: HashMap<usize, Vec<u8>>,
        scan_entropy: bool,
        average_entropy: f64,
    ) {
        self.data = PatternData {
            scan_strings,
            string_patterns,
            scan_byte_sequences,
            byte_sequences,
            scan_entropy,
            average_entropy,
        }
    }

    pub fn add_other_data(
        &mut self,
        total_scanned_files: usize,
        entropy_bytes: HashMap<u8, usize>,
    ) {
        self.other_data = PatternOtherData {
            total_scanned_files,
            entropy_bytes,
        };
    }

    pub fn add_submitter_data(
        &mut self,
        scanned_by: String,
        scanned_by_email: String,
        submitted_on: DateTime<Utc>,
        refined_by: Vec<String>,
        refined_by_email: Vec<String>,
    ) {
        self.submitter_data = PatternSubmitterData {
            scanned_by,
            scanned_by_email,
            submitted_on,
            refined_by,
            refined_by_email,
        };
    }

    pub fn build_patterns_from_data(
        &mut self,
        source_directory: &str,
        target_extension: &str,
        scan_strings: bool,
        scan_bytes: bool,
        scan_entropy: bool,
    ) {
        let mut first_byte_sequence_pass = true;

        let mut common_byte_sequences = HashMap::new();
        let mut all_strings = Vec::new();
        let mut entropy = HashMap::new();

        let files = utils::list_files_of_type(source_directory, target_extension);
        for file_path in &files {
            if VERBOSE {
                println!("Analyzing candidate file - {file_path}");
            }

            // If we made it here then we have a valid file.
            let chunk =
                file_processor::read_file_header_chunk(file_path).expect("failed to read file");

            if scan_entropy {
                file_processor::count_byte_frequencies(&chunk, &mut entropy);
            }

            if scan_strings {
                let string_hashset = file_processor::generate_file_string_hashset(&chunk);
                all_strings.push(string_hashset);
            }

            // On the first pass, we simply set the matching sequence as the entire byte block.
            // This will get trimmed down and split into sections over future loop iterations.
            if first_byte_sequence_pass {
                common_byte_sequences.insert(0, chunk);
                first_byte_sequence_pass = false;
                continue;
            }

            if scan_bytes {
                file_processor::refine_common_byte_sequences_v2(&chunk, &mut common_byte_sequences);
            }
        }

        // Sieve the strings to retain only the common ones.
        file_processor::strip_sequences_by_length(&mut common_byte_sequences);
        let common_strings = file_processor::common_string_sieve(&mut all_strings);

        // Compute the new average file entropy.
        let merged_entropy_bytes =
            utils::merge_hashmaps(vec![&self.other_data.entropy_bytes, &entropy]);

        // Add the computed information into the struct.
        self.data.scan_strings = scan_strings;
        self.data.string_patterns = Vec::from_iter(common_strings);
        self.data.scan_byte_sequences = scan_bytes;
        self.data.byte_sequences = common_byte_sequences;
        self.data.scan_entropy = scan_entropy;
        self.data.average_entropy =
            file_processor::calculate_shannon_entropy(&merged_entropy_bytes);

        self.other_data.total_scanned_files += files.len();
        self.other_data.entropy_bytes = merged_entropy_bytes;
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct PatternTypeData {
    /// The name of this file type.
    pub name: String,
    /// The description of this file type.
    pub description: String,
    /// Any known extensions for this file type.
    pub known_extensions: Vec<String>,
    /// Any known mimetypes for this file type.
    pub known_mimetypes: Vec<String>,
    /// The UUID of the pattern file.
    pub uuid: Uuid,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct PatternData {
    /// Should we scan for strings in this file type?
    pub scan_strings: bool,
    /// Any strings that may be associated with this file type.
    /// This field will be empty if string scanning is disabled.
    ///
    /// # Notes
    /// String matches are optional and a missing string will not render the match void.
    pub string_patterns: Vec<String>,
    /// Should we scan for byte sequences?
    pub scan_byte_sequences: bool,
    /// Any positional byte sequences that may be associated with this file type.
    /// This field will be empty if byte sequence scanning is disabled.
    ///
    /// # Notes
    /// Byte sequence matches are not optional - a missing sequence will result in a no-match.
    pub byte_sequences: HashMap<usize, Vec<u8>>,
    /// Should we scan the file's entropy?
    pub scan_entropy: bool,
    /// The average entropy for this file type.
    /// This will be zero if entropy scanning is disabled.
    ///
    /// # Notes
    /// Entropy will be evaluated based by its percentage of deviation from the stored average.
    pub average_entropy: f64,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct PatternOtherData {
    /// The total number of files that have been scanned to build this pattern.
    /// Refinements to the pattern will add to this total.
    pub total_scanned_files: usize,
    /// The raw byte entropy counts, stored for refinement purposes.
    pub entropy_bytes: HashMap<u8, usize>,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct PatternSubmitterData {
    pub scanned_by: String,
    pub scanned_by_email: String,
    pub submitted_on: DateTime<Utc>,
    pub refined_by: Vec<String>,
    pub refined_by_email: Vec<String>,
}
