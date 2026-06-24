# Telegram Bot Remote Control Plan

Build a Windows-friendly Telegram remote control feature for CC Switch. The bot will only accept commands from trusted chat IDs, reuse existing provider switching services, and restart Codex by launching a configured local command in a new window by default. Force-stopping existing Codex processes will be an explicit optional setting.

## Scope

- In: Telegram bot backend worker, frontend settings UI, chat ID allowlist, provider list/current/switch commands, Codex proxy restart, configurable Codex client launch command, optional force-stop toggle, tests, and docs.
- Out: Editing API keys over Telegram, arbitrary remote PowerShell execution, remote control while CC Switch is not running, and Linux/macOS service-specific behavior.

## Action Items

1. Add Telegram settings fields in `src-tauri/src/settings.rs`.
   - `telegram_bot_enabled`
   - `telegram_bot_token`
   - `telegram_allowed_chat_ids`
   - `telegram_codex_restart_command`
   - `telegram_codex_restart_force_stop`
   - Ensure frontend-facing settings redact or omit the stored bot token unless the existing settings pattern supports password-style preservation.

2. Add frontend settings controls.
   - Add an enable switch, token input, allowed chat IDs input, Codex restart command input, and force-stop checkbox.
   - Preserve existing secret values on save when the token input is blank, following the existing WebDAV password merge pattern in `src-tauri/src/commands/settings.rs`.
   - Show concise safety copy near the restart controls: Telegram messages must not be able to supply arbitrary commands.

3. Add a backend Telegram bot service module, likely `src-tauri/src/services/telegram_bot.rs`.
   - Use existing `reqwest` and `tokio` dependencies.
   - Use Telegram long polling via `getUpdates`.
   - Track update offset in memory.
   - Send replies with `sendMessage`.
   - Support clean start/stop or worker cancellation when settings change.

4. Wire the bot lifecycle into app startup.
   - Initialize the worker after `AppState` is managed in `src-tauri/src/lib.rs`.
   - Start only when `telegram_bot_enabled` is true and token plus allowlist are valid.
   - Restart or stop the worker after settings are saved if Telegram settings changed.

5. Implement strict authorization.
   - Reject all messages whose `chat.id` is not in `telegram_allowed_chat_ids`.
   - Log rejected attempts without storing message text or secrets.
   - Keep all command responses free of API keys, tokens, and full provider credentials.

6. Implement fixed command parsing.
   - `/help`
   - `/list <app>`
   - `/current <app>`
   - `/switch <app> <provider_id>`
   - `/restart-proxy codex`
   - `/restart-codex`
   - Accept app names already supported by `AppType`, but document that the initial restart command is Codex-specific.

7. Reuse existing provider and proxy services.
   - Use `ProviderService::list`, `ProviderService::current`, and `ProviderService::switch`.
   - For `/restart-proxy codex`, use existing proxy service stop/start or app takeover APIs without modifying live config directly.
   - Return switch warnings from `SwitchResult` in a compact Telegram reply.

8. Add a Windows Codex launch helper.
   - Default behavior: launch a new terminal/window running `telegram_codex_restart_command`.
   - Optional behavior: if `telegram_codex_restart_force_stop` is true, stop existing Codex processes by executable name before launching.
   - Do not accept process names or shell text from Telegram.
   - Validate that the saved command is non-empty and reasonably bounded before execution.

9. Add tests.
   - Command parser tests.
   - Chat ID authorization tests.
   - Settings secret-preservation tests.
   - Restart command validation tests.
   - Provider switch handler tests using existing test hooks where practical.

10. Validate and document.
    - Run targeted Rust tests for settings, parser, and provider command behavior.
    - Run frontend typecheck/tests if UI changes touch TypeScript.
    - Add user docs under `docs/` covering BotFather setup, allowed chat ID discovery, supported commands, and the requirement that CC Switch must be running on Windows.

## Decisions

- Codex restart will open a new Codex window by default.
- Killing existing Codex processes will be optional and controlled by a local settings checkbox, not by Telegram command text.
- The first implementation includes both backend and frontend settings UI.

## Risks

- Bot token leakage would allow attackers to send messages to the bot, so chat ID allowlisting is mandatory.
- Allowing arbitrary remote commands would be equivalent to a remote shell, so only a saved local command is allowed.
- Codex CLI sessions started outside CC Switch may have custom environment or working directory expectations; the launch command must be user-configurable.
- Some provider switches still require restarting the Codex client to fully reload model catalogs or config.
