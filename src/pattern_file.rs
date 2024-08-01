use std::collections::HashMap;

use chrono::{DateTime, Utc};

#[derive(Default)]
pub struct Pattern {
    pub type_data: PatternTypeData,
    pub data: PatternData,
    pub other_data: PatternOtherData,
    pub submitter_data: PatternSubmitterData,
}

impl Pattern {
    pub fn new(
        name: String,
        description: String,
        known_extensions: Vec<String>,
        known_mimetypes: Vec<String>,
    ) -> Self {
        Self {
            type_data: PatternTypeData {
                name,
                description,
                known_extensions,
                known_mimetypes,
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
}

#[derive(Default)]
pub struct PatternTypeData {
    /// The name of this file type.
    pub name: String,
    /// The description of this file type.
    pub description: String,
    /// Any known extensions for this file type.
    pub known_extensions: Vec<String>,
    /// Any known mimetypes for this file type.
    pub known_mimetypes: Vec<String>,
}

#[derive(Default)]
pub struct PatternData {
    /// Should we scan for strings in this file type?
    pub scan_strings: bool,
    /// Any strings that may be associated with this file type.
    /// This field will be empty if string scanning is disabled.
    pub string_patterns: Vec<String>,
    /// Should we scan for byte sequences?
    pub scan_byte_sequences: bool,
    /// Any positional byte sequences that may be associated with this file type.
    /// This field will be empty if byte sequence scanning is disabled.
    pub byte_sequences: HashMap<usize, Vec<u8>>,
    /// Should we scan the file's entropy?
    pub scan_entropy: bool,
    /// The average entropy for this file type.
    /// This will be zero if entropy scanning is disabled.
    pub average_entropy: f64,
}

#[derive(Default)]
pub struct PatternOtherData {
    /// The total number of files that have been scanned to build this pattern.
    /// Refinements to the pattern will add to this total.
    pub total_scanned_files: usize,
    /// The raw byte entropy counts, stored for refinement purposes.
    pub entropy_bytes: HashMap<u8, usize>,
}

#[derive(Default)]
pub struct PatternSubmitterData {
    pub scanned_by: String,
    pub scanned_by_email: String,
    pub submitted_on: DateTime<Utc>,
    pub refined_by: Vec<String>,
    pub refined_by_email: Vec<String>,
}
