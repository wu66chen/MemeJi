<script lang="ts">
  let { name, size = 18, class: className = '' }: { name: string; size?: number; class?: string } = $props();
  const paths: Record<string, string> = {
    search: 'm21 21-4.5-4.5 M19 10.5a8.5 8.5 0 1 1-17 0 8.5 8.5 0 0 1 17 0',
    plus: 'M12 5v14 M5 12h14', x: 'm6 6 12 12 M18 6 6 18', check: 'm5 12 4 4L19 6',
    folder: 'M3 7V5a2 2 0 0 1 2-2h5l2 3h7a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z',
    'folder-plus': 'M3 7V5a2 2 0 0 1 2-2h5l2 3h7a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z M12 10v7 M8.5 13.5h7',
    grid: 'M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z',
    heart: 'M20.8 4.6a5.5 5.5 0 0 0-7.8 0L12 5.7l-1.1-1.1a5.5 5.5 0 0 0-7.8 7.8L12 21l8.8-8.6a5.5 5.5 0 0 0 0-7.8Z',
    clock: 'M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0 M12 6v6l4 2',
    'chevron-down': 'm6 9 6 6 6-6', 'chevron-right': 'm9 6 6 6-6 6',
    more: 'M5 12h.01 M12 12h.01 M19 12h.01',
    image: 'M4 3h16a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Z M3 16l5-5 5 5 3-3 5 5 M8 7h.01',
    upload: 'M12 16V3 m-5 5 5-5 5 5 M4 16v4a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-4',
    tag: 'M20 13 11 22 2 13V3h10l8 8a1.5 1.5 0 0 1 0 2Z M7 8h.01',
    trash: 'M3 6h18 M9 6V3h6v3 M5 6l1 15h12l1-15 M10 10v7 M14 10v7',
    move: 'M4 7h13 m-4-4 4 4-4 4 M20 17H7 m4-4-4 4 4 4',
    'arrow-left': 'M20 12H4 m7-7-7 7 7 7', 'arrow-right': 'M4 12h16 m-7-7 7 7-7 7',
    info: 'M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0 M12 11v6 M12 7h.01',
    sliders: 'M4 7h16 M4 17h16 M8 4v6 M16 14v6',
    keyboard: 'M3 5h18v14H3z M7 9h.01 M11 9h.01 M15 9h.01 M18 9h.01 M7 13h.01 M11 13h.01 M15 13h.01 M7 16h10',
    monitor: 'M3 3h18v14H3z M12 17v4 M8 21h8',
    sun: 'M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0 M12 2v2 M12 20v2 M2 12h2 M20 12h2 m-3-9-1.5 1.5 M5.5 18.5 4 20 m0-16 1.5 1.5 M18.5 18.5 20 20',
    moon: 'M21 12.8A9 9 0 0 1 11.2 3 9 9 0 1 0 21 12.8Z',
    sparkles: 'm12 3 2.3 6.7L21 12l-6.7 2.3L12 21l-2.3-6.7L3 12l6.7-2.3L12 3Z',
    panel: 'M3 3h18v18H3z M15 3v18',
    refresh: 'M20 7a9 9 0 1 0 1 9 M20 2v6h-6',
    alert: 'm12 3 10 18H2L12 3Z M12 9v5 M12 18h.01',
    copy: 'M8 8h13v13H8z M16 8V3H3v13h5',
    layers: 'm12 2 10 6-10 6L2 8l10-6Z M2 12l10 6 10-6 M2 16l10 6 10-6',
    external: 'M14 3h7v7 m0-7-11 11 M10 3H3v18h18v-7',
  };
</script>
<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width={name === 'more' ? 3 : 1.7} stroke-linecap="round" stroke-linejoin="round" class={className} aria-hidden="true">
  {#if name === 'settings'}
    <path d="m9 3-.6 2.1-1.9 1.1-2.1-.5-2 3.5L4 10.8v2.4l-1.6 1.6 2 3.5 2.1-.5 1.9 1.1L9 21h6l.6-2.1 1.9-1.1 2.1.5 2-3.5-1.6-1.6v-2.4l1.6-1.6-2-3.5-2.1.5-1.9-1.1L15 3H9Z" /><circle cx="12" cy="12" r="3" />
  {:else}<path d={paths[name] ?? paths.image} />{/if}
</svg>
