use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

pub trait TypescriptSerialize {
    type RustType: Serialize;
}

pub trait JsonValue: serde::Serialize {
    fn serialize_jsvalue<R: From<JsValue> + TypescriptSerialize<RustType = Self>>(
        &self,
    ) -> Result<R, crate::Error> {
        let jsvalue = JsValue::from_serde(self)?;
        Ok(jsvalue.into())
    }
}

impl<T> JsonValue for T where T: serde::Serialize {}

pub trait TypescriptDeserialize: Into<JsValue> {
    type RustType: DeserializeOwned;

    fn ts_deserialize(self) -> serde_json::Result<Self::RustType> {
        let jsv: JsValue = self.into();
        let rust: Self::RustType = jsv.into_serde()?;
        Ok(rust)
    }
}

macro_rules! typescript_serialize {
    ($ty:ty, $name:ident, $stringified:literal, $definition:literal) => {
        typescript_serialize!(@rust $ty, $name, $stringified);
        typescript_serialize!(@typescript $definition);
    };
    ($ty:ty, $name:ident, $stringified:literal) => {
        typescript_serialize!(@rust $ty, $name, $stringified);
    };
    (@rust $ty:ty, $name:ident, $stringified:literal) => {
        #[wasm_bindgen]
        extern "C" {
            #[wasm_bindgen(typescript_type = $stringified)]
            pub type $name;
        }
        impl TypescriptSerialize for $name {
            type RustType = $ty;
        }
    };
    (@typescript $definition:literal) => {
        #[wasm_bindgen(typescript_custom_section)]
        const TS_APPEND_CONTENT_5: &'static str = $definition;
    };
}

macro_rules! typescript_deserialize {
    ($ty:ty, $name:ident, $stringified:literal, $definition:literal) => {
        typescript_deserialize!(@rust $ty, $name, $stringified);
        typescript_deserialize!(@typescript $definition);
    };
    ($ty:ty, $name:ident, $stringified:literal) => {
        typescript_deserialize!(@rust $ty, $name, $stringified);
    };
    (@rust $ty:ty, $name:ident, $stringified:literal) => {
        #[wasm_bindgen]
        extern "C" {
            #[wasm_bindgen(typescript_type = $stringified)]
            pub type $name;
        }
        impl TypescriptDeserialize for $name {
            type RustType = $ty;
        }
    };
    (@typescript $definition:literal) => {
        #[wasm_bindgen(typescript_custom_section)]
        const TS_APPEND_CONTENT_5: &'static str = $definition;
    };
}

typescript_deserialize!(crate::options::WasmInitOptions, InitOptions, "InitOptions");
typescript_deserialize!(
    crate::options::FormatOptionsArg,
    FormatOptions,
    "FormatOptions"
);

// TODO: include note about free()-ing the Driver before an async fetchLocale() call comes back (in
// which case the Driver reference held to by the promise handler function is now a dangling
// wasm-bindgen pointer).
#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT_1: &'static str = r#"
interface FormatOptions {
    linkAnchors?: boolean;
}

interface InitOptions {
    /** A CSL style as an XML string */
    style: string;

    /** A Fetcher implementation for fetching locales.
      *
      * If not provided, then no locales can be fetched, and default-locale and localeOverride will
      * not be respected; the only locale used will be the bundled en-US. */
    fetcher?: Fetcher;

    /** The output format for this driver instance (default: html) */
    format?: "html" | "rtf" | "plain";
    /** Configuration for the formatter */
    formatOptions?: FormatOptions;

    /** Optional array of CSL feature names to activate globally. Features are kebab-case. */
    cslFeatures?: string[];

    /** A locale to use instead of the style's default-locale.
      *
      * For dependent styles, use parseStyleMetadata to find out which locale it prefers, and pass
      * in the parent style with a localeOverride set to that value.
      */
    localeOverride?: string;

    /** Disables sorting in the bibliography; items appear in cited order. */
    bibliographyNoSort?: boolean;
}

/** This interface lets citeproc retrieve locales or modules asynchronously,
    according to which ones are needed. */
export interface Fetcher {
    /** Return locale XML for a particular locale. */
    fetchLocale(lang: string): Promise<string>;
}
"#;

typescript_serialize!(
    citeproc::string_id::UpdateSummary,
    UpdateSummary,
    "UpdateSummary",
    r#"
interface BibliographyUpdate {
    updatedEntries: Map<string, string>;
    entryIds?: string[];
}

type UpdateSummary<Output = string> = {
    clusters: [string, Output][];
    bibliography?: BibliographyUpdate;
};
"#
);
typescript_serialize!(
    Vec<citeproc::BibEntry>,
    BibEntries,
    "BibEntry[]",
    r#"
interface BibEntry {
    id: string;
    value: string;
}
"#
);
typescript_serialize!(
    citeproc::string_id::FullRender,
    FullRender,
    "FullRender",
    r#"
interface FullRender {
    allClusters: Map<string, string>;
    bibEntries: BibEntry[];
}
"#
);
typescript_serialize!(
    Option<citeproc::BibliographyMeta>,
    BibliographyMeta,
    "BibliographyMeta",
    r#"
interface BibliographyMeta {
    maxOffset: number;
    entrySpacing: number;
    lineSpacing: number;
    hangingIndent: boolean;
    /** the second-field-align value of the CSL style */
    secondFieldAlign: null  | "flush" | "margin";
    /** Format-specific metadata */
    formatMeta: any;
}
"#
);
typescript_serialize!(Vec<String>, StringArray, "string[]");

typescript_serialize!(
    csl::StyleMeta,
    StyleMeta,
    "StyleMeta",
    r#"
interface StyleMeta {
    info: StyleInfo;
    features: { [feature: string]: boolean };
    defaultLocale: string;
    /** May be absent on a dependent style */
    class?: "in-text" | "note";
    cslVersionRequired: string;
    /** May be absent on a dependent style */
    independentMeta?: IndependentMeta;
}
type CitationFormat = "author-date" | "author" | "numeric" | "label" | "note";
interface LocalizedString {
    value: string;
    lang?: string;
}
interface ParentLink {
    href: string;
    lang?: string;
}
interface Link {
    href: string;
    rel: "self" | "documentation" | "template";
    lang?: string;
}
interface Rights {
    value: string;
    lang?: string;
    license?: string;
}
interface StyleInfo {
    id: string;
    updated: string;
    title: LocalizedString;
    titleShort?: LocalizedString;
    parent?: ParentLink;
    links: Link[];
    rights?: Rights;
    citationFormat?: CitationFormat;
    categories: string[];
    issn?: string;
    eissn?: string;
    issnl?: string;
}
interface IndependentMeta {
    /** A list of languages for which a locale override was specified.
      * Does not include the language-less final override. */
    localeOverrides: string[];
    hasBibliography: boolean;
}
"#
);

typescript_deserialize!(citeproc::string_id::Cluster, Cluster, "Cluster");

typescript_deserialize!(citeproc::PreviewCluster, PreviewCluster, "PreviewCluster");

#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT_3: &'static str = r#"
/** Locator type, and a locator string */
export type Locator = {
    label?: string;
    locator?: string;
    locators: undefined;
};

export type CiteLocator = Locator | { locator: undefined; locators: Locator[]; };
export type CiteMode = { mode?: "SuppressAuthor" | "AuthorOnly"; };

export type Cite = {
    id: string;
    prefix?: string;
    suffix?: string;
} & Partial<CiteLocator> & CiteMode;

export type ClusterMode
    = { mode: "Composite"; infix?: string; suppressFirst?: number; } 
    | { mode: "SuppressAuthor"; suppressFirst?: number; }
    | { mode: "AuthorOnly"; }
    | {};

export type Cluster = {
    id: string;
    cites: Cite[];
} & ClusterMode;

export type PreviewCluster = {
    cites: Cite[];
} & ClusterMode;

export type ClusterPosition = {
    id: string;
    /** Leaving off this field means this cluster is in-text. */
    note?: number;
}
"#;

typescript_deserialize!(
    citeproc_io::Reference,
    Reference,
    "Reference",
    r#"
type Reference = {
    id: string;
    type: CslType;
    [key: string]: any;
};
export type CslType = "book" | "legal_case" | "article-journal" | string;
"#
);

typescript_serialize!(
    citeproc::IncludeUncited,
    IncludeUncited,
    "IncludeUncited",
    r#"
    type IncludeUncited = "None" | "All" | { Specific: string[] };
"#
);

// Some misc date objects, mostly made redundant by CSL 1.1 EDTF
#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT_2: &'static str = r#"
type DateLiteral = { "literal": string; };
type DateRaw = { "raw": string; };
type DatePartsDate = [number] | [number, number] | [number, number, number];
type DatePartsSingle = { "date-parts": [DatePartsDate]; };
type DatePartsRange = { "date-parts": [DatePartsDate, DatePartsDate]; };
type DateParts = DatePartsSingle | DatePartsRange;
type DateOrRange = DateLiteral | DateRaw | DateParts;
"#;
