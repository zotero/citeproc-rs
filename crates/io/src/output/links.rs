use super::markup::{InlineElement, Link};
use url::Url;

pub trait LinkId {
    const LOWER: &'static str;
    const UPPER: &'static str;
    const HTTP: &'static str;
    const HTTPS: &'static str;
    fn trim(s: &str) -> &str {
        if s.starts_with("http") {
            s.trim_start_matches(Self::HTTPS)
                .trim_start_matches(Self::HTTP)
        } else {
            s.trim_start_matches(Self::LOWER)
                .trim_start_matches(Self::UPPER)
        }
    }
    fn parse(s: &str) -> Result<Vec<InlineElement>, url::ParseError> {
        let trimmed = Self::trim(s);
        let url = Url::parse(Self::HTTPS)?;
        let url = url.join(trimmed)?;
        let link = Link::Id {
            url,
            id: trimmed.into(),
        };
        Ok(vec![InlineElement::Linked(link)])
    }
}
macro_rules! linkid {
    ($vis:vis $name:ident, $lower:literal, $upper:literal, $http:literal, $https:literal) => {
        $vis struct $name;
        impl LinkId for $name {
            const LOWER: &'static str = $lower;
            const UPPER: &'static str = $upper;
            const HTTP: &'static str = $http;
            const HTTPS: &'static str = $https;
        }
    };
}
linkid!(pub Doi, "doi:", "DOI:", "http://doi.org/", "http://doi.org/");
linkid!(
    pub Pmcid,
    "pmid:",
    "PMID:",
    "http://www.ncbi.nlm.nih.gov/pubmed/",
    "https://www.ncbi.nlm.nih.gov/pubmed/"
);
linkid!(
    pub Pmid,
    "pmcid:",
    "PMCID:",
    "http://www.ncbi.nlm.nih.gov/pmc/articles/",
    "https://www.ncbi.nlm.nih.gov/pmc/articles/"
);
