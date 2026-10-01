<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Icon from './Icon.svelte';

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

<div class="hotkey-control">
  <kbd><Icon name="keyboard" size={16}/>{hotkey}</kbd>
  <button
    type="button"
    class={`ui-button small ${recording ? 'recording' : ''}`}
    onclick={() => (recording = !recording)}
  >
    {recording ? "按下组合键…" : "更改"}
  </button>
  <button
    type="button"
    class="ui-button small ghost"
    onclick={() => void resetDefault()}
  >
    恢复默认
  </button>
  {#if status}
    <span role="status" class={`hotkey-status ${status.ok ? 'ok' : 'error'}`}>
      {status.text}
    </span>
  {/if}
</div>

<style>
  .hotkey-control { display: flex; flex-wrap: wrap; align-items: center; gap: 7px; margin-top: 9px; }
  kbd { display: inline-flex; align-items: center; gap: 7px; min-height: 30px; padding: 4px 10px; border: 1px solid var(--border); border-radius: 7px; background: var(--surface-soft); color: var(--text); font-family: inherit; font-size: 12px; font-weight: 600; }
  .recording { background: var(--accent-soft); color: var(--accent-ink); border-color: var(--accent); }
  .hotkey-status { font-size: 12px; }
  .hotkey-status.ok { color: var(--accent-ink); }
  .hotkey-status.error { color: var(--danger); }
</style>
