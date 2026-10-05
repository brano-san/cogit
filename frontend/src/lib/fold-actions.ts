/** Room the wide buttons must have beyond the minimum before they come back, so a header at
    the boundary does not flip on every pixel of a resize. */
export const FOLD_HYSTERESIS_PX = 8;

/** Folded into one menu button when the buttons need more than the header has left; back
    only with the margin above. */
export function foldDecision(available: number, needed: number, folded: boolean): boolean {
  return folded ? available < needed + FOLD_HYSTERESIS_PX : available < needed;
}

/** A panel header's wide buttons, measured as rendered (fonts loaded, gaps and padding in):
    `onfold(true)` when they do not fit beside the title. The group stays laid out while folded
    (hidden, out of flow) so its width is always known. */
export function foldActions(group: HTMLElement, onfold: (folded: boolean) => void) {
  let folded = false;
  let frame = 0;
  const header = group.closest<HTMLElement>(".panel-header");
  const measure = () => {
    frame = 0;
    const title = header?.querySelector<HTMLElement>(".panel-title");
    if (!header || !title) return;
    const style = getComputedStyle(header);
    const pad = parseFloat(style.paddingLeft) + parseFloat(style.paddingRight);
    const gap = parseFloat(style.columnGap) || 0;
    const available = header.clientWidth - pad - title.scrollWidth - gap;
    const next = foldDecision(available, group.scrollWidth, folded);
    if (next !== folded) {
      folded = next;
      onfold(folded);
    }
  };
  const later = () => {
    if (frame === 0) frame = requestAnimationFrame(measure);
  };
  const observer = new ResizeObserver(later);
  if (header) observer.observe(header);
  observer.observe(group);
  void document.fonts?.ready.then(later);
  measure();
  return {
    destroy() {
      observer.disconnect();
      cancelAnimationFrame(frame);
    },
  };
}
