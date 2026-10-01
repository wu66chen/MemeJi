<script lang="ts">
  import { onMount } from 'svelte';
  interface Collection { id: number; name: string; group_id: number | null }
  interface Group { id: number; name: string }
  let { collections, groups, count, sourceId, mode, busy, error, onChoose, onClose }: {
    collections: Collection[]; groups: Group[]; count: number; sourceId: number | null;
    mode: 'add' | 'move'; busy: boolean; error: string;
    onChoose: (id: number) => void; onClose: () => void;
  } = $props();
  let dialog: HTMLDialogElement;
  let search = $state('');
  const targets = $derived(collections.filter(c =>
    !(mode === 'move' && c.id === sourceId) &&
    `${groups.find(g => g.id === c.group_id)?.name ?? ''} ${c.name}`.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase())
  ));
  const source = $derived(collections.find(c => c.id === sourceId)?.name ?? '来源收藏夹');
  onMount(() => { dialog.showModal(); });
</script>

<dialog bind:this={dialog} aria-labelledby="transfer-title" oncancel={(e) => { if (busy) e.preventDefault(); }} onclose={onClose}
  class="m-auto max-h-[80vh] w-[min(440px,90vw)] rounded-xl border border-neutral-200 bg-white p-5 text-neutral-900 shadow-xl dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-100">
  <div class="mb-2 flex items-center justify-between gap-3">
    <h2 id="transfer-title" class="font-semibold">{mode === 'move' ? '移动到收藏夹' : '加入收藏夹'} · {count} 张</h2>
    <button class="rounded px-2 py-1 text-sm disabled:opacity-40" disabled={busy} onclick={onClose}>取消</button>
  </div>
  <p class="mb-3 text-xs text-neutral-500 dark:text-neutral-400">
    {mode === 'move' ? `从「${source}」移出，保留其他收藏夹归属。点击目标即可移动。` : '保留已有归属。点击目标即可加入，不会重复添加。'}
  </p>
  <input aria-label="搜索目标收藏夹" bind:value={search} disabled={busy} placeholder="搜索收藏夹或分组…"
    class="mb-3 w-full rounded border border-neutral-300 bg-transparent px-3 py-2 text-sm dark:border-neutral-600" />
  {#if error}<p class="mb-2 text-sm text-red-600 dark:text-red-400" role="alert">{error}</p>{/if}
  <div class="max-h-[45vh] overflow-y-auto space-y-1" aria-busy={busy}>
    {#each targets as c (c.id)}
      <button disabled={busy} class="flex w-full items-center justify-between gap-3 rounded-lg border border-neutral-200 px-3 py-2 text-left hover:bg-neutral-100 disabled:opacity-50 dark:border-neutral-700 dark:hover:bg-neutral-800" onclick={() => onChoose(c.id)}>
        <span class="min-w-0 truncate" title={c.name}>{c.name}</span>
        <span class="max-w-[40%] truncate text-xs text-neutral-500">{groups.find(g => g.id === c.group_id)?.name ?? '未分组'}</span>
      </button>
    {:else}
      <p class="py-6 text-center text-sm text-neutral-500">{collections.length === 0 ? '还没有收藏夹，请先在左侧新建。' : '没有可用的目标收藏夹'}</p>
    {/each}
  </div>
  {#if busy}<p class="mt-3 text-sm" role="status">正在处理…</p>{/if}
</dialog>

<style>dialog::backdrop { background: rgb(0 0 0 / 0.5); }</style>
