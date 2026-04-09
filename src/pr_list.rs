use anyhow::{Context, Result};
use octocrab::Octocrab;
use serde::Deserialize;
use std::io::Write;

#[derive(Debug, Deserialize)]
struct PrUser {
    login: String,
}

#[derive(Debug, Deserialize)]
struct PullRequest {
    number: u64,
    title: String,
    html_url: String,
    user: PrUser,
    state: String,
    created_at: String,
    merged_at: Option<String>,
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

    eprintln!(
        "Fetching PRs for {}/{} from {} to {}...",
        owner, repo_name, after, before
    );

    let prs = fetch_all_prs(&octocrab, owner, repo_name, &after, &before).await?;

    eprintln!("Found {} PR(s)", prs.len());

    let writer: Box<dyn Write> = if let Some(path) = &output {
        Box::new(
            std::fs::File::create(path).context("Failed to create output file")?,
        )
    } else {
        Box::new(std::io::stdout())
    };

    write_csv(writer, &prs)?;

    Ok(())
}

async fn fetch_all_prs(
    octocrab: &Octocrab,
    owner: &str,
    repo: &str,
    after: &str,
    before: &str,
) -> Result<Vec<PullRequest>> {
    // ISO 8601 strings are lexicographically comparable when the format is uniform
    let since = format!("{}T00:00:00Z", after);
    let until = format!("{}T23:59:59Z", before);

    let mut all_prs = Vec::new();
    let mut page = 1u32;
    let per_page = 100u32;

    // Paginate newest-first so we can stop as soon as created_at drops below `since`
    loop {
        let route = format!(
            "/repos/{}/{}/pulls?state=all&sort=created&direction=desc&per_page={}&page={}",
            owner, repo, per_page, page
        );

        let prs: Vec<PullRequest> = octocrab
            .get(&route, None::<&()>)
            .await
            .context("Failed to fetch PRs")?;

        let count = prs.len();
        let mut done = false;

        for pr in prs {
            if pr.created_at.as_str() < since.as_str() {
                // All remaining pages are older; stop
                done = true;
                break;
            }
            if pr.created_at.as_str() <= until.as_str() {
                all_prs.push(pr);
            }
            // else: created_at > until — PR is newer than our window, skip but keep paginating
        }

        if done || count < per_page as usize {
            break;
        }

        page += 1;
    }

    // Restore chronological order (oldest first)
    all_prs.reverse();

    Ok(all_prs)
}

fn effective_state(pr: &PullRequest) -> &str {
    if pr.merged_at.is_some() {
        "merged"
    } else {
        &pr.state
    }
}

fn write_csv(writer: Box<dyn Write>, prs: &[PullRequest]) -> Result<()> {
    let mut csv_writer = csv::Writer::from_writer(writer);

    csv_writer.write_record(["number", "title", "url", "author", "state", "created_at", "merged_at"])?;

    for pr in prs {
        let number = pr.number.to_string();
        let created = pr.created_at.get(..10).unwrap_or(&pr.created_at);
        let merged = pr
            .merged_at
            .as_deref()
            .and_then(|d| d.get(..10))
            .unwrap_or("");

        csv_writer.write_record([
            &number,
            &pr.title,
            &pr.html_url,
            &pr.user.login,
            effective_state(pr),
            created,
            merged,
        ])?;
    }

    csv_writer.flush()?;

    Ok(())
}
