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
    meme: Meme | null;
    allTags: Tag[];
    collections: Collection[];
    memberOf: Collection[];
    onToggleFavorite: () => Promise<void>;
    onSaveDescription: (text: string) => Promise<void>;
    onAddTag: (name: string) => Promise<void>;
    onRemoveTag: (tagId: number) => Promise<void>;
    onAddToCollection: (collectionId: number) => Promise<void>;
    onRemoveFromCollection: (collectionId: number) => Promise<void>;
    onDelete: () => Promise<void>;
  }

  let {
    meme,
    allTags,
    collections,
    memberOf,
    onToggleFavorite,
    onSaveDescription,
    onAddTag,
    onRemoveTag,
    onAddToCollection,
    onRemoveFromCollection,
    onDelete,
  }: Props = $props();

  let description = $state("");
  let descriptionDirty = $state(false);
  let tagInput = $state("");
  let collectionToJoin = $state<string>("");

  // 切换选中图片时重置编辑态
  $effect(() => {
    description = meme?.description ?? "";
    descriptionDirty = false;
    tagInput = "";
    collectionToJoin = "";
  });

  const joinable = $derived(
    collections.filter((c) => !memberOf.some((m) => m.id === c.id))
  );

  function joinCollection() {
    const id = Number(collectionToJoin);
    if (!id || !meme) return;
    collectionToJoin = "";
    void onAddToCollection(id).catch((e) => alert(String(e)));
  }

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

<aside class="w-64 shrink-0 overflow-y-auto border-l border-neutral-200 p-3 text-sm dark:border-neutral-800">
  <p class="mb-2 font-medium">详情</p>
  {#if meme}
    <img
      src={convertFileSrc(meme.internal_path)}
      alt={meme.original_filename}
      class="mb-3 max-h-44 w-full rounded border border-neutral-200 bg-neutral-100 object-contain dark:border-neutral-700 dark:bg-neutral-800"
    />

    <div class="mb-3 flex items-center justify-between">
      <span class="truncate text-xs text-neutral-500" title={meme.original_filename}>{meme.original_filename}</span>
      <button
        class={`rounded px-2 py-0.5 text-xs ${
          meme.is_favorite
            ? "bg-amber-500 text-white hover:bg-amber-600"
            : "bg-neutral-200 text-neutral-700 hover:bg-neutral-300 dark:bg-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-600"
        }`}
        onclick={() => void onToggleFavorite().catch((e) => alert(String(e)))}
      >
        {meme.is_favorite ? "★ 已收藏" : "☆ 收藏"}
      </button>
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
    <form
      class="mb-3 flex gap-1"
      onsubmit={(e) => {
        e.preventDefault();
        joinCollection();
      }}
    >
      <select
        bind:value={collectionToJoin}
        class="min-w-0 flex-1 rounded border border-neutral-300 bg-white px-1.5 py-1 text-xs outline-none dark:border-neutral-600 dark:bg-neutral-900"
      >
        <option value="">选择收藏夹…</option>
        {#each joinable as c (c.id)}
          <option value={c.id}>{c.name}</option>
        {/each}
      </select>
      <button
        type="submit"
        disabled={!collectionToJoin}
        class="shrink-0 rounded bg-neutral-800 px-2 py-1 text-xs text-white disabled:opacity-40 dark:bg-neutral-700"
      >加入</button>
    </form>

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

    <button
      class="mt-2 w-full rounded border border-red-300 px-2 py-1 text-xs text-red-600 hover:bg-red-50 dark:border-red-900 dark:hover:bg-red-950"
      onclick={() => void onDelete().catch((e) => alert(String(e)))}
    >
      删除这张表情
    </button>
  {:else}
    <p class="text-xs text-neutral-400">点击中间的图片查看与编辑详情</p>
  {/if}
</aside>
