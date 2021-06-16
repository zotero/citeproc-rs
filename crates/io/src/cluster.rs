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
/// , { "id": 2, "cites": [{ "id": "smith" }], "mode": "author-only" }
/// , { "id": 3, "cites": [{ "id": "smith" }, { "id": "jones" }],
///     "mode": "suppress-author", "suppressMax": 1 }
/// ]"#;
/// let clusters: Vec<Cluster> = serde_json::from_str(json).unwrap();
/// assert_eq!(clusters, vec![
///     Cluster { id: 1, cites: vec![Cite::basic("smith")], mode: None, },
///     Cluster { id: 2, cites: vec![Cite::basic("smith")], mode: Some(ClusterMode::AuthorOnly), },
///     Cluster { id: 3, cites: vec![Cite::basic("smith"), Cite::basic("jones")],
///               mode: Some(ClusterMode::SuppressAuthor { suppress_max: 1 }), },
/// ])
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(tag = "mode")]
#[serde(rename_all = "kebab-case")]
// #[serde(untagged)]
pub enum ClusterMode {
    /// For author-in-text, or whatever the style author wants to put inline.
    ///
    /// E.g. the author, or party names for a legal case.
    AuthorOnly,
    /// E.g. the cite with the author suppressed, or a legal case without party names.
    #[serde(rename_all = "camelCase")]
    SuppressAuthor {
        /// Suppress authors in the first `n` cites in the cluster. If this is absent or zero, then
        /// all cites have their authors suppressed.
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
        ///         suppress_max: 1,
        ///     },
        /// };
        /// ```
        ///
        /// In the note_after cluster, the @smith reference won't have an author, but @jones will.
        ///
        /// > Smith et al in their paper[^1]
        ///
        /// [^1]: 'A paper', 1968; Jones et al. 'A different paper', 1993.
        #[serde(default)]
        suppress_max: u32,
    },
    /// Render `AuthorOnly` + infix + `SuppressAuthor`. Infix is given leading spaces automatically, if there is
    /// no leading punctuation (`'s Magic Castle` does not attract a leading space). The default
    /// for Infix is a single space.
    Composite { infix: Option<String> },
}

/// [Special Citation Forms](https://citeproc-js.readthedocs.io/en/latest/running.html#special-citation-forms)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClusterSplit {
    /// Produced with [ClusterMode::AuthorOnly]
    AuthorOnly(String),
    /// Produced by [ClusterMode::SuppressAuthor]
    ///
    /// `ClusterMode::SuppressAuthor`
    SuppressAuthor(String),
    /// Produced by [ClusterMode::Composite]
    ///
    /// `ClusterMode::Composite { infix: Some("’s early work".into()) }`
    ///
    /// > Kesey’s early work (1962, 1964; cf. <i>Le Guin</i> 1969)
    ///
    Composite(String),
}
