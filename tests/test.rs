use tracing_collector::TracingCollector;

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
