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
  let selectedId = $state<number | null>(null);
  let importSummary = $state("");
  let query = $state("");
  let globalSearch = $state(false);
  let onboarded = $state<boolean | null>(null);
  let showSettings = $state(false);

  const selected = $derived(memes.find((m) => m.id === selectedId) ?? null);

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
    const result = await invoke<Meme[]>("list_memes", { view, query: q || undefined });
    if (requestId === memesRequestId) memes = result;
    collections = await invoke<Collection[]>("list_collections");
    groups = await invoke<CollectionGroup[]>("list_collection_groups");
    allTags = await invoke<Tag[]>("list_tags");
  }

  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  function onSearchInput() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => void refresh(), 150);
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
    activeView = view;
    selectedId = null;
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
    await invoke("set_favorite", { memeId: selected.id, favorite: !selected.is_favorite });
    await refresh();
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
    if (!selected) return;
    const ok = await ask("删除后库内文件与缩略图缓存一并清理，且不可恢复，确定？", {
      title: "删除表情",
      kind: "warning",
    });
    if (!ok) return;
    await invoke("delete_meme", { id: selected.id });
    selectedId = null;
    await refresh();
  }

  refresh();

  onMount(() => {
    void checkForUpdatesOnStartup();
    // 首启向导门 + 托盘「设置」唤起
    void invoke<{ onboarded: boolean }>("get_config")
      .then((cfg) => (onboarded = cfg.onboarded))
      .catch(() => (onboarded = true));
    void listen("open-settings", () => (showSettings = true));
  });
</script>

<main class="flex h-screen flex-col bg-neutral-50 text-neutral-900 dark:bg-neutral-900 dark:text-neutral-100">
  <header class="flex items-center justify-between border-b border-neutral-200 px-4 py-2 dark:border-neutral-800">
    <h1 class="text-sm font-semibold">表情包管理器</h1>
    <div class="flex items-center gap-2">
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
    />
    <section class="flex flex-1 flex-col overflow-hidden">
      <div class="flex items-center gap-2 border-b border-neutral-200 px-3 py-2 dark:border-neutral-800">
        <input
          bind:value={query}
          oninput={onSearchInput}
          placeholder="搜索文件名、标签、描述…（空格分隔多关键词）"
          class="min-w-0 flex-1 rounded bg-neutral-200/70 px-3 py-1.5 text-sm outline-none placeholder:text-neutral-400 focus:ring-1 focus:ring-neutral-400 dark:bg-neutral-800 dark:focus:ring-neutral-600"
        />
        {#if activeView.kind !== "all"}
          <label
            class="flex shrink-0 cursor-pointer items-center gap-1 text-xs text-neutral-500"
            title="开启后跨全库搜索，关闭则只在当前视图内搜索"
          >
            <input type="checkbox" bind:checked={globalSearch} onchange={() => void refresh()} />
            全局
          </label>
        {/if}
      </div>
      <div class="flex-1 overflow-y-auto p-3">
      {#if memes.length === 0}
        <div class="grid h-full place-items-center text-sm text-neutral-400">
          {query.trim() ? "没有匹配的表情" : emptyHints[activeView.kind]}
        </div>
      {:else}
        <div class="grid grid-cols-[repeat(auto-fill,minmax(96px,1fr))] gap-2">
          {#each memes as m (m.id)}
            <button
              type="button"
              class={`block w-full overflow-hidden rounded border bg-white text-left dark:bg-neutral-800 ${
                m.id === selectedId
                  ? "border-neutral-700 ring-1 ring-neutral-700 dark:border-neutral-400 dark:ring-neutral-400"
                  : "border-neutral-200 dark:border-neutral-800"
              }`}
              onclick={() => (selectedId = m.id)}
            >
              <MemeThumb
                contentHash={m.content_hash}
                thumbnailPath={m.thumbnail_path}
                internalPath={m.internal_path}
                hoverPlay
              />
              <span class="block truncate px-1 py-0.5 text-[10px] text-neutral-500">{m.original_filename}</span>
            </button>
          {/each}
        </div>
      {/if}
      </div>
    </section>
    <DetailPanel
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
  </div>

  {#if onboarded === false}
    <OnboardingWizard onFinish={() => { onboarded = true; void refresh(); }} />
  {/if}
  {#if showSettings}
    <SettingsPanel onClose={() => (showSettings = false)} onMemesChanged={() => void refresh()} />
  {/if}
</main>
