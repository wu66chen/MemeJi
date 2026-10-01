<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from './Icon.svelte';
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
  class="ui-dialog transfer-dialog">
  <div class="ui-dialog-header">
    <div><h2 id="transfer-title">{mode === 'move' ? '移动到收藏夹' : '加入收藏夹'}</h2><p class="ui-muted">已选 {count} 张</p></div>
    <button type="button" class="icon-button round" aria-label="关闭" disabled={busy} onclick={onClose}><Icon name="x" size={16}/></button>
  </div>
  <div class="ui-dialog-body">
  {#if mode === 'move'}<p class="transfer-explainer">从「{source}」移出，其他收藏夹的归属会保留。</p>{/if}
  <div class="transfer-search"><Icon name="search" size={16}/><input aria-label="搜索目标收藏夹" bind:value={search} disabled={busy} placeholder="搜索收藏夹或分组" /></div>
  {#if error}<p class="transfer-error" role="alert">{error}</p>{/if}
  <div class="transfer-list" aria-busy={busy}>
    {#each targets as c (c.id)}
      <button disabled={busy} class="transfer-target" onclick={() => onChoose(c.id)}><Icon name="folder" size={17}/>
        <span class="transfer-name" title={c.name}>{c.name}</span>
        <span class="transfer-group">{groups.find(g => g.id === c.group_id)?.name ?? '未分组'}</span>
      </button>
    {:else}
      <div class="ui-empty"><p>{collections.length === 0 ? '还没有收藏夹，请先在左侧新建。' : '没有可用的目标收藏夹'}</p></div>
    {/each}
  </div>
  {#if busy}<p class="transfer-status" role="status">正在处理…</p>{/if}
  </div>
</dialog>

<style>
  .transfer-dialog { width: min(460px,calc(100vw - 30px)); max-width: none; max-height: calc(100vh - 30px); margin: auto; padding: 0; }
  .transfer-dialog .ui-dialog-header p { font-size: 12px; margin-top: 2px; }
  .transfer-explainer { color: var(--muted); font-size: 12px; margin-bottom: 11px; }
  .transfer-search { display: flex; align-items: center; gap: 9px; height: 35px; padding: 0 10px; margin-bottom: 12px; border: 1px solid var(--border); border-radius: 8px; background: var(--surface-soft); color: var(--faint); }
  .transfer-search:focus-within { border-color: var(--accent); }
  .transfer-search input { min-width: 0; flex: 1; border: 0; outline: 0; background: transparent; color: var(--text); }
  .transfer-list { max-height: 45vh; min-height: 90px; overflow-y: auto; }
  .transfer-target { display: flex; width: 100%; min-height: 41px; align-items: center; gap: 10px; padding: 7px 10px; border-radius: 8px; text-align: left; color: var(--muted); }
  .transfer-target:hover { background: var(--hover); color: var(--text); }
  .transfer-name { min-width: 0; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text); }
  .transfer-group { max-width: 34%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--faint); font-size: 12px; }
  .transfer-error { color: var(--danger); background: var(--danger-soft); padding: 8px 10px; border-radius: 7px; font-size: 12px; margin-bottom: 10px; }
  .transfer-status { margin-top: 9px; color: var(--muted); font-size: 12px; }
</style>
