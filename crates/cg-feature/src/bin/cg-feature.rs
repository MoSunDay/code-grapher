//! CLI over the branch-model feature review store.

use cg_feature::ops::{self, DraftFile};
use cg_feature::docs_prefix;
use cg_feature::{
    repo_id_from_git_url, resolve_feature_root, validate_feature_name,
};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(about = "Git-backed feature review storage (branch model)")]
struct Args {
    /// Feature root override (default: CODE_GRAPHER_DATA_DIR / XDG / HOME).
    #[arg(long, global = true)]
    feature_root: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print repo_id = sha256(canonical git_url).
    RepoId { git_url: String },
    /// List MRs (branches mr/*) with state.
    Mrs { git_url: String },
    /// List files of main or one MR branch.
    Tree {
        git_url: String,
        /// MR name; omit for main (stable).
        #[arg(long)]
        mr: Option<String>,
    },
    /// Print one file at main or an MR branch tip.
    Cat {
        git_url: String,
        /// Repository-relative path.
        path: String,
        #[arg(long)]
        mr: Option<String>,
    },
    /// Unified diff main...mr/<name>.
    Diff { git_url: String, feature_name: String },
    /// Commits of one MR.
    Commits { git_url: String, feature_name: String },
    /// Mirror a source directory onto mr/<name> and commit; never approves.
    Draft {
        git_url: String,
        feature_name: String,
        /// Directory whose file tree becomes the MR content.
        source_dir: PathBuf,
        /// Fail unless the MR name equals this value.
        #[arg(long, value_name = "NAME")]
        expected_feature_name: Option<String>,
    },
    /// Merge mr/<name> into main as the reviewer + changelog entry.
    Approve {
        git_url: String,
        feature_name: String,
        #[arg(long, value_name = "NAME")]
        reviewer: String,
        #[arg(long, value_name = "TEXT")]
        note: Option<String>,
        /// Require this mr branch tip hash before merging.
        #[arg(long, value_name = "HASH")]
        expected_tip: Option<String>,
    },
    /// Print _changelog entries (newest first).
    Changelog { git_url: String },
}

fn main() -> anyhow::Result<ExitCode> {
    let args = Args::parse();
    let root = resolve_feature_root(args.feature_root.as_deref())?;
    match args.command {
        Command::RepoId { git_url } => {
            println!("{}", repo_id_from_git_url(&git_url)?);
        }
        Command::Mrs { git_url } => {
            let repo_id = repo_id_from_git_url(&git_url)?;
            for mr in ops::mrs(&root, &repo_id)? {
                println!(
                    "{}\t{}\tahead={} behind={}\treviewer={}",
                    mr.name,
                    match mr.state {
                        ops::MrState::Open => "open",
                        ops::MrState::Merged => "merged",
                    },
                    mr.ahead,
                    mr.behind,
                    mr.reviewer.as_deref().unwrap_or("-")
                );
            }
        }
        Command::Tree { git_url, mr } => {
            let repo_id = repo_id_from_git_url(&git_url)?;
            for file in ops::tree(&root, &repo_id, mr.as_deref())? {
                println!("{}\t{}\t{}", file.path, badge(&file.state), file.size);
            }
        }
        Command::Cat { git_url, path, mr } => {
            let repo_id = repo_id_from_git_url(&git_url)?;
            let blob = ops::blob(&root, &repo_id, mr.as_deref(), &path)?;
            print!("{}", blob.content);
        }
        Command::Diff { git_url, feature_name } => {
            let repo_id = repo_id_from_git_url(&git_url)?;
            print!("{}", ops::diff(&root, &repo_id, &feature_name)?);
        }
        Command::Commits { git_url, feature_name } => {
            let repo_id = repo_id_from_git_url(&git_url)?;
            for commit in ops::commits(&root, &repo_id, &feature_name)? {
                println!("{}\t{}\t{}", &commit.hash[..8], commit.author, commit.subject);
            }
        }
        Command::Draft {
            git_url,
            feature_name,
            source_dir,
            expected_feature_name,
        } => {
            validate_feature_name(&feature_name)?;
            let repo_id = repo_id_from_git_url(&git_url)?;
            let files = read_source_dir(&source_dir, &feature_name)?;
            match ops::draft(
                &root,
                &repo_id,
                &feature_name,
                &files,
                expected_feature_name.as_deref(),
            )? {
                ops::DraftOutcome::Noop => println!("NOOP"),
                ops::DraftOutcome::Updated { commit } => println!("DRAFT {commit}"),
            }
        }
        Command::Approve {
            git_url,
            feature_name,
            reviewer,
            note,
            expected_tip,
        } => {
            let repo_id = repo_id_from_git_url(&git_url)?;
            let params = ops::ApproveParams {
                reviewer: &reviewer,
                note: note.as_deref(),
                expected_tip: expected_tip.as_deref(),
            };
            let outcome = ops::approve(&root, &repo_id, &feature_name, &params)?;
            println!("MERGED {} {}", outcome.merge_commit, outcome.changelog);
        }
        Command::Changelog { git_url } => {
            let repo_id = repo_id_from_git_url(&git_url)?;
            for entry in ops::changelog(&root, &repo_id)? {
                println!("== {} ==", entry.file);
                println!("{}", entry.markdown);
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn badge(state: &ops::FileBadge) -> &'static str {
    match state {
        ops::FileBadge::Draft => "draft",
        ops::FileBadge::Stable => "stable",
    }
}

/// Collect UTF-8 files below `dir` as draft sources (skips dotfiles/.git),
/// mapping them into the MR namespace `docs/<name>/`.
fn read_source_dir(dir: &Path, name: &str) -> anyhow::Result<Vec<DraftFile>> {
    if !dir.is_dir() {
        anyhow::bail!("source directory {} not found", dir.display());
    }
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current)? {
            let entry = entry?;
            let entry_name = entry.file_name().to_string_lossy().into_owned();
            if entry_name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path
                .strip_prefix(dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            let content = std::fs::read_to_string(&path)?;
            out.push(DraftFile {
                path: format!("{}{rel}", docs_prefix(name)),
                content,
            });
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}
