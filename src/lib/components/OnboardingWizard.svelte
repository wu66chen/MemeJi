<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import HotkeyRecorder from "./HotkeyRecorder.svelte";

  interface Props {
    onFinish: () => void;
  }
  let { onFinish }: Props = $props();

  let step = $state(1);
  let libraryPath = $state("");
  let pendingHotkey = $state<string | null>(null);
  let importSummary = $state("");

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
      alert(String(e));
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
      alert(String(e));
    }
  }

  async function importFolder() {
    const picked = await open({ directory: true });
    if (!picked) return;
    try {
      const r = await invoke<ImportResult>("import_paths", { paths: [picked] });
      importSummary = `导入 ${r.imported} · 重复 ${r.skipped} · 不支持 ${r.unsupported} · 失败 ${r.failed}`;
    } catch (e) {
      alert(String(e));
    }
  }

  async function finish() {
    try {
      await invoke("complete_onboarding", { hotkey: pendingHotkey ?? defaultHotkey(), theme: "system" });
      onFinish();
    } catch (e) {
      alert(String(e));
    }
  }

  function defaultHotkey(): string {
    return navigator.platform.toLowerCase().includes("mac") ? "Cmd+Shift+Space" : "Ctrl+Shift+Space";
  }

  const stepTitles = ["创建表情库", "导入表情（可跳过）", "设置呼出快捷键"];
</script>

<div class="fixed inset-0 z-50 grid place-items-center bg-neutral-900/60 p-6">
  <div class="w-full max-w-md rounded-lg bg-white p-6 shadow-2xl dark:bg-neutral-800">
    <p class="text-xs text-neutral-400">第 {step} 步 / 共 3 步</p>
    <h2 class="mb-4 text-lg font-semibold">{stepTitles[step - 1]}</h2>

    {#if step === 1}
      <p class="mb-3 text-sm text-neutral-600 dark:text-neutral-300">
        表情会复制进统一管理的库目录，原文件不受影响。默认位置如下，也可以换成别的文件夹。
      </p>
      <div class="mb-4 flex gap-2">
        <input
          readonly
          value={libraryPath}
          class="min-w-0 flex-1 rounded border border-neutral-300 bg-neutral-50 px-2 py-1.5 text-xs dark:border-neutral-600 dark:bg-neutral-900"
        />
        <button
          type="button"
          class="shrink-0 rounded bg-neutral-200 px-2 py-1.5 text-xs hover:bg-neutral-300 dark:bg-neutral-700 dark:hover:bg-neutral-600"
          onclick={() => void chooseLibraryFolder()}
        >选择…</button>
      </div>
      <button
        type="button"
        class="w-full rounded bg-neutral-800 py-2 text-sm text-white hover:bg-neutral-700 dark:bg-neutral-700 dark:hover:bg-neutral-600"
        onclick={() => (step = 2)}
      >下一步</button>
    {:else if step === 2}
      <p class="mb-3 text-sm text-neutral-600 dark:text-neutral-300">
        把已有的表情包一次性搬进来。这一步也可以跳过，之后随时在主窗口导入。
      </p>
      <div class="mb-3 flex gap-2">
        <button
          type="button"
          class="flex-1 rounded bg-neutral-200 py-2 text-sm hover:bg-neutral-300 dark:bg-neutral-700 dark:hover:bg-neutral-600"
          onclick={() => void importFiles()}
        >导入图片</button>
        <button
          type="button"
          class="flex-1 rounded bg-neutral-200 py-2 text-sm hover:bg-neutral-300 dark:bg-neutral-700 dark:hover:bg-neutral-600"
          onclick={() => void importFolder()}
        >导入文件夹</button>
      </div>
      {#if importSummary}
        <p class="mb-3 text-xs text-neutral-500">{importSummary}</p>
      {/if}
      <div class="flex gap-2">
        <button
          type="button"
          class="flex-1 rounded py-2 text-sm text-neutral-500 hover:text-neutral-800 dark:hover:text-neutral-200"
          onclick={() => (step = 3)}
        >跳过</button>
        <button
          type="button"
          class="flex-1 rounded bg-neutral-800 py-2 text-sm text-white hover:bg-neutral-700 dark:bg-neutral-700 dark:hover:bg-neutral-600"
          onclick={() => (step = 3)}
        >下一步</button>
      </div>
    {:else}
      <p class="mb-3 text-sm text-neutral-600 dark:text-neutral-300">
        在任何应用里按这个组合键，就能在鼠标旁呼出表情选择器。
      </p>
      <div class="mb-4">
        <HotkeyRecorder initialHotkey={defaultHotkey()} onChange={(accel) => (pendingHotkey = accel)} />
      </div>
      <button
        type="button"
        class="w-full rounded bg-neutral-800 py-2 text-sm text-white hover:bg-neutral-700 dark:bg-neutral-700 dark:hover:bg-neutral-600"
        onclick={() => void finish()}
      >完成</button>
    {/if}
  </div>
</div>
