<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { ask } from "@tauri-apps/plugin-dialog";
  import HotkeyRecorder from "./HotkeyRecorder.svelte";
  import { applyTheme, type Theme } from "$lib/theme";
  import { checkForUpdates, type UpdateMode } from "$lib/updater";

  interface AppConfig {
    onboarded: boolean;
    hotkey: string;
    theme: Theme;
    autostart: boolean;
    library_root: string | null;
    update_mode: UpdateMode;
    auto_paste: boolean;
  }
  interface StorageInfo {
    library_root: string;
    db_bytes: number;
    cache_bytes: number;
  }

  interface Props {
    onClose: () => void;
    /** 清缓存后需要刷新图库（缩略图标记全部失效） */
    onMemesChanged: () => void;
  }
  let { onClose, onMemesChanged }: Props = $props();

  let cfg = $state<AppConfig | null>(null);
  let storage = $state<StorageInfo | null>(null);
  let pendingHotkey = $state<string | null>(null);
  let saveState = $state("");
  let updateState = $state("");
  let updateConfigured = $state<boolean | null>(null);
  let checkingUpdates = $state(false);
  let lastPasteStatus = $state('');

  function load() {
    void invoke<AppConfig>("get_config").then((c) => (cfg = c)).catch((e) => alert(String(e)));
    void invoke<StorageInfo>("get_storage_info").then((s) => (storage = s)).catch((e) => alert(String(e)));
    void invoke<{ configured: boolean }>("get_updater_status")
      .then((s) => (updateConfigured = s.configured))
      .catch(() => (updateConfigured = false));
    void invoke<string>("get_last_paste_status").then((status) => (lastPasteStatus = status)).catch(() => {});
  }
  load();

  function humanSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  async function saveHotkey() {
    if (!pendingHotkey) return;
    try {
      await invoke("set_hotkey", { accelerator: pendingHotkey });
      saveState = "已保存";
      pendingHotkey = null;
    } catch (e) {
      alert(String(e));
    }
  }

  async function changeTheme(theme: Theme) {
    if (!cfg) return;
    try {
      await invoke("set_theme", { theme });
      cfg.theme = theme;
      applyTheme(theme);
    } catch (e) {
      alert(String(e));
    }
  }

  async function toggleAutostart(enabled: boolean) {
    try {
      await invoke("set_autostart", { enabled });
      if (cfg) cfg.autostart = enabled;
    } catch (e) {
      alert(String(e));
      load();
    }
  }

  async function toggleAutoPaste(enabled: boolean) {
    try {
      await invoke("set_auto_paste", { enabled });
      if (cfg) cfg.auto_paste = enabled;
    } catch (e) {
      alert(String(e));
      load();
    }
  }

  async function changeUpdateMode(mode: UpdateMode) {
    try {
      await invoke("set_update_mode", { mode });
      if (cfg) cfg.update_mode = mode;
    } catch (e) {
      alert(String(e));
    }
  }

  async function manuallyCheckUpdates() {
    if (checkingUpdates) return;
    checkingUpdates = true;
    updateState = "";
    try {
      await checkForUpdates((status) => (updateState = status));
    } catch (e) {
      updateState = `检查更新失败：${String(e)}`;
    } finally {
      checkingUpdates = false;
    }
  }

  async function clearCache() {
    const ok = await ask("清理缩略图缓存？浏览时会按需重新生成。", {
      title: "清理缓存",
      kind: "info",
    });
    if (!ok) return;
    try {
      await invoke("clear_thumbnail_cache");
      storage = await invoke<StorageInfo>("get_storage_info");
      onMemesChanged();
    } catch (e) {
      alert(String(e));
    }
  }

  const themes: { value: Theme; label: string }[] = [
    { value: "system", label: "跟随系统" },
    { value: "light", label: "浅色" },
    { value: "dark", label: "深色" },
  ];
</script>

<div class="fixed inset-0 z-50 grid place-items-center bg-neutral-900/60 p-6">
  <div class="flex max-h-[85vh] w-full max-w-lg flex-col overflow-hidden rounded-lg bg-white shadow-2xl dark:bg-neutral-800">
    <div class="flex shrink-0 items-center justify-between border-b border-neutral-200 px-6 py-4 dark:border-neutral-700">
      <h2 class="text-lg font-semibold">设置</h2>
      <button
        type="button"
        aria-label="关闭设置"
        class="grid h-8 w-8 place-items-center rounded-full text-xl leading-none text-neutral-500 hover:bg-neutral-100 hover:text-neutral-900 dark:hover:bg-neutral-700 dark:hover:text-white"
        onclick={onClose}
      >×</button>
    </div>

    <div class="min-h-0 overflow-y-auto px-6 py-5">

    {#if cfg}
      <section class="mb-5">
        <p class="mb-2 text-sm font-medium">呼出快捷键</p>
        <HotkeyRecorder initialHotkey={cfg.hotkey} onChange={(a) => (pendingHotkey = a)} />
        {#if pendingHotkey}
          <div class="mt-2 flex items-center gap-2">
            <button
              type="button"
              class="rounded bg-neutral-800 px-2 py-1 text-xs text-white hover:bg-neutral-700 dark:bg-neutral-700 dark:hover:bg-neutral-600"
              onclick={() => void saveHotkey()}
            >保存新快捷键</button>
            {#if saveState}<span class="text-xs text-green-600 dark:text-green-400">{saveState}</span>{/if}
          </div>
        {/if}
      </section>

      <section class="mb-5">
        <p class="mb-2 text-sm font-medium">开机自启</p>
        <label class="flex cursor-pointer items-center gap-2 text-sm text-neutral-600 dark:text-neutral-300">
          <input
            type="checkbox"
            checked={cfg.autostart}
            onchange={(e) => void toggleAutostart(e.currentTarget.checked)}
          />
          系统启动时自动运行
        </label>
      </section>

      <section class="mb-5">
        <p class="mb-2 text-sm font-medium">发送表情</p>
        <label class="flex cursor-pointer items-center gap-2 text-sm text-neutral-600 dark:text-neutral-300">
          <input
            type="checkbox"
            checked={cfg.auto_paste}
            onchange={(e) => void toggleAutoPaste(e.currentTarget.checked)}
          />
          自动粘贴到原输入框
        </label>
        <p class="mt-1 text-xs text-neutral-500">若无法定位原输入框或自动粘贴失败，表情仍保留在剪贴板，可手动按 Ctrl+V 粘贴。</p>
        {#if lastPasteStatus}<p class="mt-2 text-xs text-neutral-500" role="status">最近一次：{lastPasteStatus}</p>{/if}
      </section>

      <section class="mb-5">
        <p class="mb-2 text-sm font-medium">应用更新</p>
        {#if updateConfigured === false}
          <p class="mb-2 text-xs text-amber-700 dark:text-amber-400">更新源尚未配置；偏好会保存，发布配置完成后生效。</p>
        {/if}
        <div class="space-y-1 text-sm text-neutral-600 dark:text-neutral-300">
          <label class="flex cursor-pointer items-center gap-2">
            <input type="radio" name="update-mode" checked={cfg.update_mode === "disabled"} onchange={() => void changeUpdateMode("disabled")} />
            关闭自动更新
          </label>
          <label class="flex cursor-pointer items-center gap-2">
            <input type="radio" name="update-mode" checked={cfg.update_mode === "prompt"} onchange={() => void changeUpdateMode("prompt")} />
            自动检查，发现新版本后询问
          </label>
          <label class="flex cursor-pointer items-center gap-2">
            <input type="radio" name="update-mode" checked={cfg.update_mode === "automatic"} onchange={() => void changeUpdateMode("automatic")} />
            自动下载并安装
          </label>
        </div>
        <p class="mt-1 text-xs text-neutral-500">关闭后启动时不会检查更新，仍可手动检查。</p>
        <p class="mt-1 text-xs text-neutral-500">自动安装会关闭应用并启动 Windows 安装程序。</p>
        <button
          type="button"
          class="mt-2 rounded border border-neutral-300 px-2 py-1 text-xs text-neutral-600 hover:bg-neutral-100 disabled:opacity-50 dark:border-neutral-600 dark:text-neutral-300 dark:hover:bg-neutral-700"
          disabled={checkingUpdates}
          onclick={() => void manuallyCheckUpdates()}
        >{checkingUpdates ? "检查中…" : "立即检查更新"}</button>
        {#if updateState}<p class="mt-1 text-xs text-neutral-500" role="status">{updateState}</p>{/if}
      </section>

      <section class="mb-5">
        <p class="mb-2 text-sm font-medium">主题</p>
        <div class="flex gap-4">
          {#each themes as t (t.value)}
            <label class="flex cursor-pointer items-center gap-1.5 text-sm text-neutral-600 dark:text-neutral-300">
              <input
                type="radio"
                name="theme"
                value={t.value}
                checked={cfg.theme === t.value}
                onchange={() => void changeTheme(t.value)}
              />
              {t.label}
            </label>
          {/each}
        </div>
      </section>

      <section>
        <p class="mb-2 text-sm font-medium">存储</p>
        {#if storage}
          <dl class="mb-2 space-y-1 text-xs text-neutral-500">
            <div class="flex justify-between gap-4">
              <dt>库位置</dt>
              <dd class="truncate" title={storage.library_root}>{storage.library_root}</dd>
            </div>
            <div class="flex justify-between"><dt>数据库</dt><dd>{humanSize(storage.db_bytes)}</dd></div>
            <div class="flex justify-between"><dt>缩略图缓存</dt><dd>{humanSize(storage.cache_bytes)}</dd></div>
          </dl>
          <button
            type="button"
            class="rounded border border-neutral-300 px-2 py-1 text-xs text-neutral-600 hover:bg-neutral-100 dark:border-neutral-600 dark:text-neutral-300 dark:hover:bg-neutral-700"
            onclick={() => void clearCache()}
          >清理缓存</button>
        {/if}
      </section>
    {:else}
      <p class="text-sm text-neutral-400">加载中…</p>
    {/if}
    </div>
  </div>
</div>
