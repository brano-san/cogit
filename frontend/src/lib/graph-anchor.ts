/** Where the list scrolls to when the panel or its rows change height (#13): the row at the
    top stays at the top, down to the part of it already scrolled off. Only the end of the
    history can move it, when there are no rows left to fill the space below. */
export function anchoredScrollTop(
  before: { scrollTop: number; rowHeight: number },
  after: { rowHeight: number; viewportHeight: number; totalRows: number },
): number {
  const wanted = (before.scrollTop / before.rowHeight) * after.rowHeight;
  const furthest = Math.max(after.totalRows * after.rowHeight - after.viewportHeight, 0);
  return Math.min(Math.max(wanted, 0), furthest);
}
