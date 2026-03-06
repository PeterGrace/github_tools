# GitHub Tools - Combined Application

This application's core requirements:
  - written in rust
  - uses clap command line with derived struct for arguments
  - uses clap subcommands to combine two separate functionalities
  - interacts with github api

## Application Structure

This is a combined CLI tool that merges two separate GitHub utilities:
1. **notify** - Notification management
2. **why-reviewer** - CODEOWNERS analysis

## Subcommands

### `notify` subcommand
Based on the original `ghnotify` application.

Operation:
  - reads a list of all pull request reviews for the github user,
  - marks the pull request notification as 'read' if the PR was already merged or closed

Implementation:
  - uses `octocrab` for GitHub API interactions
  - supports pagination for notifications
  - provides detailed output with status indicators

### `why-reviewer` subcommand
Based on the original `gh_why_reviewer` application.

Operation:
  - takes a repository and a PR number as arguments,
  - uses the github api to retrieve the CODEOWNERS,
  - parses the CODEOWNERS to understand what paths I would be automatically tagged for review,
  - checks the PR in question to find which files changed that match the CODEOWNERS,
  - thereby telling me why I was tagged in for the specified PR.

Implementation:
  - uses `octocrab` for GitHub API interactions
  - supports multiple CODEOWNERS file locations (.github/CODEOWNERS, CODEOWNERS, docs/CODEOWNERS)
  - implements pattern matching for CODEOWNERS rules
  - follows "last match wins" rule from GitHub's CODEOWNERS specification

## Authentication

Both subcommands share a common authentication mechanism:
  - GitHub token can be provided via `--token` flag or `GITHUB_TOKEN` environment variable
  - Token is specified at the root command level and passed to subcommands
  - Both subcommands use `octocrab` as the standardized GitHub API client

## Module Organization

- `src/main.rs` - CLI definition and routing
- `src/notify.rs` - Notification management implementation
- `src/why_reviewer.rs` - CODEOWNERS analysis implementation

Each module is self-contained with its own types and helper functions.
