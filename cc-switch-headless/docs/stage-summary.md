# CC Switch Headless Stage Summary

## Current Status

Design discussion completed. No implementation code has been created yet.

The user wants a new server/headless edition of CC Switch for VPS and pure SSH environments where desktop UI and Tauri cannot be used.

## Confirmed Product Direction

- Build a new independent project folder.
- Reuse/adapt logic from the existing CC Switch repository where practical.
- Target headless Linux servers first.
- Product form: CLI plus Web admin console.
- First supported tools: Claude Code, Codex, and Gemini CLI.
- Tech stack: Rust core/server/CLI plus React Web UI.
- Access mode: default `127.0.0.1` with optional `0.0.0.0`.
- Public binding requires authentication.

## Recommended New Project Name

`cc-switch-headless`

## Recommended Architecture

```text
cc-switch-headless/
  crates/
    ccs-core/
    ccs-server/
    ccs-cli/
  web/
  docs/
```

`ccs-core` should hold all business logic so both CLI and server reuse the same behavior.

## MVP Scope

Included:

- `ccs init`
- Provider add/list/edit/remove/switch
- Claude Code support
- Codex support
- Gemini CLI support
- SQLite state
- Config backup before writes
- Basic restore from backup
- Basic proxy start/stop/status
- HTTP API
- React Web admin console
- Token authentication

Deferred:

- MCP
- Skills
- Session Manager
- Usage statistics
- WebDAV/cloud sync
- OpenCode/OpenClaw/Hermes
- Tauri desktop UI
- Tray menu
- Auto updater
- Deep links
- Multi-user/RBAC

## Security Decisions

- Default bind host: `127.0.0.1`.
- Public bind requires token/password auth.
- Generate strong token during init.
- API uses `Authorization: Bearer <token>`.
- Web stores token in `sessionStorage`.
- Logs must redact API keys and authorization headers.
- No built-in HTTPS management in MVP; recommend reverse proxy for public deployments.

## Suggested First Implementation Slice

Start with Codex only for the first vertical slice:

1. Create Rust workspace and crates.
2. Implement config directory initialization.
3. Implement SQLite setup.
4. Implement provider model.
5. Implement Codex config read/write with backup.
6. Add CLI commands for Codex provider list/add/switch.
7. Add server API for the same.
8. Add minimal Web UI for login and provider switching.

After this works, add Claude Code and Gemini using the same interfaces.

## Existing Repository Areas To Inspect

- `src-tauri/src/provider.rs`
- `src-tauri/src/codex_config.rs`
- `src-tauri/src/gemini_config.rs`
- `src-tauri/src/claude_desktop_config.rs`
- `src-tauri/src/database`
- `src-tauri/src/proxy`
- `src-tauri/src/services`
- `src/components/providers`
- `src/components/proxy`
- `src/lib/api`

## Full Design Document

See `cc-switch-headless/docs/superpowers/specs/2026-06-01-cc-switch-headless-design.md`.
