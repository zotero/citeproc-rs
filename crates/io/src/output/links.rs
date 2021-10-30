use crate::String;
use csl::{Affixes, Variable};
use url::Url;

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub enum Link {
    /// handles a full valid url only
    Url { url: Url, trailing_slash: bool },
    /// e.g. a DOI that only puts the full url in a link.
    /// The url is an optional addition, if we are rendering anchors.
    Id { url: Url, id: String },
    // TODO: allow internal linking (e.g. first-reference-note-number)
    // Href(String),
}

impl Link {
    fn url(url: Url, orig: &str) -> Self {
        Self::Url {
            url,
            trailing_slash: orig.ends_with("/"),
        }
    }
}

fn trim_affixes(affixes: &Affixes, trim_end_https: fn(&str) -> Option<&str>) -> Option<Affixes> {
    let prefix = &affixes.prefix[..];
    let trimmed = trim_end_https(prefix);
    trimmed.map(|p| Affixes {
        prefix: p.into(),
        suffix: affixes.suffix.clone(),
    })
}

/// Returns a parsed link, and (if necessary) rewritten Affixes with e.g. https://doi.org/ removed
///
/// The affixes returned will be None if it was not necessary to rewrite them.
pub fn try_link_affixed(
    var: Variable,
    value: &str,
    affixes: Option<&Affixes>,
) -> Result<Option<(Link, Option<Affixes>)>, url::ParseError> {
    match var {
        Variable::DOI => Doi::parse(value, affixes).map(Some),
        Variable::PMID => Pmid::parse(value, affixes).map(Some),
        Variable::PMCID => Pmcid::parse(value, affixes).map(Some),
        Variable::URL => Url::parse(value).map(|url| Some((Link::url(url, value), None))),
        _ => return Ok(None),
    }
}

/// Same as [try_link_affixed], but logs any url parsing error.
pub fn try_link_affixed_opt(
    var: Variable,
    value: &str,
    affixes: Option<&Affixes>,
) -> Option<(Link, Option<Affixes>)> {
    match try_link_affixed(var, value, affixes) {
        Ok(pair) => pair,
        Err(e) => {
            warn!("invalid url due to {}: {}", e, value);
            None
        }
    }
}

trait LinkId {
    const LOWER: &'static str;
    const UPPER: &'static str;
    const CANONICAL_HTTPS: &'static str;
    fn trim_start(s: &str) -> &str;
    fn trim_end_https(s: &str) -> Option<&str>;
    fn parse(
        s: &str,
        affixes: Option<&Affixes>,
    ) -> Result<(Link, Option<Affixes>), url::ParseError> {
        let trimmed_id = Self::trim_start(s);
        let url = Url::parse(Self::CANONICAL_HTTPS)?;
        let url = url.join(trimmed_id)?;

        // If we do strip `https://...` out of the affixes, then something like this would break if
        // we turned off link_anchors:
        //
        //   <text prefix="&lt;https://doi.org/" variable="doi" suffix="&gt;" />
        //
        // So we should rewrite the id as well, to carry the prefix all the way to write_url.
        //
        // In a very technical sense, this is incorrect as the prefix should not normally receive
        // the formatting that the variable content does; however, in this case, the intention is
        // clear as we only ever add `https://....`. This will also convert any use of HTTP by
        // *styles* to CANONICAL_HTTPS.
        let overridden = affixes
            .map(|a| trim_affixes(a, Self::trim_end_https))
            .flatten();
        let id = if overridden.is_some() {
            let mut id = String::new();
            id.push_str(Self::CANONICAL_HTTPS);
            id.push_str(trimmed_id);
            id
        } else {
            trimmed_id.into()
        };
        Ok((Link::Id { url, id }, overridden))
    }
}

macro_rules! linkid {
    (
        $vis:vis $name:ident,
        LOWER = $lower:literal,
        UPPER = $upper:literal,
        CANONICAL_HTTPS = $https:literal,
        OTHER_HTTP = [$($http:literal,)*]
    ) => {
        $vis struct $name;
        impl LinkId for $name {
            const LOWER: &'static str = $lower;
            const UPPER: &'static str = $upper;
            const CANONICAL_HTTPS: &'static str = $https;

            fn trim_start(s: &str) -> &str {
                // at most, strip one of these, once
                s.strip_prefix(Self::CANONICAL_HTTPS)
                    .or(s.strip_prefix(Self::LOWER))
                    .or(s.strip_prefix(Self::UPPER))
                    $(.or(s.strip_prefix($http)))*
                    .unwrap_or(s)
            }

            fn trim_end_https(s: &str) -> Option<&str> {
                s.strip_suffix(Self::CANONICAL_HTTPS)
                $(.or(s.strip_suffix($http)))*
            }
        }
    };
}

linkid!(
    pub Doi,
    LOWER = "doi:",
    UPPER =  "DOI:",
    CANONICAL_HTTPS = "https://doi.org/",
    OTHER_HTTP = [
        "http://doi.org/",
    ]
);

linkid!(
    pub Pmid,
    LOWER = "pmid:",
    UPPER = "PMID:",
    CANONICAL_HTTPS = "https://www.ncbi.nlm.nih.gov/pubmed/",
    OTHER_HTTP = [
        "http://www.ncbi.nlm.nih.gov/pubmed/",
        // These days all your pmid links get redirected to links like these
        // So probably at some point we should update the CANONICAL_HTTPS link
        "http://pubmed.ncbi.nlm.nih.gov/",
        "https://pubmed.ncbi.nlm.nih.gov/",
    ]
);

linkid!(
    pub Pmcid,
    LOWER = "pmcid:",
    UPPER = "PMCID:",
    CANONICAL_HTTPS = "https://www.ncbi.nlm.nih.gov/pmc/articles/",
    OTHER_HTTP = [
        "http://www.ncbi.nlm.nih.gov/pmc/articles/",
        // in case you're using PMC Labs
        "https://www.ncbi.nlm.nih.gov/labs/pmc/articles/",
        "http://www.ncbi.nlm.nih.gov/labs/pmc/articles/",
    ]
);
