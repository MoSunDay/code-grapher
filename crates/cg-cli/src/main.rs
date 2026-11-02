//! code-grapher CLI: local-embedded by default, `--server <url>` for remote.

mod local;
mod remote;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use serde_json::Value;

#[derive(Parser)]
#[command(name = "code-grapher", version, about = "Repository code graph: index, analyze, query")]
struct Cli {
    /// Database file (local mode); env TURSO_DATABASE_URL overrides for remote Turso.
    #[arg(long, global = true)]
    db: Option<std::path::PathBuf>,
    /// Run against a remote code-grapher daemon instead of embedding.
    #[arg(long, global = true)]
    server: Option<String>,
    /// Repo selection: id, path or name (defaults to the first repo).
    #[arg(long, global = true)]
    repo: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Manage indexed repositories.
    Repo {
        #[command(subcommand)]
        action: RepoAction,
    },
    /// Index the repository (incremental by default).
    Index {
        #[arg(long)]
        full: bool,
    },
    /// Run the background daemon (API + watcher + analyzer).
    Serve {
        #[arg(long, default_value = "7800")]
        port: u16,
        #[arg(long)]
        no_watch: bool,
        #[arg(long)]
        no_analyzer: bool,
    },
    /// Watch for file changes and re-index (no HTTP server).
    Watch,
    /// Run LLM hidden-call analysis for pending tasks.
    Analyze {
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// Query the graph: files | symbols | calls.
    Query {
        #[command(subcommand)]
        what: QueryCmd,
    },
    /// Show an indexed file with its symbols.
    File {
        #[command(subcommand)]
        action: FileAction,
    },
    /// Analysis task queue management.
    Tasks {
        #[command(subcommand)]
        action: TaskAction,
    },
}

#[derive(Subcommand)]
enum RepoAction {
    Add {
        path: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        git_url: Option<String>,
    },
    List,
    Remove { target: String },
}

#[derive(Subcommand)]
enum QueryCmd {
    Files,
    Symbols {
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        q: Option<String>,
    },
    Calls { symbol_id: i64 },
}

#[derive(Subcommand)]
enum FileAction {
    Show { path: String },
}

#[derive(Subcommand)]
enum TaskAction {
    List {
        #[arg(long)]
        status: Option<String>,
    },
    Retry { task_id: i64 },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,cg_cli=info".into()),
        )
        .init();
    let cli = Cli::parse();
    let db_path = cli
        .db
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("./code-grapher.db"));
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(dispatch(cli, db_path))
}

async fn dispatch(cli: Cli, db_path: std::path::PathBuf) -> Result<()> {
    if let Some(url) = cli.server.as_deref() {
        let r = remote::Remote::new(url);
        return remote_dispatch(r, cli.repo.as_deref(), cli.cmd).await;
    }
    let conn = local::open(&db_path).await?;
    local_dispatch(conn, cli.repo.as_deref(), cli.cmd).await
}

async fn local_dispatch(conn: libsql::Connection, sel: Option<&str>, cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Repo { action } => match action {
            RepoAction::Add { path, name, git_url } => {
                let repo = local::add_repo(&conn, &path, name.as_deref(), git_url.as_deref()).await?;
                print_json(serde_json::to_value(&repo)?)
            }
            RepoAction::List => print_json(serde_json::to_value(cg_db::repos::list(&conn).await?)?),
            RepoAction::Remove { target } => {
                let repo = local::resolve_repo(&conn, Some(&target)).await?;
                cg_db::repos::delete(&conn, repo.id).await?;
                println!("removed {}", repo.name);
                Ok(())
            }
        },
        Cmd::Index { full } => {
            let repo = local::resolve_repo(&conn, sel).await?;
            let outcome = local::index(&conn, &repo, full).await?;
            print_json(serde_json::to_value(&outcome)?)
        }
        Cmd::Serve { port, no_watch, no_analyzer } => {
            let cfg = cg_server::state::ServerConfig {
                db_path: std::path::PathBuf::from(
                    std::env::var("CG_DB_PATH").unwrap_or_else(|_| "./code-grapher.db".into()),
                ),
                port,
                watch: !no_watch,
                analyzer: !no_analyzer,
            };
            cg_server::run(cfg).await
        }
        Cmd::Watch => {
            let repo = local::resolve_repo(&conn, sel).await?;
            local::watch(conn, repo).await
        }
        Cmd::Analyze { limit } => {
            let summaries = local::analyze(&conn, limit).await?;
            print_json(serde_json::to_value(&summaries)?)
        }
        Cmd::Query { what } => {
            let repo = local::resolve_repo(&conn, sel).await?;
            match what {
                QueryCmd::Files => print_json(local::query_files(&conn, &repo).await?),
                QueryCmd::Symbols { kind, q } => {
                    print_json(local::query_symbols(&conn, &repo, kind.as_deref(), q.as_deref()).await?)
                }
                QueryCmd::Calls { symbol_id } => print_json(local::query_calls(&conn, &repo, symbol_id).await?),
            }
        }
        Cmd::File { action } => match action {
            FileAction::Show { path } => {
                let repo = local::resolve_repo(&conn, sel).await?;
                print_json(local::file_show(&conn, &repo, &path).await?)
            }
        },
        Cmd::Tasks { action } => {
            let repo = local::resolve_repo(&conn, sel).await?;
            match action {
                TaskAction::List { status } => {
                    print_json(local::tasks_list(&conn, &repo, status.as_deref()).await?)
                }
                TaskAction::Retry { task_id } => {
                    cg_db::tasks::retry(&conn, task_id).await?;
                    println!("task {task_id} requeued");
                    Ok(())
                }
            }
        }
    }
}

async fn remote_dispatch(r: remote::Remote, sel: Option<&str>, cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Repo { action } => match action {
            RepoAction::Add { path, name, git_url } => {
                print_json(r.add_repo(&path, name.as_deref(), git_url.as_deref()).await?)
            }
            RepoAction::List => print_json(r.list_repos().await?),
            RepoAction::Remove { target } => {
                let id = r.repo_id(Some(&target)).await?;
                print_json(r.remove_repo(id).await?)
            }
        },
        Cmd::Index { full } => {
            let id = r.repo_id(sel).await?;
            print_json(r.index(id, full).await?)
        }
        Cmd::Serve { .. } => Err(anyhow!("`serve` starts the daemon locally; do not pass --server")),
        Cmd::Watch => Err(anyhow!("`watch` runs locally; connect to the daemon API instead")),
        Cmd::Analyze { .. } => {
            let id = r.repo_id(sel).await?;
            print_json(r.analyze(id).await?)
        }
        Cmd::Query { what } => {
            let id = r.repo_id(sel).await?;
            match what {
                QueryCmd::Files => print_json(r.files(id).await?),
                QueryCmd::Symbols { kind, q } => print_json(r.symbols(id, kind.as_deref(), q.as_deref()).await?),
                QueryCmd::Calls { symbol_id } => print_json(r.calls(symbol_id).await?),
            }
        }
        Cmd::File { action } => match action {
            FileAction::Show { path } => {
                let id = r.repo_id(sel).await?;
                print_json(r.file(id, &path).await?)
            }
        },
        Cmd::Tasks { action } => {
            match action {
                TaskAction::List { status } => {
                    let id = r.repo_id(sel).await?;
                    print_json(r.tasks(id, status.as_deref()).await?)
                }
                TaskAction::Retry { task_id } => print_json(r.retry_task(task_id).await?),
            }
        }
    }
}

fn print_json(v: Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}
