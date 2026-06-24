use crate::app_config::AppType;
use crate::error::AppError;
use crate::provider::Provider;
use crate::services::ProviderService;
use crate::settings::TelegramBotSettings;
use crate::store::AppState;
use once_cell::sync::OnceCell;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

static GLOBAL_TELEGRAM_SERVICE: OnceCell<Arc<TelegramBotService>> = OnceCell::new();

pub fn init_global_service(state: Arc<AppState>) -> Arc<TelegramBotService> {
    let service = Arc::new(TelegramBotService::new(state));
    let _ = GLOBAL_TELEGRAM_SERVICE.set(service.clone());
    service
}

pub fn global_service() -> Option<Arc<TelegramBotService>> {
    GLOBAL_TELEGRAM_SERVICE.get().cloned()
}

#[derive(Clone)]
pub struct TelegramBotService {
    state: Arc<AppState>,
    client: Client,
    worker: Arc<Mutex<Option<BotWorker>>>,
}

struct BotWorker {
    task: JoinHandle<()>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramCommand {
    Help,
    List { app: AppType },
    Current { app: AppType },
    Switch { app: AppType, provider_id: String },
    RestartProxyCodex,
    RestartCodex,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TelegramButtonAction {
    Menu,
    App { app: AppType },
    List { app: AppType },
    Current { app: AppType },
    SwitchMenu { app: AppType },
    Switch { app: AppType, provider_id: String },
    RestartProxyCodex,
    RestartCodex,
    Unknown,
}

#[derive(Debug, Clone)]
struct BotRuntimeConfig {
    token: String,
    allowed_chat_ids: HashSet<i64>,
    codex_restart_command: String,
    codex_restart_force_stop: bool,
}

#[derive(Debug, Deserialize)]
struct GetUpdatesResponse {
    ok: bool,
    result: Vec<TelegramUpdate>,
}

#[derive(Debug, Deserialize)]
struct TelegramUpdate {
    update_id: i64,
    message: Option<TelegramMessage>,
    callback_query: Option<TelegramCallbackQuery>,
}

#[derive(Debug, Deserialize)]
struct TelegramMessage {
    message_id: Option<i64>,
    chat: TelegramChat,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TelegramChat {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct TelegramCallbackQuery {
    id: String,
    from: TelegramUser,
    message: Option<TelegramMessage>,
    data: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TelegramUser {
    id: i64,
}

#[derive(Debug, Serialize)]
struct InlineKeyboardMarkup {
    inline_keyboard: Vec<Vec<InlineKeyboardButton>>,
}

#[derive(Debug, Serialize)]
struct InlineKeyboardButton {
    text: String,
    callback_data: String,
}

struct BotReply {
    text: String,
    keyboard: Option<InlineKeyboardMarkup>,
}

impl TelegramBotService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            client: Client::new(),
            worker: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn apply_current_settings(&self) -> Result<(), AppError> {
        self.apply_settings(crate::settings::get_settings().telegram_bot)
            .await
    }

    pub async fn apply_settings(
        &self,
        settings: Option<TelegramBotSettings>,
    ) -> Result<(), AppError> {
        let runtime = match settings {
            Some(settings) => match BotRuntimeConfig::from_settings(settings) {
                Ok(runtime) => Some(runtime),
                Err(err) => {
                    self.stop().await;
                    log::warn!("Telegram bot disabled: {err}");
                    None
                }
            },
            None => None,
        };

        let mut guard = self.worker.lock().await;
        if let Some(worker) = guard.take() {
            worker.task.abort();
        }

        if let Some(runtime) = runtime {
            let service = self.clone();
            let task = tokio::spawn(async move {
                service.run(runtime).await;
            });
            *guard = Some(BotWorker { task });
            log::info!("Telegram bot worker started");
        } else {
            log::info!("Telegram bot worker stopped");
        }

        Ok(())
    }

    pub async fn stop(&self) {
        let mut guard = self.worker.lock().await;
        if let Some(worker) = guard.take() {
            worker.task.abort();
        }
    }

    async fn run(self, runtime: BotRuntimeConfig) {
        let mut offset: Option<i64> = None;
        loop {
            match self.poll_once(&runtime, offset).await {
                Ok(next_offset) => offset = next_offset,
                Err(err) => {
                    log::warn!("Telegram bot poll failed: {err}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn poll_once(
        &self,
        runtime: &BotRuntimeConfig,
        offset: Option<i64>,
    ) -> Result<Option<i64>, String> {
        let url = telegram_api_url(&runtime.token, "getUpdates");
        let mut request = self.client.get(url).query(&[
            ("timeout", "30"),
            ("allowed_updates", r#"["message","callback_query"]"#),
        ]);
        if let Some(offset) = offset {
            request = request.query(&[("offset", offset.to_string())]);
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("getUpdates request failed: {}", e.without_url()))?
            .error_for_status()
            .map_err(|e| format!("getUpdates returned HTTP error: {}", e.without_url()))?
            .json::<GetUpdatesResponse>()
            .await
            .map_err(|e| format!("getUpdates JSON parse failed: {}", e.without_url()))?;

        if !response.ok {
            return Err("getUpdates returned ok=false".to_string());
        }

        let mut next_offset = offset;
        for update in response.result {
            next_offset = Some(update.update_id + 1);
            if let Some(message) = update.message {
                self.handle_message(runtime, message).await;
            }
            if let Some(callback_query) = update.callback_query {
                self.handle_callback_query(runtime, callback_query).await;
            }
        }

        Ok(next_offset)
    }

    async fn handle_message(&self, runtime: &BotRuntimeConfig, message: TelegramMessage) {
        let chat_id = message.chat.id;
        if !runtime.allowed_chat_ids.contains(&chat_id) {
            log::warn!("Rejected Telegram message from unauthorized chat id {chat_id}");
            return;
        }

        let text = message.text.unwrap_or_default();
        let command = parse_command(&text);
        let reply = match handle_command(&self.state, runtime, command).await {
            Ok(reply) => reply,
            Err(err) => BotReply::text(format!("错误: {err}")),
        };

        if let Err(err) = send_message(&self.client, &runtime.token, chat_id, &reply).await {
            log::warn!("Failed to send Telegram reply: {err}");
        }
    }

    async fn handle_callback_query(
        &self,
        runtime: &BotRuntimeConfig,
        callback_query: TelegramCallbackQuery,
    ) {
        let chat_id = callback_query
            .message
            .as_ref()
            .map(|message| message.chat.id)
            .unwrap_or(callback_query.from.id);
        if !runtime.allowed_chat_ids.contains(&chat_id) {
            log::warn!("Rejected Telegram callback from unauthorized chat id {chat_id}");
            let _ = answer_callback_query(
                &self.client,
                &runtime.token,
                &callback_query.id,
                Some("未授权"),
            )
            .await;
            return;
        }

        let action = parse_button_action(callback_query.data.as_deref().unwrap_or_default());
        let reply = match handle_button_action(&self.state, runtime, action).await {
            Ok(reply) => reply,
            Err(err) => BotReply::text(format!("错误: {err}")),
        };

        if let Err(err) =
            answer_callback_query(&self.client, &runtime.token, &callback_query.id, None).await
        {
            log::warn!("Failed to answer Telegram callback: {err}");
        }

        let edit_target = callback_query.message.as_ref().and_then(|message| {
            message
                .message_id
                .map(|message_id| (message.chat.id, message_id))
        });
        let result = if let Some((chat_id, message_id)) = edit_target {
            edit_message(&self.client, &runtime.token, chat_id, message_id, &reply).await
        } else {
            send_message(&self.client, &runtime.token, chat_id, &reply).await
        };
        if let Err(err) = result {
            log::warn!("Failed to send Telegram callback reply: {err}");
        }
    }
}

impl BotRuntimeConfig {
    fn from_settings(mut settings: TelegramBotSettings) -> Result<Self, String> {
        settings.normalize();
        if !settings.enabled {
            return Err("disabled".to_string());
        }
        if settings.token.is_empty() {
            return Err("bot token is empty".to_string());
        }
        let allowed_chat_ids = parse_allowed_chat_ids(&settings.allowed_chat_ids)?;
        validate_restart_command(&settings.codex_restart_command)?;
        Ok(Self {
            token: settings.token,
            allowed_chat_ids,
            codex_restart_command: settings.codex_restart_command,
            codex_restart_force_stop: settings.codex_restart_force_stop,
        })
    }
}

pub fn parse_allowed_chat_ids(input: &str) -> Result<HashSet<i64>, String> {
    let ids: Result<HashSet<_>, _> = input
        .split(|c: char| c == ',' || c == '\n' || c == ';' || c.is_whitespace())
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<i64>()
                .map_err(|_| format!("invalid chat id: {part}"))
        })
        .collect();
    let ids = ids?;
    if ids.is_empty() {
        return Err("allowed chat ID list is empty".to_string());
    }
    Ok(ids)
}

pub fn parse_command(text: &str) -> TelegramCommand {
    let mut parts = text.split_whitespace();
    let command = parts.next().unwrap_or_default();
    let command = command.split('@').next().unwrap_or(command);
    match command {
        "/help" | "help" | "/start" | "start" | "/menu" | "menu" => TelegramCommand::Help,
        "/list" => parse_app_arg(parts.next())
            .map(|app| TelegramCommand::List { app })
            .unwrap_or_else(|| TelegramCommand::Unknown("Usage: /list <app>".to_string())),
        "/current" => parse_app_arg(parts.next())
            .map(|app| TelegramCommand::Current { app })
            .unwrap_or_else(|| TelegramCommand::Unknown("Usage: /current <app>".to_string())),
        "/switch" => match (parse_app_arg(parts.next()), parts.next()) {
            (Some(app), Some(provider_id)) => TelegramCommand::Switch {
                app,
                provider_id: provider_id.to_string(),
            },
            _ => TelegramCommand::Unknown("Usage: /switch <app> <provider_id>".to_string()),
        },
        "/restart-proxy" => match parts.next() {
            Some("codex") => TelegramCommand::RestartProxyCodex,
            _ => TelegramCommand::Unknown("Usage: /restart-proxy codex".to_string()),
        },
        "/restart-codex" => TelegramCommand::RestartCodex,
        _ => TelegramCommand::Unknown("Unknown command. Send /help.".to_string()),
    }
}

fn parse_button_action(data: &str) -> TelegramButtonAction {
    let mut parts = data.split(':');
    match (parts.next(), parts.next(), parts.next()) {
        (Some("menu"), None, None) => TelegramButtonAction::Menu,
        (Some("app"), Some(app), None) => parse_app_arg(Some(app))
            .map(|app| TelegramButtonAction::App { app })
            .unwrap_or(TelegramButtonAction::Unknown),
        (Some("list"), Some(app), None) => parse_app_arg(Some(app))
            .map(|app| TelegramButtonAction::List { app })
            .unwrap_or(TelegramButtonAction::Unknown),
        (Some("current"), Some(app), None) => parse_app_arg(Some(app))
            .map(|app| TelegramButtonAction::Current { app })
            .unwrap_or(TelegramButtonAction::Unknown),
        (Some("switchmenu"), Some(app), None) => parse_app_arg(Some(app))
            .map(|app| TelegramButtonAction::SwitchMenu { app })
            .unwrap_or(TelegramButtonAction::Unknown),
        (Some("switch"), Some(app), Some(provider_id)) => parse_app_arg(Some(app))
            .map(|app| TelegramButtonAction::Switch {
                app,
                provider_id: provider_id.to_string(),
            })
            .unwrap_or(TelegramButtonAction::Unknown),
        (Some("restartproxy"), Some("codex"), None) => TelegramButtonAction::RestartProxyCodex,
        (Some("restartcodex"), None, None) => TelegramButtonAction::RestartCodex,
        _ => TelegramButtonAction::Unknown,
    }
}

fn parse_app_arg(value: Option<&str>) -> Option<AppType> {
    value.and_then(|value| value.parse::<AppType>().ok())
}

async fn handle_command(
    state: &AppState,
    runtime: &BotRuntimeConfig,
    command: TelegramCommand,
) -> Result<BotReply, String> {
    match command {
        TelegramCommand::Help => Ok(BotReply::new(help_text(), Some(main_menu_keyboard()))),
        TelegramCommand::List { app } => list_reply(state, app),
        TelegramCommand::Current { app } => current_reply(state, app),
        TelegramCommand::Switch { app, provider_id } => switch_reply(state, app, provider_id),
        TelegramCommand::RestartProxyCodex => {
            state
                .proxy_service
                .set_takeover_for_app("codex", false)
                .await
                .map_err(|e| format!("停止 Codex 代理接管失败: {e}"))?;
            state
                .proxy_service
                .set_takeover_for_app("codex", true)
                .await
                .map_err(|e| format!("启动 Codex 代理接管失败: {e}"))?;
            Ok(BotReply::new(
                "Codex 代理已重启。".to_string(),
                Some(codex_action_keyboard()),
            ))
        }
        TelegramCommand::RestartCodex => {
            restart_codex(runtime)?;
            Ok(BotReply::new(
                "Codex 启动命令已执行。".to_string(),
                Some(codex_action_keyboard()),
            ))
        }
        TelegramCommand::Unknown(message) => Ok(BotReply::text(message)),
    }
}

async fn handle_button_action(
    state: &AppState,
    runtime: &BotRuntimeConfig,
    action: TelegramButtonAction,
) -> Result<BotReply, String> {
    match action {
        TelegramButtonAction::Menu => Ok(BotReply::new(help_text(), Some(main_menu_keyboard()))),
        TelegramButtonAction::App { app } => Ok(BotReply::new(
            format!("{} 操作:", app_display_name(&app)),
            Some(app_action_keyboard(&app)),
        )),
        TelegramButtonAction::List { app } => list_reply(state, app),
        TelegramButtonAction::Current { app } => current_reply(state, app),
        TelegramButtonAction::SwitchMenu { app } => switch_menu_reply(state, app),
        TelegramButtonAction::Switch { app, provider_id } => switch_reply(state, app, provider_id),
        TelegramButtonAction::RestartProxyCodex => {
            handle_command(state, runtime, TelegramCommand::RestartProxyCodex).await
        }
        TelegramButtonAction::RestartCodex => {
            handle_command(state, runtime, TelegramCommand::RestartCodex).await
        }
        TelegramButtonAction::Unknown => Ok(BotReply::text("未知按钮操作。")),
    }
}

fn list_reply(state: &AppState, app: AppType) -> Result<BotReply, String> {
    let providers = ProviderService::list(state, app.clone()).map_err(|e| e.to_string())?;
    if providers.is_empty() {
        return Ok(BotReply::new(
            format!("{} 暂无已配置供应商。", app_display_name(&app)),
            Some(app_action_keyboard(&app)),
        ));
    }

    let mut lines = vec![format!("{} 的供应商:", app_display_name(&app))];
    for provider in providers.values() {
        let request_address =
            provider_request_address(provider).unwrap_or_else(|| "未配置请求地址".to_string());
        lines.push(format!("{}:{}", provider.name, request_address));
    }
    Ok(BotReply::new(
        lines.join("\n"),
        Some(app_action_keyboard(&app)),
    ))
}

fn current_reply(state: &AppState, app: AppType) -> Result<BotReply, String> {
    let current = ProviderService::current(state, app.clone()).map_err(|e| e.to_string())?;
    let text = if current.is_empty() {
        format!("{} 暂无当前供应商。", app_display_name(&app))
    } else {
        format!("{} 当前供应商: {current}", app_display_name(&app))
    };
    Ok(BotReply::new(text, Some(app_action_keyboard(&app))))
}

fn switch_menu_reply(state: &AppState, app: AppType) -> Result<BotReply, String> {
    let providers = ProviderService::list(state, app.clone()).map_err(|e| e.to_string())?;
    Ok(BotReply::new(
        format!("请选择 {} 供应商:", app_display_name(&app)),
        Some(provider_keyboard(&app, providers.values())),
    ))
}

fn switch_reply(state: &AppState, app: AppType, provider_id: String) -> Result<BotReply, String> {
    let result =
        ProviderService::switch(state, app.clone(), &provider_id).map_err(|e| e.to_string())?;
    let mut reply = format!("已将 {} 切换到 {provider_id}。", app_display_name(&app));
    if !result.warnings.is_empty() {
        reply.push_str("\n警告: ");
        reply.push_str(&result.warnings.join(", "));
    }
    Ok(BotReply::new(reply, Some(app_action_keyboard(&app))))
}

fn help_text() -> String {
    [
        "请选择下面的应用，或使用命令:",
        "/list <app>",
        "/current <app>",
        "/switch <app> <provider_id>",
        "/restart-proxy codex",
        "/restart-codex",
    ]
    .join("\n")
}

fn app_display_name(app: &AppType) -> &'static str {
    match app {
        AppType::Claude => "Claude",
        AppType::Codex => "Codex",
        AppType::Gemini => "Gemini",
        AppType::ClaudeDesktop => "Claude Desktop",
        AppType::OpenCode => "OpenCode",
        AppType::OpenClaw => "OpenClaw",
        AppType::Hermes => "Hermes",
    }
}

fn provider_request_address(provider: &Provider) -> Option<String> {
    let config = &provider.settings_config;
    [
        "/base_url",
        "/baseUrl",
        "/baseURL",
        "/apiEndpoint",
        "/url",
        "/env/ANTHROPIC_BASE_URL",
        "/env/OPENAI_BASE_URL",
        "/env/GEMINI_BASE_URL",
        "/env/GOOGLE_GEMINI_BASE_URL",
        "/env/BASE_URL",
        "/options/baseURL",
        "/options/baseUrl",
        "/options/base_url",
    ]
    .iter()
    .find_map(|pointer| {
        config
            .pointer(pointer)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
    .or_else(|| {
        config
            .get("config")
            .and_then(|value| value.as_str())
            .and_then(crate::codex_config::extract_codex_base_url)
    })
}

impl BotReply {
    fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            keyboard: None,
        }
    }

    fn new(text: String, keyboard: Option<InlineKeyboardMarkup>) -> Self {
        Self { text, keyboard }
    }
}

fn button(text: impl Into<String>, callback_data: impl Into<String>) -> InlineKeyboardButton {
    InlineKeyboardButton {
        text: text.into(),
        callback_data: callback_data.into(),
    }
}

fn main_menu_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![
                button("Claude", "app:claude"),
                button("Codex", "app:codex"),
                button("Gemini", "app:gemini"),
            ],
            vec![
                button("Claude Desktop", "app:claude-desktop"),
                button("OpenCode", "app:opencode"),
            ],
            vec![
                button("OpenClaw", "app:openclaw"),
                button("Hermes", "app:hermes"),
            ],
        ],
    }
}

fn app_action_keyboard(app: &AppType) -> InlineKeyboardMarkup {
    let mut rows = vec![
        vec![
            button("当前供应商", format!("current:{}", app.as_str())),
            button("供应商列表", format!("list:{}", app.as_str())),
        ],
        vec![button("切换供应商", format!("switchmenu:{}", app.as_str()))],
    ];
    if matches!(app, AppType::Codex) {
        rows.push(vec![
            button("重启代理", "restartproxy:codex"),
            button("重启 Codex", "restartcodex"),
        ]);
    }
    rows.push(vec![button("返回", "menu")]);
    InlineKeyboardMarkup {
        inline_keyboard: rows,
    }
}

fn codex_action_keyboard() -> InlineKeyboardMarkup {
    app_action_keyboard(&AppType::Codex)
}

fn provider_keyboard<'a>(
    app: &AppType,
    providers: impl Iterator<Item = &'a Provider>,
) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = providers
        .map(|provider| {
            vec![button(
                format!("{} ({})", provider.name, provider.id),
                format!("switch:{}:{}", app.as_str(), provider.id),
            )]
        })
        .collect();
    rows.push(vec![button("返回", format!("app:{}", app.as_str()))]);
    InlineKeyboardMarkup {
        inline_keyboard: rows,
    }
}

pub fn validate_restart_command(command: &str) -> Result<(), String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err("restart command is empty".to_string());
    }
    if trimmed.len() > 512 {
        return Err("restart command is too long".to_string());
    }
    if trimmed.contains('\0') || trimmed.lines().count() > 1 {
        return Err("restart command must be a single line".to_string());
    }
    Ok(())
}

fn restart_codex(runtime: &BotRuntimeConfig) -> Result<(), String> {
    validate_restart_command(&runtime.codex_restart_command)?;

    if runtime.codex_restart_force_stop {
        stop_codex_processes()?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "CC Switch Codex", "cmd", "/K"])
            .arg(&runtime.codex_restart_command)
            .spawn()
            .map_err(|e| format!("failed to launch Codex command: {e}"))?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("sh")
            .arg("-lc")
            .arg(&runtime.codex_restart_command)
            .spawn()
            .map_err(|e| format!("failed to launch Codex command: {e}"))?;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn stop_codex_processes() -> Result<(), String> {
    Command::new("taskkill")
        .args(["/IM", "codex.exe", "/F"])
        .status()
        .map_err(|e| format!("failed to stop codex.exe: {e}"))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn stop_codex_processes() -> Result<(), String> {
    Ok(())
}

async fn send_message(
    client: &Client,
    token: &str,
    chat_id: i64,
    reply: &BotReply,
) -> Result<(), String> {
    let url = telegram_api_url(token, "sendMessage");
    let mut body = serde_json::json!({
        "chat_id": chat_id,
        "text": reply.text,
        "disable_web_page_preview": true,
    });
    if let Some(keyboard) = &reply.keyboard {
        body["reply_markup"] = serde_json::to_value(keyboard)
            .map_err(|e| format!("keyboard serialization failed: {e}"))?;
    }
    client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("sendMessage request failed: {}", e.without_url()))?
        .error_for_status()
        .map_err(|e| format!("sendMessage returned HTTP error: {}", e.without_url()))?;
    Ok(())
}

async fn edit_message(
    client: &Client,
    token: &str,
    chat_id: i64,
    message_id: i64,
    reply: &BotReply,
) -> Result<(), String> {
    let url = telegram_api_url(token, "editMessageText");
    let mut body = serde_json::json!({
        "chat_id": chat_id,
        "message_id": message_id,
        "text": reply.text,
        "disable_web_page_preview": true,
    });
    if let Some(keyboard) = &reply.keyboard {
        body["reply_markup"] = serde_json::to_value(keyboard)
            .map_err(|e| format!("keyboard serialization failed: {e}"))?;
    }
    client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("editMessageText request failed: {}", e.without_url()))?
        .error_for_status()
        .map_err(|e| format!("editMessageText returned HTTP error: {}", e.without_url()))?;
    Ok(())
}

async fn answer_callback_query(
    client: &Client,
    token: &str,
    callback_query_id: &str,
    text: Option<&str>,
) -> Result<(), String> {
    let url = telegram_api_url(token, "answerCallbackQuery");
    let mut body = serde_json::json!({
        "callback_query_id": callback_query_id,
    });
    if let Some(text) = text {
        body["text"] = serde_json::json!(text);
    }
    client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("answerCallbackQuery request failed: {}", e.without_url()))?
        .error_for_status()
        .map_err(|e| {
            format!(
                "answerCallbackQuery returned HTTP error: {}",
                e.without_url()
            )
        })?;
    Ok(())
}

fn telegram_api_url(token: &str, method: &str) -> String {
    format!("https://api.telegram.org/bot{token}/{method}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_commands() {
        assert_eq!(parse_command("/help"), TelegramCommand::Help);
        assert_eq!(parse_command("/start"), TelegramCommand::Help);
        assert_eq!(
            parse_command("/list codex"),
            TelegramCommand::List {
                app: AppType::Codex
            }
        );
        assert_eq!(
            parse_command("/switch claude provider-1"),
            TelegramCommand::Switch {
                app: AppType::Claude,
                provider_id: "provider-1".to_string()
            }
        );
        assert_eq!(
            parse_command("/restart-proxy codex"),
            TelegramCommand::RestartProxyCodex
        );
    }

    #[test]
    fn parses_button_actions() {
        assert_eq!(parse_button_action("menu"), TelegramButtonAction::Menu);
        assert_eq!(
            parse_button_action("app:codex"),
            TelegramButtonAction::App {
                app: AppType::Codex
            }
        );
        assert_eq!(
            parse_button_action("switch:codex:p1"),
            TelegramButtonAction::Switch {
                app: AppType::Codex,
                provider_id: "p1".to_string()
            }
        );
        assert_eq!(
            parse_button_action("restartproxy:codex"),
            TelegramButtonAction::RestartProxyCodex
        );
    }

    #[test]
    fn parses_allowed_chat_ids() {
        let ids = parse_allowed_chat_ids("123, -456\n789").expect("ids");
        assert!(ids.contains(&123));
        assert!(ids.contains(&-456));
        assert!(ids.contains(&789));
    }

    #[test]
    fn rejects_invalid_chat_ids() {
        assert!(parse_allowed_chat_ids("abc").is_err());
        assert!(parse_allowed_chat_ids("").is_err());
    }

    #[test]
    fn validates_restart_command() {
        assert!(validate_restart_command("codex").is_ok());
        assert!(validate_restart_command("").is_err());
        assert!(validate_restart_command("codex\nwhoami").is_err());
        assert!(validate_restart_command(&"x".repeat(513)).is_err());
    }
}
