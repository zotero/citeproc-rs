use crate::String;

/// See [Special Citation Forms](https://citeproc-js.readthedocs.io/en/latest/running.html#special-citation-forms)
///
///
/// ```
/// use serde::Deserialize;
/// use citeproc_io::{Cite, ClusterMode, output::markup::Markup};
/// #[derive(Deserialize, Debug, Clone, PartialEq)]
/// pub struct Cluster {
///     pub id: u32,
///     pub cites: Vec<Cite<Markup>>,
///     #[serde(flatten)]
///     #[serde(default, skip_serializing_if = "Option::is_none")]
///     pub mode: Option<ClusterMode>,
/// }
/// let json = r#"
/// [ { "id": 1, "cites": [{ "id": "smith" }] }
/// , { "id": 2, "cites": [{ "id": "smith" }], "mode": "AuthorOnly" }
/// , { "id": 3, "cites": [{ "id": "smith" }, { "id": "jones" }],
///     "mode": "SuppressAuthor", "suppressFirst": 1 }
/// ]"#;
/// let clusters: Vec<Cluster> = serde_json::from_str(json).unwrap();
/// assert_eq!(clusters, vec![
///     Cluster { id: 1, cites: vec![Cite::basic("smith")], mode: None, },
///     Cluster { id: 2, cites: vec![Cite::basic("smith")], mode: Some(ClusterMode::AuthorOnly), },
///     Cluster { id: 3, cites: vec![Cite::basic("smith"), Cite::basic("jones")],
///               mode: Some(ClusterMode::SuppressAuthor { suppress_first: 1 }), },
/// ])
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(tag = "mode")]
pub enum ClusterMode {
    /// For author-in-text, or whatever the style author wants to put inline.
    ///
    /// E.g. the author, or party names for a legal case.
    AuthorOnly,
    /// E.g. the cite with the author suppressed, or a legal case without party names.
    #[serde(rename_all = "camelCase")]
    SuppressAuthor {
        /// Suppress authors in the first `n` cites in the cluster, or if cite grouping is enabled,
        /// the first `n` same-author groups. The default value is 1. If this is zero, then all
        /// cites have their authors suppressed.
        ///
        /// ```ignore
        /// // imagine @refid is a Cite to a reference with id 'refid'
        /// let in_text = Cluster {
        ///     id: 1,
        ///     cites: vec![Cite::basic("smith")],
        ///     mode: ClusterMode::AuthorOnly,
        /// };
        /// let note_after = Cluster {
        ///     id: 2,
        ///     cites: vec![Cite::basic("smith"), Cite::basic("jones")],
        ///     mode: ClusterMode::SuppressAuthor {
        ///         suppress_first: 1,
        ///     },
        /// };
        /// ```
        ///
        /// In the note_after cluster, the @smith reference won't have an author, but @jones will.
        ///
        /// > Smith et al in their paper[^1]
        ///
        /// [^1]: 'A paper', 1968; Jones et al. 'A different paper', 1993.
        #[serde(default = "default_one")]
        suppress_first: u32,
    },
    /// Render `AuthorOnly` + infix + `SuppressAuthor`. Infix is given leading spaces automatically, if there is
    /// no leading punctuation (`'s Magic Castle` does not attract a leading space). The default
    /// for Infix is a single space.
    #[serde(rename_all = "camelCase")]
    Composite {
        infix: Option<String>,
        /// This has the same effect and same default (1) as `ClusterMode::SuppressAuthor {
        /// suppress_first }`. The number of prepended author-only representations is equal to the
        /// number of cites whose author is suppressed in the main part of the rendered cluster.
        ///
        /// For a cluster normally rendered as:
        ///
        /// > (Author1 1996, 1997; Author2 1999; Author3 2009)
        ///
        /// with `ClusterMode::Composite { infix: ", infix".into(), suppress_first: 2 }`:
        ///
        /// > Author1; Author2, infix (1996, 1997; 1999; Author3 2009)
        #[serde(default = "default_one")]
        suppress_first: u32,
    },
}

fn default_one() -> u32 {
    1
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
#[serde(remote = "ClusterMode", tag = "mode")]
pub enum CompatClusterMode {
    AuthorOnly,
    #[serde(rename_all = "kebab-case")]
    SuppressAuthor {
        #[serde(default = "default_one")]
        suppress_first: u32,
    },
    #[serde(rename_all = "kebab-case")]
    Composite {
        infix: Option<String>,
        #[serde(default = "default_one")]
        suppress_first: u32,
    },
}

use serde::Deserialize;

impl ClusterMode {
    pub fn compat_opt<'de, D>(d: D) -> Result<Option<ClusterMode>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper(#[serde(with = "CompatClusterMode")] ClusterMode);
        Option::<Helper>::deserialize(d).map(|x| x.map(|Helper(y)| y))
    }
}
