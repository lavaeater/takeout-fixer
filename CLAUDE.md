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

Environment variables are loaded from `.env` via `dotenv`. See `example.env` for required variables (`GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `DATABASE_URL`, `TARGET_FOLDER`).

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

**Two views:** `FileListWidgetViewState::Files` (browse Google Drive) and `::Processing` (monitor ZIP pipeline). Switching views is done with the `SwitchView` action (`v` key). `s` stores Drive files to DB / toggles processing depending on view.

**Database:** SQLite by default (`DATABASE_URL` env var); SeaORM also supports Postgres. The three tables track ZIPs (`takeout_zip`), files within ZIPs (`file_in_zip`), and output media files (`media_file`) with status strings defined as constants in `src/db.rs`.

---

## Known Issues / Code Smells

These were identified in a code review and should be addressed before adding new features.

### Critical

**New DB connection per operation** — `get_db_connection()` opens a fresh connection on every call, including multiple times within a single function (e.g., `create_file_in_zip` opens it twice). With 20 concurrent tasks this exhausts connections. Fix: create a connection pool once at startup and pass it around or share via `Arc`.

**TOCTOU race in `fetch_next_takeout`** — fetches a row then updates its status in a separate query with no transaction. Two concurrent workers can claim the same item. Fix: wrap fetch + update in a `db.transaction(...)`.

### Serious

**Two full ZIP scans in `examine_zip_with_progress`** — the archive is decompressed and iterated twice (once to count entries for progress, once to extract). Fix: single-pass with a running byte count for progress.

**`unwrap()`/`expect()` in spawned tasks** — panics inside `tokio::spawn` silently kill the task without decrementing the task counter, permanently leaking a concurrency slot. Affects `processing.rs:79`, `118`, `148`, `500` and `db.rs:110`.

**Task slot leak on `fetch_next_takeout` error** — if the DB call returns `Err`, `stop_task` is never called. The slot is permanently consumed until restart.

### Moderate

**Statuses should be enums** — `ZIP_STATUS_*` and `MEDIA_STATUS_*` are stringly-typed. Error details are appended as `format!("{}: {}", ZIP_STATUS_FAILED, err)` producing strings that can never match the constant again. Use `ZipStatus`/`MediaStatus` enums with a `Failed(String)` variant.

**Poll interval comment is wrong** — `processing.rs:57` says "Poll every 3 seconds" but the interval is `Duration::from_millis(10)`.

**CSRF token not verified in OAuth flow** — `_csrf_state` from the authorization URL and `_state` from the redirect are both discarded without comparison, removing CSRF protection.

### Minor

**Dead code** — `store_files`, `fetch_new_media_and_set_status_to_processing`, `fetch_json_without_media_and_set_status_to_processing`, `fetch_file_in_zip_by_id`, and `process_json_file` all have `#[allow(dead_code)]`. The entire `JsonProcessing` task block is commented out. These should be wired up or deleted.

**`path.contains(".json")` is fragile** — `db.rs:102`: a file named `photo.json.jpg` would be misclassified. Use `path.ends_with(".json")`.

**`local_path` empty-string sentinel** — `db.rs:157`: uses `Set("".to_string())` where `Set(None)` on an `Option<String>` column would be correct.
