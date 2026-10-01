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
  type BrowseView =
    | { kind: "all" | "favorites" | "recent" }
    | { kind: "collection"; id: number };
  type View = BrowseView | { kind: "collections"; id: number[] };

  let query = $state("");
  let memes = $state<Meme[]>([]);
  let collections = $state<Collection[]>([]);
  let groups = $state<CollectionGroup[]>([]);
  let activeView = $state<BrowseView>({ kind: "recent" });
  let searchCollectionIds = $state<number[]>([]);
  let scopeOpen = $state(false);
  let loading = $state(false);
  let loadError = $state(false);
  let collectionsLoading = $state(false);
  let collectionsError = $state(false);
  let selectedId = $state<number | null>(null);
  let searchInput: HTMLInputElement | undefined = $state();
  let scopeContainer: HTMLDivElement | undefined = $state();
  let gridEl: HTMLDivElement | undefined = $state();
  let requestId = 0;
  let collectionsRequestId = 0;

  const selectedIndex = $derived(memes.findIndex((m) => m.id === selectedId));

  interface ViewItem {
    key: string;
    label: string;
    view: BrowseView;
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

  function viewKey(v: BrowseView): string {
    return v.kind === "collection" ? `collection:${v.id}` : v.kind;
  }
  const activeKey = $derived(viewKey(activeView));
  const scopeLabel = $derived(
    searchCollectionIds.length === 0
      ? "全部图库"
      : searchCollectionIds.length === 1
        ? collections.find((c) => c.id === searchCollectionIds[0])?.name ?? "1 个收藏夹"
        : `${searchCollectionIds.length} 个收藏夹`,
  );

  async function loadMemes() {
    const id = ++requestId;
    const q = query.trim();
    const view: View = q
      ? searchCollectionIds.length === 0
        ? { kind: "all" }
        : { kind: "collections", id: [...searchCollectionIds] }
      : activeView;
    loading = true;
    loadError = false;
    selectedId = null;
    try {
      const result = await invoke<Meme[]>("list_memes", { view, query: q || undefined });
      if (id !== requestId) return;
      memes = result;
      selectedId = result[0]?.id ?? null;
    } catch (e) {
      if (id !== requestId) return;
      console.error(e);
      memes = [];
      loadError = true;
    } finally {
      if (id === requestId) loading = false;
    }
  }

  async function loadCollections() {
    const id = ++collectionsRequestId;
    collectionsLoading = true;
    collectionsError = false;
    try {
      const [nextCollections, nextGroups] = await Promise.all([
        invoke<Collection[]>("list_collections"),
        invoke<CollectionGroup[]>("list_collection_groups"),
      ]);
      if (id !== collectionsRequestId) return;
      collections = nextCollections;
      groups = nextGroups;
    } catch (e) {
      if (id !== collectionsRequestId) return;
      console.error(e);
      collectionsError = true;
    } finally {
      if (id === collectionsRequestId) collectionsLoading = false;
    }
  }

  function switchView(view: BrowseView) {
    activeView = view;
    query = "";
    searchCollectionIds = view.kind === "collection" ? [view.id] : [];
    scopeOpen = false;
    void loadMemes();
  }

  function clearSearchScope() {
    searchCollectionIds = [];
    activeView = { kind: "all" };
    void loadMemes();
  }

  function toggleSearchCollection(id: number) {
    searchCollectionIds = searchCollectionIds.includes(id)
      ? searchCollectionIds.filter((selected) => selected !== id)
      : [...searchCollectionIds, id];
    if (!query.trim()) activeView = searchCollectionIds.length === 1
      ? { kind: "collection", id: searchCollectionIds[0] }
      : { kind: "all" };
    void loadMemes();
  }

  function onWindowPointerDown(event: PointerEvent) {
    if (scopeOpen && !scopeContainer?.contains(event.target as Node)) scopeOpen = false;
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
    if (loading || loadError || memes.length === 0) return;
    const current = selectedIndex < 0 ? 0 : selectedIndex;
    const next = Math.min(memes.length - 1, Math.max(0, current + delta));
    if (next !== current) selectedId = memes[next].id;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.isComposing || event.keyCode === 229) return;
    if (event.key === "Escape") {
      event.preventDefault();
      if (scopeOpen) {
        scopeOpen = false;
        scopeContainer?.querySelector("button")?.focus();
        return;
      }
      invoke("hide_picker");
      return;
    }
    if (scopeContainer?.contains(event.target as Node)) return;
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
      if (event.target instanceof HTMLButtonElement && !event.target.id.startsWith("picker-cell-")) return;
      event.preventDefault();
      void copySelected();
    }
  }

  async function copySelected() {
    if (selectedId === null || loading || loadError) return;
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
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen("picker-shown", () => {
      query = "";
      searchCollectionIds = [];
      scopeOpen = false;
      selectedId = null;
      activeView = { kind: "recent" };
      void loadCollections();
      void loadMemes();
      searchInput?.focus();
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    }).catch(console.error);
    return () => {
      disposed = true;
      unlisten?.();
      ++requestId;
      ++collectionsRequestId;
    };
  });
</script>

<svelte:window onkeydown={onKeydown} onpointerdown={onWindowPointerDown} />

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
      {#if loading}
        <div class="grid h-full place-items-center text-xs text-neutral-500">正在加载…</div>
      {:else if loadError}
        <div class="grid h-full place-items-center text-xs text-red-400">
          <button type="button" onclick={() => void loadMemes()}>加载失败，点击重试</button>
        </div>
      {:else if memes.length === 0}
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
    <div class="flex items-center gap-2 border-t border-neutral-800 p-2">
      <input
        bind:this={searchInput}
        bind:value={query}
        oninput={(event) => {
          query = event.currentTarget.value;
          void loadMemes();
        }}
        placeholder="搜索文件名、标签、描述…"
        aria-label="搜索表情"
        class="min-w-0 flex-1 rounded bg-neutral-800 px-3 py-2 text-sm outline-none placeholder:text-neutral-500"
      />
      <div bind:this={scopeContainer} class="relative shrink-0">
        <button
          type="button"
          class="max-w-32 truncate rounded bg-neutral-800 px-2 py-2 text-xs hover:bg-neutral-700"
          title={`搜索范围：${scopeLabel}`}
          aria-label={`搜索范围：${scopeLabel}`}
          aria-expanded={scopeOpen}
          onclick={() => (scopeOpen = !scopeOpen)}
        >
          {scopeLabel} ▾
        </button>
        {#if scopeOpen}
          <div class="absolute bottom-full right-0 z-10 mb-2 w-48 rounded border border-neutral-700 bg-neutral-900 p-1.5 text-xs shadow-xl" role="group" aria-label="搜索范围">
            <div class="flex items-center justify-between px-1 py-1 text-neutral-400">
              <span>搜索范围</span>
              {#if searchCollectionIds.length > 0}
                <button type="button" class="hover:text-neutral-100" onclick={clearSearchScope}>清除所有</button>
              {/if}
            </div>
            <button
              type="button"
              class={`w-full rounded px-2 py-1.5 text-left ${searchCollectionIds.length === 0 ? "bg-neutral-700 text-white" : "hover:bg-neutral-800"}`}
              aria-pressed={searchCollectionIds.length === 0}
              onclick={clearSearchScope}
            >全部图库</button>
            <div class="my-1 border-t border-neutral-700"></div>
            <div class="max-h-52 overflow-y-auto">
              {#if collectionsLoading}
                <p class="px-2 py-1.5 text-neutral-500">正在加载收藏夹…</p>
              {:else if collectionsError}
                <button type="button" class="px-2 py-1.5 text-red-400" onclick={() => void loadCollections()}>加载失败，点击重试</button>
              {:else if orderedCollections.length === 0}
                <p class="px-2 py-1.5 text-neutral-500">暂无收藏夹</p>
              {:else}
                {#each orderedCollections as c (c.id)}
                  <label class="flex cursor-pointer items-center gap-2 rounded px-2 py-1.5 hover:bg-neutral-800" title={c.name}>
                    <input
                      type="checkbox"
                      checked={searchCollectionIds.includes(c.id)}
                      onchange={() => toggleSearchCollection(c.id)}
                      class="accent-neutral-300"
                    />
                    <span class="truncate">{c.name}</span>
                  </label>
                {/each}
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </div>
  </main>
</div>
