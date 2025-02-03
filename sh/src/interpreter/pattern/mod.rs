use crate::interpreter::pattern::parse::{parse_pattern, PatternItem};
use crate::interpreter::pattern::regex::{parsed_pattern_to_regex, Regex};
use crate::interpreter::ExpandedWord;
use std::ffi::{CStr, CString};

mod parse;
mod regex;

pub struct Pattern {
    pattern_string: String,
    regex: Regex,
}

impl Pattern {
    pub fn new(word: &ExpandedWord) -> Result<Self, String> {
        let parsed_pattern = parse_pattern(word, false)?;
        let regex = parsed_pattern_to_regex(&parsed_pattern)?;
        Ok(Self {
            pattern_string: word.to_string(),
            regex,
        })
    }

    pub fn remove_largest_prefix(&self, s: String) -> String {
        let cstring = CString::new(s).expect("trying to match a string containing null");
        let mut prefix_end = 0;
        if let Some(regex_match) = self.regex.match_locations(&cstring).next() {
            if regex_match.start == 0 {
                prefix_end = regex_match.end;
            }
        }
        let mut bytes = cstring.into_bytes();
        bytes.drain(..prefix_end);
        String::from_utf8(bytes).expect("failed to create string after removing largest prefix")
    }

    pub fn remove_shortest_prefix(&self, s: String) -> String {
        assert!(
            !s.as_bytes().contains(&b'\0'),
            "trying to match a string containing null"
        );
        let mut bytes = s.into_bytes();
        bytes.push(b'\0');
        let mut prefix_end = 0;
        for i in 1..bytes.len() - 1 {
            let t = bytes[i];
            bytes[i] = b'\0';
            // we know there is a null, so this unwrap will never fail
            if self
                .regex
                .matches(CStr::from_bytes_until_nul(&bytes).unwrap())
            {
                prefix_end = i;
                bytes[i] = t;
                break;
            }
            bytes[i] = t;
        }
        // remove '\0'
        bytes.pop();
        bytes.drain(..prefix_end);
        String::from_utf8(bytes).expect("failed to create string after removing shortest prefix")
    }

    pub fn remove_largest_suffix(&self, s: String) -> String {
        let cstring = CString::new(s).expect("trying to match a string containing null");
        let len = cstring.as_bytes().len();
        let mut suffix_start = len - 1;
        for regex_match in self.regex.match_locations(&cstring) {
            if regex_match.end == len {
                suffix_start = regex_match.start;
                break;
            }
        }
        let mut bytes = cstring.into_bytes();
        bytes.drain(suffix_start..);
        String::from_utf8(bytes).expect("failed to create string after removing largest suffix")
    }

    pub fn remove_shortest_suffix(&self, s: String) -> String {
        assert!(
            !s.as_bytes().contains(&b'\0'),
            "trying to match a string containing null"
        );
        let mut bytes = s.into_bytes();
        bytes.push(b'\0');
        let mut suffix_start = bytes.len();
        for i in (1..bytes.len() - 2).rev() {
            // we know there is a null, so this unwrap will never fail
            if self
                .regex
                .matches(CStr::from_bytes_until_nul(&bytes[i..]).unwrap())
            {
                suffix_start = i;
                break;
            }
        }
        // remove terminating '\0'
        bytes.pop();
        bytes.drain(suffix_start..);
        String::from_utf8(bytes).expect("failed to create string after removing shortest suffix")
    }
}

impl From<Pattern> for String {
    fn from(value: Pattern) -> Self {
        value.pattern_string
    }
}

pub struct FilenamePattern {
    path_parts: Vec<Regex>,
    pattern_string: String,
}

impl FilenamePattern {
    pub fn new(word: &ExpandedWord) -> Result<Self, String> {
        let parsed_pattern = parse_pattern(word, true)?;
        let mut path_parts = Vec::new();

        parsed_pattern
            .split(|item| *item == PatternItem::Char('/'))
            .filter(|items| !items.is_empty())
            .try_for_each(|items| {
                path_parts.push(parsed_pattern_to_regex(items)?);
                Ok::<(), String>(())
            })?;

        Ok(Self {
            path_parts,
            pattern_string: word.to_string(),
        })
    }

    /// # Panics
    /// panics if `depth` is smaller than 1 or bigger than `component_count`
    pub fn matches(&self, depth: usize, s: &CStr) -> bool {
        assert!(
            depth > 0 && depth <= self.component_count(),
            "invalid depth"
        );
        let component_index = depth - 1;
        if component_index == 0 && s.to_bytes()[0] == b'.' && !self.pattern_string.starts_with('.')
        {
            // dot at the start is only matched explicitly
            return false;
        }
        self.path_parts[component_index].matches(s)
    }

    /// Returns number of components in the path
    /// If it returns 0 then the pattern is just a directory (root if it starts
    /// with '/', the current directory otherwise)
    pub fn component_count(&self) -> usize {
        self.path_parts.len()
    }

    pub fn is_absolute(&self) -> bool {
        self.pattern_string.starts_with('/')
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::interpreter::ExpandedWordPart;

    pub fn pattern_from_str(pat: &str) -> Pattern {
        Pattern::new(&ExpandedWord {
            parts: vec![ExpandedWordPart::UnquotedLiteral(pat.to_string())],
        })
        .expect("failed to create pattern")
    }

    pub fn filename_pattern_from_str(pat: &str) -> FilenamePattern {
        FilenamePattern::new(&ExpandedWord {
            parts: vec![ExpandedWordPart::UnquotedLiteral(pat.to_string())],
        })
        .expect("failed to create filename pattern")
    }

    fn cstring_from_str(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    #[test]
    fn remove_largest_prefix() {
        assert_eq!(
            pattern_from_str("*b").remove_largest_prefix("abaaaaabtest".to_string()),
            "test"
        )
    }

    #[test]
    fn remove_smallest_prefix() {
        assert_eq!(
            pattern_from_str("*b").remove_shortest_prefix("abaaaaabtest".to_string()),
            "aaaaabtest"
        )
    }

    #[test]
    fn remove_largest_suffix() {
        assert_eq!(
            pattern_from_str("b*").remove_largest_suffix("testbaaaaaba".to_string()),
            "test"
        )
    }

    #[test]
    fn remove_smallest_suffix() {
        assert_eq!(
            pattern_from_str("b*").remove_shortest_suffix("testbaaaaaba".to_string()),
            "testbaaaaa"
        )
    }

    #[test]
    fn filename_pattern_matches_simple_components_in_path() {
        let pattern = filename_pattern_from_str("/path/to/file");
        assert!(pattern.matches(1, &cstring_from_str("path")));
        assert!(pattern.matches(2, &cstring_from_str("to")));
        assert!(pattern.matches(3, &cstring_from_str("file")));
    }

    #[test]
    fn period_at_the_start_is_only_matched_explicitly() {
        let pattern = filename_pattern_from_str("*test");
        assert!(!pattern.matches(1, &cstring_from_str(".test")));
        assert!(pattern.matches(1, &cstring_from_str("atest")));

        let pattern = filename_pattern_from_str(".test");
        assert!(pattern.matches(1, &cstring_from_str(".test")));
    }

    #[test]
    fn period_at_the_start_is_not_matched_by_bracket_expression_with_multiple_chars() {
        // the standard leaves this case to the implementation, here we follow what bash does
        let pattern = filename_pattern_from_str("[.abc]*");
        assert!(!pattern.matches(1, &cstring_from_str(".a")));
    }
}
