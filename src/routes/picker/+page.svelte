<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import MemeThumb from "$lib/components/MemeThumb.svelte";

  interface Meme {
    id: number;
    original_filename: string;
    content_hash: string;
    internal_path: string;
    thumbnail_path: string | null;
  }
  interface Collection {
    id: number;
    name: string;
    sort_order: number;
    group_id: number | null;
  }
  interface CollectionGroup {
    id: number;
    name: string;
    sort_order: number;
  }
  type View = { kind: "all" | "favorites" | "recent" } | { kind: "collection"; id: number };

  let query = $state("");
  let memes = $state<Meme[]>([]);
  let collections = $state<Collection[]>([]);
  let groups = $state<CollectionGroup[]>([]);
  let activeView = $state<View>({ kind: "recent" });
  let selectedId = $state<number | null>(null);
  let searchInput: HTMLInputElement | undefined = $state();
  let gridEl: HTMLDivElement | undefined = $state();
  let requestId = 0;

  const selectedIndex = $derived(memes.findIndex((m) => m.id === selectedId));

  interface ViewItem {
    key: string;
    label: string;
    view: View;
  }
  const orderedCollections = $derived([
    ...collections.filter((c) => c.group_id === null),
    ...groups.flatMap((g) => collections.filter((c) => c.group_id === g.id)),
  ]);
  const viewItems = $derived<ViewItem[]>([
    { key: "all", label: "全部", view: { kind: "all" } },
    { key: "favorites", label: "收藏", view: { kind: "favorites" } },
    { key: "recent", label: "最近使用", view: { kind: "recent" } },
    ...orderedCollections.map((c): ViewItem => ({
      key: `collection:${c.id}`,
      label: c.name,
      view: { kind: "collection", id: c.id },
    })),
  ]);

  function viewKey(v: View): string {
    return v.kind === "collection" ? `collection:${v.id}` : v.kind;
  }
  const activeKey = $derived(viewKey(activeView));

  async function loadMemes() {
    const id = ++requestId;
    const q = query.trim();
    const result = await invoke<Meme[]>("list_memes", { view: activeView, query: q || undefined });
    if (id !== requestId) return;
    memes = result;
    // 自动高亮首项；当前选中项不在结果里时回落到首项
    if (memes.length === 0) selectedId = null;
    else if (selectedId === null || !memes.some((m) => m.id === selectedId)) {
      selectedId = memes[0].id;
    }
  }

  async function loadCollections() {
    collections = await invoke<Collection[]>("list_collections");
    groups = await invoke<CollectionGroup[]>("list_collection_groups");
  }

  function switchView(view: View) {
    activeView = view;
    selectedId = null;
    void loadMemes();
  }

  function cycleCollection(step: 1 | -1) {
    if (viewItems.length === 0) return;
    const idx = viewItems.findIndex((v) => v.key === activeKey);
    const next = ((idx < 0 ? 0 : idx) + step + viewItems.length) % viewItems.length;
    switchView(viewItems[next].view);
  }

  /** auto-fill 列数估算：minmax(72px) + gap 6px */
  function gridColumns(): number {
    if (!gridEl) return 1;
    return Math.max(1, Math.floor((gridEl.clientWidth + 6) / (72 + 6)));
  }

  function moveSelection(delta: number) {
    if (memes.length === 0) return;
    const current = selectedIndex < 0 ? 0 : selectedIndex;
    const next = Math.min(memes.length - 1, Math.max(0, current + delta));
    if (next !== current) selectedId = memes[next].id;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      invoke("hide_picker");
      return;
    }
    // Ctrl+方向键在收藏夹间循环切换
    if ((event.ctrlKey || event.metaKey) && event.key.startsWith("Arrow")) {
      event.preventDefault();
      cycleCollection(event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : -1);
      return;
    }
    if (event.key.startsWith("Arrow")) {
      event.preventDefault();
      const cols = gridColumns();
      const delta = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : event.key === "ArrowDown" ? cols : -cols;
      moveSelection(delta);
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      void copySelected();
    }
  }

  async function copySelected() {
    if (selectedId === null) return;
    try {
      // Smart Copy：后端完成剪贴板写入 + 隐藏窗口 + 恢复焦点
      await invoke("smart_copy", { memeId: selectedId });
    } catch (e) {
      console.error(e);
    }
  }

  // 高亮项滚入可视区
  $effect(() => {
    if (selectedId === null) return;
    document.getElementById(`picker-cell-${selectedId}`)?.scrollIntoView({ block: "nearest" });
  });

  onMount(() => {
    void loadCollections();
    void loadMemes();
    searchInput?.focus();
    // 每次呼出重置为「最近使用 + 清空搜索 + 搜索聚焦」
    void listen("picker-shown", () => {
      query = "";
      selectedId = null;
      activeView = { kind: "recent" };
      void loadCollections();
      void loadMemes();
      searchInput?.focus();
    });
  });
</script>

<svelte:window onkeydown={onKeydown} />

<div class="flex h-screen overflow-hidden rounded-lg border border-neutral-700 bg-neutral-900 text-neutral-100 shadow-2xl">
  <aside class="w-36 shrink-0 overflow-y-auto border-r border-neutral-800 p-2 text-sm">
    <p class="mb-1 px-1 text-xs text-neutral-500">收藏夹</p>
    <ul class="space-y-0.5">
      {#each viewItems.slice(0, 3) as item (item.key)}
        <li>
          <button
            class={`w-full truncate rounded px-2 py-1 text-left ${
              item.key === activeKey ? "bg-neutral-800" : "hover:bg-neutral-800/60"
            }`}
            title={item.label}
            onclick={() => switchView(item.view)}
          >
            {item.label}
          </button>
        </li>
      {/each}
      {#each collections.filter((c) => c.group_id === null) as c (c.id)}
        <li><button class={`w-full truncate rounded px-2 py-1 text-left ${activeKey === `collection:${c.id}` ? "bg-neutral-800" : "hover:bg-neutral-800/60"}`} title={c.name} onclick={() => switchView({ kind: "collection", id: c.id })}>{c.name}</button></li>
      {/each}
      {#each groups as g (g.id)}
        <li class="mt-2 truncate px-2 pt-1 text-xs text-neutral-500" title={g.name}>{g.name}</li>
        {#each collections.filter((c) => c.group_id === g.id) as c (c.id)}
          <li><button class={`w-full truncate rounded py-1 pl-4 pr-2 text-left ${activeKey === `collection:${c.id}` ? "bg-neutral-800" : "hover:bg-neutral-800/60"}`} title={c.name} onclick={() => switchView({ kind: "collection", id: c.id })}>{c.name}</button></li>
        {/each}
      {/each}
    </ul>
  </aside>
  <main class="flex flex-1 flex-col">
    <div bind:this={gridEl} class="flex-1 overflow-y-auto p-2">
      {#if memes.length === 0}
        <div class="grid h-full place-items-center text-xs text-neutral-600">
          {query.trim() ? "没有匹配的表情" : "还没有表情，先去主窗口导入吧"}
        </div>
      {:else}
        <div class="grid grid-cols-[repeat(auto-fill,minmax(72px,1fr))] gap-1.5">
          {#each memes as m (m.id)}
            <button
              id={`picker-cell-${m.id}`}
              type="button"
              class={`overflow-hidden rounded border bg-neutral-800 ${
                m.id === selectedId
                  ? "border-neutral-300 ring-1 ring-neutral-300"
                  : "border-neutral-700"
              }`}
              title={m.original_filename}
              onmouseenter={() => (selectedId = m.id)}
              onclick={() => void copySelected()}
            >
              <MemeThumb
                contentHash={m.content_hash}
                thumbnailPath={m.thumbnail_path}
                internalPath={m.internal_path}
                hoverPlay
              />
            </button>
          {/each}
        </div>
      {/if}
    </div>
    <div class="border-t border-neutral-800 p-2">
      <input
        bind:this={searchInput}
        bind:value={query}
        oninput={() => void loadMemes()}
        placeholder="搜索文件名、标签、描述…（Ctrl+方向键切收藏夹）"
        class="w-full rounded bg-neutral-800 px-3 py-2 text-sm outline-none placeholder:text-neutral-500"
      />
    </div>
  </main>
</div>
