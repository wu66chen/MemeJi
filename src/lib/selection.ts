export interface SelectionModifiers { range: boolean; toggle: boolean }
export function selectIds(visible: number[], selected: number[], anchor: number | null, clicked: number, modifiers: SelectionModifiers): { ids: number[]; anchor: number | null } {
  const visibleSet = new Set(visible);
  const current = selected.filter((id) => visibleSet.has(id));
  const index = visible.indexOf(clicked);
  if (index < 0) return { ids: current, anchor: visibleSet.has(anchor ?? -1) ? anchor : null };
  const start = anchor === null ? -1 : visible.indexOf(anchor);
  if (modifiers.range && start >= 0) {
    const range = visible.slice(Math.min(start, index), Math.max(start, index) + 1);
    return { ids: modifiers.toggle ? [...new Set([...current, ...range])] : range, anchor };
  }
  return {
    ids: modifiers.toggle ? (current.includes(clicked) ? current.filter((id) => id !== clicked) : [...current, clicked]) : [clicked],
    anchor: clicked,
  };
}

export function sourceCollectionId(view: { kind: string; id?: number }, query: string, globalSearch: boolean): number | null {
  return view.kind === 'collection' && !(query.trim() && globalSearch) ? view.id ?? null : null;
}
