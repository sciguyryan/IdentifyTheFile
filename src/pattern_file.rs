use chrono::{DateTime, Utc};

pub struct Pattern {
    pub type_data: PatternTypeData,
    pub data: PatternData,
    pub other_data: PatternOtherData,
    pub submitter_data: PatternSubmitterData,
}

impl Pattern {}

pub struct PatternTypeData {
    pub name: String,
    pub description: String,
    pub known_extensions: Vec<String>,
    pub known_mimetypes: Vec<String>,
}

pub struct PatternData {
    pub scan_strings: bool,
    pub string_patterns: Vec<String>,
    pub scan_byte_patterns: bool,
    pub byte_patterns: Vec<Vec<u8>>,
}

pub struct PatternOtherData {
    pub total_scanned_files: usize,
}

pub struct PatternSubmitterData {
    pub scanned_by: String,
    pub scanned_by_email: String,
    pub submitted_at: DateTime<Utc>,
    pub refined_by: Vec<String>,
    pub refined_by_email: Vec<String>,
}
