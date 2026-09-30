<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

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
  let cell: HTMLDivElement | undefined = $state();

  function ensureRequested() {
    if (requested || src) return;
    requested = true;
    invoke("request_thumbnail", { contentHash }).catch(() => {});
  }

  // 父列表刷新后若已生成，直接切到缩略图
  $effect(() => {
    if (thumbnailPath) src = convertFileSrc(thumbnailPath);
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
      if (e.payload.hash === contentHash && e.payload.path) {
        src = convertFileSrc(e.payload.path);
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
  onmouseenter={() => (hovered = true)}
  onmouseleave={() => (hovered = false)}
  class="aspect-square w-full overflow-hidden bg-neutral-100 dark:bg-neutral-800"
>
  {#if hoverPlay && internalPath && hovered}
    <img src={convertFileSrc(internalPath)} class="h-full w-full object-contain" alt="" />
  {:else if src}
    <img {src} class="h-full w-full object-contain" alt="" />
  {:else}
    <div class="h-full w-full animate-pulse bg-neutral-200 dark:bg-neutral-700"></div>
  {/if}
</div>
