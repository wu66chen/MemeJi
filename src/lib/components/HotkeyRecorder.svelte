<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  interface Props {
    initialHotkey: string;
    /** 录制确认（已通过冲突探测）后回调，尚未持久化 */
    onChange?: (accelerator: string) => void;
  }

  let { initialHotkey, onChange }: Props = $props();

  let hotkeyOverride = $state<string | null>(null);
  const hotkey = $derived(hotkeyOverride ?? initialHotkey);
  let recording = $state(false);
  let status = $state<{ ok: boolean; text: string } | null>(null);

  const defaultHotkey = navigator.platform.toLowerCase().includes("mac")
    ? "Cmd+Shift+Space"
    : "Ctrl+Shift+Space";

  function accelFromEvent(e: KeyboardEvent): string | null {
    const mods: string[] = [];
    if (e.ctrlKey) mods.push("Ctrl");
    if (e.altKey) mods.push("Alt");
    if (e.shiftKey) mods.push("Shift");
    if (e.metaKey) mods.push("Super");
    let key = e.key;
    if (["Control", "Alt", "Shift", "Meta"].includes(key)) return null; // 只按了修饰键
    if (key === " ") key = "Space";
    if (/^[a-z]$/i.test(key)) key = key.toUpperCase();
    const okKey =
      /^[A-Z0-9]$/.test(key) ||
      /^F([1-9]|1[0-2])$/.test(key) ||
      [
        "Space", "Enter", "Escape", "Tab", "Backspace", "Delete", "Insert",
        "Home", "End", "PageUp", "PageDown", "Up", "Down", "Left", "Right",
      ].includes(key);
    if (!okKey || mods.length === 0) return null;
    return [...mods, key].join("+");
  }

  async function probe(accel: string): Promise<boolean> {
    try {
      await invoke("probe_hotkey", { accelerator: accel });
      return true;
    } catch (e) {
      status = { ok: false, text: String(e) };
      return false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      recording = false;
      status = null;
      return;
    }
    const accel = accelFromEvent(e);
    if (!accel) return;
    recording = false;
    hotkeyOverride = accel;
    void probe(accel).then((ok) => {
      if (ok) {
        status = { ok: true, text: "可用" };
        onChange?.(accel);
      }
    });
  }

  async function resetDefault() {
    if (!(await probe(defaultHotkey))) return;
    hotkeyOverride = defaultHotkey;
    status = { ok: true, text: "已恢复默认" };
    onChange?.(defaultHotkey);
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="flex items-center gap-2">
  <kbd class="rounded border border-neutral-300 bg-neutral-100 px-2 py-1 text-xs dark:border-neutral-600 dark:bg-neutral-800">
    {hotkey}
  </kbd>
  <button
    type="button"
    class={`rounded px-2 py-1 text-xs ${
      recording
        ? "bg-red-600 text-white"
        : "bg-neutral-200 text-neutral-700 hover:bg-neutral-300 dark:bg-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-600"
    }`}
    onclick={() => (recording = !recording)}
  >
    {recording ? "录制中…（Esc 取消）" : "重新录制"}
  </button>
  <button
    type="button"
    class="rounded px-2 py-1 text-xs text-neutral-500 hover:text-neutral-800 dark:hover:text-neutral-200"
    onclick={() => void resetDefault()}
  >
    恢复默认
  </button>
  {#if status}
    <span class={`text-xs ${status.ok ? "text-green-600 dark:text-green-400" : "text-red-600 dark:text-red-400"}`}>
      {status.text}
    </span>
  {/if}
</div>
