use clap::{Parser, Subcommand};
use anyhow::Result;

mod commit_list;
mod notify;
mod pr_list;
mod why_reviewer;

#[derive(Parser, Debug)]
#[command(name = "github-tools")]
#[command(about = "Combined GitHub tools for managing notifications and understanding reviewer assignments", long_about = None)]
#[command(version)]
struct Cli {
    /// GitHub personal access token (can also use GITHUB_TOKEN env var)
    #[arg(short, long, env = "GITHUB_TOKEN")]
    token: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Manage GitHub pull request notifications
    #[command(about = "Mark merged/closed PR notifications as read")]
    Notify {
        /// GitHub username (currently unused, reserved for future features)
        #[arg(short, long, env = "GITHUB_USERNAME")]
        username: Option<String>,
    },
    /// Explain why you were tagged as a reviewer on a PR
    #[command(about = "Analyze CODEOWNERS to see why you were tagged as reviewer")]
    WhyReviewer {
        /// Repository in the format "owner/repo"
        #[arg(short, long)]
        repo: String,

        /// Pull request number
        #[arg(short, long)]
        pr: u64,

        /// Print team memberships, all matched CODEOWNERS rules, and ownership decisions
        #[arg(short, long)]
        verbose: bool,
    },
    /// Export pull requests in a date range to CSV
    #[command(about = "List pull requests created in a date range and output as CSV")]
    PrList {
        /// Repository in the format "owner/repo"
        #[arg(short, long)]
        repo: String,

        /// Include PRs created on or after this date (YYYY-MM-DD); ignored when --query is set
        #[arg(long)]
        after: Option<String>,

        /// Include PRs created on or before this date (YYYY-MM-DD); ignored when --query is set
        #[arg(long)]
        before: Option<String>,

        /// Override the entire search query (ignores --after/--before; repo: qualifier is still injected unless already present)
        #[arg(short, long)]
        query: Option<String>,

        /// Write CSV output to this file (defaults to stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Export commits in a date range to CSV
    #[command(about = "List commits in a date range and output as CSV")]
    CommitList {
        /// Repository in the format "owner/repo"
        #[arg(short, long)]
        repo: String,

        /// Include commits on or after this date (YYYY-MM-DD)
        #[arg(long)]
        after: String,

        /// Include commits on or before this date (YYYY-MM-DD)
        #[arg(long)]
        before: String,

        /// Write CSV output to this file (defaults to stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Ensure token is available
    let token = cli.token
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
        .ok_or_else(|| anyhow::anyhow!("GitHub token not provided. Use --token or set GITHUB_TOKEN environment variable"))?;

    match cli.command {
        Commands::Notify { username } => {
            notify::run(token, username).await?;
        }
        Commands::WhyReviewer { repo, pr, verbose } => {
            why_reviewer::run(token, repo, pr, verbose).await?;
        }
        Commands::PrList { repo, after, before, query, output } => {
            pr_list::run(token, repo, after, before, query, output).await?;
        }
        Commands::CommitList { repo, after, before, output } => {
            commit_list::run(token, repo, after, before, output).await?;
        }
    }

    Ok(())
}
