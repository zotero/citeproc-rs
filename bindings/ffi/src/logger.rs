use crate::ErrorCode;
use backtrace::Backtrace;
use env_logger::filter::{Builder, Filter};
use libc::{c_char, c_void};
use log::{Level as LogLevel, LevelFilter};
use std::cell::RefCell;

mod private {
    #[allow(dead_code)]
    #[repr(usize)]
    pub enum LogLevel {
        Error = 1,
        Warn,
        Info,
        Debug,
        Trace,
    }
    #[test]
    fn test_eq_log_crate() {
        assert_eq!(LogLevel::Error as usize, log::Level::Error as usize);
        assert_eq!(LogLevel::Warn as usize, log::Level::Warn as usize);
        assert_eq!(LogLevel::Info as usize, log::Level::Info as usize);
        assert_eq!(LogLevel::Debug as usize, log::Level::Debug as usize);
        assert_eq!(LogLevel::Trace as usize, log::Level::Trace as usize);
    }
    #[allow(dead_code)]
    #[repr(usize)]
    pub enum LevelFilter {
        Off,
        /// Corresponds to the `Error` log level.
        Error,
        /// Corresponds to the `Warn` log level.
        Warn,
        /// Corresponds to the `Info` log level.
        Info,
        /// Corresponds to the `Debug` log level.
        Debug,
        /// Corresponds to the `Trace` log level.
        Trace,
    }
    #[test]
    fn test_eq_log_crate_filter() {
        assert_eq!(LevelFilter::Off as usize, log::LevelFilter::Off as usize);
        assert_eq!(
            LevelFilter::Error as usize,
            log::LevelFilter::Error as usize
        );
        assert_eq!(LevelFilter::Warn as usize, log::LevelFilter::Warn as usize);
        assert_eq!(LevelFilter::Info as usize, log::LevelFilter::Info as usize);
        assert_eq!(
            LevelFilter::Debug as usize,
            log::LevelFilter::Debug as usize
        );
        assert_eq!(
            LevelFilter::Trace as usize,
            log::LevelFilter::Trace as usize
        );
    }
}

pub type LoggerWriteCallback = unsafe extern "C" fn(
    user_data: *mut c_void,
    level: LogLevel,
    module_path: *const u8,
    module_path_len: usize,
    src: *const u8,
    src_len: usize,
);

// /// It would be fun to allow arbitrary implementations of this, but env_logger's configuration is
// /// too easy to give up.
// ///
// /// Must return a boolean, strictly `0 => false, 1 => true`.
// pub type LoggerEnabledCallback =
//     unsafe extern "C" fn(user_data: *mut c_void, level: LogLevel) -> bool;

pub type LoggerFlushCallback = Option<unsafe extern "C" fn(user_data: *mut c_void)>;

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct FFILoggerVTable {
    write: LoggerWriteCallback,
    flush: LoggerFlushCallback,
}

///
/// # Safety
///
/// Instance must remain alive for the rest of the program's execution, and it also must be safe to
/// send across threads and to access from multiple concurrent threads.
#[no_mangle]
pub unsafe extern "C" fn citeproc_rs_set_logger(
    instance: *mut c_void,
    vtable: FFILoggerVTable,
    min_severity: LevelFilter,
    filters: *const c_char,
    filters_len: usize,
) -> ErrorCode {
    crate::util::result_to_error_code(move || {
        let filters = unsafe { crate::util::borrow_utf8_slice(filters, filters_len) }?;
        let logger = FFILogger::new(instance, vtable, min_severity, filters);
        logger.try_init()?;

        std::panic::set_hook(Box::new(|panic_info| {
            let message = panic_info
                .payload()
                .downcast_ref::<String>()
                .map(|x| x.as_str())
                .or(panic_info.payload().downcast_ref::<&str>().map(|x| *x))
                .unwrap_or("<unknown message>");
            let backtrace = Backtrace::new();
            if let Some(location) = panic_info.location() {
                log::error!(
                    "panic occurred in file '{}' at line {}: {}\n\nbacktrace: \n\n{:?}",
                    location.file(),
                    location.line(),
                    message,
                    backtrace,
                );
            } else {
                log::error!(
                    "panic occurred (unknown location): {}\n\nbacktrace: \n\n{:?}",
                    message,
                    backtrace
                );
            }
            //  Do something with backtrace and panic_info.
        }));

        Ok(ErrorCode::None)
    })
}

pub(crate) struct FFILogger {
    /// This must have a static lifetime.
    instance: *mut c_void,
    vtable: FFILoggerVTable,
    filter: Filter,
}

impl FFILogger {
    unsafe fn new(
        instance: *mut c_void,
        vtable: FFILoggerVTable,
        min_severity: LevelFilter,
        filter_string: &str,
    ) -> Self {
        let filter = Builder::new()
            .parse(filter_string)
            .filter_level(min_severity)
            .build();
        Self {
            instance,
            vtable,
            filter,
        }
    }

    pub fn try_init(self) -> Result<(), log::SetLoggerError> {
        let max_level = self.filter.filter();
        let r = log::set_boxed_logger(Box::new(self));

        if r.is_ok() {
            log::set_max_level(max_level);
        }

        r
    }
}

/// This is safe because `fn new` is unsafe.
///
/// 1. You are required to pass in an instance (that responds
///    safely to FFILoggerVTable methods) that is also valid for the rest of the program's life.
///    (For example, a Swift object with a +1 refcount.)
/// 2. The instance is also required to be safe to share between threads.
/// 3. Further, the instance is required to be safe to use in two threads at the same time.
///
/// This is a reasonable demand for a global logger instance. They will often acquire a global lock
/// on stdout for this purpose.
unsafe impl Send for FFILogger {}
unsafe impl Sync for FFILogger {}

impl log::Log for FFILogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.filter.enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        if !self.filter.matches(record) {
            return;
        }
        thread_local! {
            static SCRATCH: RefCell<String> = RefCell::default();
        }

        // TODO: make this re-entrant safe, using try_borrow_mut.
        let result: Result<ErrorCode, core::fmt::Error> = SCRATCH.with(|scratch| {
            let mut f = scratch.borrow_mut();
            f.clear();
            use core::fmt::Write;
            write!(f, "{}", record.args())?;
            let modpath = record.module_path().unwrap_or("citeproc_rs").as_bytes();
            let f_bytes = f.as_str().as_bytes();
            unsafe {
                (self.vtable.write)(
                    self.instance,
                    record.level(),
                    modpath.as_ptr(),
                    modpath.len(),
                    f_bytes.as_ptr(),
                    f_bytes.len(),
                );
            }
            Ok(ErrorCode::None)
        });

        if let Err(_) = result {
            eprintln!("failed to log");
        }
    }

    fn flush(&self) {
        if let Some(flush) = self.vtable.flush {
            unsafe {
                flush(self.instance);
            }
        }
    }
}

#[cfg(feature = "testability")]
#[no_mangle]
pub extern "C" fn test_log_msg(level: LogLevel, msg: *const c_char, msg_len: usize) {
    let msg =
        unsafe { crate::util::borrow_utf8_slice(msg, msg_len) }.expect("msg / msg_len not valid?");
    log::log!(level, "{}", msg);
}
