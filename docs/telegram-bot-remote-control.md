# Telegram Bot Remote Control

CC Switch can run a Telegram bot while the desktop app is open. The bot only accepts messages from chat IDs saved in local settings.

## Setup

1. Create a bot with BotFather and copy the bot token.
2. Open CC Switch settings and enable Telegram Remote Control.
3. Paste the token.
4. Add allowed chat IDs, separated by commas, spaces, or new lines.
5. Set the Codex restart command. The default is `codex`.
6. Save settings.

To discover your chat ID, message your bot once and use a trusted Telegram bot/user info tool, or inspect `getUpdates` from Telegram while setting it up. Group IDs are often negative numbers.

## Commands

- `/help`
- `/list <app>`
- `/current <app>`
- `/switch <app> <provider_id>`
- `/restart-proxy codex`
- `/restart-codex`

Supported app IDs are `claude`, `claude-desktop`, `codex`, `gemini`, `opencode`, `openclaw`, and `hermes`.

## Codex Restart

`/restart-codex` launches the locally saved Codex restart command. Telegram messages cannot provide shell commands.

On Windows, CC Switch opens a new command window for the saved command. Force-stopping existing `codex.exe` processes is optional and controlled only by the local settings checkbox.

CC Switch must be running for all Telegram remote control commands to work.
