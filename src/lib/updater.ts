import { invoke } from "@tauri-apps/api/core";
import { ask } from "@tauri-apps/plugin-dialog";
import { check } from "@tauri-apps/plugin-updater";

export type UpdateMode = "disabled" | "prompt" | "automatic";

interface UpdateConfig {
  update_mode: UpdateMode;
}

interface UpdaterStatus {
  configured: boolean;
}

/** Called once at startup and also by the settings panel's manual check. */
export async function checkForUpdates(
  onStatus?: (status: string) => void,
  respectDisabled = false,
): Promise<void> {
  const config = await invoke<UpdateConfig>("get_config");
  if (respectDisabled && config.update_mode === "disabled") return;
  const source = await invoke<UpdaterStatus>("get_updater_status");
  if (!source.configured) {
    onStatus?.("更新源未配置，暂时无法检查更新。");
    return;
  }

  onStatus?.("正在检查更新…");
  const update = await check({ timeout: 15_000 });
  if (!update) {
    onStatus?.("当前已是最新版本");
    return;
  }

  try {
    if (config.update_mode !== "automatic") {
      const approved = await ask(
        `发现新版本 ${update.version}${update.body ? `\n\n${update.body}` : ""}\n\n现在下载并安装吗？`,
        { title: "应用更新", kind: "info" },
      );
      if (!approved) {
        onStatus?.(`已发现版本 ${update.version}，等待安装`);
        return;
      }
    }

    onStatus?.(`正在下载 ${update.version}…`);
    let downloaded = 0;
    await update.downloadAndInstall((event) => {
      if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        onStatus?.(`已下载 ${(downloaded / 1024 / 1024).toFixed(1)} MB`);
      } else if (event.event === "Finished") {
        onStatus?.("下载完成，正在启动安装程序…");
      }
    });
  } finally {
    await update.close();
  }
}

/** Keeps network or release failures from blocking the app's startup. */
export function checkForUpdatesOnStartup(): void {
  void checkForUpdates(undefined, true).catch((error) => {
    console.warn("更新检查失败", error);
  });
}
