import { useTranslation } from "react-i18next";
import {
  Bot,
  KeyRound,
  MessageSquareLock,
  RotateCw,
  ShieldAlert,
} from "lucide-react";
import { ToggleRow } from "@/components/ui/toggle-row";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Checkbox } from "@/components/ui/checkbox";
import type { SettingsFormState } from "@/hooks/useSettings";

interface TelegramBotSettingsProps {
  settings: SettingsFormState;
  onChange: (updates: Partial<SettingsFormState>) => void;
}

export function TelegramBotSettings({
  settings,
  onChange,
}: TelegramBotSettingsProps) {
  const { t } = useTranslation();
  const telegramBot = settings.telegramBot ?? {};

  const updateTelegram = (
    updates: NonNullable<SettingsFormState["telegramBot"]>,
  ) => {
    onChange({
      telegramBot: {
        ...telegramBot,
        ...updates,
      },
    });
  };

  return (
    <section className="space-y-4">
      <div className="flex items-center gap-2 pb-2 border-b border-border/40">
        <Bot className="h-4 w-4 text-primary" />
        <h3 className="text-sm font-medium">
          {t("settings.telegramBot.title", {
            defaultValue: "Telegram 远程控制",
          })}
        </h3>
      </div>

      <div className="space-y-3">
        <ToggleRow
          icon={<Bot className="h-4 w-4 text-sky-500" />}
          title={t("settings.telegramBot.enabled", {
            defaultValue: "启用 Telegram Bot",
          })}
          description={t("settings.telegramBot.enabledDescription", {
            defaultValue:
              "允许可信 Telegram 会话在 CC Switch 运行时查看和切换供应商。",
          })}
          checked={!!telegramBot.enabled}
          onCheckedChange={(enabled) => updateTelegram({ enabled })}
        />

        <div className="grid gap-4 rounded-xl border border-border bg-card/50 p-4">
          <div className="grid gap-2">
            <Label className="flex items-center gap-2 text-sm">
              <KeyRound className="h-4 w-4 text-amber-500" />
              {t("settings.telegramBot.token", {
                defaultValue: "Bot Token",
              })}
            </Label>
            <Input
              type="password"
              value={telegramBot.token ?? ""}
              placeholder={t("settings.telegramBot.tokenPlaceholder", {
                defaultValue: "留空则保留已保存的 Token",
              })}
              onChange={(event) =>
                updateTelegram({ token: event.currentTarget.value })
              }
            />
          </div>

          <div className="grid gap-2">
            <Label className="flex items-center gap-2 text-sm">
              <MessageSquareLock className="h-4 w-4 text-emerald-500" />
              {t("settings.telegramBot.allowedChatIds", {
                defaultValue: "允许的 Chat ID",
              })}
            </Label>
            <Input
              value={telegramBot.allowedChatIds ?? ""}
              placeholder="123456789, -1001234567890"
              onChange={(event) =>
                updateTelegram({ allowedChatIds: event.currentTarget.value })
              }
            />
          </div>

          <div className="grid gap-2">
            <Label className="flex items-center gap-2 text-sm">
              <RotateCw className="h-4 w-4 text-blue-500" />
              {t("settings.telegramBot.codexRestartCommand", {
                defaultValue: "Codex 重启命令",
              })}
            </Label>
            <Input
              value={telegramBot.codexRestartCommand ?? "codex"}
              placeholder="codex"
              onChange={(event) =>
                updateTelegram({
                  codexRestartCommand: event.currentTarget.value,
                })
              }
            />
          </div>

          <label className="flex items-start gap-3 rounded-lg border border-border/60 bg-background/60 p-3">
            <Checkbox
              checked={!!telegramBot.codexRestartForceStop}
              onCheckedChange={(value) =>
                updateTelegram({ codexRestartForceStop: value === true })
              }
              aria-label={t("settings.telegramBot.forceStop", {
                defaultValue: "强制停止现有 Codex 进程",
              })}
            />
            <span className="space-y-1">
              <span className="block text-sm font-medium">
                {t("settings.telegramBot.forceStop", {
                  defaultValue: "强制停止现有 Codex 进程",
                })}
              </span>
              <span className="block text-xs text-muted-foreground">
                {t("settings.telegramBot.forceStopDescription", {
                  defaultValue:
                    "只能启动本地保存的命令。Telegram 消息不能提供 shell 命令。",
                })}
              </span>
            </span>
          </label>

          <div className="flex items-start gap-2 rounded-lg border border-amber-500/30 bg-amber-500/10 p-3 text-xs text-amber-700 dark:text-amber-300">
            <ShieldAlert className="mt-0.5 h-4 w-4 flex-shrink-0" />
            <p>
              {t("settings.telegramBot.safetyHint", {
                defaultValue:
                  "不要通过 Telegram 暴露任意命令执行能力。重启只会使用这里保存的本地命令。",
              })}
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}
