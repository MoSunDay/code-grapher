//! `cg-feature`: git-backed feature review CLI keyed by canonical git URL.

use anyhow::{Context, Result};
use cg_feature::ops::{self, ApproveParams};
use cg_feature::{repo_id_from_git_url, resolve_feature_root};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "cg-feature",
    about = "Git-backed feature review keyed by sha256(canonical git_url)"
)]
struct Args {
    /// Feature root override (default: CODE_GRAPHER_DATA_DIR or XDG data home).
    #[arg(long, global = true, value_name = "FEATURE_ROOT")]
    feature_root: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print repo_id = sha256(canonical GIT_URL).
    RepoId { git_url: String },
    /// List features with draft/stable states.
    List { git_url: String },
    /// Aggregate status as JSON.
    Status { git_url: String },
    /// Unified diff against HEAD (all features, or one).
    Diff {
        git_url: String,
        #[arg(default_value = None)]
        feature_name: Option<String>,
    },
    /// Refresh the draft for one feature; never commits.
    Draft {
        git_url: String,
        /// Path to the source feature JSON file.
        source_json: PathBuf,
        /// Fail unless the source feature name equals this value.
        #[arg(long, value_name = "NAME")]
        expected_feature_name: Option<String>,
    },
    /// Commit one reviewed feature plus its changelog entry.
    Approve {
        git_url: String,
        feature_name: String,
        /// Human reviewer recorded in the commit and changelog.
        #[arg(long, value_name = "NAME")]
        reviewer: String,
        /// Optional review note for the changelog entry.
        #[arg(long, value_name = "TEXT")]
        note: Option<String>,
        /// Require this sha256 of the current feature.json before committing.
        #[arg(long, value_name = "HASH")]
        expected_hash: Option<String>,
    },
}

fn main() -> Result<ExitCode> {
    let args = Args::parse();
    let root = resolve_feature_root(args.feature_root.as_deref())?;
    let code = match args.command {
        Command::RepoId { git_url } => {
            println!("{}", repo_id_from_git_url(&git_url)?);
            ExitCode::SUCCESS
        }
        Command::List { git_url } => {
            let repo_id = repo_id_from_git_url(&git_url)?;
            for feature in ops::list(&root, &repo_id)? {
                println!("{}\t{}\t{}", feature.name, json_state(&feature.state), short(&feature.hash));
            }
            ExitCode::SUCCESS
        }
        Command::Status { git_url } => {
            let repo_id = repo_id_from_git_url(&git_url)?;
            println!("{}", serde_json::to_string_pretty(&ops::status(&root, &repo_id)?)?);
            ExitCode::SUCCESS
        }
        Command::Diff {
            git_url,
            feature_name,
        } => {
            let repo_id = repo_id_from_git_url(&git_url)?;
            print!("{}", ops::diff(&root, &repo_id, feature_name.as_deref())?);
            ExitCode::SUCCESS
        }
        Command::Draft {
            git_url,
            source_json,
            expected_feature_name,
        } => {
            let repo_id = repo_id_from_git_url(&git_url)?;
            let source = std::fs::read(&source_json)
                .with_context(|| format!("read {}", source_json.display()))?;
            match ops::draft(&root, &repo_id, &source, expected_feature_name.as_deref())? {
                ops::DraftOutcome::Noop => println!("NOOP"),
                ops::DraftOutcome::Created { rel_path } => println!("DRAFT {rel_path}"),
            }
            ExitCode::SUCCESS
        }
        Command::Approve {
            git_url,
            feature_name,
            reviewer,
            note,
            expected_hash,
        } => {
            let repo_id = repo_id_from_git_url(&git_url)?;
            let params = ApproveParams {
                reviewer: &reviewer,
                note: note.as_deref(),
                expected_hash: expected_hash.as_deref(),
            };
            let outcome = ops::approve(&root, &repo_id, &feature_name, &params)?;
            println!("APPROVED {} {}", outcome.commit, outcome.changelog);
            ExitCode::SUCCESS
        }
    };
    Ok(code)
}

fn json_state(state: &cg_feature::ops::FeatureState) -> &'static str {
    match state {
        cg_feature::ops::FeatureState::Draft => "draft",
        cg_feature::ops::FeatureState::Stable => "stable",
    }
}

fn short(hash: &Option<String>) -> String {
    hash.as_deref().map(|h| h.get(..12).unwrap_or(h).to_string()).unwrap_or_default()
}
