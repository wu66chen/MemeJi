<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import MemeThumb from "$lib/components/MemeThumb.svelte";
  import Icon from "$lib/components/Icon.svelte";

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
  let copyError = $state('');
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
  const activeTitle = $derived.by(() => {
    if (query.trim()) return '搜索结果';
    const view = activeView;
    if (view.kind === 'collection') return collections.find(c => c.id === view.id)?.name ?? '收藏夹';
    return { all: '全部表情', favorites: '收藏', recent: '最近使用' }[view.kind];
  });
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
    const view: View = searchCollectionIds.length
      ? { kind: "collections", id: [...searchCollectionIds] }
      : q ? { kind: "all" } : activeView;
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
    copyError = '';
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
      copyError = '';
    } catch (e) {
      console.error(e);
      copyError = `复制失败：${String(e)}`;
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
      copyError = '';
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

<div class="picker-shell">
  <header class="picker-toolbar">
    <div class="picker-search"><Icon name="search" size={17}/><input bind:this={searchInput} bind:value={query} aria-label="搜索表情" placeholder="搜索表情" oninput={() => void loadMemes()} /></div>
    <div bind:this={scopeContainer} class="picker-scope-wrap">
      <button type="button" class="ui-button picker-scope" title={`搜索范围：${scopeLabel}`} aria-label={`搜索范围：${scopeLabel}`} aria-expanded={scopeOpen} onclick={() => (scopeOpen = !scopeOpen)}><span class="truncate">{scopeLabel}</span><Icon name="chevron-down" size={14}/></button>
      {#if scopeOpen}
        <div class="ui-menu picker-scope-menu" role="group" aria-label="搜索范围">
          <div class="scope-heading"><span>搜索范围</span><button type="button" onclick={clearSearchScope}>清除所有</button></div>
          <div class="picker-scope-options">
            {#if collectionsLoading}<p class="scope-empty">正在加载收藏夹…</p>
            {:else if collectionsError}<button class="ui-menu-item danger" onclick={() => void loadCollections()}>加载失败，重试</button>
            {:else}
              {#each orderedCollections as c (c.id)}<label class="scope-option" title={c.name}><input class="ui-checkbox" type="checkbox" checked={searchCollectionIds.includes(c.id)} onchange={() => toggleSearchCollection(c.id)} /><span class="truncate">{c.name}</span></label>
              {:else}<p class="scope-empty">暂无收藏夹</p>{/each}
            {/if}
          </div>
        </div>
      {/if}
    </div>
    <button type="button" class="icon-button round" aria-label="关闭 Quick Picker" title="关闭" onclick={() => void invoke('hide_picker')}><Icon name="x" size={16}/></button>
  </header>
  <div class="picker-body">
    <aside class="picker-sidebar"><div class="picker-side-label">图库</div>
      {#each viewItems.slice(0, 3) as item (item.key)}
        <button class={`picker-nav ${item.key === activeKey && searchCollectionIds.length < 2 ? 'active' : ''}`} title={item.label} onclick={() => switchView(item.view)}><Icon name={item.key === 'all' ? 'grid' : item.key === 'favorites' ? 'heart' : 'clock'} size={16}/><span class="truncate">{item.label}</span></button>
      {/each}
      <div class="picker-side-label collections-label">收藏夹</div>
      {#each collections.filter((c) => c.group_id === null) as c (c.id)}<button class={`picker-nav ${activeKey === `collection:${c.id}` && searchCollectionIds.length === 1 ? 'active' : ''}`} title={c.name} onclick={() => switchView({ kind: 'collection', id: c.id })}><Icon name="folder" size={15}/><span class="truncate">{c.name}</span></button>{/each}
      {#each groups as g (g.id)}<div class="picker-group" title={g.name}>{g.name}</div>{#each collections.filter((c) => c.group_id === g.id) as c (c.id)}<button class={`picker-nav nested ${activeKey === `collection:${c.id}` && searchCollectionIds.length === 1 ? 'active' : ''}`} title={c.name} onclick={() => switchView({ kind: 'collection', id: c.id })}><Icon name="folder" size={15}/><span class="truncate">{c.name}</span></button>{/each}{/each}
    </aside>
    <main class="picker-main">
      <div class="picker-results-heading"><span>{activeTitle}</span><span class="ui-faint">{memes.length} 张</span></div>
      <div bind:this={gridEl} class="picker-grid-scroll">
        {#if loading}<div class="ui-empty"><p>正在加载…</p></div>
        {:else if loadError}<div class="ui-empty"><div class="ui-empty-mark"><Icon name="alert" size={24}/></div><h2>加载失败</h2><button class="ui-button small" onclick={() => void loadMemes()}>重试</button></div>
        {:else if memes.length === 0}<div class="ui-empty"><div class="ui-empty-mark"><Icon name={query.trim() ? 'search' : 'image'} size={25}/></div><h2>{query.trim() ? '没有找到表情' : activeView.kind === 'recent' ? '还没有使用记录' : activeView.kind === 'favorites' ? '还没有收藏的表情' : activeView.kind === 'collection' ? '这个收藏夹还没有图片' : '图库里还没有图片'}</h2><p>{query.trim() ? '换个词或清除搜索范围再试。' : activeView.kind === 'recent' ? '复制过的表情会出现在这里。' : '可以在主窗口导入或整理表情。'}</p></div>
        {:else}<div class="picker-grid">{#each memes as m (m.id)}<button id={`picker-cell-${m.id}`} type="button" class={`picker-cell ${m.id === selectedId ? 'active' : ''}`} title={m.original_filename} aria-label={m.original_filename} onmouseenter={() => (selectedId = m.id)} onclick={() => void copySelected()}><MemeThumb contentHash={m.content_hash} thumbnailPath={m.thumbnail_path} internalPath={m.internal_path} hoverPlay /></button>{/each}</div>{/if}
      </div>
      {#if copyError}<div class="picker-error" role="alert">{copyError}</div>{/if}
    </main>
  </div>
</div>

<style>
  .picker-shell { display: flex; flex-direction: column; height: 100vh; overflow: hidden; border: 1px solid var(--border); border-radius: 12px; background: var(--surface); color: var(--text); box-shadow: var(--shadow); }
  .picker-toolbar { display: flex; align-items: center; gap: 8px; flex-shrink: 0; min-height: 55px; padding: 8px 11px; border-bottom: 1px solid var(--border); }
  .picker-search { display: flex; align-items: center; gap: 9px; min-width: 0; flex: 1; height: 36px; padding: 0 11px; border: 1px solid var(--border); border-radius: 9px; background: var(--surface-soft); color: var(--muted); }
  .picker-search:focus-within { border-color: var(--accent); box-shadow: 0 0 0 3px color-mix(in srgb,var(--accent) 12%,transparent); }
  .picker-search input { width: 100%; min-width: 0; outline: 0; border: 0; background: transparent; color: var(--text); }
  .picker-search input::placeholder { color: var(--faint); }
  .picker-scope-wrap { position: relative; }
  .picker-scope { max-width: 140px; width: 140px; justify-content: space-between; }
  .picker-scope-menu { position: absolute; top: 43px; right: 0; z-index: 20; width: 200px; }
  .picker-scope-options { max-height: 242px; overflow: auto; }
  .picker-body { display: flex; flex: 1; min-height: 0; }
  .picker-sidebar { width: 142px; flex-shrink: 0; overflow: auto; padding: 10px 8px; background: var(--sidebar); border-right: 1px solid var(--border); }
  .picker-side-label { padding: 3px 8px 7px; color: var(--faint); font-size: 11px; font-weight: 700; letter-spacing: .08em; }
  .picker-side-label.collections-label { margin-top: 12px; }
  .picker-nav { display: flex; width: 100%; align-items: center; gap: 8px; min-height: 32px; padding: 5px 8px; border-radius: 7px; text-align: left; color: var(--muted); }
  .picker-nav:hover { background: var(--hover); color: var(--text); }
  .picker-nav.active { background: var(--accent-soft); color: var(--accent-ink); font-weight: 600; }
  .picker-nav.nested { padding-left: 17px; }
  .picker-group { margin-top: 8px; padding: 4px 8px; color: var(--faint); font-size: 11px; font-weight: 650; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .picker-main { display: flex; min-width: 0; flex: 1; flex-direction: column; }
  .picker-results-heading { display: flex; align-items: baseline; gap: 8px; padding: 11px 14px 3px; font-size: 13px; font-weight: 600; }
  .picker-results-heading .ui-faint { font-size: 11px; font-weight: 400; }
  .picker-grid-scroll { flex: 1; min-height: 0; overflow: auto; padding: 8px 13px 13px; }
  .picker-grid { display: grid; grid-template-columns: repeat(auto-fill,minmax(74px,1fr)); gap: 8px; align-content: start; }
  .picker-cell { border: 1px solid var(--border); border-radius: 10px; background: var(--surface-soft); overflow: hidden; transition: border-color 100ms, box-shadow 100ms; }
  .picker-cell:hover, .picker-cell.active { border-color: var(--accent); box-shadow: 0 0 0 2px var(--accent-soft); }
  .picker-error { padding: 7px 13px; background: var(--danger-soft); color: var(--danger); font-size: 12px; }
</style>
