use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use octocrab::Octocrab;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
struct CodeOwnerRule {
    pattern: String,
    owners: Vec<String>,
}

#[derive(Debug)]
struct Match {
    file: String,
    rule: CodeOwnerRule,
}

#[derive(Deserialize)]
struct UserTeamOrg {
    login: String,
}

#[derive(Deserialize)]
struct UserTeam {
    slug: String,
    organization: UserTeamOrg,
}

async fn get_current_user(octocrab: &Octocrab) -> Result<String> {
    let user = octocrab
        .current()
        .user()
        .await
        .context("Failed to fetch current user")?;
    Ok(user.login)
}

/// Fetches all teams the authenticated user belongs to, as a set of "org/team-slug" strings.
async fn get_user_teams(octocrab: &Octocrab) -> Result<HashSet<String>> {
    let first_page: octocrab::Page<UserTeam> = octocrab
        .get("/user/teams", Some(&[("per_page", "100")]))
        .await
        .context("Failed to fetch user teams")?;

    let all_teams = octocrab
        .all_pages::<UserTeam>(first_page)
        .await
        .context("Failed to paginate user teams")?;

    Ok(all_teams
        .into_iter()
        .map(|t| format!("{}/{}", t.organization.login.to_lowercase(), t.slug.to_lowercase()))
        .collect())
}

fn user_is_owner(username: &str, user_teams: &HashSet<String>, owners: &[String]) -> bool {
    for owner in owners {
        let owner = owner.trim_start_matches('@');
        if owner.contains('/') {
            // Team entry like "org/team-slug"
            if user_teams.contains(&owner.to_lowercase()) {
                return true;
            }
        } else if owner.eq_ignore_ascii_case(username) {
            return true;
        }
    }
    false
}

async fn get_codeowners(octocrab: &Octocrab, owner: &str, repo: &str) -> Result<String> {
    let locations = [
        "CODEOWNERS",
        ".github/CODEOWNERS",
        "docs/CODEOWNERS",
    ];

    for location in locations {
        match octocrab
            .repos(owner, repo)
            .get_content()
            .path(location)
            .send()
            .await
        {
            Ok(content) => {
                if let Some(file) = content.items.first() {
                    if let Some(content_str) = &file.content {
                        let decoded = String::from_utf8(
                            general_purpose::STANDARD
                                .decode(content_str.replace('\n', ""))
                                .context("Failed to decode CODEOWNERS content")?,
                        )?;
                        return Ok(decoded);
                    }
                }
            }
            Err(_) => continue,
        }
    }

    anyhow::bail!("CODEOWNERS file not found in repository")
}

fn parse_codeowners(content: &str) -> Vec<CodeOwnerRule> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                return None;
            }

            Some(CodeOwnerRule {
                pattern: parts[0].to_string(),
                owners: parts[1..].iter().map(|s| s.to_string()).collect(),
            })
        })
        .collect()
}

async fn get_pr_files(
    octocrab: &Octocrab,
    owner: &str,
    repo: &str,
    pr_number: u64,
) -> Result<Vec<String>> {
    let files = octocrab
        .pulls(owner, repo)
        .list_files(pr_number)
        .await
        .context("Failed to fetch PR files")?;

    Ok(files.items.iter().map(|f| f.filename.clone()).collect())
}

fn find_matching_rules(files: &[String], rules: &[CodeOwnerRule]) -> Vec<Match> {
    let mut matches = Vec::new();

    for file in files {
        // CODEOWNERS uses "last match wins"
        let mut last_match: Option<&CodeOwnerRule> = None;

        for rule in rules {
            if file_matches_pattern(file, &rule.pattern) {
                last_match = Some(rule);
            }
        }

        if let Some(rule) = last_match {
            matches.push(Match {
                file: file.clone(),
                rule: CodeOwnerRule {
                    pattern: rule.pattern.clone(),
                    owners: rule.owners.clone(),
                },
            });
        }
    }

    matches
}

fn file_matches_pattern(file: &str, pattern: &str) -> bool {
    // Strip leading slash for root-anchored patterns
    let pattern = pattern.trim_start_matches('/');

    if pattern == "*" {
        return true;
    }

    if pattern.ends_with("/*") {
        let dir = &pattern[..pattern.len() - 2];
        return file.starts_with(&format!("{}/", dir));
    }

    if pattern.ends_with('/') {
        // Directory pattern: matches any file within this directory
        return file.starts_with(pattern);
    }

    if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len() - 1];
        return file.starts_with(prefix);
    }

    if pattern.starts_with("*.") {
        let ext = &pattern[1..];
        return file.ends_with(ext);
    }

    // Exact match, or file lives under this path as a directory, or path-component suffix match
    file == pattern
        || file.starts_with(&format!("{}/", pattern))
        || file.ends_with(&format!("/{}", pattern))
}

fn display_results(username: &str, matches: &[Match]) {
    if matches.is_empty() {
        println!(
            "\nNo CODEOWNERS rules assign @{} (or a team the user is on) as a reviewer for the changed files.",
            username
        );
        return;
    }

    let mut pattern_groups: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();

    for m in matches {
        let entry = pattern_groups
            .entry(m.rule.pattern.clone())
            .or_insert_with(|| (Vec::new(), m.rule.owners.clone()));
        entry.0.push(m.file.clone());
    }

    println!("\n=== Why @{} Was Tagged as Reviewer ===\n", username);

    for (pattern, (files, owners)) in pattern_groups.iter() {
        let file_count = files.len();
        println!("Pattern: {}", pattern);
        println!("  Owners: {}", owners.join(", "));
        println!(
            "  Matched {} file{}:",
            file_count,
            if file_count == 1 { "" } else { "s" }
        );
        for file in files {
            println!("    - {}", file);
        }
        println!();
    }
}

pub async fn run(token: String, repo: String, pr: u64, verbose: bool) -> Result<()> {
    let repo_parts: Vec<&str> = repo.split('/').collect();
    if repo_parts.len() != 2 {
        anyhow::bail!("Repository must be in format 'owner/repo'");
    }
    let (owner, repo_name) = (repo_parts[0], repo_parts[1]);

    let octocrab = Octocrab::builder()
        .personal_token(token)
        .build()?;

    let current_user = get_current_user(&octocrab).await?;
    let user_teams = get_user_teams(&octocrab).await?;
    println!(
        "Analyzing PR #{} in {}/{} for @{} (member of {} team{})...",
        pr, owner, repo_name, current_user,
        user_teams.len(),
        if user_teams.len() == 1 { "" } else { "s" }
    );

    if verbose {
        if user_teams.is_empty() {
            println!("  [verbose] No team memberships found.");
        } else {
            println!("  [verbose] Teams:");
            let mut sorted: Vec<&String> = user_teams.iter().collect();
            sorted.sort();
            for t in sorted {
                println!("    - {}", t);
            }
        }
    }

    let codeowners = get_codeowners(&octocrab, owner, repo_name).await?;
    println!("\nFound CODEOWNERS file");

    let rules = parse_codeowners(&codeowners);
    println!("Parsed {} CODEOWNERS rules", rules.len());

    let changed_files = get_pr_files(&octocrab, owner, repo_name, pr).await?;
    println!("\nPR has {} changed files", changed_files.len());

    let all_matches = find_matching_rules(&changed_files, &rules);

    if verbose {
        println!("\n  [verbose] CODEOWNERS rule matches before ownership filter:");
        if all_matches.is_empty() {
            println!("    (no rules matched any changed files)");
        }
        for m in &all_matches {
            let owned = user_is_owner(&current_user, &user_teams, &m.rule.owners);
            println!(
                "    {} => pattern '{}' owners [{}] -- you are{}an owner",
                m.file,
                m.rule.pattern,
                m.rule.owners.join(", "),
                if owned { " " } else { " NOT " }
            );
        }
    }

    // Filter to only rules where the current user is a direct owner or team member
    let user_matches: Vec<_> = all_matches
        .into_iter()
        .filter(|m| user_is_owner(&current_user, &user_teams, &m.rule.owners))
        .collect();

    display_results(&current_user, &user_matches);

    Ok(())
}
