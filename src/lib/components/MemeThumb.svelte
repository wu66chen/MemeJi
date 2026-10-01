<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import Icon from "$lib/components/Icon.svelte";

  interface Props {
    contentHash: string;
    thumbnailPath: string | null;
    /** 原图路径：提供 + hoverPlay 时悬停播放原始字节（动图动画） */
    internalPath?: string | null;
    hoverPlay?: boolean;
  }

  let { contentHash, thumbnailPath, internalPath = null, hoverPlay = false }: Props = $props();

  let src = $state<string | null>(null);
  let requested = false;
  let hovered = $state(false);
  let hoveredOnce = $state(false);
  let originalLoaded = $state(false);
  let failed = $state(false);
  let cell: HTMLDivElement | undefined = $state();

  function ensureRequested() {
    if (requested || src) return;
    requested = true;
    invoke("request_thumbnail", { contentHash }).catch(() => (failed = true));
  }

  // 父列表刷新后若已生成，直接切到缩略图
  $effect(() => {
    if (thumbnailPath) {
      failed = false;
      src = convertFileSrc(thumbnailPath);
    }
  });

  // 可视区驱动：进入视口才入队；生成完由 thumbnail-ready 事件原地刷新
  $effect(() => {
    const io = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) ensureRequested();
    });
    if (cell) io.observe(cell);

    let disposed = false;
    let unlisten: (() => void) | undefined;
    listen<{ hash: string; path: string | null }>("thumbnail-ready", (e) => {
      if (e.payload.hash === contentHash) {
        if (e.payload.path) {
          failed = false;
          src = convertFileSrc(e.payload.path);
        } else {
          failed = true;
        }
      }
    }).then((u) => {
      if (disposed) u();
      else unlisten = u;
    });
    return () => {
      disposed = true;
      unlisten?.();
      io.disconnect();
    };
  });
</script>

<div
  bind:this={cell}
  role="presentation"
  onmouseenter={() => { hovered = true; hoveredOnce = true; }}
  onmouseleave={() => (hovered = false)}
  class="thumb-surface relative aspect-square w-full overflow-hidden"
>
  {#if src && !failed}
    <img draggable="false" {src} onerror={() => (failed = true)} class="h-full w-full object-contain" alt="" />
  {:else if failed}
    <div class="thumb-failed" title="无法预览"><Icon name="image" size={23} /></div>
  {:else}
    <div class="thumb-loading h-full w-full"></div>
  {/if}
  {#if hoverPlay && internalPath && hoveredOnce}
    <img draggable="false" src={convertFileSrc(internalPath)} onload={() => (originalLoaded = true)}
      class={`absolute inset-0 h-full w-full object-contain transition-opacity duration-75 ${hovered && originalLoaded ? 'opacity-100' : 'opacity-0'}`} alt="" />
  {/if}
</div>

<style>
  .thumb-surface { background: var(--surface-soft); }
  .thumb-loading { background: linear-gradient(110deg, var(--surface-soft) 25%, var(--hover) 45%, var(--surface-soft) 65%); background-size: 200% 100%; animation: shimmer 1.8s ease-in-out infinite; }
  .thumb-failed { display: grid; width: 100%; height: 100%; place-items: center; color: var(--faint); }
  @keyframes shimmer { to { background-position-x: -200%; } }
  @media (prefers-reduced-motion: reduce) { .thumb-loading { animation: none; } }
</style>
