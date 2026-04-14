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
    pull_request: Option<PrLinks>,
}

// Search results include a `pull_request` object on issues that are PRs.
// We use it only for the `merged_at` timestamp when the top-level field is absent.
#[derive(Debug, Deserialize)]
struct PrLinks {
    merged_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    items: Vec<PullRequest>,
    total_count: u64,
}

pub async fn run(
    token: String,
    repo: String,
    after: Option<String>,
    before: Option<String>,
    query: Option<String>,
    output: Option<String>,
) -> Result<()> {
    let repo_parts: Vec<&str> = repo.split('/').collect();
    if repo_parts.len() != 2 {
        anyhow::bail!("Repository must be in format 'owner/repo'");
    }

    let search_query = build_query(&repo, after.as_deref(), before.as_deref(), query.as_deref());

    let octocrab = Octocrab::builder()
        .personal_token(token)
        .build()?;

    eprintln!("Search query: {}", search_query);

    let prs = fetch_all_prs(&octocrab, &search_query).await?;

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

fn build_query(repo: &str, after: Option<&str>, before: Option<&str>, query: Option<&str>) -> String {
    match query {
        Some(q) => {
            // If the user's query doesn't already scope to a repo, prepend ours.
            if q.contains("repo:") {
                q.to_string()
            } else {
                format!("repo:{} {}", repo, q)
            }
        }
        None => {
            let date_range = match (after, before) {
                (Some(a), Some(b)) => format!(" created:{}..{}", a, b),
                (Some(a), None) => format!(" created:>={}",  a),
                (None, Some(b)) => format!(" created:<={}", b),
                (None, None) => String::new(),
            };
            format!("repo:{} is:pr{}", repo, date_range)
        }
    }
}

async fn fetch_all_prs(octocrab: &Octocrab, query: &str) -> Result<Vec<PullRequest>> {
    let mut all_prs = Vec::new();
    let mut page = 1u32;
    let per_page = 100u32;

    loop {
        let route = format!(
            "/search/issues?q={}&sort=created&order=asc&per_page={}&page={}",
            urlencoding::encode(query),
            per_page,
            page,
        );

        let result: SearchResult = octocrab
            .get(&route, None::<&()>)
            .await
            .context("Failed to fetch PRs via search API")?;

        let count = result.items.len();
        all_prs.extend(result.items);

        if count < per_page as usize {
            break;
        }

        // GitHub's search API caps at 1 000 results (10 pages × 100).
        if all_prs.len() as u64 >= result.total_count || all_prs.len() >= 1000 {
            break;
        }

        page += 1;
    }

    Ok(all_prs)
}

fn effective_state(pr: &PullRequest) -> &str {
    let merged = pr.merged_at.as_deref()
        .or_else(|| pr.pull_request.as_ref()?.merged_at.as_deref());
    if merged.is_some() {
        "merged"
    } else {
        &pr.state
    }
}

fn merged_at(pr: &PullRequest) -> &str {
    pr.merged_at.as_deref()
        .or_else(|| pr.pull_request.as_ref()?.merged_at.as_deref())
        .and_then(|d| d.get(..10))
        .unwrap_or("")
}

fn write_csv(writer: Box<dyn Write>, prs: &[PullRequest]) -> Result<()> {
    let mut csv_writer = csv::Writer::from_writer(writer);

    csv_writer.write_record(["number", "title", "url", "author", "state", "created_at", "merged_at"])?;

    for pr in prs {
        let number = pr.number.to_string();
        let created = pr.created_at.get(..10).unwrap_or(&pr.created_at);

        csv_writer.write_record([
            &number,
            &pr.title,
            &pr.html_url,
            &pr.user.login,
            effective_state(pr),
            created,
            merged_at(pr),
        ])?;
    }

    csv_writer.flush()?;

    Ok(())
}
