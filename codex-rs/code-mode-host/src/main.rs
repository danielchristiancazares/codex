use clap::Parser;

#[derive(Debug, Parser)]
struct Cli {
    /// Transport endpoint: `stdio`, `stdio://`, `ws://IP:PORT`, or `grpc://IP:PORT`.
    #[arg(
        long,
        value_name = "URL",
        default_value = codex_code_mode_host::DEFAULT_LISTEN_URL
    )]
    listen: String,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let listen = Cli::parse().listen;
    let mut runtime_builder = if listen.starts_with("ws://") {
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        builder.worker_threads(/*worker_threads*/ 2);
        builder
    } else {
        tokio::runtime::Builder::new_current_thread()
    };
    runtime_builder
        .enable_all()
        .build()?
        .block_on(codex_code_mode_host::run_main(&listen))
}
