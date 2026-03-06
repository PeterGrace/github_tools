# GitHub Tools

A combined Rust CLI application that provides GitHub utilities for managing notifications and understanding reviewer assignments.

## Features

### 1. Notification Manager (`notify`)
Manages GitHub pull request notifications by:
- Fetching all participating PR notifications
- Checking if PRs are merged or closed
- Automatically marking those notifications as read
- Providing a summary of actions taken

### 2. Why Reviewer (`why-reviewer`)
Explains why you were tagged as a reviewer on a PR by:
- Retrieving the repository's CODEOWNERS file
- Parsing CODEOWNERS rules (supports `CODEOWNERS`, `.github/CODEOWNERS`, `docs/CODEOWNERS`)
- Analyzing which changed files in the PR match your CODEOWNERS patterns
- Resolving team memberships so team-based ownership is detected
- Following the "last match wins" rule from GitHub's CODEOWNERS specification

### 3. Commit List (`commit-list`)
Exports commits from a repository within a date range to CSV by:
- Fetching all commits between `--after` and `--before` dates (inclusive)
- Outputting CSV with columns: `date`, `commit_url`, `message`, `author`
- Writing to a file or stdout

## Installation

```bash
cargo build --release
```

The binary will be available at `target/release/github-tools`.

## Usage

### Authentication

Both commands require a GitHub personal access token. You can provide it in two ways:

1. Environment variable:
   ```bash
   export GITHUB_TOKEN="your_token_here"
   ```

2. Command-line flag:
   ```bash
   github-tools --token "your_token_here" <subcommand>
   ```

### Notify Command

Mark merged or closed PR notifications as read:

```bash
github-tools notify
```

### Why Reviewer Command

Analyze why you were tagged as a reviewer:

```bash
github-tools why-reviewer --repo owner/repo --pr 123
```

Use `--verbose` to print team memberships, all matched CODEOWNERS rules, and ownership decisions:

```bash
github-tools why-reviewer --repo owner/repo --pr 123 --verbose
```

### Commit List Command

Export commits in a date range to CSV:

```bash
github-tools commit-list --repo owner/repo --after 2024-01-01 --before 2024-01-31
```

Write output to a file instead of stdout:

```bash
github-tools commit-list --repo owner/repo --after 2024-01-01 --before 2024-01-31 --output commits.csv
```

## Examples

```bash
# Mark closed/merged PR notifications as read
github-tools notify

# Find out why you're a reviewer on PR #456 in the kubernetes/kubernetes repo
github-tools why-reviewer --repo kubernetes/kubernetes --pr 456

# Same, with verbose output showing all rule matches and team memberships
github-tools why-reviewer --repo kubernetes/kubernetes --pr 456 --verbose

# Export January 2024 commits from a repo to a CSV file
github-tools commit-list --repo owner/repo --after 2024-01-01 --before 2024-01-31 --output jan.csv

# Use explicit token
github-tools --token ghp_xxx notify
```

## Requirements

- Rust 1.70 or later
- GitHub personal access token with appropriate permissions:
  - `notifications` scope for the `notify` command
  - `repo` scope for the `why-reviewer` and `commit-list` commands

## Architecture

The application is structured with:
- `main.rs`: CLI argument parsing and routing
- `notify.rs`: Notification management functionality
- `why_reviewer.rs`: CODEOWNERS analysis functionality
- `commit_list.rs`: Commit export functionality

All subcommands use `octocrab` for GitHub API interactions.
