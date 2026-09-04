use crate::error::Result;
use crate::regex::Regex;

pub struct Filter(Option<Regex>);

impl Filter {
    pub fn new(pattern: &Option<String>) -> Result<Filter> {
        Ok(Filter(match pattern {
            Some(p) => Some(Regex::new(p)?),
            None => None,
        }))
    }

    pub fn matches(&self, path: &str) -> bool {
        match &self.0 {
            Some(re) => re.is_match(path),
            None => true,
        }
    }
}
