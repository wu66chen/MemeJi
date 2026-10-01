<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount } from 'svelte';
  import HotkeyRecorder from "./HotkeyRecorder.svelte";
  import Icon from './Icon.svelte';

  interface Props {
    onFinish: () => void;
  }
  let { onFinish }: Props = $props();

  let step = $state(1);
  let libraryPath = $state("");
  let pendingHotkey = $state<string | null>(null);
  let importSummary = $state("");
  let error = $state('');
  let dialogEl: HTMLDialogElement | undefined = $state();
  onMount(() => { dialogEl?.showModal(); });

  interface ImportResult {
    imported: number;
    skipped: number;
    unsupported: number;
    failed: number;
  }

  onInit();

  function onInit() {
    invoke<{ library_root: string }>("get_storage_info")
      .then((info) => (libraryPath = info.library_root))
      .catch(() => {});
  }

  async function chooseLibraryFolder() {
    const picked = await open({ directory: true });
    if (!picked) return;
    libraryPath = picked;
    try {
      await invoke("set_library_root", { path: picked });
    } catch (e) {
      error = String(e);
    }
  }

  async function importFiles() {
    const picked = await open({ multiple: true });
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked : [picked];
    try {
      const r = await invoke<ImportResult>("import_paths", { paths });
      importSummary = `导入 ${r.imported} · 重复 ${r.skipped} · 不支持 ${r.unsupported} · 失败 ${r.failed}`;
    } catch (e) {
      error = String(e);
    }
  }

  async function importFolder() {
    const picked = await open({ directory: true });
    if (!picked) return;
    try {
      const r = await invoke<ImportResult>("import_paths", { paths: [picked] });
      importSummary = `导入 ${r.imported} · 重复 ${r.skipped} · 不支持 ${r.unsupported} · 失败 ${r.failed}`;
    } catch (e) {
      error = String(e);
    }
  }

  async function finish() {
    try {
      await invoke("complete_onboarding", { hotkey: pendingHotkey ?? defaultHotkey(), theme: "system" });
      onFinish();
    } catch (e) {
      error = String(e);
    }
  }

  function defaultHotkey(): string {
    return navigator.platform.toLowerCase().includes("mac") ? "Cmd+Shift+Space" : "Ctrl+Shift+Space";
  }

  const stepTitles = ["创建表情库", "导入表情（可跳过）", "设置呼出快捷键"];
</script>

<dialog bind:this={dialogEl} class="ui-dialog onboarding-dialog" aria-label="欢迎使用 MemeJi" oncancel={(e) => e.preventDefault()}>
  <div class="onboard-header"><img src="/memeji.png" alt=""/><div><p>欢迎使用 MemeJi</p><h2>{stepTitles[step - 1]}</h2></div></div>
  <div class="onboard-progress" aria-label={`第 ${step} 步，共 3 步`}>{#each [1,2,3] as n}<span class:active={n <= step}></span>{/each}</div>
  <div class="onboard-body">
    {#if step === 1}
      <p>选择表情库的位置。导入时会复制图片，原文件保留在原处。</p>
      <label class="ui-label" for="library-path">表情库位置</label>
      <div class="onboard-field"><input id="library-path" readonly value={libraryPath} class="ui-input" /><button type="button" class="ui-button" onclick={() => void chooseLibraryFolder()}>选择文件夹</button></div>
    {:else if step === 2}
      <p>把已有的表情导入图库。之后也可以随时添加。</p>
      <div class="onboard-imports"><button class="ui-button" onclick={() => void importFiles()}><Icon name="upload" size={18}/>导入图片</button><button class="ui-button" onclick={() => void importFolder()}><Icon name="folder-plus" size={18}/>导入文件夹</button></div>
      {#if importSummary}<p class="onboard-status" role="status">{importSummary}</p>{/if}
    {:else}
      <p>按快捷键，即可在当前窗口旁呼出 Quick Picker。</p>
      <p class="ui-label">呼出快捷键</p>
      <HotkeyRecorder initialHotkey={defaultHotkey()} onChange={(accel) => (pendingHotkey = accel)} />
    {/if}
    {#if error}<p class="onboard-error" role="alert">{error}</p>{/if}
  </div>
  <div class="ui-dialog-footer">
    {#if step > 1}<button class="ui-button ghost" onclick={() => { step -= 1; error=''; }}>上一步</button>{/if}
    <span class="onboard-spacer"></span>
    {#if step === 2}<button class="ui-button ghost" onclick={() => { step=3; error=''; }}>跳过</button>{/if}
    {#if step < 3}<button class="ui-button primary" onclick={() => { step += 1; error=''; }}>下一步<Icon name="arrow-right" size={15}/></button>
    {:else}<button class="ui-button primary" onclick={() => void finish()}>开始使用<Icon name="check" size={16}/></button>{/if}
  </div>
</dialog>

<style>
  .onboarding-dialog { width: min(520px, calc(100vw - 30px)); max-width: none; max-height: calc(100vh - 30px); margin: auto; padding: 0; }
  .onboarding-dialog[open] { display: flex; flex-direction: column; }
  .onboard-header { display: flex; align-items: center; gap: 13px; padding: 22px 24px 14px; }
  .onboard-header img { width: 42px; height: 42px; object-fit: contain; }
  .onboard-header p { color: var(--muted); font-size: 12px; }
  .onboard-header h2 { font-size: 19px; font-weight: 650; line-height: 1.2; }
  .onboard-progress { display: flex; gap: 5px; padding: 0 24px 18px; }
  .onboard-progress span { height: 4px; flex: 1; border-radius: 6px; background: var(--border); }
  .onboard-progress span.active { background: var(--accent); }
  .onboard-body { min-height: 150px; overflow: auto; padding: 0 24px 20px; }
  .onboard-body > p:first-child { color: var(--muted); margin-bottom: 20px; }
  .onboard-field, .onboard-imports { display: flex; gap: 8px; margin-top: 8px; }
  .onboard-field .ui-input { min-width: 0; flex: 1; }
  .onboard-imports .ui-button { flex: 1; }
  .onboard-status { margin-top: 15px; color: var(--accent-ink); font-size: 12px; }
  .onboard-error { margin-top: 15px; padding: 8px 10px; background: var(--danger-soft); color: var(--danger); border-radius: 8px; font-size: 12px; }
  .onboard-spacer { flex: 1; }
</style>
