pub(super) fn init_logging() {
    let mut builder = env_logger::builder();
    let level = std::env::var("RUST_LOG")
        .map(|s| s.parse().expect("Failed to parse RUST_LOG"))
        .unwrap_or(log::LevelFilter::Info);
    builder.filter_level(level);
    builder.init();
}
