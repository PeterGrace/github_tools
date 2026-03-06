use anyhow::{Context, Result};
use octocrab::Octocrab;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Notification {
    pub id: String,
    pub subject: Subject,
    pub repository: Repository,
    pub unread: bool,
}

#[derive(Debug, Deserialize)]
pub struct Subject {
    pub title: String,
    pub url: Option<String>,
    #[serde(rename = "type")]
    pub subject_type: String,
}

#[derive(Debug, Deserialize)]
pub struct Repository {
    pub full_name: String,
}

#[derive(Debug, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub merged: bool,
}

pub struct GitHubClient {
    octocrab: Octocrab,
}

impl GitHubClient {
    pub fn new(token: String) -> Result<Self> {
        let octocrab = Octocrab::builder()
            .personal_token(token)
            .build()
            .context("Failed to build Octocrab client")?;

        Ok(Self { octocrab })
    }

    /// Fetch all unread notifications for the authenticated user (with pagination)
    pub async fn get_notifications(&self) -> Result<Vec<Notification>> {
        let mut all_notifications = Vec::new();
        let mut page = 1u32;
        let per_page = 100u8;

        loop {
            let route = format!(
                "/notifications?participating=true&per_page={}&page={}",
                per_page, page
            );

            let notifications: Vec<Notification> = self
                .octocrab
                .get(&route, None::<&()>)
                .await
                .context("Failed to fetch notifications")?;

            let notification_count = notifications.len();
            all_notifications.extend(notifications);

            // If we got fewer notifications than per_page, we're done
            if notification_count < per_page as usize {
                break;
            }

            page += 1;
        }

        Ok(all_notifications)
    }

    /// Get pull request details from a subject URL
    pub async fn get_pull_request(&self, api_url: &str) -> Result<PullRequest> {
        // Extract the path from the full API URL
        let path = api_url
            .strip_prefix("https://api.github.com")
            .unwrap_or(api_url);

        let pr: PullRequest = self
            .octocrab
            .get(path, None::<&()>)
            .await
            .context("Failed to fetch pull request")?;

        Ok(pr)
    }

    /// Mark a notification thread as read
    pub async fn mark_notification_as_read(&self, thread_id: &str) -> Result<()> {
        let route = format!("/notifications/threads/{}", thread_id);

        // PATCH request doesn't return content, just check for success
        self.octocrab
            .patch::<(), _, _>(&route, None::<&()>)
            .await
            .context("Failed to mark notification as read")?;

        Ok(())
    }

    /// Process notifications and mark merged or closed PRs as read
    pub async fn process_notifications(&self) -> Result<()> {
        let notifications = self.get_notifications().await?;

        println!("Found {} notification(s)", notifications.len());

        let mut marked_count = 0;
        let mut skipped_count = 0;

        for notification in notifications {
            if !notification.unread {
                continue;
            }

            // Only process pull request notifications
            if notification.subject.subject_type != "PullRequest" {
                println!(
                    "  [SKIP] {} - {} (not a PR)",
                    notification.repository.full_name,
                    notification.subject.title
                );
                skipped_count += 1;
                continue;
            }

            // Get PR details to check if it's merged or closed
            if let Some(api_url) = &notification.subject.url {
                match self.get_pull_request(api_url).await {
                    Ok(pr) => {
                        // Mark as read if merged or closed
                        if pr.merged || pr.state == "closed" {
                            let status_label = if pr.merged { "MERGED" } else { "CLOSED" };
                            println!(
                                "  [{}] {} - #{} {}",
                                status_label,
                                notification.repository.full_name,
                                pr.number,
                                pr.title
                            );

                            // Mark as read
                            match self.mark_notification_as_read(&notification.id).await {
                                Ok(_) => {
                                    println!("    ✓ Marked as read");
                                    marked_count += 1;
                                }
                                Err(e) => {
                                    println!("    ✗ Failed to mark as read: {}", e);
                                }
                            }
                        } else {
                            println!(
                                "  [OPEN] {} - #{} {} ({})",
                                notification.repository.full_name,
                                pr.number,
                                pr.title,
                                pr.state
                            );
                            skipped_count += 1;
                        }
                    }
                    Err(e) => {
                        println!(
                            "  [ERROR] {} - {} - Failed to fetch PR: {}",
                            notification.repository.full_name,
                            notification.subject.title,
                            e
                        );
                        skipped_count += 1;
                    }
                }
            } else {
                println!(
                    "  [SKIP] {} - {} (no API URL)",
                    notification.repository.full_name,
                    notification.subject.title
                );
                skipped_count += 1;
            }
        }

        // Re-query notifications to get the current count of unread PR notifications
        println!("\nRefreshing notification list...");
        let updated_notifications = self.get_notifications().await?;

        let remaining_pr_count = updated_notifications
            .iter()
            .filter(|n| n.unread && n.subject.subject_type == "PullRequest")
            .count();

        println!("\nSummary:");
        println!("  Marked as read: {}", marked_count);
        println!("  Skipped: {}", skipped_count);
        println!("  Remaining unread PR notifications: {}", remaining_pr_count);

        Ok(())
    }
}

pub async fn run(token: String, _username: Option<String>) -> Result<()> {
    println!("GitHub Notification Manager\n");

    // Create GitHub client
    let client = GitHubClient::new(token)?;

    // Process notifications: fetch, check if PRs are merged/closed, and mark as read
    client.process_notifications().await?;

    Ok(())
}
