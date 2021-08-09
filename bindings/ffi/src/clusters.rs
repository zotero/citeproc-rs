use libc::{c_char, c_void};

use citeproc::prelude as rust;
use rust::{Atom, Markup, SmartString};

use crate::buffer::BufferWriter;
use crate::macros::nullify_on_panic;
use crate::util::*;
use crate::{Driver, ErrorCode, FFIError, U32OrError};

/// A number identifying a cluster.
pub type ClusterId = u32;

/// An opaque, boxed wrapper for a [citeproc::prelude::Cluster].
pub struct Cluster(rust::Cluster);

#[cfg(doc)]
use super::*;

ffi_fn_nullify! {
    /// Creates a new cluster with the given cluster id. Free with [citeproc_rs_cluster_free].
    fn citeproc_rs_cluster_new(id: ClusterId) -> *mut Cluster {
        let boxed = Box::new(Cluster(rust::Cluster {
            id: rust::ClusterId(id),
            cites: Vec::new(),
            mode: None,
        }));
        Box::into_raw(boxed)
    }
}

ffi_fn_nullify! {
    /// Deallocates a cluster.
    ///
    /// # Safety
    ///
    /// The cluster must be from [citeproc_rs_cluster_new] and not freed.
    @safety unsafe fn citeproc_rs_cluster_free(cluster: *mut Cluster) -> ErrorCode {
        result_to_error_code(|| {
            if cluster.is_null() {
                return Err(FFIError::NullPointer);
            }
            let _ = unsafe { Box::from_raw(cluster) };
            Ok(ErrorCode::None)
        })
    }
}

ffi_fn_nullify! {
    /// Removes all data and sets a new id on the cluster.
    ///
    /// # Safety
    ///
    /// The cluster must be from [citeproc_rs_cluster_new] and not freed.
    @safety unsafe fn citeproc_rs_cluster_reset(cluster: *mut Cluster, new_id: ClusterId) -> ErrorCode {
        result_to_error_code(|| {
            let cluster = unsafe { borrow_raw_ptr_mut(cluster) } ?;
            cluster.0.id = rust::ClusterId(new_id);
            cluster.0.cites.clear();
            cluster.0.mode = None;
            Ok(ErrorCode::None)
        })
    }
}

ffi_fn_nullify! {
    /// Sets the id of the given cluster object.
    ///
    /// # Safety
    ///
    /// The cluster must be from [citeproc_rs_cluster_new] and not freed.
    @safety unsafe fn citeproc_rs_cluster_set_id(cluster: *mut Cluster, id: ClusterId) -> ErrorCode {
        result_to_error_code(|| {
            let boxed = unsafe { borrow_raw_ptr_mut(cluster) }?;
            boxed.0.id = rust::ClusterId(id);
            Ok(ErrorCode::None)
        })
    }
}

unsafe fn with_cite_mut<T, F>(
    cluster: *mut Cluster,
    cite_index: usize,
    mut f: F,
) -> Result<T, FFIError>
where
    F: FnMut(&mut rust::Cite<Markup>) -> Result<T, FFIError>,
{
    let cluster = borrow_raw_ptr_mut(cluster)?;
    let len = cluster.0.cites.len();
    let cite = cluster
        .0
        .cites
        .get_mut(cite_index)
        .ok_or_else(|| FFIError::Indexing {
            index: cite_index,
            len,
        })?;
    f(cite)
}

ffi_fn_nullify! {
    /// Sets the reference id for a cite.
    ///
    /// # Safety
    ///
    /// The cluster must be from [citeproc_rs_cluster_new] and not freed.
    ///
    /// Either `ref_id` must refer to a byte array of length `ref_id_len`, or `ref_id_len` must be zero.
    @safety unsafe fn citeproc_rs_cluster_cite_set_ref(cluster: *mut Cluster, cite_index: usize, ref_id: *const c_char, ref_id_len: usize) -> ErrorCode {
        result_to_error_code(|| unsafe {
            with_cite_mut(cluster, cite_index, |cite| {
                let ref_id = Atom::from(borrow_utf8_slice(ref_id, ref_id_len)?);
                cite.ref_id = ref_id;
                Ok(ErrorCode::None)
            })
        })
    }
}

ffi_fn_nullify! {
    /// Returns either an index (>=0) representing the position of a newly created cite within a
    /// cluster, or a negative error code.
    ///
    /// # Safety
    ///
    /// The cluster must be from [citeproc_rs_cluster_new] and not freed.
    /// The ref_id pair must refer to a valid byte array.
    @safety unsafe fn citeproc_rs_cluster_cite_new(cluster: *mut Cluster, ref_id: *const c_char, ref_id_len: usize) -> U32OrError {
        result_to_error_code(|| {
            let cluster = unsafe { borrow_raw_ptr_mut(cluster) } ?;
            let ref_id = unsafe { borrow_utf8_slice(ref_id, ref_id_len) }?;
            let cite = rust::Cite::basic(ref_id);
            cluster.0.cites.push(cite);
            Ok(U32OrError(cluster.0.cites.len() as i64 - 1))
        })
    }
}

#[repr(C)]
pub enum ClusterMode {
    Normal,
    AuthorOnly,
    SuppressAuthor { suppress_first: u32 },
    Composite,
}

#[repr(C)]
pub enum CiteMode {
    Normal,
    AuthorOnly,
    SuppressAuthor,
}

ffi_fn_nullify! {
    /// Interns a cluster id. Returns -1 on error, hence the i64 return type; [ClusterId] is
    /// actually a u32, so you can cast it safely after checking for -1.
    ///
    /// Returns an error code indicative of what the LAST_ERROR will contain when checked.
    ///
    /// # Safety
    ///
    /// Either `str` must refer to a byte array of length `str_len`, or `str_len` must be zero.
    @safety unsafe fn citeproc_rs_driver_intern_cluster_id(#[nullify_on_panic] driver: *mut Driver, str: *const c_char, str_len: usize) -> U32OrError {
        result_to_error_code(|| {
            let driver = unsafe { borrow_raw_ptr_mut(driver) } ?;
            let proc = driver.processor.as_mut().ok_or(FFIError::Poisoned)?;
            let slice = unsafe { borrow_utf8_slice(str, str_len) } ?;
            let id = proc.cluster_id(slice);
            Ok(U32OrError(id.0 as i64))
        })
    }
}

ffi_fn_nullify! {
    /// Writes a random cluster_id into user_buf, and returns a ClusterId that represents it.
    /// [citeproc::Processor::random_cluster_id]
    ///
    /// Useful for allocating string ids to citation clusters in a real document, that need to be
    /// read back later.
    fn citeproc_rs_driver_random_cluster_id(#[nullify_on_panic] driver: *mut Driver, user_buf: *mut c_void) -> U32OrError {
        result_to_error_code(|| {
            let driver = unsafe { borrow_raw_ptr_mut(driver) } ?;
            let mut proc = driver.processor.as_mut().ok_or(FFIError::Poisoned)?;
            let string = proc.random_cluster_id_str();
            let mut buffer = unsafe { BufferWriter::new(driver.buffer_ops, user_buf) };
            let id = proc.cluster_id(&string);
            buffer.clear();
            buffer.write_str(&string)?;
            Ok(U32OrError(id.0 as i64))
        })
    }
}

ffi_fn_nullify! {
    /// Inserts a cluster, overwriting any previously written cluster with that ID.
    /// [citeproc::Processor::insert_cluster]
    ///
    /// # Safety
    ///
    /// Driver must be from [citeproc_rs_driver_new]. The cluster must be from
    /// [citeproc_rs_cluster_new].
    @safety unsafe fn citeproc_rs_driver_insert_cluster(#[nullify_on_panic] driver: *mut Driver, cluster: *const Cluster) -> ErrorCode {
        result_to_error_code(|| {
            let driver = unsafe { borrow_raw_ptr_mut(driver) } ?;
            let proc = driver.processor.as_mut().ok_or(FFIError::Poisoned)?;
            let cluster = unsafe { borrow_raw_ptr(cluster) } ?;
            let cloned = cluster.0.clone();
            proc.insert_cluster(cloned);
            Ok(ErrorCode::None)
        })
    }
}

macro_rules! enum_redef {
    (
        $(#[$attr:meta])*
        $vis:vis enum $name:ident = $original:path {
            $(
                $(#[$vattr:meta])*
                $variant:ident,
            )*
        }
    ) => {
        $(#[$attr])*
        #[derive(Debug)]
        $vis enum $name {
            $(
                $(#[$vattr])*
                $variant,
            )*
        }

        impl From<$original> for $name {
            fn from(orig: $original) -> Self {
                match orig {
                    $(
                        <$original>::$variant => $name::$variant,
                     )*
                    _ => panic!("non-exhaustive enum not matched"),
                }
            }
        }
        impl $name {
            fn into_original(self) -> $original {
                match self {
                    $(
                        Self::$variant => <$original>::$variant,
                    )*
                    // _ => panic!("non-exhaustive enum not matched"),
                }
            }
        }
    };
}

enum_redef! {
    #[derive(Clone, Copy)]
    #[repr(u32)]
    pub enum LocatorType = csl::LocatorType {
        Book,
        Chapter,
        Column,
        Figure,
        Folio,
        Issue,
        Line,
        Note,
        Opus,
        Page,
        Paragraph,
        Part,
        Section,
        SubVerbo,
        Verse,
        Volume,
        Article,
        Subparagraph,
        Rule,
        Subsection,
        Schedule,
        Title,
        Unpublished,
        Supplement,
    }
}

ffi_fn_nullify! {
    /// Sets the string locator and [LocatorType] for a cite.
    @safety unsafe fn citeproc_rs_cluster_cite_set_locator(cluster: *mut Cluster, cite_index: usize, locator: *const c_char, locator_len: usize, loc_type: LocatorType) -> ErrorCode {
        result_to_error_code(|| unsafe {
            with_cite_mut(cluster, cite_index, |cite| {
                let locator = String::from(borrow_utf8_slice(locator, locator_len)?);
                use citeproc::io::{Locator, Locators, NumberLike};
                if locator.is_empty() {
                    cite.locators = None;
                } else {
                    cite.locators = Some(Locators::Single(Locator { locator: NumberLike::Str(locator), loc_type: loc_type.into_original() }))
                }
                Ok(ErrorCode::None)
            })
        })
    }
}

ffi_fn_nullify! {
    /// Sets the string prefix for a cite.
    @safety unsafe fn citeproc_rs_cluster_cite_set_prefix(cluster: *mut Cluster, cite_index: usize, prefix: *const c_char, prefix_len: usize) -> ErrorCode {
        result_to_error_code(|| unsafe {
            with_cite_mut(cluster, cite_index, |cite| {
                let prefix = borrow_utf8_slice(prefix, prefix_len)?;
                let prefix = Some(prefix)
                    .filter(|x| !x.is_empty())
                    .map(SmartString::from);
                cite.prefix = prefix;
                Ok(ErrorCode::None)
            })
        })
    }
}

ffi_fn_nullify! {
    /// Sets the string suffix on a cite. Pass in a zero length string for no suffix.
    @safety unsafe fn citeproc_rs_cluster_cite_set_suffix(cluster: *mut Cluster, cite_index: usize, suffix: *const c_char, suffix_len: usize) -> ErrorCode {
        result_to_error_code(|| unsafe {
            with_cite_mut(cluster, cite_index, |cite| {
                let suffix = borrow_utf8_slice(suffix, suffix_len)?;
                let suffix = Some(suffix)
                    .filter(|x| !x.is_empty())
                    .map(SmartString::from);
                cite.suffix = suffix;
                Ok(ErrorCode::None)
            })
        })
    }
}
