/** Um único listener para todos os cards, inclusive os criados após a montagem. */
export function installCardSpotlight() {
  const fine = matchMedia('(hover: hover) and (pointer: fine)');
  const reduced = matchMedia('(prefers-reduced-motion: reduce)');
  let active: HTMLElement | null = null;
  let frame: number | null = null;
  let x = 0;
  let y = 0;

  const clear = () => {
    if (frame !== null) cancelAnimationFrame(frame);
    frame = null;
    active?.removeAttribute('data-spotlight');
    active?.style.removeProperty('--el-spotlight-x');
    active?.style.removeProperty('--el-spotlight-y');
    active = null;
  };
  const move = (event: PointerEvent) => {
    const card = fine.matches && !reduced.matches && event.pointerType === 'mouse' && event.target instanceof Element
      ? event.target.closest<HTMLElement>('.card') : null;
    if (!card || card.closest('.app-sidebar, .app-navigation, .app-toolbar')) { clear(); return; }
    if (card !== active) { clear(); active = card; }
    x = event.clientX;
    y = event.clientY;
    if (frame !== null) return;
    frame = requestAnimationFrame(() => {
      frame = null;
      if (!active?.isConnected) { clear(); return; }
      const rect = active.getBoundingClientRect();
      active.style.setProperty('--el-spotlight-x', `${x - rect.left}px`);
      active.style.setProperty('--el-spotlight-y', `${y - rect.top}px`);
      active.setAttribute('data-spotlight', '');
    });
  };
  const out = (event: PointerEvent) => {
    if (!(event.relatedTarget instanceof Node) || !active?.contains(event.relatedTarget)) clear();
  };
  document.addEventListener('pointermove', move, { passive: true });
  document.addEventListener('pointerout', out, { passive: true });
  document.addEventListener('scroll', clear, true);
  document.addEventListener('visibilitychange', clear);
  window.addEventListener('blur', clear);
  fine.addEventListener('change', clear);
  reduced.addEventListener('change', clear);
  return () => {
    clear();
    document.removeEventListener('pointermove', move);
    document.removeEventListener('pointerout', out);
    document.removeEventListener('scroll', clear, true);
    document.removeEventListener('visibilitychange', clear);
    window.removeEventListener('blur', clear);
    fine.removeEventListener('change', clear);
    reduced.removeEventListener('change', clear);
  };
}
