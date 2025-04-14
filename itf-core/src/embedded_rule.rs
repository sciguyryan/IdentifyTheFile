use regex::Regex;

#[derive(Debug)]
pub struct EmbeddedRule {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub mimes: &'static [&'static str],
    pub uuid: &'static str,
    pub sequences: &'static [(u64, &'static [u8])],
    pub regexes: &'static [(&'static str, Regex)],
    pub strings: &'static [&'static str],
    pub min_entropy: u16,
    pub max_entropy: u16,
}
