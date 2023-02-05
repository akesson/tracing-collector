use std::{
    fmt::{self},
    io::{self},
    mem,
    sync::{Mutex, MutexGuard},
};
use tracing::{subscriber::DefaultGuard, Level};
use tracing_subscriber::fmt::{writer::Tee, MakeWriter};
use tracing_subscriber::util::SubscriberInitExt;

/// `TracingCollector` creates a tracing subscriber that collects a copy of all traces into a buffer.
/// These traces can be retrieved by calling its Display implementation, i.e. calling `log.to_string()` or `format!("{log}")`.
/// This is useful for testing with [insta](https://crates.io/crates/insta) snapshots.
///
/// IMPORTANT! `TracingCollector` is meant for use when testing. It collects logs into a memory buffer
/// which keeps growing until it is read, the program exits or it is dropped. This means that if you are using `TracingCollector`
/// in production the program will eventually run out of memory.
///
/// When the `TracingCollector` is dropped, the buffer is emptied and the tracing subscriber is released but
/// the memory equivalent of a Mutex and an empty Vec<u8> is leaked.
///
/// When reading the traces, they are stripped of ANSI escape codes and prefixed with a `㏒` character. The former allows
/// the use of colored & formatted terminal output when the test fails or is run with `--nocapture` and the latter
/// makes the insta inline snapshots work since rust's `r###` raw string literals strips leading whitespace. The prefix can be
/// changed or removed using the `set_prefix` and `remove_prefix` methods.
///
/// Example:
///
/// ```rust
/// #[test]
/// fn test_logs() {
///     let log = TracingCollector::init_debug_level();
///     tracing::info!("First log");
///
///     insta::assert_display_snapshot!(log, @r###"
///     ㏒   INFO  First log
///         at tests/test.rs:6
///
///     "###);
///
///     tracing::debug!("Second log");
///     tracing::info!("Third log");
///
///     insta::assert_display_snapshot!(log, @r###"
///     ㏒  DEBUG  Second log
///         at tests/test.rs:14
///
///       INFO  Third log
///        at tests/test.rs:15
///
///    "###);
///}
/// ```
pub struct TracingCollector {
    buf: &'static Mutex<Vec<u8>>,
    trace_guard: Mutex<Option<DefaultGuard>>,
    prefix: Option<char>,
}

impl TracingCollector {
    fn new() -> Self {
        TracingCollector {
            buf: Box::leak(Box::new(Mutex::new(vec![]))),
            trace_guard: Mutex::new(None),
            prefix: Some('㏒'),
        }
    }

    pub fn set_prefix(&mut self, prefix: char) {
        self.prefix = Some(prefix);
    }

    pub fn remove_prefix(&mut self) {
        self.prefix = None;
    }

    fn set_guard(&self, trace_guard: DefaultGuard) {
        let mut guard = self.trace_guard.lock().expect("failed to lock mutex");
        *guard = Some(trace_guard);
    }

    /// Create a `TracingCollector` that collects traces up to the `TRACE` level.
    pub fn init_trace_level() -> Self {
        Self::init(Level::TRACE)
    }

    /// Create a `TracingCollector` that collects traces up to the `DEBUG` level.
    pub fn init_debug_level() -> Self {
        Self::init(Level::DEBUG)
    }

    /// Create a `TracingCollector` that collects traces up to the `INFO` level.
    pub fn init_info_level() -> Self {
        Self::init(Level::INFO)
    }

    /// Create a new `TracingCollector` that collects traces up to the specified level.
    pub fn init(max_level: Level) -> Self {
        let collector = TracingCollector::new();

        let saver = CollectingWriter::new(&collector.buf);
        let guard = tracing_subscriber::fmt()
            .pretty()
            .with_max_level(max_level)
            .without_time()
            .with_file(true)
            .with_line_number(true)
            .with_target(false)
            .with_ansi(true)
            .with_writer(Tee::new(saver, io::stdout))
            .finish()
            .set_default();

        collector.set_guard(guard);
        collector
    }

    pub fn clear(&self) {
        self.buf.lock().expect("failed to lock mutex").clear();
    }
}

impl fmt::Display for TracingCollector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = vec![];
        let mut guard = self.buf.lock().expect("failed to lock mutex");
        mem::swap(&mut buf, &mut *guard);
        let cleaned_buf = strip_ansi_escapes::strip(&*buf).expect("failed to strip ansi escapes");
        let cleaned = String::from_utf8(cleaned_buf).expect("log contains invalid utf8");
        if let Some(prefix) = self.prefix {
            write!(f, "{prefix}{cleaned}",)
        } else {
            write!(f, "{cleaned}",)
        }
    }
}

impl Drop for TracingCollector {
    fn drop(&mut self) {
        let mut vec = self.buf.lock().expect("msg");
        vec.clear();
        // reduce the size of the vector as much as possible so that the
        // leaked memory is only the size of a mutex and an empty vector
        vec.shrink_to(0);
    }
}

struct CollectingWriter<'a> {
    buf: &'a Mutex<Vec<u8>>,
}

impl<'a> CollectingWriter<'a> {
    /// Create a new `CollectingWriter` that writes into the specified buffer (behind a mutex).
    fn new(buf: &'a Mutex<Vec<u8>>) -> Self {
        Self { buf }
    }

    /// Give access to the internal buffer (behind a `MutexGuard`).
    fn buf(&self) -> io::Result<MutexGuard<'a, Vec<u8>>> {
        // Note: The `lock` will block. This would be a problem in production code,
        // but is fine in tests.
        self.buf
            .lock()
            .map_err(|_| io::Error::from(io::ErrorKind::Other))
    }
}

impl<'a> io::Write for CollectingWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Lock target buffer
        let mut target = self.buf()?;
        // Write to buffer
        target.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buf()?.flush()
    }
}

impl<'a> MakeWriter<'_> for CollectingWriter<'a> {
    type Writer = Self;

    fn make_writer(&self) -> Self::Writer {
        CollectingWriter::new(self.buf)
    }
}
