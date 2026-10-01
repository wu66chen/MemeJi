<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, tick } from "svelte";
  import { open, ask } from "@tauri-apps/plugin-dialog";
  import MemeThumb from "$lib/components/MemeThumb.svelte";
  import CollectionSidebar from "$lib/components/CollectionSidebar.svelte";
  import DetailPanel from "$lib/components/DetailPanel.svelte";
  import OnboardingWizard from "$lib/components/OnboardingWizard.svelte";
  import SettingsPanel from "$lib/components/SettingsPanel.svelte";
  import CollectionTransferDialog from "$lib/components/CollectionTransferDialog.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { selectIds } from "$lib/selection";
  import { checkForUpdatesOnStartup } from "$lib/updater";

  interface Meme {
    id: number;
    internal_path: string;
    original_filename: string;
    mime_type: string;
    extension: string;
    width: number;
    height: number;
    file_size: number;
    content_hash: string;
    description: string;
    is_favorite: boolean;
    last_used_at: number | null;
    tags: string[];
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
  interface Tag {
    id: number;
    name: string;
  }
  interface ImportResult {
    imported: number;
    skipped: number;
    unsupported: number;
    failed: number;
  }
  type View =
    | { kind: "all" }
    | { kind: "favorites" }
    | { kind: "recent" }
    | { kind: "collection"; id: number };
  type QueryView = View | { kind: "collections"; id: number[] };

  let memes = $state<Meme[]>([]);
  let collections = $state<Collection[]>([]);
  let groups = $state<CollectionGroup[]>([]);
  let allTags = $state<Tag[]>([]);
  let activeView = $state<View>({ kind: "all" });
  let selectedIds = $state<number[]>([]);
  let anchorId = $state<number | null>(null);
  let searchScopeIds = $state<number[]>([]);
  let scopeOpen = $state(false);
  let busy = $state(false);
  let loading = $state(false);
  let loadError = $state('');
  let actionMessage = $state('');
  let actionError = $state(false);
  let bulkTag = $state('');
  let tagDialogOpen = $state(false);
  let contextMenu = $state<{ x: number; y: number } | null>(null);
  let menuEl: HTMLDivElement | undefined = $state();
  let marquee = $state<{ x1: number; y1: number; x2: number; y2: number } | null>(null);
  let marqueeBase: number[] = [];
  let marqueeAdd = false;
  let transferMode = $state<'add' | 'move' | null>(null);
  let transferIds = $state<number[]>([]);
  let transferSource = $state<number | null>(null);
  let dragSelection = $state<{ ids: number[]; sourceId: number | null } | null>(null);
  let query = $state("");
  let onboarded = $state<boolean | null>(null);
  let showSettings = $state(false);
  let detailsOpen = $state(true);
  $effect(() => {
    if (!actionMessage || actionError) return;
    const message = actionMessage;
    const timer = setTimeout(() => { if (actionMessage === message) actionMessage = ''; }, 4500);
    return () => clearTimeout(timer);
  });

  const selected = $derived(selectedIds.length === 1 ? memes.find((m) => m.id === selectedIds[0]) ?? null : null);
  const selectedSet = $derived(new Set(selectedIds));
  const allSelectedFavorite = $derived(selectedIds.length > 0 && selectedIds.every(id => memes.find(m => m.id === id)?.is_favorite));
  const sourceId = $derived(searchScopeIds.length === 1 ? searchScopeIds[0] : searchScopeIds.length > 1 ? null : activeView.kind === 'collection' && !query.trim() ? activeView.id : null);
  const scopeLabel = $derived(searchScopeIds.length === 0 ? '全局' : searchScopeIds.length === 1 ? collections.find(c => c.id === searchScopeIds[0])?.name ?? '收藏夹' : `${searchScopeIds.length} 个收藏夹`);
  const viewTitle = $derived.by(() => {
    const view = activeView;
    if (searchScopeIds.length) return scopeLabel;
    if (query.trim()) return '全库搜索';
    if (view.kind === 'collection') return collections.find(c => c.id === view.id)?.name ?? '收藏夹';
    return { all: '全部', favorites: '收藏', recent: '最近使用' }[view.kind];
  });

  const emptyHints: Record<View["kind"], string> = {
    all: "导入常用表情，随时搜索和发送。",
    favorites: "常用表情会出现在这里。",
    recent: "用过的表情会按时间出现在这里。",
    collection: "把图片导入这个收藏夹，或者从其他位置拖进来。",
  };

  let memesRequestId = 0;
  async function refresh() {
    // 范围：默认在当前视图内搜索；勾选「全局」后跨全库（仅在有查询词时生效）
    const requestId = ++memesRequestId;
    const q = query.trim();
    const view: QueryView = searchScopeIds.length ? { kind: 'collections', id: searchScopeIds } : q ? { kind: 'all' } : activeView;
    loading = true;
    loadError = '';
    try {
      const [result, cols, grps, tags] = await Promise.all([
        invoke<Meme[]>("list_memes", { view, query: q || undefined }),
        invoke<Collection[]>("list_collections"), invoke<CollectionGroup[]>("list_collection_groups"), invoke<Tag[]>("list_tags")
      ]);
      if (requestId !== memesRequestId) return;
      memes = result; collections = cols; groups = grps; allTags = tags;
      const visible = new Set(result.map(m => m.id));
      selectedIds = selectedIds.filter(id => visible.has(id));
      if (anchorId !== null && !visible.has(anchorId)) anchorId = null;
    } catch (e) {
      if (requestId === memesRequestId) loadError = `加载失败：${String(e)}`;
    } finally { if (requestId === memesRequestId) loading = false; }
  }

  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  function onSearchInput() {
    clearTimeout(searchTimer);
    clearSelection();
    memes = [];
    ++memesRequestId;
    loading = true;
    searchTimer = setTimeout(() => void refresh(), 150);
  }

  function clearSelection() { selectedIds = []; anchorId = null; dragSelection = null; contextMenu = null; }
  function pick(id: number, event: MouseEvent, checkbox = false) {
    if (busy || loading || loadError) return;
    const result = selectIds(memes.map(m => m.id), selectedIds, anchorId, id,
      { range: event.shiftKey, toggle: checkbox || event.ctrlKey || event.metaKey });
    selectedIds = result.ids; anchorId = result.anchor;
  }
  function selectAll() {
    if (busy || loading || loadError) return;
    selectedIds = memes.map(m => m.id); anchorId = selectedIds[0] ?? null;
  }
  function showTagDialog(node: HTMLDialogElement) {
    node.showModal();
  }
  function onKeydown(event: KeyboardEvent) {
    if (event.isComposing || busy || showSettings || onboarded === false || transferMode || tagDialogOpen) return;
    if (contextMenu && event.key === 'Escape') { event.preventDefault(); contextMenu = null; return; }
    const target = event.target as HTMLElement;
    if (target.closest('input:not([type="checkbox"]):not([type="radio"]), textarea, select, [contenteditable]:not([contenteditable="false"]), dialog')) return;
    if ((event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey)) && selectedIds.length) {
      event.preventDefault();
      const card = document.querySelector<HTMLElement>(`[data-meme-card="${selectedIds.at(-1)}"]`);
      const rect = card?.getBoundingClientRect();
      openContextMenu(new MouseEvent('contextmenu', { clientX: rect?.left ?? 8, clientY: rect?.bottom ?? 8 }));
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'a') { event.preventDefault(); selectAll(); }
    if (event.key === 'Escape') { clearSelection(); scopeOpen = false; }
    if (event.key === 'Delete' && selectedIds.length) { event.preventDefault(); void deleteSelected(); }
  }

  function toggleSearchScope(id: number) {
    searchScopeIds = searchScopeIds.includes(id) ? searchScopeIds.filter(current => current !== id) : [...searchScopeIds, id];
    onSearchInput();
  }
  function clearSearchScopes() {
    searchScopeIds = [];
    activeView = { kind: 'all' };
    onSearchInput();
  }
  function openContextMenu(event: MouseEvent, id?: number) {
    event.preventDefault();
    event.stopPropagation();
    if (busy || loading || loadError) return;
    if (id !== undefined && !selectedIds.includes(id)) {
      selectedIds = [id];
      anchorId = id;
    }
    if (!selectedIds.length) return;
    scopeOpen = false;
    contextMenu = {
      x: Math.max(8, Math.min(event.clientX, window.innerWidth - 232)),
      y: Math.max(8, Math.min(event.clientY, window.innerHeight - 310)),
    };
    void tick().then(() => menuEl?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus());
  }
  function closeContextMenu() { contextMenu = null; }
  function menuCommand(command: () => void) { closeContextMenu(); command(); }
  function onMenuKeydown(event: KeyboardEvent) {
    const menu = event.currentTarget as HTMLElement;
    const items = [...menu.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
    const index = items.indexOf(document.activeElement as HTMLButtonElement);
    if (event.key === 'Escape') { event.preventDefault(); closeContextMenu(); return; }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      items[(index + (event.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length]?.focus();
    }
  }
  function startMarquee(event: PointerEvent) {
    if (event.button !== 0 || busy || loading || loadError || (event.target as Element).closest('[data-meme-card]')) return;
    const container = event.currentTarget as HTMLElement;
    container.setPointerCapture(event.pointerId);
    marquee = { x1: event.clientX, y1: event.clientY, x2: event.clientX, y2: event.clientY };
    marqueeBase = [...selectedIds];
    marqueeAdd = event.ctrlKey || event.metaKey;
    contextMenu = null;
    event.preventDefault();
  }
  function moveMarquee(event: PointerEvent) {
    if (!marquee) return;
    marquee = { ...marquee, x2: event.clientX, y2: event.clientY };
    const left = Math.min(marquee.x1, marquee.x2);
    const right = Math.max(marquee.x1, marquee.x2);
    const top = Math.min(marquee.y1, marquee.y2);
    const bottom = Math.max(marquee.y1, marquee.y2);
    if (right - left < 4 && bottom - top < 4) return;
    const hits = [...document.querySelectorAll<HTMLElement>('[data-meme-card]')]
      .filter(card => { const rect = card.getBoundingClientRect(); return rect.left < right && rect.right > left && rect.top < bottom && rect.bottom > top; })
      .map(card => Number(card.dataset.memeCard));
    selectedIds = marqueeAdd ? [...new Set([...marqueeBase, ...hits])] : hits;
    anchorId = selectedIds[0] ?? null;
  }
  function endMarquee(event: PointerEvent) {
    if (!marquee) return;
    const distance = Math.hypot(event.clientX - marquee.x1, event.clientY - marquee.y1);
    marquee = null;
    if (distance < 4 && !marqueeAdd) clearSelection();
  }

  type BatchAction =
    | { kind: 'favorite'; favorite: boolean }
    | { kind: 'add_tag'; name: string }
    | { kind: 'add_to_collection' | 'remove_from_collection'; collection_id: number }
    | { kind: 'move_to_collection'; source_id: number; target_id: number };
  async function batchEdit(action: BatchAction, label: string, ids = [...selectedIds]): Promise<boolean> {
    if (!ids.length || busy || loading || loadError) return false;
    busy = true; actionMessage = ''; actionError = false;
    try {
      const count = await invoke<number>('batch_edit_memes', { memeIds: ids, action });
      actionMessage = `${label} · ${count} 张`;
      await refresh();
      return true;
    } catch (e) { actionMessage = `操作失败：${String(e)}`; actionError = true; return false; }
    finally { busy = false; }
  }
  function openTransfer(mode: 'add' | 'move') {
    if (!selectedIds.length || busy || loading || loadError || (mode === 'move' && sourceId === null)) return;
    transferIds = [...selectedIds]; transferSource = sourceId; transferMode = mode;
    actionMessage = ''; actionError = false;
  }
  async function chooseTarget(id: number) {
    const moving = transferMode === 'move' && transferSource !== null;
    const action: BatchAction = moving
      ? { kind:'move_to_collection', source_id:transferSource!, target_id:id }
      : { kind:'add_to_collection', collection_id:id };
    if (await batchEdit(action, moving ? '已移动' : '已加入收藏夹', transferIds)) transferMode = null;
  }
  function startMemeDrag(event: DragEvent, id: number) {
    if (busy || loading || loadError) { event.preventDefault(); return; }
    const ids = selectedIds.includes(id) ? [...selectedIds] : [id];
    selectedIds = ids; anchorId = id;
    dragSelection = { ids, sourceId };
    event.dataTransfer?.setData('application/x-memeji-memes', 'selection');
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'copyMove';
  }
  async function dropMemes(targetId: number, copy: boolean) {
    const selection = dragSelection; dragSelection = null;
    if (!selection) return;
    if (selection.sourceId === targetId) { actionMessage = '这些表情已经在该收藏夹中'; actionError = false; return; }
    const moving = selection.sourceId !== null && !copy;
    await batchEdit(moving
      ? { kind:'move_to_collection', source_id:selection.sourceId!, target_id:targetId }
      : { kind:'add_to_collection', collection_id:targetId }, moving ? '已移动' : '已加入收藏夹', selection.ids);
  }
  async function addBulkTag() {
    const name = bulkTag.trim();
    if (name && await batchEdit({ kind:'add_tag', name }, '已添加标签')) { bulkTag = ''; tagDialogOpen = false; }
  }

  function showResult(r: ImportResult) {
    actionMessage = `导入 ${r.imported} 张 · 已有 ${r.skipped} 张 · 不支持 ${r.unsupported} 张 · 失败 ${r.failed} 张`;
    actionError = r.failed > 0;
  }

  async function importFiles() {
    if (busy) return;
    const targetCollectionId = activeView.kind === 'collection' ? activeView.id : null;
    const picked = await open({ multiple: true });
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked : [picked];
    busy = true;
    try {
      showResult(await invoke<ImportResult>("import_paths", { paths, targetCollectionId }));
      await refresh();
    } catch (e) { actionMessage = `导入失败：${String(e)}`; actionError = true; }
    finally { busy = false; }
  }

  async function importFolder() {
    if (busy) return;
    const targetCollectionId = activeView.kind === 'collection' ? activeView.id : null;
    const picked = await open({ directory: true });
    if (!picked) return;
    busy = true;
    try {
      showResult(await invoke<ImportResult>("import_paths", { paths: [picked], targetCollectionId }));
      await refresh();
    } catch (e) { actionMessage = `导入失败：${String(e)}`; actionError = true; }
    finally { busy = false; }
  }

  function switchView(view: View) {
    if (busy) return;
    clearTimeout(searchTimer);
    activeView = view;
    query = '';
    searchScopeIds = view.kind === 'collection' ? [view.id] : [];
    scopeOpen = false;
    clearSelection();
    memes = [];
    void refresh();
  }

  // ---- 收藏夹 ----
  async function createCollection(name: string) {
    await invoke("create_collection", { name });
    await refresh();
  }
  async function renameCollection(id: number, name: string) {
    await invoke("rename_collection", { id, name });
    await refresh();
  }
  async function deleteCollection(id: number) {
    const ok = await ask("删除收藏夹不会删除里面的图片，确定删除？", {
      title: "删除收藏夹",
      kind: "warning",
    });
    if (!ok) return;
    await invoke("delete_collection", { id });
    if (searchScopeIds.includes(id)) searchScopeIds = searchScopeIds.filter(scopeId => scopeId !== id);
    if (activeView.kind === "collection" && activeView.id === id) switchView({ kind: "all" });
    else await refresh();
  }
  async function reorderCollections(ids: number[]) {
    await invoke("reorder_collections", { ids });
    await refresh();
  }
  async function createGroup(name: string) {
    await invoke("create_collection_group", { name });
    await refresh();
  }
  async function renameGroup(id: number, name: string) {
    await invoke("rename_collection_group", { id, name });
    await refresh();
  }
  async function deleteGroup(id: number) {
    const ok = await ask("删除分组后，里面的收藏夹会移至未分组，图片不会删除。确定删除？", { title: "删除分组", kind: "warning" });
    if (!ok) return;
    await invoke("delete_collection_group", { id });
    await refresh();
  }
  async function reorderGroups(ids: number[]) {
    await invoke("reorder_collection_groups", { ids });
    await refresh();
  }
  async function moveCollection(id: number, groupId: number | null) {
    await invoke("move_collection_to_group", { id, groupId });
    await refresh();
  }

  // ---- 表情 ----
  async function saveDescription(text: string) {
    if (!selected) return;
    await invoke("set_description", { memeId: selected.id, description: text });
    await refresh();
  }
  async function addTag(name: string) {
    if (!selected) return;
    await invoke("add_tag", { memeId: selected.id, name });
    await refresh();
  }
  async function removeTag(tagId: number) {
    if (!selected) return;
    await invoke("remove_tag", { memeId: selected.id, tagId });
    await refresh();
  }
  let memberOf = $state<Collection[]>([]);
  // 跟随选中项取归属收藏夹；refresh() 后（加入/移出）会因引用变化重新拉取
  $effect(() => {
    const id = selected?.id;
    if (id == null) {
      memberOf = [];
      return;
    }
    invoke<Collection[]>("collections_of_meme", { memeId: id })
      .then((cols) => {
        if (selected?.id === id) memberOf = cols;
      })
      .catch(() => {});
  });
  async function removeFromCollection(collectionId: number) {
    if (!selected) return;
    await invoke("remove_meme_from_collection", { memeId: selected.id, collectionId });
    await refresh();
  }
  async function deleteSelected() {
    const ids = [...selectedIds];
    if (!ids.length || busy || loading || loadError) return;
    busy = true;
    try {
      const ok = await ask(`永久删除选中的 ${ids.length} 张表情？将从所有收藏夹移除并删除库内文件，无法撤销。导入前的原文件不受影响。`, { title:'删除表情', kind:'warning' });
      if (!ok) return;
      const result = await invoke<{ deleted_ids:number[]; failures:{id:number;message:string}[] }>('delete_memes', { memeIds:ids });
      selectedIds = result.failures.map(f => f.id);
      actionError = result.failures.length > 0;
      actionMessage = `已删除 ${result.deleted_ids.length} 张${actionError ? `，失败 ${result.failures.length} 张。${result.failures.slice(0,3).map(f => `${memes.find(m => m.id === f.id)?.original_filename ?? f.id}：${f.message}`).join('；')}` : ''}`;
      await refresh();
    } catch (e) { actionError = true; actionMessage = `删除失败：${String(e)}`; }
    finally { busy = false; }
  }

  refresh();

  onMount(() => {
    void checkForUpdatesOnStartup();
    // 首启向导门 + 托盘「设置」唤起
    void invoke<{ onboarded: boolean }>("get_config")
      .then((cfg) => (onboarded = cfg.onboarded))
      .catch(() => (onboarded = true));
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen("open-settings", () => { if (!busy) showSettings = true; }).then(u => { if (disposed) u(); else unlisten = u; });
    return () => { disposed = true; unlisten?.(); clearTimeout(searchTimer); };
  });
</script>

<svelte:window onkeydown={onKeydown} onclick={(e) => {
  const target = e.target as Element;
  if (contextMenu && !target.closest('[data-meme-menu]')) closeContextMenu();
  if (scopeOpen && !target.closest('[data-search-scope]')) scopeOpen = false;
}} />

<main class="app-shell">
  <header class="app-toolbar">
    <div class="app-search">
      <Icon name="search" size={17} />
      <input bind:value={query} disabled={busy} oninput={onSearchInput}
        aria-label="搜索表情" placeholder="搜索文件名、标签或描述" />
      {#if query}<button type="button" class="icon-button small" aria-label="清空搜索" onclick={() => { query=''; onSearchInput(); }}><Icon name="x" size={14} /></button>{/if}
    </div>
    <div data-search-scope class="scope-wrap">
      <button type="button" class="ui-button scope-trigger" aria-expanded={scopeOpen} aria-label={`搜索范围：${scopeLabel}`}
        onclick={() => (scopeOpen = !scopeOpen)}><span class="truncate">{scopeLabel}</span><Icon name="chevron-down" size={14} /></button>
      {#if scopeOpen}
        <div class="scope-popover ui-menu">
          <div class="scope-heading"><span>搜索范围</span><button type="button" onclick={clearSearchScopes}>清除所有</button></div>
          {#each collections as c (c.id)}
            <label class="scope-option"><input class="ui-checkbox" type="checkbox" checked={searchScopeIds.includes(c.id)} onchange={() => toggleSearchScope(c.id)} /><span class="truncate" title={c.name}>{c.name}</span></label>
          {:else}<p class="scope-empty">还没有收藏夹</p>{/each}
        </div>
      {/if}
    </div>
    <div class="toolbar-divider"></div>
    <button class="ui-button primary import-button" disabled={busy} onclick={importFiles}><Icon name="upload" size={16} /><span>导入图片</span></button>
    <button class="ui-button folder-import" disabled={busy} onclick={importFolder} title="导入文件夹"><Icon name="folder-plus" size={17} /><span>导入文件夹</span></button>
    <button class="icon-button settings-button" aria-label="设置" title="设置" onclick={() => (showSettings = true)}><Icon name="settings" size={18} /></button>
  </header>
  <div class="flex flex-1 overflow-hidden">
    <CollectionSidebar
      {collections}
      {groups}
      activeView={activeView}
      onSelect={switchView}
      onCreate={createCollection}
      onRename={renameCollection}
      onDelete={deleteCollection}
      onReorder={reorderCollections}
      onCreateGroup={createGroup}
      onRenameGroup={renameGroup}
      onDeleteGroup={deleteGroup}
      onReorderGroups={reorderGroups}
      onMoveCollection={moveCollection}
      disabled={busy}
      draggingMemes={dragSelection !== null}
      copyDrop={dragSelection?.sourceId === null}
      onDropMemes={dropMemes}
    />
    <section class="flex min-w-0 flex-1 flex-col overflow-hidden" aria-label="图库管理">
      <div class="gallery-heading" aria-busy={busy}>
        <h1>{viewTitle}</h1><span class="gallery-count">{memes.length} 张</span>
        <span class="flex-1"></span>
        {#if selectedIds.length}
          <span class="selection-count">已选 {selectedIds.length} 张</span>
          <button class="ui-button small selected-action" onclick={() => openTransfer('add')}><Icon name="folder-plus" size={15} />加入收藏夹</button>
          <button class="ui-button small selected-action" onclick={() => menuCommand(() => { tagDialogOpen = true; })}><Icon name="tag" size={15} />添加标签</button>
          <button class="icon-button" aria-label="更多所选表情操作" title="更多操作" onclick={(e) => openContextMenu(e)}><Icon name="more" size={17} /></button>
          <button class="icon-button" aria-label="清空选择" title="清空选择" onclick={clearSelection}><Icon name="x" size={16} /></button>
        {/if}
        {#if selectedIds.length === 1}
          <button class={`icon-button ${detailsOpen ? 'details-active' : ''}`} aria-label={detailsOpen ? '隐藏详情' : '显示详情'} title={detailsOpen ? '隐藏详情' : '显示详情'} onclick={() => (detailsOpen = !detailsOpen)}><Icon name="panel" size={17} /></button>
        {/if}
        {#if busy}<span class="ui-muted" role="status">正在处理…</span>{/if}
      </div>
      {#if loadError}<div role="alert" class="gallery-error"><Icon name="alert" size={17} />{loadError} <button class="ui-button small" onclick={() => void refresh()}>重试</button></div>{/if}
      <div role="region" aria-label="图片选择区域" class="gallery-scroll" onpointerdown={startMarquee} onpointermove={moveMarquee} onpointerup={endMarquee} onpointercancel={endMarquee}>
      {#if memes.length === 0}
        <div class="ui-empty">
          <div class="ui-empty-mark"><Icon name={query.trim() ? 'search' : 'image'} size={30} /></div>
          <h2>{loading ? '正在加载' : loadError ? '图库暂时无法加载' : query.trim() ? '没有找到表情' : activeView.kind === 'all' ? '从第一张表情开始' : activeView.kind === 'collection' ? '这个收藏夹还没有图片' : activeView.kind === 'favorites' ? '还没有收藏的表情' : '还没有使用记录'}</h2>
          {#if !loading && !loadError}<p>{query.trim() ? '换个关键词，或清除搜索范围再试。' : emptyHints[activeView.kind]}</p>{/if}
          {#if !loading && !loadError && !query.trim() && (activeView.kind === 'all' || activeView.kind === 'collection')}
            <button class="ui-button primary" onclick={importFiles}><Icon name="upload" size={16} />导入图片</button>
          {/if}
        </div>
      {:else}
        <div class="gallery-grid">
          {#each memes as m (m.id)}
            <div role="presentation" class="group relative min-w-0" data-meme-card={m.id} oncontextmenu={(e) => openContextMenu(e, m.id)}>
            <button
              type="button"
              disabled={busy || loading || !!loadError}
              draggable={!busy && !loading && !loadError}
              ondragstart={(e) => startMemeDrag(e, m.id)}
              ondragend={() => (dragSelection = null)}
              aria-label={m.original_filename}
              aria-pressed={selectedSet.has(m.id)}
              class={`gallery-card ${selectedSet.has(m.id) ? 'is-selected' : ''}`}
              onclick={(e) => pick(m.id, e)}
            >
              <MemeThumb
                contentHash={m.content_hash}
                thumbnailPath={m.thumbnail_path}
                internalPath={m.internal_path}
                hoverPlay
              />
              <span class="gallery-filename" title={m.original_filename}>{m.original_filename}</span>
            </button>
            <input type="checkbox" aria-label={`选择 ${m.original_filename}`} checked={selectedSet.has(m.id)} disabled={busy || loading || !!loadError}
              class={`gallery-checkbox ui-checkbox ${selectedSet.has(m.id) ? 'is-visible' : ''}`}
              onclick={(e) => pick(m.id, e, true)} />
            </div>
          {/each}
        </div>
      {/if}
      </div>
    </section>
    {#if selected && detailsOpen}
    <DetailPanel
      disabled={busy}
      meme={selected}
      {allTags}
      {memberOf}
      onSaveDescription={saveDescription}
      onAddTag={addTag}
      onRemoveTag={removeTag}
      onRemoveFromCollection={removeFromCollection}
      onClose={() => (detailsOpen = false)}
    />
    {/if}
  </div>

  {#if onboarded === false}
    <OnboardingWizard onFinish={() => { onboarded = true; void refresh(); }} />
  {/if}
  {#if showSettings}
    <SettingsPanel onClose={() => (showSettings = false)} onMemesChanged={() => void refresh()} />
  {/if}
  {#if transferMode}
    <CollectionTransferDialog {collections} {groups} mode={transferMode} count={transferIds.length} sourceId={transferSource} {busy}
      error={actionError ? actionMessage : ''} onChoose={(id) => void chooseTarget(id)} onClose={() => { if (!busy) transferMode = null; }} />
  {/if}
  {#if contextMenu && selectedIds.length}
    <div bind:this={menuEl} data-meme-menu role="menu" aria-label={`所选 ${selectedIds.length} 张表情的操作`} tabindex="-1"
      onkeydown={onMenuKeydown}
      class="ui-menu fixed z-40 w-56"
      style={`left:${contextMenu.x}px;top:${contextMenu.y}px`}
    >
      <p class="menu-caption">已选 {selectedIds.length} 张</p>
      <button role="menuitem" class="ui-menu-item"
        onclick={() => menuCommand(() => void batchEdit({kind:'favorite', favorite:!allSelectedFavorite}, allSelectedFavorite ? '已取消收藏' : '已收藏'))}>
        <Icon name="heart" size={16} />{allSelectedFavorite ? '取消收藏' : '收藏'}
      </button>
      <div role="separator" class="ui-menu-separator"></div>
      <button role="menuitem" class="ui-menu-item"
        onclick={() => menuCommand(() => openTransfer('add'))}><Icon name="folder-plus" size={16} />加入收藏夹…</button>
      <button role="menuitem" disabled={sourceId === null} class="ui-menu-item"
        onclick={() => menuCommand(() => openTransfer('move'))}><Icon name="move" size={16} />移动到…</button>
      {#if sourceId !== null}
        <button role="menuitem" class="ui-menu-item"
          onclick={() => menuCommand(() => void batchEdit({kind:'remove_from_collection',collection_id:sourceId!},'已移出收藏夹'))}><Icon name="folder" size={16} />从当前收藏夹移出</button>
      {/if}
      <div role="separator" class="ui-menu-separator"></div>
      <button role="menuitem" class="ui-menu-item"
        onclick={() => menuCommand(() => { tagDialogOpen = true; })}><Icon name="tag" size={16} />添加标签…</button>
      <div role="separator" class="ui-menu-separator"></div>
      <button role="menuitem" class="ui-menu-item danger"
        onclick={() => menuCommand(() => void deleteSelected())}><Icon name="trash" size={16} />删除…</button>
    </div>
  {/if}
  {#if tagDialogOpen}
    <dialog use:showTagDialog onclose={() => (tagDialogOpen = false)} aria-label="为所选表情添加标签" class="ui-dialog w-full max-w-sm p-0">
      <form onsubmit={(e) => { e.preventDefault(); void addBulkTag(); }}>
        <div class="ui-dialog-header"><h2>添加标签</h2><button type="button" class="icon-button round" aria-label="关闭" onclick={() => (tagDialogOpen=false)}><Icon name="x" size={16}/></button></div>
        <div class="ui-dialog-body"><label class="ui-label mb-2" for="bulk-tag-input">为 {selectedIds.length} 张表情添加标签</label>
        <input id="bulk-tag-input" class="ui-input"
          aria-label="标签名称" list="bulk-existing-tags" bind:value={bulkTag} placeholder="输入标签名称" />
        <datalist id="bulk-existing-tags">{#each allTags as tag (tag.id)}<option value={tag.name}></option>{/each}</datalist>
        </div><div class="ui-dialog-footer"><button type="button" class="ui-button" disabled={busy} onclick={() => (tagDialogOpen = false)}>取消</button><button type="submit" class="ui-button primary" disabled={busy || !bulkTag.trim()}>添加</button></div>
      </form>
    </dialog>
  {/if}
  {#if actionMessage}
    <div role={actionError ? 'alert' : 'status'} class={`app-toast ${actionError ? 'is-error' : ''}`}>
      <span class="break-words">{actionMessage}</span>
      <button type="button" aria-label="关闭提示" class="icon-button small" onclick={() => (actionMessage = '')}><Icon name="x" size={15}/></button>
    </div>
  {/if}
  {#if marquee && (Math.abs(marquee.x2 - marquee.x1) > 3 || Math.abs(marquee.y2 - marquee.y1) > 3)}
    <div class="marquee-rect pointer-events-none fixed z-30"
      style={`left:${Math.min(marquee.x1,marquee.x2)}px;top:${Math.min(marquee.y1,marquee.y2)}px;width:${Math.abs(marquee.x2-marquee.x1)}px;height:${Math.abs(marquee.y2-marquee.y1)}px`}></div>
  {/if}
</main>
