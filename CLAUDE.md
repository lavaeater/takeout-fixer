# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build

# Run the TUI application
cargo run

# Run tests
cargo test

# Run a single test
cargo test test_name

# Lint
cargo clippy

# Run migrations (from the migration/ directory)
cargo run -- up
cargo run -- fresh   # drop all tables and reapply
cargo run -- status
```

Environment variables are loaded from `.env` via `dotenv`. See `example.env` for required variables (`GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `DATABASE_URL`).

## Architecture

This is a Rust TUI app (ratatui + crossterm + tokio) that automates fixing Google Takeout ZIP files: downloading them from Google Drive, extracting/examining them, applying metadata from JSON sidecar files to media files, and then removing processed ZIPs.

**Workspace members:**
- Root crate — the TUI application
- `entity/` — SeaORM entity models (`takeout_zip`, `file_in_zip`, `media_file`)
- `migration/` — SeaORM migration CLI

**Key modules:**
- `src/app.rs` — thin `App` struct wrapping `FileListWidget`
- `src/file_list_widget/` — the core widget, split into:
  - `mod.rs` — `FileListWidget` (holds `Arc<RwLock<FileListState>>`), `DriveItem`, state management
  - `ui_actions.rs` — keyboard event handling, `UiActions` enum
  - `rendering.rs` — ratatui rendering
  - `processing.rs` — async processing pipeline (download → examine → media/JSON processing → remove)
- `src/drive.rs` — Google OAuth2 login and Google Drive API calls (list, download)
- `src/db.rs` — SeaORM database operations; status constants for zips and media files
- `src/media_utils.rs` — EXIF/metadata utilities using `nom-exif`
- `src/event.rs` — crossterm event loop wrapper
- `src/tui.rs` — terminal init/teardown

**Concurrency pattern:** `FileListWidget` is cloned cheaply (it wraps `Arc<RwLock<FileListState>>`). Background work is spawned with `tokio::spawn(this.clone().some_async_method())`. The processing pipeline polls on a 10ms interval and uses `Task` + `max_task_counts` / `task_counts` to cap concurrent workers per task type (e.g., 20 concurrent media processing tasks, 5 downloads).

**Two views:** `FileListWidgetViewState::Files` (browse Google Drive) and `::Processing` (monitor ZIP pipeline). Switching views is done with the `SwitchView` action.

**Database:** SQLite by default (`DATABASE_URL` env var); SeaORM also supports Postgres. The three tables track ZIPs (`takeout_zip`), files within ZIPs (`file_in_zip`), and output media files (`media_file`) with status strings defined as constants in `src/db.rs`.
