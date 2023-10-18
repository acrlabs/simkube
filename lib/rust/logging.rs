use tracing_subscriber::fmt::format::FmtSpan;

pub fn setup(env_filter: &str) {
    tracing_subscriber::fmt()
        .with_file(true)
        .with_line_number(true)
        .with_span_events(FmtSpan::NEW)
        .with_target(false)
        .with_env_filter(env_filter)
        .compact()
        .init();
}
