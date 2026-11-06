import { getScrollOffset } from "vitepress";

function referenceTarget(hash: string) {
  try {
    return document.getElementById(decodeURIComponent(hash.slice(1)));
  } catch {
    return null;
  }
}

/** Reveal a linked type even when its definition is in a closed details group. */
export function revealApiReferenceTarget(hash = location.hash) {
  const target = referenceTarget(hash);
  if (!target || (!target.id.startsWith("type-") && target.id !== "aix-instance")) return;
  for (let parent = target?.parentElement; parent; parent = parent.parentElement) {
    if (parent instanceof HTMLDetailsElement) parent.open = true;
  }
  return target;
}

/** Locale components may render after the router's initial hash scroll. */
export function restoreApiReferenceTarget() {
  const hash = location.hash;
  const target = revealApiReferenceTarget(hash);
  if (!target) return;
  requestAnimationFrame(() => {
    if (target.isConnected && location.hash === hash) {
      window.scrollTo(0, window.scrollY + target.getBoundingClientRect().top - getScrollOffset());
    }
  });
}

export function installApiReferenceNavigation() {
  const onClick = (event: MouseEvent) => {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    const link = event.target instanceof Element
      ? event.target.closest<HTMLAnchorElement>("a.api-reference-link")
      : null;
    const hash = link?.getAttribute("href");
    // Restore after revealing, including when the current hash is clicked again.
    if (hash?.startsWith("#")) {
      revealApiReferenceTarget(hash);
      const target = referenceTarget(hash);
      if (target) {
        target.tabIndex = -1;
        target.focus({ preventScroll: true });
      }
      restoreApiReferenceTarget();
    }
  };
  const onHashChange = () => revealApiReferenceTarget();
  document.addEventListener("click", onClick, true);
  window.addEventListener("hashchange", onHashChange);
  if (import.meta.hot) import.meta.hot.dispose(() => {
    document.removeEventListener("click", onClick, true);
    window.removeEventListener("hashchange", onHashChange);
  });
}

