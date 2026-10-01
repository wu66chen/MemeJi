<script lang="ts">
  import Icon from './Icon.svelte';
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
  type View =
    | { kind: "all" }
    | { kind: "favorites" }
    | { kind: "recent" }
    | { kind: "collection"; id: number };

  interface Props {
    disabled?: boolean;
    draggingMemes?: boolean;
    copyDrop?: boolean;
    onDropMemes?: (collectionId: number, copy: boolean) => Promise<void>;
    collections: Collection[];
    groups: CollectionGroup[];
    activeView: View;
    onSelect: (view: View) => void;
    onCreate: (name: string) => Promise<void>;
    onRename: (id: number, name: string) => Promise<void>;
    onDelete: (id: number) => Promise<void>;
    onReorder: (ids: number[]) => Promise<void>;
    onCreateGroup: (name: string) => Promise<void>;
    onRenameGroup: (id: number, name: string) => Promise<void>;
    onDeleteGroup: (id: number) => Promise<void>;
    onReorderGroups: (ids: number[]) => Promise<void>;
    onMoveCollection: (id: number, groupId: number | null) => Promise<void>;
  }

  let { disabled = false, draggingMemes = false, copyDrop = false, onDropMemes, collections, groups, activeView, onSelect, onCreate, onRename, onDelete, onReorder,
    onCreateGroup, onRenameGroup, onDeleteGroup, onReorderGroups, onMoveCollection }: Props =
    $props();

  let newName = $state("");
  let renamingId = $state<number | null>(null);
  let renameText = $state("");
  let dragId = $state<number | null>(null);
  let dragOverId = $state<number | null>(null);
  let newGroupName = $state("");
  let renamingGroupId = $state<number | null>(null);
  let groupRenameText = $state("");
  let groupDragId = $state<number | null>(null);
  let collapsedGroupIds = $state<number[]>([]);
  let sideMenu = $state<{ kind: 'create' | 'collection' | 'group'; id?: number; x: number; y: number } | null>(null);
  let createMode = $state<'collection' | 'group' | null>(null);

  function showMenu(event: MouseEvent, kind: 'create' | 'collection' | 'group', id?: number) {
    event.stopPropagation();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    sideMenu = { kind, id, x: Math.min(rect.right + 4, window.innerWidth - 200), y: Math.max(8, Math.min(rect.top, window.innerHeight - 230)) };
  }
  function closeMenu() { sideMenu = null; }
  function onWindowClick(event: MouseEvent) {
    if (sideMenu && !(event.target as Element).closest('[data-side-popup],[data-side-trigger]')) closeMenu();
  }

  const systemViews: { view: View; label: string }[] = [
    { view: { kind: "all" }, label: "全部" },
    { view: { kind: "favorites" }, label: "收藏" },
    { view: { kind: "recent" }, label: "最近使用" },
  ];

  function isSystemActive(view: View): boolean {
    return activeView.kind === view.kind;
  }

  function isCollectionActive(c: Collection): boolean {
    return activeView.kind === "collection" && activeView.id === c.id;
  }

  function itemClass(active: boolean): string {
    return `side-item ${active ? 'active' : ''}`;
  }

  async function submitCreate() {
    const name = newName.trim();
    if (!name) return;
    try {
      await onCreate(name);
      newName = "";
      createMode = null;
    } catch (e) {
      alert(String(e));
    }
  }

  function startRename(c: Collection) {
    renamingId = c.id;
    renameText = c.name;
  }

  async function submitRename(id: number) {
    const name = renameText.trim();
    if (!name) return;
    try {
      await onRename(id, name);
      renamingId = null;
    } catch (e) {
      alert(String(e));
    }
  }

  function resetDrag() {
    dragId = null;
    dragOverId = null;
  }

  function onDrop(targetId: number) {
    if (dragId === null || dragId === targetId) return resetDrag();
    const target = collections.find((c) => c.id === targetId);
    if (!target) return resetDrag();
    if (collections.find((c) => c.id === dragId)?.group_id !== target.group_id) {
      const movedId = dragId;
      resetDrag();
      void onMoveCollection(movedId, target.group_id);
      return;
    }
    const ids = collections.filter((c) => c.group_id === target.group_id).map((c) => c.id);
    const from = ids.indexOf(dragId);
    const to = ids.indexOf(targetId);
    if (from < 0 || to < 0) return resetDrag();
    ids.splice(to, 0, ...ids.splice(from, 1));
    resetDrag();
    void onReorder(ids);
  }

  function toggleGroup(id: number) {
    collapsedGroupIds = collapsedGroupIds.includes(id)
      ? collapsedGroupIds.filter((g) => g !== id) : [...collapsedGroupIds, id];
  }

  async function submitGroupCreate() {
    const name = newGroupName.trim();
    if (!name) return;
    try { await onCreateGroup(name); newGroupName = ""; createMode = null; }
    catch (e) { alert(String(e)); }
  }

  async function submitGroupRename(id: number) {
    const name = groupRenameText.trim();
    if (!name) return;
    try { await onRenameGroup(id, name); renamingGroupId = null; }
    catch (e) { alert(String(e)); }
  }

  function dropGroup(targetId: number) {
    if (groupDragId === null || groupDragId === targetId) return;
    const ids = groups.map((g) => g.id);
    ids.splice(ids.indexOf(targetId), 0, ...ids.splice(ids.indexOf(groupDragId), 1));
    groupDragId = null;
    void onReorderGroups(ids);
  }
</script>

<svelte:window onclick={onWindowClick} />
<aside inert={disabled} class="side-panel">
  <div class="side-brand"><img src="/memeji.png" alt=""/><span>MemeJi</span></div>
  <p class="side-caption">图库</p>
  <ul class="side-list">
    {#each systemViews as sv (sv.view.kind)}
      <li>
        <button class={itemClass(isSystemActive(sv.view))} onclick={() => onSelect(sv.view)}>
          <Icon name={sv.view.kind === 'all' ? 'grid' : sv.view.kind === 'favorites' ? 'heart' : 'clock'} size={17}/><span>{sv.label}</span>
        </button>
      </li>
    {/each}
    <li class="side-section-heading"><span>收藏夹</span><button data-side-trigger type="button" class="icon-button small" aria-label="新建收藏夹或分组" title="新建" onclick={(e) => showMenu(e, 'create')}><Icon name="plus" size={17}/></button></li>
    {#snippet collectionRow(c: Collection)}
      <li
        class={`side-row ${c.group_id === null ? '' : 'nested-collection'} ${dragOverId === c.id ? 'drop-target' : ''} ${isCollectionActive(c) ? 'active' : ''}`}
        draggable={renamingId !== c.id}
        ondragstart={(e) => { dragId = c.id; e.dataTransfer?.setData('application/x-memeji-collection', String(c.id)); }}
        ondragover={(e) => {
          if (!draggingMemes && dragId === null) return;
          e.preventDefault();
          if (draggingMemes && e.dataTransfer) e.dataTransfer.dropEffect = copyDrop || e.ctrlKey || e.metaKey ? 'copy' : 'move';
          dragOverId = c.id;
        }}
        ondragleave={() => (dragOverId = null)}
        ondrop={(e) => {
          e.preventDefault();
          e.stopPropagation();
          if (draggingMemes) {
            resetDrag();
            void onDropMemes?.(c.id, e.ctrlKey || e.metaKey);
            return;
          }
          onDrop(c.id);
        }}
        ondragend={resetDrag}
      >
        {#if renamingId === c.id}
          <input
            bind:value={renameText}
            class="ui-input side-rename"
            onkeydown={(e) => {
              if (e.key === "Enter") void submitRename(c.id);
              if (e.key === "Escape") renamingId = null;
            }}
            onblur={() => (renamingId = null)}
          />
        {:else}
          <button title={c.name} class="side-row-name" onclick={() => onSelect({ kind: "collection", id: c.id })}>
            <Icon name="folder" size={16}/><span class="truncate">{c.name}</span>
          </button>
          <button data-side-trigger type="button" class="icon-button small side-more" title={`${c.name} 的操作`} aria-label={`${c.name} 的操作`} onclick={(e) => showMenu(e,'collection',c.id)}><Icon name="more" size={15}/></button>
        {/if}
      </li>
    {/snippet}
    {#each collections.filter((c) => c.group_id === null) as c (c.id)}
      {@render collectionRow(c)}
    {/each}
    {#each groups as g (g.id)}
      <li
        class="side-group-row"
        draggable={renamingGroupId !== g.id}
        ondragstart={(e) => { e.stopPropagation(); groupDragId = g.id; e.dataTransfer?.setData('application/x-memeji-group',String(g.id)); }}
        ondragover={(e) => { if (!draggingMemes) e.preventDefault(); }}
        ondrop={(e) => {
          if (draggingMemes) return;
          e.preventDefault();
          if (dragId !== null) { const id = dragId; resetDrag(); void onMoveCollection(id, g.id); }
          else dropGroup(g.id);
        }}
        ondragend={() => (groupDragId = null)}
      >
        <button class="icon-button small" title="展开或折叠" aria-label={`${collapsedGroupIds.includes(g.id) ? '展开' : '折叠'}${g.name}`} onclick={() => toggleGroup(g.id)}><Icon name={collapsedGroupIds.includes(g.id) ? 'chevron-right' : 'chevron-down'} size={14}/></button>
        {#if renamingGroupId === g.id}
          <input
            bind:value={groupRenameText}
            class="ui-input side-rename"
            onkeydown={(e) => {
              if (e.key === "Enter") void submitGroupRename(g.id);
              if (e.key === "Escape") renamingGroupId = null;
            }}
          />
        {:else}
          <span class="side-group-name" title={g.name}>{g.name}</span>
          <button data-side-trigger type="button" class="icon-button small side-more" title={`${g.name} 分组操作`} aria-label={`${g.name} 分组操作`} onclick={(e) => showMenu(e,'group',g.id)}><Icon name="more" size={15}/></button>
        {/if}
      </li>
      {#if !collapsedGroupIds.includes(g.id)}
        {#each collections.filter((c) => c.group_id === g.id) as c (c.id)}
          {@render collectionRow(c)}
        {/each}
      {/if}
    {/each}
  </ul>
  {#if createMode}
    <form class="side-create" onsubmit={(e) => { e.preventDefault(); if (createMode === 'group') void submitGroupCreate(); else void submitCreate(); }}>
      <label class="ui-label" for="side-create-name">{createMode === 'group' ? '新建分组' : '新建收藏夹'}</label>
      {#if createMode === 'group'}
        <input id="side-create-name" class="ui-input" aria-label="分组名称" bind:value={newGroupName} placeholder="输入名称" />
      {:else}
        <input id="side-create-name" class="ui-input" aria-label="收藏夹名称" bind:value={newName} placeholder="输入名称" />
      {/if}
      <div class="side-create-actions"><button type="button" class="ui-button small ghost" onclick={() => (createMode = null)}>取消</button><button type="submit" class="ui-button small primary">创建</button></div>
    </form>
  {/if}
</aside>
{#if sideMenu}
  <div data-side-popup role="menu" class="ui-menu side-popup" style={`left:${sideMenu.x}px;top:${sideMenu.y}px`}>
    {#if sideMenu.kind === 'create'}
      <button role="menuitem" class="ui-menu-item" onclick={() => { createMode='collection'; closeMenu(); }}><Icon name="folder-plus" size={16}/>新建收藏夹</button>
      <button role="menuitem" class="ui-menu-item" onclick={() => { createMode='group'; closeMenu(); }}><Icon name="layers" size={16}/>新建分组</button>
    {:else if sideMenu.kind === 'collection'}
      <button role="menuitem" class="ui-menu-item" onclick={() => { const c=collections.find(item=>item.id===sideMenu?.id); if(c) startRename(c); closeMenu(); }}><Icon name="tag" size={16}/>重命名</button>
      <div class="side-popup-label">移动到分组</div>
      <select class="ui-select side-popup-select" aria-label="移动到分组" value={collections.find(c=>c.id===sideMenu!.id)?.group_id ?? ''}
        onchange={(e) => { void onMoveCollection(sideMenu!.id!,e.currentTarget.value ? Number(e.currentTarget.value) : null); closeMenu(); }}>
        <option value="">未分组</option>{#each groups as g (g.id)}<option value={g.id}>{g.name}</option>{/each}
      </select>
      <div class="ui-menu-separator"></div>
      <button role="menuitem" class="ui-menu-item danger" onclick={() => { void onDelete(sideMenu!.id!); closeMenu(); }}><Icon name="trash" size={16}/>删除收藏夹</button>
    {:else}
      <button role="menuitem" class="ui-menu-item" onclick={() => { renamingGroupId=sideMenu!.id!; groupRenameText=groups.find(g=>g.id===sideMenu?.id)?.name ?? ''; closeMenu(); }}><Icon name="tag" size={16}/>重命名分组</button>
      <button role="menuitem" class="ui-menu-item danger" onclick={() => { void onDeleteGroup(sideMenu!.id!); closeMenu(); }}><Icon name="trash" size={16}/>删除分组</button>
    {/if}
  </div>
{/if}

<style>
  .side-panel { width: 208px; flex-shrink: 0; display: flex; flex-direction: column; min-height: 0; overflow-y: auto; border-right: 1px solid var(--border); background: var(--sidebar); padding: 13px 10px; }
  .side-brand { display: flex; align-items: center; gap: 9px; padding: 3px 8px 18px; font-weight: 700; font-size: 14px; letter-spacing: -.02em; }
  .side-brand img { width: 24px; height: 24px; object-fit: contain; }
  .side-caption { padding: 0 9px 8px; font-size: 11px; font-weight: 700; letter-spacing: .08em; color: var(--faint); }
  .side-list { display: flex; flex-direction: column; gap: 2px; }
  .side-item, .side-row { display: flex; align-items: center; min-height: 34px; width: 100%; border-radius: 8px; color: var(--muted); }
  .side-item { gap: 10px; padding: 0 10px; text-align: left; font-weight: 500; }
  .side-item:hover, .side-row:hover { background: var(--hover); color: var(--text); }
  .side-item.active, .side-row.active { background: var(--accent-soft); color: var(--accent-ink); font-weight: 600; }
  .side-row.drop-target { box-shadow: inset 0 0 0 1px var(--accent); }
  .side-row.nested-collection { padding-left: 14px; }
  .side-row-name { display: flex; align-items: center; min-width: 0; flex: 1; gap: 9px; padding: 7px 8px; text-align: left; }
  .side-more { opacity: 0; margin-right: 2px; }
  .side-row:hover .side-more, .side-row:focus-within .side-more, .side-group-row:hover .side-more, .side-group-row:focus-within .side-more { opacity: 1; }
  .side-section-heading { display: flex; align-items: center; justify-content: space-between; padding: 21px 5px 5px 10px; font-size: 11px; font-weight: 700; letter-spacing: .08em; color: var(--faint); }
  .side-group-row { display: flex; align-items: center; min-height: 32px; margin-top: 12px; padding: 0 3px; color: var(--muted); }
  .side-group-name { min-width: 0; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; font-weight: 650; }
  .side-rename { min-width: 0; height: 28px; min-height: 28px; padding: 3px 6px; }
  .side-create { margin-top: 12px; padding: 12px 8px; border-top: 1px solid var(--border); }
  .side-create .ui-input { margin-top: 7px; }
  .side-create-actions { display: flex; justify-content: flex-end; gap: 5px; margin-top: 9px; }
  .side-popup { position: fixed; z-index: 45; width: 184px; }
  .side-popup-label { color: var(--muted); font-size: 11px; font-weight: 600; padding: 9px 10px 3px; }
  .side-popup-select { width: calc(100% - 12px); margin: 2px 6px 5px; font-size: 12px; }
  @media (max-width: 900px) { .side-panel { width: 176px; } }
</style>
