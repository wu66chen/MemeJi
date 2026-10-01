<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { open, ask } from "@tauri-apps/plugin-dialog";
  import MemeThumb from "$lib/components/MemeThumb.svelte";
  import CollectionSidebar from "$lib/components/CollectionSidebar.svelte";
  import DetailPanel from "$lib/components/DetailPanel.svelte";
  import OnboardingWizard from "$lib/components/OnboardingWizard.svelte";
  import SettingsPanel from "$lib/components/SettingsPanel.svelte";
  import CollectionTransferDialog from "$lib/components/CollectionTransferDialog.svelte";
  import { selectIds, sourceCollectionId } from "$lib/selection";
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

  let memes = $state<Meme[]>([]);
  let collections = $state<Collection[]>([]);
  let groups = $state<CollectionGroup[]>([]);
  let allTags = $state<Tag[]>([]);
  let activeView = $state<View>({ kind: "all" });
  let selectedIds = $state<number[]>([]);
  let anchorId = $state<number | null>(null);
  let multiSelect = $state(false);
  let busy = $state(false);
  let loading = $state(false);
  let loadError = $state('');
  let actionMessage = $state('');
  let actionError = $state(false);
  let bulkTag = $state('');
  let transferMode = $state<'add' | 'move' | null>(null);
  let transferIds = $state<number[]>([]);
  let transferSource = $state<number | null>(null);
  let dragSelection = $state<{ ids: number[]; sourceId: number | null } | null>(null);
  const actionClass = 'rounded border border-neutral-300 px-2 py-1 text-xs hover:bg-neutral-200 disabled:opacity-40 disabled:cursor-not-allowed dark:border-neutral-600 dark:hover:bg-neutral-700';
  let importSummary = $state("");
  let query = $state("");
  let globalSearch = $state(false);
  let onboarded = $state<boolean | null>(null);
  let showSettings = $state(false);

  const selected = $derived(selectedIds.length === 1 ? memes.find((m) => m.id === selectedIds[0]) ?? null : null);
  const selectedSet = $derived(new Set(selectedIds));
  const sourceId = $derived(sourceCollectionId(activeView, query, globalSearch));
  const viewTitle = $derived.by(() => {
    const view = activeView;
    if (query.trim() && globalSearch) return '全库搜索';
    if (view.kind === 'collection') return collections.find(c => c.id === view.id)?.name ?? '收藏夹';
    return { all: '全部', favorites: '收藏', recent: '最近使用' }[view.kind];
  });

  const emptyHints: Record<View["kind"], string> = {
    all: "还没有表情。点击「导入图片」或「导入文件夹」开始整理 — 按 Ctrl+Shift+Space 呼出 Quick Picker",
    favorites: "还没有收藏。在详情里点「收藏」，常用的图会出现在这里",
    recent: "还没有使用记录。复制过的表情会按最近使用排在前面",
    collection: "这个收藏夹还是空的",
  };

  let memesRequestId = 0;
  async function refresh() {
    // 范围：默认在当前视图内搜索；勾选「全局」后跨全库（仅在有查询词时生效）
    const requestId = ++memesRequestId;
    const q = query.trim();
    const view: View = q && globalSearch ? { kind: "all" } : activeView;
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

  function clearSelection() { selectedIds = []; anchorId = null; dragSelection = null; }
  function pick(id: number, event: MouseEvent, checkbox = false) {
    if (busy || loading || loadError) return;
    const result = selectIds(memes.map(m => m.id), selectedIds, anchorId, id,
      { range: event.shiftKey, toggle: checkbox || event.ctrlKey || event.metaKey || (multiSelect && !event.shiftKey) });
    selectedIds = result.ids; anchorId = result.anchor;
  }
  function selectAll() {
    if (busy || loading || loadError) return;
    selectedIds = memes.map(m => m.id); anchorId = selectedIds[0] ?? null; multiSelect = true;
  }
  function onKeydown(event: KeyboardEvent) {
    if (event.isComposing || busy || showSettings || onboarded === false || transferMode) return;
    const target = event.target as HTMLElement;
    if (target.closest('input:not([type="checkbox"]):not([type="radio"]), textarea, select, [contenteditable]:not([contenteditable="false"]), dialog')) return;
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'a') { event.preventDefault(); selectAll(); }
    if (event.key === 'Escape') { clearSelection(); multiSelect = false; }
    if (event.key === 'Delete' && selectedIds.length) { event.preventDefault(); void deleteSelected(); }
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
    if (name && await batchEdit({ kind:'add_tag', name }, '已添加标签')) bulkTag = '';
  }

  function showResult(r: ImportResult) {
    importSummary = `导入 ${r.imported} · 重复 ${r.skipped} · 不支持 ${r.unsupported} · 失败 ${r.failed}`;
  }

  async function importFiles() {
    const picked = await open({ multiple: true });
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked : [picked];
    showResult(await invoke<ImportResult>("import_paths", { paths }));
    await refresh();
  }

  async function importFolder() {
    const picked = await open({ directory: true });
    if (!picked) return;
    showResult(await invoke<ImportResult>("import_paths", { paths: [picked] }));
    await refresh();
  }

  function switchView(view: View) {
    if (busy) return;
    clearTimeout(searchTimer);
    activeView = view;
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
  async function toggleFavorite() {
    if (!selected) return;
    await batchEdit({ kind:'favorite', favorite:!selected.is_favorite }, selected.is_favorite ? '已取消收藏' : '已收藏');
  }
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
  async function addToCollection(collectionId: number) {
    if (!selected) return;
    await invoke("add_meme_to_collection", { memeId: selected.id, collectionId });
    await refresh();
  }
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

<svelte:window onkeydown={onKeydown} />

<main class="flex h-screen flex-col bg-neutral-50 text-neutral-900 dark:bg-neutral-900 dark:text-neutral-100">
  <header class="flex items-center justify-between border-b border-neutral-200 px-4 py-2 dark:border-neutral-800">
    <h1 class="text-sm font-semibold">表情包管理器</h1>
    <div class="flex items-center gap-2" inert={busy}>
      {#if importSummary}
        <span class="text-xs text-neutral-500">{importSummary}</span>
      {/if}
      <button class="rounded bg-neutral-800 px-3 py-1 text-xs text-white hover:bg-neutral-700 dark:bg-neutral-700 dark:hover:bg-neutral-600" onclick={importFiles}>
        导入图片
      </button>
      <button class="rounded bg-neutral-800 px-3 py-1 text-xs text-white hover:bg-neutral-700 dark:bg-neutral-700 dark:hover:bg-neutral-600" onclick={importFolder}>
        导入文件夹
      </button>
      <button class="rounded border border-neutral-300 px-3 py-1 text-xs text-neutral-600 hover:bg-neutral-100 dark:border-neutral-600 dark:text-neutral-300 dark:hover:bg-neutral-800" onclick={() => (showSettings = true)}>
        设置
      </button>
    </div>
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
      <div class="flex items-center gap-2 border-b border-neutral-200 px-3 py-2 dark:border-neutral-800">
        <input
          bind:value={query}
          disabled={busy}
          oninput={onSearchInput}
          placeholder="搜索文件名、标签、描述…（空格分隔多关键词）"
          class="min-w-0 flex-1 rounded bg-neutral-200/70 px-3 py-1.5 text-sm outline-none placeholder:text-neutral-400 focus:ring-1 focus:ring-neutral-400 dark:bg-neutral-800 dark:focus:ring-neutral-600"
        />
        {#if activeView.kind !== "all"}
          <label
            class="flex shrink-0 cursor-pointer items-center gap-1 text-xs text-neutral-500"
            title="开启后跨全库搜索，关闭则只在当前视图内搜索"
          >
            <input type="checkbox" disabled={busy} bind:checked={globalSearch} onchange={onSearchInput} />
            全局
          </label>
        {/if}
      </div>
      <div class="space-y-2 border-b border-neutral-200 p-3 dark:border-neutral-800" aria-busy={busy}>
        <div class="flex flex-wrap items-center gap-2">
          <span class="mr-auto text-sm font-medium">{viewTitle} · {memes.length} 张{selectedIds.length ? ` / 已选 ${selectedIds.length} 张` : ''}</span>
          <button class={actionClass} disabled={busy} aria-pressed={multiSelect} onclick={() => { multiSelect = !multiSelect; clearSelection(); }}>{multiSelect ? '完成多选' : '多选'}</button>
          <button class={actionClass} disabled={busy || loading || !!loadError || !memes.length} onclick={selectAll}>全选当前结果</button>
          {#if selectedIds.length}<button class={actionClass} disabled={busy} onclick={clearSelection}>清空选择</button>{/if}
        </div>
        {#if selectedIds.length}
          <div class="flex flex-wrap items-center gap-2" role="group" aria-label="选中表情的操作">
            <button class={actionClass} disabled={busy || loading || !!loadError} onclick={() => openTransfer('add')}>加入收藏夹…</button>
            <button class={actionClass} disabled={busy || loading || !!loadError || sourceId === null} title={sourceId === null ? '进入一个收藏夹后，可从该收藏夹移动；当前可使用加入收藏夹' : '保留其他收藏夹归属'} onclick={() => openTransfer('move')}>移动到…</button>
            {#if sourceId !== null}<button class={actionClass} disabled={busy || loading || !!loadError} onclick={() => void batchEdit({ kind:'remove_from_collection',collection_id:sourceId! }, '已从当前收藏夹移出')}>移出当前收藏夹</button>{/if}
            <button class={actionClass} disabled={busy || loading || !!loadError} onclick={() => void batchEdit({kind:'favorite',favorite:true},'已收藏')}>收藏</button>
            <button class={actionClass} disabled={busy || loading || !!loadError} onclick={() => void batchEdit({kind:'favorite',favorite:false},'已取消收藏')}>取消收藏</button>
            <button class={`${actionClass} text-red-600 dark:text-red-400`} disabled={busy || loading || !!loadError} onclick={() => void deleteSelected()}>删除…</button>
            <form class="flex items-center gap-1" onsubmit={(e) => { e.preventDefault(); void addBulkTag(); }}>
              <input class="w-28 rounded border border-neutral-300 bg-transparent px-2 py-1 text-xs dark:border-neutral-600" aria-label="批量标签" placeholder="添加标签" bind:value={bulkTag} disabled={busy} />
              <button class={actionClass} disabled={busy || loading || !!loadError || !bulkTag.trim()}>添加标签</button>
            </form>
          </div>
        {/if}
        <p class="text-[11px] text-neutral-500 dark:text-neutral-400">Ctrl 点选 · Shift 连选 · Ctrl+A 全选当前结果 · 拖到收藏夹{sourceId !== null ? '移动，按住 Ctrl 拖放则加入并保留来源' : '加入'} · Esc 清空</p>
        {#if busy}<p class="text-xs" role="status">正在处理，请稍候…</p>{/if}
        {#if actionMessage}<p role={actionError ? 'alert' : 'status'} class={`break-words text-xs ${actionError ? 'text-red-600 dark:text-red-400' : 'text-green-700 dark:text-green-400'}`}>{actionMessage}</p>{/if}
        {#if loadError}<div role="alert" class="text-xs text-red-600 dark:text-red-400">{loadError} <button class="underline" onclick={() => void refresh()}>重试</button></div>{/if}
      </div>
      <div class="flex-1 overflow-y-auto p-3">
      {#if memes.length === 0}
        <div class="grid h-full place-items-center text-sm text-neutral-400">
          {loading ? '正在加载…' : loadError ? '图库暂时无法加载，请重试' : query.trim() ? "没有匹配的表情" : emptyHints[activeView.kind]}
        </div>
      {:else}
        <div class="grid grid-cols-[repeat(auto-fill,minmax(96px,1fr))] gap-2">
          {#each memes as m (m.id)}
            <div class="group relative min-w-0">
            <button
              type="button"
              disabled={busy || loading || !!loadError}
              draggable={!busy && !loading && !loadError}
              ondragstart={(e) => startMemeDrag(e, m.id)}
              ondragend={() => (dragSelection = null)}
              aria-label={m.original_filename}
              aria-pressed={selectedSet.has(m.id)}
              class={`block w-full overflow-hidden rounded border bg-white text-left dark:bg-neutral-800 ${
                selectedSet.has(m.id)
                  ? "border-blue-500 ring-2 ring-blue-500"
                  : "border-neutral-200 dark:border-neutral-800"
              }`}
              onclick={(e) => pick(m.id, e)}
            >
              <MemeThumb
                contentHash={m.content_hash}
                thumbnailPath={m.thumbnail_path}
                internalPath={m.internal_path}
                hoverPlay
              />
              <span class="block truncate px-1 py-0.5 text-[10px] text-neutral-500">{m.original_filename}</span>
            </button>
            <input type="checkbox" aria-label={`选择 ${m.original_filename}`} checked={selectedSet.has(m.id)} disabled={busy || loading || !!loadError}
              class={`absolute left-1 top-1 h-4 w-4 cursor-pointer accent-blue-600 ${multiSelect || selectedSet.has(m.id) ? '' : 'opacity-0 group-hover:opacity-100 focus:opacity-100'}`}
              onclick={(e) => pick(m.id, e, true)} />
            </div>
          {/each}
        </div>
      {/if}
      </div>
    </section>
    {#if selectedIds.length > 1}
      <aside class="w-56 shrink-0 border-l border-neutral-200 p-4 dark:border-neutral-800">
        <h2 class="font-medium">已选择 {selectedIds.length} 张</h2>
        <p class="mt-2 text-sm text-neutral-500 dark:text-neutral-400">使用上方工具栏批量整理，或将任意一张选中表情拖到左侧收藏夹。</p>
        <p class="mt-3 text-xs text-neutral-500">全选仅包含当前显示的搜索结果。切换收藏夹或修改搜索会清空选择。</p>
      </aside>
    {:else}
    <DetailPanel
      disabled={busy}
      meme={selected}
      {allTags}
      {collections}
      {memberOf}
      onToggleFavorite={toggleFavorite}
      onSaveDescription={saveDescription}
      onAddTag={addTag}
      onRemoveTag={removeTag}
      onAddToCollection={addToCollection}
      onRemoveFromCollection={removeFromCollection}
      onDelete={deleteSelected}
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
</main>
