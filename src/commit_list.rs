use anyhow::{Context, Result};
use octocrab::Octocrab;
use serde::Deserialize;
use std::io::Write;

#[derive(Debug, Deserialize)]
struct CommitAuthor {
    name: String,
    date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CommitDetail {
    message: String,
    author: CommitAuthor,
}

#[derive(Debug, Deserialize)]
struct GitHubAuthor {
    login: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Commit {
    commit: CommitDetail,
    html_url: String,
    author: Option<GitHubAuthor>,
}

pub async fn run(
    token: String,
    repo: String,
    after: String,
    before: String,
    output: Option<String>,
) -> Result<()> {
    let repo_parts: Vec<&str> = repo.split('/').collect();
    if repo_parts.len() != 2 {
        anyhow::bail!("Repository must be in format 'owner/repo'");
    }
    let (owner, repo_name) = (repo_parts[0], repo_parts[1]);

    let octocrab = Octocrab::builder()
        .personal_token(token)
        .build()?;

    // Treat --after as start-of-day and --before as end-of-day (inclusive)
    let since = format!("{}T00:00:00Z", after);
    let until = format!("{}T23:59:59Z", before);

    eprintln!(
        "Fetching commits for {}/{} from {} to {}...",
        owner, repo_name, after, before
    );

    let commits = fetch_all_commits(&octocrab, owner, repo_name, &since, &until).await?;

    eprintln!("Found {} commit(s)", commits.len());

    let writer: Box<dyn Write> = if let Some(path) = &output {
        Box::new(
            std::fs::File::create(path).context("Failed to create output file")?,
        )
    } else {
        Box::new(std::io::stdout())
    };

    write_csv(writer, &commits)?;

    Ok(())
}

async fn fetch_all_commits(
    octocrab: &Octocrab,
    owner: &str,
    repo: &str,
    since: &str,
    until: &str,
) -> Result<Vec<Commit>> {
    let mut all_commits = Vec::new();
    let mut page = 1u32;
    let per_page = 100u8;

    loop {
        let route = format!(
            "/repos/{}/{}/commits?since={}&until={}&per_page={}&page={}",
            owner, repo, since, until, per_page, page
        );

        let commits: Vec<Commit> = octocrab
            .get(&route, None::<&()>)
            .await
            .context("Failed to fetch commits")?;

        let count = commits.len();
        all_commits.extend(commits);

        if count < per_page as usize {
            break;
        }

        page += 1;
    }

    Ok(all_commits)
}

fn write_csv(writer: Box<dyn Write>, commits: &[Commit]) -> Result<()> {
    let mut csv_writer = csv::Writer::from_writer(writer);

    csv_writer.write_record(["date", "commit_url", "message", "author"])?;

    for commit in commits {
        let date = commit
            .commit
            .author
            .date
            .as_deref()
            .unwrap_or("")
            .get(..10)
            .unwrap_or("");

        let url = &commit.html_url;

        // Use only the first line of the commit message as the summary
        let message = commit.commit.message.lines().next().unwrap_or("");

        let author = commit
            .author
            .as_ref()
            .and_then(|a| a.login.as_deref())
            .unwrap_or(&commit.commit.author.name);

        csv_writer.write_record([date, url, message, author])?;
    }

    csv_writer.flush()?;

    Ok(())
}
