[![crates.io](https://img.shields.io/crates/v/tracing-collector)](https://crates.io/crates/tracing-collector)

# TracingCollector

`TracingCollector` creates a tracing subscriber that collects a copy of all traces into a buffer.
These traces can be retrieved by calling its Display implementation, i.e. calling `log.to_string()` or `format!("{log}")`.
This is useful for testing with [insta](https://crates.io/crates/insta) snapshots.

IMPORTANT! `TracingCollector` is meant for use when testing. It collects logs into a memory buffer
which keeps growing until it is read, the program exits or it is dropped. This means that if you are using `TracingCollector`
in production the program will eventually run out of memory.

When the `TracingCollector` is dropped, the buffer is emptied and the tracing subscriber is released but
the memory equivalent of a Mutex and an empty Vec<u8> is leaked.

When reading the traces, they are stripped of ANSI escape codes and prefixed with a `㏒` character. The former allows
the use of colored & formatted terminal output when the test fails or is run with `--nocapture` and the latter
makes the insta inline snapshots work since rust's `r###` raw string literals strips leading whitespace. The prefix can be
changed or removed using the `set_prefix` and `remove_prefix` methods.

## Example

```rust
#[test]
fn test_logs() {
    let log = TracingCollector::init_debug_level();
    tracing::info!("First log");

    insta::assert_display_snapshot!(log, @r###"
    ㏒   INFO  First log
        at tests/test.rs:6

    "###);

    tracing::debug!("Second log");
    tracing::info!("Third log");

    insta::assert_display_snapshot!(log, @r###"
    ㏒  DEBUG  Second log
        at tests/test.rs:14

      INFO  Third log
       at tests/test.rs:15

   "###);
}
```
