<script lang="ts">
  import "../app.css";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { applyTheme, type Theme } from "$lib/theme";

  let { children } = $props();

  onMount(() => {
    // 两个窗口各自应用主题偏好；跟随系统时监听系统切换
    invoke<{ theme: Theme }>("get_config")
      .then((cfg) => applyTheme(cfg.theme))
      .catch(() => {});
    window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
      invoke<{ theme: Theme }>("get_config")
        .then((cfg) => applyTheme(cfg.theme))
        .catch(() => {});
    });
    void listen<Theme>("theme-changed", (e) => applyTheme(e.payload));
  });
</script>

{@render children()}
