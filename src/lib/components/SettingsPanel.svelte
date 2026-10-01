<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { onMount } from 'svelte';
  import HotkeyRecorder from "./HotkeyRecorder.svelte";
  import Icon from './Icon.svelte';
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
  let activeTab = $state<'general' | 'appearance' | 'updates' | 'storage'>('general');
  let dialogEl: HTMLDialogElement | undefined = $state();
  onMount(() => { dialogEl?.showModal(); });

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

<dialog bind:this={dialogEl} class="ui-dialog settings-dialog" aria-label="设置" onclose={onClose}>
  <div class="ui-dialog-header settings-header"><div class="settings-title"><img src="/memeji.png" alt="" /><h2>设置</h2></div><button type="button" aria-label="关闭设置" class="icon-button round" onclick={() => dialogEl?.close()}><Icon name="x" size={16}/></button></div>
  <div class="settings-layout">
    <nav class="settings-nav" aria-label="设置分类">
      <button class:active={activeTab==='general'} onclick={() => (activeTab='general')}><Icon name="sliders" size={17}/>通用</button>
      <button class:active={activeTab==='appearance'} onclick={() => (activeTab='appearance')}><Icon name="sun" size={17}/>外观</button>
      <button class:active={activeTab==='updates'} onclick={() => (activeTab='updates')}><Icon name="refresh" size={17}/>更新</button>
      <button class:active={activeTab==='storage'} onclick={() => (activeTab='storage')}><Icon name="folder" size={17}/>存储</button>
    </nav>
    <div class="settings-content">

    {#if cfg}
      {#if activeTab === 'general'}
      <h3 class="settings-section-title">通用</h3>
      <section class="settings-row">
        <div class="settings-row-text"><h4>呼出快捷键</h4><p>从任意应用打开 Quick Picker</p></div>
        <HotkeyRecorder initialHotkey={cfg.hotkey} onChange={(a) => (pendingHotkey = a)} />
        {#if pendingHotkey}
          <div class="mt-2 flex items-center gap-2">
            <button
              type="button"
              class="ui-button small primary"
              onclick={() => void saveHotkey()}
            >保存新快捷键</button>
            {#if saveState}<span class="settings-status">{saveState}</span>{/if}
          </div>
        {/if}
      </section>

      <section class="settings-row settings-inline-row">
        <div class="settings-row-text"><h4>开机自启</h4><p>登录 Windows 后在后台运行</p></div>
        <label class="settings-toggle" aria-label="开机自启">
          <input
            type="checkbox"
            checked={cfg.autostart}
            onchange={(e) => void toggleAutostart(e.currentTarget.checked)}
          />
          <span></span>
        </label>
      </section>

      <section class="settings-row settings-inline-row">
        <div class="settings-row-text"><h4>自动粘贴</h4><p>选择表情后尝试粘贴到原输入框</p></div>
        <label class="settings-toggle" aria-label="自动粘贴">
          <input
            type="checkbox"
            checked={cfg.auto_paste}
            onchange={(e) => void toggleAutoPaste(e.currentTarget.checked)}
          />
          <span></span>
        </label>
      </section>
      {#if lastPasteStatus}<p class="settings-callout" role="status">最近一次粘贴：{lastPasteStatus}</p>{/if}
      {/if}

      {#if activeTab === 'updates'}
      <h3 class="settings-section-title">应用更新</h3>
      <section class="settings-row">
        <div class="settings-row-text"><h4>更新方式</h4><p>选择发现新版本后的处理方式</p></div>
        {#if updateConfigured === false}
          <p class="settings-callout">更新源尚未配置</p>
        {/if}
        <div class="settings-options">
          <label>
            <input type="radio" name="update-mode" checked={cfg.update_mode === "disabled"} onchange={() => void changeUpdateMode("disabled")} />
            <span>关闭</span>
          </label>
          <label>
            <input type="radio" name="update-mode" checked={cfg.update_mode === "prompt"} onchange={() => void changeUpdateMode("prompt")} />
            <span>检查后询问</span>
          </label>
          <label>
            <input type="radio" name="update-mode" checked={cfg.update_mode === "automatic"} onchange={() => void changeUpdateMode("automatic")} />
            <span>自动下载安装</span>
          </label>
        </div>
      </section>
      <section class="settings-row settings-inline-row">
        <div class="settings-row-text"><h4>检查新版本</h4><p>从 GitHub Release 获取更新</p></div>
        <button
          type="button"
          class="ui-button small"
          disabled={checkingUpdates}
          onclick={() => void manuallyCheckUpdates()}
        >{checkingUpdates ? "检查中…" : "立即检查更新"}</button>
      </section>
      {#if updateState}<p class="settings-callout" role="status">{updateState}</p>{/if}
      {/if}

      {#if activeTab === 'appearance'}
      <h3 class="settings-section-title">外观</h3>
      <section class="settings-row">
        <div class="settings-row-text"><h4>界面主题</h4><p>与系统保持一致，或选择偏好的外观</p></div>
        <div class="theme-options">
          {#each themes as t (t.value)}
            <label class:active={cfg.theme === t.value}>
              <input
                type="radio"
                name="theme"
                value={t.value}
                checked={cfg.theme === t.value}
                onchange={() => void changeTheme(t.value)}
              />
              <Icon name={t.value === 'system' ? 'monitor' : t.value === 'light' ? 'sun' : 'moon'} size={20}/><span>{t.label}</span>
            </label>
          {/each}
        </div>
      </section>
      {/if}

      {#if activeTab === 'storage'}
      <h3 class="settings-section-title">存储</h3>
      <section class="settings-row">
        <div class="settings-row-text"><h4>表情库</h4><p>表情和缩略图保存在本机</p></div>
        {#if storage}
          <dl class="storage-list">
            <div>
              <dt>库位置</dt>
              <dd class="truncate" title={storage.library_root}>{storage.library_root}</dd>
            </div>
            <div><dt>数据库</dt><dd>{humanSize(storage.db_bytes)}</dd></div>
            <div><dt>缩略图缓存</dt><dd>{humanSize(storage.cache_bytes)}</dd></div>
          </dl>
          <button
            type="button"
            class="ui-button small"
            onclick={() => void clearCache()}
          ><Icon name="refresh" size={15}/>清理缩略图缓存</button>
        {/if}
      </section>
      {/if}
    {:else}
      <p class="text-sm text-neutral-400">加载中…</p>
    {/if}
    </div>
  </div>
</dialog>

<style>
  .settings-dialog { width: min(640px, calc(100vw - 28px)); max-width: none; max-height: min(660px, calc(100vh - 28px)); margin: auto; padding: 0; }
  .settings-dialog[open] { display: flex; flex-direction: column; }
  .settings-header { padding: 14px 20px; }
  .settings-title { display: flex; align-items: center; gap: 10px; }
  .settings-title img { width: 28px; height: 28px; object-fit: contain; }
  .settings-layout { display: flex; min-height: 0; flex: 1; }
  .settings-nav { width: 150px; flex-shrink: 0; display: flex; flex-direction: column; gap: 4px; padding: 15px 10px; background: var(--sidebar); border-right: 1px solid var(--border); }
  .settings-nav button { display: flex; align-items: center; gap: 10px; min-height: 37px; padding: 7px 11px; border-radius: 8px; text-align: left; color: var(--muted); }
  .settings-nav button:hover { background: var(--hover); color: var(--text); }
  .settings-nav button.active { background: var(--accent-soft); color: var(--accent-ink); font-weight: 650; }
  .settings-content { min-width: 0; flex: 1; overflow-y: auto; padding: 22px 25px 28px; }
  .settings-section-title { font-size: 18px; font-weight: 650; letter-spacing: -.02em; margin: 0 0 15px; }
  .settings-row { padding: 15px 0; border-top: 1px solid var(--border); }
  .settings-row-text h4 { font-size: 13px; font-weight: 650; }
  .settings-row-text p { color: var(--muted); font-size: 12px; margin-top: 2px; }
  .settings-row > :global(div:not(.settings-row-text)), .settings-row > :global(button:not(.settings-toggle)) { margin-top: 10px; }
  .settings-inline-row { display: flex; align-items: center; justify-content: space-between; gap: 15px; }
  .settings-inline-row .settings-row-text { min-width: 0; }
  .settings-inline-row > :global(button) { margin-top: 0; }
  .settings-toggle { flex-shrink: 0; position: relative; width: 38px; height: 23px; cursor: pointer; }
  .settings-toggle input { position: absolute; opacity: 0; width: 100%; height: 100%; margin: 0; cursor: pointer; }
  .settings-toggle span { display: block; width: 38px; height: 23px; border-radius: 20px; background: var(--border); transition: background 120ms; }
  .settings-toggle span::after { content: ''; display: block; position: absolute; left: 3px; top: 3px; width: 17px; height: 17px; border-radius: 50%; background: white; box-shadow: 0 1px 3px #0002; transition: transform 120ms; }
  .settings-toggle input:checked + span { background: var(--accent); }
  .settings-toggle input:checked + span::after { transform: translateX(15px); }
  .settings-toggle input:focus-visible + span { outline: 2px solid var(--accent); outline-offset: 2px; }
  .settings-options { display: grid; gap: 6px; }
  .settings-options label { display: flex; align-items: center; gap: 9px; min-height: 34px; padding: 5px 9px; border-radius: 7px; cursor: pointer; }
  .settings-options label:hover { background: var(--hover); }
  .settings-options input, .theme-options input { accent-color: var(--accent); }
  .theme-options { display: grid; grid-template-columns: repeat(3,1fr); gap: 9px; }
  .theme-options label { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 7px; min-height: 92px; border: 1px solid var(--border); border-radius: 10px; cursor: pointer; color: var(--muted); }
  .theme-options label:hover { background: var(--hover); }
  .theme-options label.active { background: var(--accent-soft); color: var(--accent-ink); border-color: var(--accent); }
  .theme-options input { position: absolute; opacity: 0; }
  .theme-options input:focus-visible + :global(svg) { outline: 2px solid var(--accent); outline-offset: 4px; }
  .storage-list { display: grid; gap: 8px; margin: 14px 0; }
  .storage-list div { display: flex; justify-content: space-between; gap: 12px; }
  .storage-list dt { color: var(--muted); flex-shrink: 0; }
  .storage-list dd { min-width: 0; text-align: right; }
  .settings-callout { margin: 12px 0; padding: 9px 11px; background: var(--surface-soft); border-radius: 8px; color: var(--muted); font-size: 12px; }
  .settings-status { color: var(--accent-ink); font-size: 12px; }
  @media (max-width: 600px) { .settings-nav { width: 126px; } .settings-content { padding: 16px; } }
</style>
