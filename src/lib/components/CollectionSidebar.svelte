<script lang="ts">
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
    return `group flex w-full items-center rounded px-2 py-1 text-left ${
      active ? "bg-neutral-200 dark:bg-neutral-800" : "hover:bg-neutral-100 dark:hover:bg-neutral-800/60"
    }`;
  }

  async function submitCreate() {
    const name = newName.trim();
    if (!name) return;
    try {
      await onCreate(name);
      newName = "";
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
    try { await onCreateGroup(name); newGroupName = ""; }
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

<aside inert={disabled} class="w-44 shrink-0 overflow-y-auto border-r border-neutral-200 p-2 text-sm dark:border-neutral-800">
  <p class="mb-1 px-1 text-xs text-neutral-500">收藏夹</p>
  <ul class="space-y-0.5">
    {#each systemViews as sv (sv.view.kind)}
      <li>
        <button class={itemClass(isSystemActive(sv.view))} onclick={() => onSelect(sv.view)}>
          {sv.label}
        </button>
      </li>
    {/each}
    {#snippet collectionRow(c: Collection)}
      <li
        class={`group flex items-center rounded ${
          dragOverId === c.id ? "ring-1 ring-neutral-400 dark:ring-neutral-600" : ""
        } ${isCollectionActive(c) ? "bg-neutral-200 dark:bg-neutral-800" : "hover:bg-neutral-100 dark:hover:bg-neutral-800/60"}`}
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
            class="mx-1 w-full rounded border border-neutral-300 bg-white px-2 py-1 outline-none dark:border-neutral-600 dark:bg-neutral-900"
            onkeydown={(e) => {
              if (e.key === "Enter") void submitRename(c.id);
              if (e.key === "Escape") renamingId = null;
            }}
            onblur={() => (renamingId = null)}
          />
        {:else}
          <button title={c.name} class={`min-w-0 flex-1 truncate py-1 pr-2 text-left ${c.group_id === null ? "pl-2" : "pl-5"}`} onclick={() => onSelect({ kind: "collection", id: c.id })}>
            {c.name}
          </button>
          <span class="hidden shrink-0 gap-0.5 pr-1 group-hover:flex">
            <select
              class="max-w-16 bg-transparent text-[10px]"
              title="移动到分组"
              value={c.group_id == null ? "" : String(c.group_id)}
              onchange={(e) => void onMoveCollection(c.id, e.currentTarget.value ? Number(e.currentTarget.value) : null)}
            >
              <option value="">未分组</option>
              {#each groups as g (g.id)}<option value={g.id}>{g.name}</option>{/each}
            </select>
            <button
              class="rounded px-1 text-xs text-neutral-400 hover:text-neutral-700 dark:hover:text-neutral-200"
              title="重命名"
              onclick={() => startRename(c)}
            >✎</button>
            <button
              class="rounded px-1 text-xs text-neutral-400 hover:text-red-600"
              title="删除收藏夹（不删图）"
              onclick={() => void onDelete(c.id)}
            >✕</button>
          </span>
        {/if}
      </li>
    {/snippet}
    {#each collections.filter((c) => c.group_id === null) as c (c.id)}
      {@render collectionRow(c)}
    {/each}
    {#each groups as g (g.id)}
      <li
        class="group mt-2 flex items-center rounded bg-neutral-100 px-1 dark:bg-neutral-800/60"
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
        <button class="px-1 text-xs" title="展开或折叠" onclick={() => toggleGroup(g.id)}>{collapsedGroupIds.includes(g.id) ? "▸" : "▾"}</button>
        {#if renamingGroupId === g.id}
          <input
            bind:value={groupRenameText}
            class="min-w-0 flex-1 rounded border bg-white px-1 text-xs dark:bg-neutral-900"
            onkeydown={(e) => {
              if (e.key === "Enter") void submitGroupRename(g.id);
              if (e.key === "Escape") renamingGroupId = null;
            }}
          />
        {:else}
          <span class="min-w-0 flex-1 truncate px-1 py-1 text-xs font-semibold" title={g.name}>{g.name}</span>
          <span class="hidden group-hover:flex">
            <button class="px-1 text-xs" title="重命名分组" onclick={() => { renamingGroupId = g.id; groupRenameText = g.name; }}>✎</button>
            <button class="px-1 text-xs text-red-500" title="删除分组，收藏夹移至未分组" onclick={() => void onDeleteGroup(g.id)}>✕</button>
          </span>
        {/if}
      </li>
      {#if !collapsedGroupIds.includes(g.id)}
        {#each collections.filter((c) => c.group_id === g.id) as c (c.id)}
          {@render collectionRow(c)}
        {/each}
      {/if}
    {/each}
  </ul>
  <form class="mt-2 flex gap-1" onsubmit={(e) => { e.preventDefault(); void submitGroupCreate(); }}>
    <input bind:value={newGroupName} placeholder="新建分组" class="min-w-0 flex-1 rounded border border-neutral-300 bg-white px-2 py-1 text-xs dark:border-neutral-600 dark:bg-neutral-900" />
    <button type="submit" class="rounded bg-neutral-800 px-2 py-1 text-xs text-white dark:bg-neutral-700">添加</button>
  </form>
  <form
    class="mt-2 flex gap-1"
    onsubmit={(e) => {
      e.preventDefault();
      void submitCreate();
    }}
  >
    <input
      bind:value={newName}
      placeholder="新建收藏夹"
      class="min-w-0 flex-1 rounded border border-neutral-300 bg-white px-2 py-1 text-xs outline-none placeholder:text-neutral-400 dark:border-neutral-600 dark:bg-neutral-900"
    />
    <button type="submit" class="rounded bg-neutral-800 px-2 py-1 text-xs text-white hover:bg-neutral-700 dark:bg-neutral-700 dark:hover:bg-neutral-600">
      添加
    </button>
  </form>
</aside>
