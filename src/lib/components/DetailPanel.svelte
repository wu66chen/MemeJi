<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import Icon from './Icon.svelte';

  interface Meme {
    id: number;
    internal_path: string;
    original_filename: string;
    extension: string;
    width: number;
    height: number;
    file_size: number;
    description: string;
    is_favorite: boolean;
    tags: string[];
  }
  interface Tag {
    id: number;
    name: string;
  }
  interface Collection {
    id: number;
    name: string;
    sort_order: number;
  }

  interface Props {
    disabled?: boolean;
    meme: Meme | null;
    allTags: Tag[];
    memberOf: Collection[];
    onSaveDescription: (text: string) => Promise<void>;
    onAddTag: (name: string) => Promise<void>;
    onRemoveTag: (tagId: number) => Promise<void>;
    onRemoveFromCollection: (collectionId: number) => Promise<void>;
    onClose?: () => void;
  }

  let {
    disabled = false,
    meme,
    allTags,
    memberOf,
    onSaveDescription,
    onAddTag,
    onRemoveTag,
    onRemoveFromCollection,
    onClose,
  }: Props = $props();

  let description = $state("");
  let descriptionDirty = $state(false);
  let tagInput = $state("");
  let editingId = $state<number | null>(null);
  let editError = $state('');

  // 切换选中图片时重置编辑态
  $effect(() => {
    if (meme?.id !== editingId) {
      editingId = meme?.id ?? null;
      description = meme?.description ?? "";
      descriptionDirty = false;
      tagInput = "";
      editError = '';
    } else if (!descriptionDirty) {
      description = meme?.description ?? "";
    }
  });

  function submitTag() {
    const name = tagInput.trim();
    if (!name || !meme) return;
    tagInput = "";
    void onAddTag(name).catch((e) => (editError = String(e)));
  }

  function humanSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
</script>

<aside inert={disabled} class="detail-panel">
  <div class="detail-heading"><h2>图片详情</h2>{#if onClose}<button class="icon-button round" aria-label="关闭详情" onclick={onClose}><Icon name="x" size={15}/></button>{/if}</div>
  {#if meme}
    <div class="detail-scroll">
    <div class="detail-preview"><img src={convertFileSrc(meme.internal_path)} alt={meme.original_filename} /></div>
    <div class="detail-file"><span title={meme.original_filename}>{meme.original_filename}</span>{#if meme.is_favorite}<Icon name="heart" size={16} class="favorite-mark" />{/if}</div>
    <dl class="detail-meta"><div><dt>格式</dt><dd class="uppercase">{meme.extension}</dd></div><div><dt>尺寸</dt><dd>{meme.width} × {meme.height}</dd></div><div><dt>大小</dt><dd>{humanSize(meme.file_size)}</dd></div></dl>
    {#if editError}<p role="alert" class="detail-error">{editError}</p>{/if}
    <div class="detail-section"><label class="ui-label" for="detail-tag-input">标签</label><span class="ui-faint">{meme.tags.length}</span></div>
    <div class="detail-chips">
      {#each allTags.filter((t) => meme.tags.includes(t.name)) as t (t.id)}
        <span class="ui-chip accent">{t.name}<button class="chip-remove" aria-label={`移除标签 ${t.name}`} onclick={() => void onRemoveTag(t.id).catch((e) => (editError=String(e)))}><Icon name="x" size={12}/></button>
        </span>
      {/each}
      {#if meme.tags.length === 0}<span class="ui-faint">暂无标签</span>{/if}
    </div>
    <form class="detail-tag-form" onsubmit={(e) => { e.preventDefault(); submitTag(); }}><input id="detail-tag-input" class="ui-input" bind:value={tagInput} list="existing-tags" placeholder="添加标签，按 Enter" /><datalist id="existing-tags">{#each allTags as t (t.id)}<option value={t.name}></option>{/each}</datalist></form>
    <div class="detail-section"><span class="ui-label">所属收藏夹</span><span class="ui-faint">{memberOf.length}</span></div>
    <div class="detail-chips">
      {#each memberOf as c (c.id)}
        <span class="ui-chip"><Icon name="folder" size={13}/>{c.name}<button class="chip-remove" aria-label={`从 ${c.name} 移出`} onclick={() => void onRemoveFromCollection(c.id).catch((e) => (editError=String(e)))}><Icon name="x" size={12}/></button></span>
      {/each}
      {#if memberOf.length === 0}<span class="ui-faint">未加入收藏夹</span>{/if}
    </div>
    <div class="detail-section"><label class="ui-label" for="detail-description">描述</label></div>
    <textarea id="detail-description" class="ui-input detail-description" bind:value={description} oninput={() => (descriptionDirty = true)} rows="4" placeholder="写一点备注，方便以后找到"></textarea>
    {#if descriptionDirty}
      <div class="detail-save"><button class="ui-button small ghost" onclick={() => { description=meme.description; descriptionDirty=false; }}>取消</button><button class="ui-button small primary" onclick={() => { descriptionDirty=false; void onSaveDescription(description).catch((e)=>(editError=String(e))); }}>保存描述</button></div>
    {/if}
    </div>
  {:else}
    <div class="ui-empty"><p>选择图片后查看详情</p></div>
  {/if}
</aside>

<style>
  .detail-panel { width: 272px; flex-shrink: 0; display: flex; flex-direction: column; min-height: 0; border-left: 1px solid var(--border); background: var(--surface); }
  .detail-heading { min-height: 59px; flex-shrink: 0; display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 0 18px; border-bottom: 1px solid var(--border); }
  .detail-heading h2 { font-size: 14px; font-weight: 650; }
  .detail-scroll { overflow-y: auto; min-height: 0; padding: 18px; }
  .detail-preview { display: grid; place-items: center; height: 205px; overflow: hidden; border-radius: 12px; background: var(--surface-soft); border: 1px solid var(--border); }
  .detail-preview img { width: 100%; height: 100%; object-fit: contain; }
  .detail-file { display: flex; align-items: center; gap: 6px; margin-top: 13px; font-weight: 600; }
  .detail-file span { min-width: 0; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .detail-file :global(.favorite-mark) { color: var(--accent); }
  .detail-meta { display: grid; grid-template-columns: repeat(3,1fr); gap: 8px; margin: 14px 0 19px; padding: 11px 0; border-top: 1px solid var(--border); border-bottom: 1px solid var(--border); }
  .detail-meta dt { font-size: 11px; color: var(--faint); }
  .detail-meta dd { font-size: 12px; font-weight: 550; margin-top: 2px; }
  .detail-section { display: flex; align-items: center; justify-content: space-between; margin: 17px 0 8px; }
  .detail-section .ui-faint { font-size: 11px; }
  .detail-chips { display: flex; flex-wrap: wrap; align-items: center; gap: 5px; min-height: 24px; }
  .detail-chips > .ui-faint { font-size: 12px; }
  .chip-remove { display: inline-flex; color: var(--muted); }
  .chip-remove:hover { color: var(--danger); }
  .detail-tag-form { margin-top: 8px; }
  .detail-description { resize: vertical; min-height: 88px; }
  .detail-save { display: flex; justify-content: flex-end; gap: 6px; margin-top: 7px; }
  .detail-error { padding: 7px 9px; border-radius: 7px; color: var(--danger); background: var(--danger-soft); font-size: 12px; }
  @media (max-width: 900px) { .detail-panel { width: 236px; } .detail-scroll { padding: 12px; } .detail-preview { height: 160px; } }
</style>
