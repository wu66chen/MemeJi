<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";

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
  }: Props = $props();

  let description = $state("");
  let descriptionDirty = $state(false);
  let tagInput = $state("");

  // 切换选中图片时重置编辑态
  $effect(() => {
    description = meme?.description ?? "";
    descriptionDirty = false;
    tagInput = "";
  });

  function submitTag() {
    const name = tagInput.trim();
    if (!name || !meme) return;
    tagInput = "";
    void onAddTag(name).catch((e) => alert(String(e)));
  }

  function humanSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
</script>

<aside inert={disabled} class="w-64 shrink-0 overflow-y-auto border-l border-neutral-200 p-3 text-sm dark:border-neutral-800">
  <p class="mb-2 font-medium">详情</p>
  {#if meme}
    <img
      src={convertFileSrc(meme.internal_path)}
      alt={meme.original_filename}
      class="mb-3 max-h-44 w-full rounded border border-neutral-200 bg-neutral-100 object-contain dark:border-neutral-700 dark:bg-neutral-800"
    />

    <div class="mb-3 flex items-center justify-between gap-2">
      <span class="truncate text-xs text-neutral-500" title={meme.original_filename}>{meme.original_filename}</span>
      {#if meme.is_favorite}<span class="text-xs text-amber-600" aria-label="已收藏">★</span>{/if}
    </div>

    <dl class="mb-3 space-y-1 text-xs text-neutral-500">
      <div class="flex justify-between"><dt>格式</dt><dd class="uppercase">{meme.extension}</dd></div>
      <div class="flex justify-between"><dt>尺寸</dt><dd>{meme.width} × {meme.height}</dd></div>
      <div class="flex justify-between"><dt>大小</dt><dd>{humanSize(meme.file_size)}</dd></div>
    </dl>

    <p class="mb-1 text-xs text-neutral-500">标签</p>
    <div class="mb-1 flex flex-wrap gap-1">
      {#each allTags.filter((t) => meme.tags.includes(t.name)) as t (t.id)}
        <span class="flex items-center gap-0.5 rounded bg-neutral-200 px-1.5 py-0.5 text-xs dark:bg-neutral-700">
          {t.name}
          <button
            class="text-neutral-400 hover:text-red-600"
            title="移除标签"
            onclick={() => void onRemoveTag(t.id).catch((e) => alert(String(e)))}
          >×</button>
        </span>
      {/each}
      {#if meme.tags.length === 0}
        <span class="text-xs text-neutral-400">暂无</span>
      {/if}
    </div>
    <form
      class="mb-3"
      onsubmit={(e) => {
        e.preventDefault();
        submitTag();
      }}
    >
      <input
        bind:value={tagInput}
        list="existing-tags"
        placeholder="输入新标签或选择已有"
        class="w-full rounded border border-neutral-300 bg-white px-2 py-1 text-xs outline-none placeholder:text-neutral-400 dark:border-neutral-600 dark:bg-neutral-900"
      />
      <datalist id="existing-tags">
        {#each allTags as t (t.id)}
          <option value={t.name}></option>
        {/each}
      </datalist>
    </form>

    <p class="mb-1 text-xs text-neutral-500">所属收藏夹</p>
    <div class="mb-1 flex flex-wrap gap-1">
      {#each memberOf as c (c.id)}
        <span class="flex items-center gap-0.5 rounded bg-neutral-200 px-1.5 py-0.5 text-xs dark:bg-neutral-700">
          {c.name}
          <button
            class="text-neutral-400 hover:text-red-600"
            title="移出收藏夹"
            onclick={() => void onRemoveFromCollection(c.id).catch((e) => alert(String(e)))}
          >×</button>
        </span>
      {/each}
      {#if memberOf.length === 0}
        <span class="text-xs text-neutral-400">未加入</span>
      {/if}
    </div>
    <p class="mb-1 text-xs text-neutral-500">描述</p>
    <textarea
      bind:value={description}
      oninput={() => (descriptionDirty = true)}
      rows="3"
      placeholder="这张图是什么？什么时候用？"
      class="mb-1 w-full resize-none rounded border border-neutral-300 bg-white px-2 py-1 text-xs outline-none placeholder:text-neutral-400 dark:border-neutral-600 dark:bg-neutral-900"
    ></textarea>
    {#if descriptionDirty}
      <div class="mb-3 flex gap-1">
        <button
          class="rounded bg-neutral-800 px-2 py-1 text-xs text-white hover:bg-neutral-700 dark:bg-neutral-700 dark:hover:bg-neutral-600"
          onclick={() => {
            descriptionDirty = false;
            void onSaveDescription(description).catch((e) => alert(String(e)));
          }}
        >保存描述</button>
        <button
          class="rounded px-2 py-1 text-xs text-neutral-500 hover:text-neutral-800 dark:hover:text-neutral-200"
          onclick={() => {
            description = meme.description;
            descriptionDirty = false;
          }}
        >取消</button>
      </div>
    {/if}

  {:else}
    <p class="text-xs text-neutral-400">点击中间的图片查看与编辑详情</p>
  {/if}
</aside>
