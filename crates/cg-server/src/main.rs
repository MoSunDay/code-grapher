//! cg-server binary entry.

fn main() -> anyhow::Result<()> {
    let cfg = cg_server::state::ServerConfig {
        db_path: std::path::PathBuf::from(
            std::env::var("CG_DB_PATH").unwrap_or_else(|_| "./code-grapher.db".into()),
        ),
        port: parse_arg("--port").and_then(|v| v.parse().ok()).unwrap_or(7800),
        watch: !has_flag("--no-watch"),
        analyzer: !has_flag("--no-analyzer"),
        web_dist: cg_server::state::default_web_dist(),
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,cg_server=debug".into()),
        )
        .init();
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(cg_server::run(cfg))
}

fn parse_arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn has_flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}
