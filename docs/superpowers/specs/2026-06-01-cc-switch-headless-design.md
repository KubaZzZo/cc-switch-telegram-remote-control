# CC Switch Headless Design

## Goal

Build a new headless/server-oriented edition of CC Switch for VPS and pure SSH environments where desktop UI, system tray, and Tauri windows are unavailable or not allowed.

The new project should be independent from the current Tauri desktop app, but it may reuse or adapt core Rust logic from this repository.

## Confirmed Decisions

- Product shape: CLI plus Web admin console.
- First supported tools: Claude Code, Codex, and Gemini CLI.
- Technology stack: Rust backend/core/CLI plus React Web UI.
- Access model: default local binding with optional public binding.
- Default security posture: safe for SSH tunnel usage, explicit authentication required for public binding.
- Implementation strategy: create a clean new project skeleton, then migrate/adapt selected Rust core logic from the desktop project.

## Recommended Architecture

```text
cc-switch-headless/
  Cargo.toml
  README.md
  config.example.toml

  crates/
    ccs-core/
      src/
        app.rs
        config/
        db/
        provider/
        tools/
          claude.rs
          codex.rs
          gemini.rs
        proxy/
        security/
        backup.rs
        error.rs

    ccs-server/
      src/
        main.rs
        api/
        auth.rs
        static_web.rs

    ccs-cli/
      src/
        main.rs
        commands/
          init.rs
          provider.rs
          proxy.rs
          server.rs
          config.rs

  web/
    package.json
    src/
      api/
      pages/
      components/
      main.tsx

  docs/
    stage-summary.md
    design.md
```

## Module Responsibilities

### ccs-core

Core library used by both CLI and server. It should not depend on Web UI or CLI concerns.

Responsibilities:

- Provider data model.
- SQLite persistence.
- Claude Code, Codex, and Gemini config read/write.
- Backup and restore.
- Atomic file writes.
- Proxy runtime core.
- Settings and path resolution.
- Redaction and security helpers.

### ccs-cli

SSH-first command-line interface.

Responsibilities:

- `ccs init`
- `ccs provider list --app <app>`
- `ccs provider add --app <app>`
- `ccs provider switch --app <app> --provider <id>`
- `ccs provider remove --app <app> --provider <id>`
- `ccs proxy start`
- `ccs proxy stop`
- `ccs proxy status`
- `ccs server start`

The CLI should be able to operate directly through `ccs-core` without requiring the server to run.

### ccs-server

HTTP API and static Web UI host.

Responsibilities:

- Start API service.
- Serve bundled React assets.
- Enforce authentication.
- Expose provider, proxy, settings, and status APIs.
- Reject unsafe public binding when auth is missing.
- Return stable JSON error responses.

### web

React admin console.

Responsibilities:

- Token login page.
- Provider list.
- Add/edit provider.
- Switch current provider.
- Proxy status/start/stop.
- Basic settings.
- Error and log display.

The Web UI must only communicate through the server API and must not expose arbitrary filesystem access.

## Runtime Modes

### Pure CLI

```bash
ccs provider list --app codex
ccs provider switch --app codex --provider openai
ccs proxy start
```

Used from SSH without browser access.

### Local Server

```bash
ccs server start --host 127.0.0.1 --port 8787
```

Default mode for Web admin access through SSH tunneling.

### Public Server

```bash
ccs server start --host 0.0.0.0 --port 8787
```

Allowed only when authentication is configured.

## Data Flow

```text
CLI / Web UI
    |
    v
ccs-cli / ccs-server
    |
    v
ccs-core
    |
    +-- SQLite state
    +-- Claude/Codex/Gemini config files
    +-- backups
    +-- proxy runtime
```

SQLite stores the headless app's state. The actual CLI tool config files should still be written in each tool's expected format.

## Default Filesystem Layout

```text
~/.config/cc-switch-headless/
  config.toml
  data.db
  logs/
  backups/
  web/
```

First MVP targets Linux/headless systems. Windows and macOS can be kept possible through path abstractions but are not first-release requirements.

Expected Linux tool paths:

```text
Claude Code: ~/.claude/
Codex:       ~/.codex/
Gemini:      ~/.gemini/
```

## Security Policy

- Bind to `127.0.0.1` by default.
- Require token/password auth for `0.0.0.0` or any non-loopback host.
- Generate a strong token during `ccs init`.
- Use `Authorization: Bearer <token>` for API calls.
- Store Web login token in browser `sessionStorage`.
- Redact API keys, authorization headers, and secret query parameters from logs.
- Require explicit confirmation for destructive actions.
- Treat the first version as a single-admin tool. No multi-user/RBAC system in MVP.
- Do not build HTTPS certificate management into the app initially. Recommend reverse proxies such as Caddy, Nginx, or Traefik for public deployments.

## MVP Scope

Included:

- Linux headless operation.
- Rust CLI.
- Rust HTTP server.
- React Web admin console.
- SQLite state.
- Claude Code provider management.
- Codex provider management.
- Gemini provider management.
- Provider add/list/edit/remove/switch.
- Config file backups before writes.
- Basic restore from backup.
- Basic proxy start/stop/status.
- Local-only default server mode.
- Public server mode with required auth.

Deferred:

- Tauri desktop UI.
- System tray.
- Auto update.
- Deep link registration.
- WebDAV/cloud sync.
- MCP management.
- Skills management.
- Session Manager.
- Usage statistics.
- OpenCode/OpenClaw/Hermes support.
- Full multilingual UI migration.
- Advanced graphical proxy rules.
- Built-in HTTPS certificate automation.
- Multi-user accounts and RBAC.

## Testing Strategy

- `ccs-core`: unit tests for provider models, config writes, atomic writes, backup/restore, path resolution, and redaction.
- `ccs-server`: integration tests for auth, public binding safety checks, and API responses.
- `ccs-cli`: command parsing and error-output tests.
- `web`: component tests for login, provider forms, provider switch flow, and proxy controls.
- Use temporary directories to simulate `~/.claude`, `~/.codex`, and `~/.gemini`.
- Avoid touching real user config files during tests.

## Migration Notes From Existing Project

Potentially reusable areas from the current desktop repository:

- Rust provider/config logic from `src-tauri/src/provider.rs`.
- Tool-specific config handling from `src-tauri/src/claude_desktop_config.rs`, `codex_config.rs`, and `gemini_config.rs`.
- Database patterns from `src-tauri/src/database`.
- Proxy logic from `src-tauri/src/proxy`.
- Service-layer patterns from `src-tauri/src/services`.
- Frontend ideas from `src/components/providers`, `src/components/proxy`, and `src/lib/api`.

Areas to avoid carrying over directly:

- Tauri window code.
- System tray integration.
- Desktop dialog/file picker assumptions.
- Auto updater integration.
- Deep link registration.
- Large UI files that mix desktop concerns with provider logic.

## Next Implementation Step

After approval, create `cc-switch-headless/` as a new independent folder and scaffold:

1. Rust workspace with `ccs-core`, `ccs-server`, and `ccs-cli`.
2. Minimal React Web app.
3. Shared config path initialization.
4. Token-based auth foundation.
5. Provider model and SQLite schema.
6. First Codex config read/write flow.

Codex is a good first tool because it is central to the target usage and gives an early end-to-end path for CLI, server, Web UI, database, and config writing.
