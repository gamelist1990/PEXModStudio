export function toggleModSelection(
  current: ReadonlySet<string>,
  id: string,
  visible: readonly string[],
  anchor: string | undefined,
  range: boolean,
): Set<string> {
  const next = new Set(current);
  const start = anchor ? visible.indexOf(anchor) : -1;
  const end = visible.indexOf(id);
  if (range && start >= 0 && end >= 0) {
    const select = !current.has(id);
    for (const item of visible.slice(
      Math.min(start, end),
      Math.max(start, end) + 1,
    )) {
      if (select) next.add(item);
      else next.delete(item);
    }
  } else if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}
export function toggleVisibleSelection(
  current: ReadonlySet<string>,
  visible: readonly string[],
): Set<string> {
  const next = new Set(current);
  const clear = visible.length > 0 && visible.every((id) => next.has(id));
  for (const id of visible) {
    if (clear) next.delete(id);
    else next.add(id);
  }
  return next;
}
