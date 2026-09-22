import { markRaw, nextTick } from "vue";
import type { Router } from "vitepress";

// Markdown remains the single source of truth. Both languages are compiled
// by VitePress; query navigation selects a module, never another public route.
const pages = import.meta.glob(["../../*.md", "../../zh-CN/*.md"]);
const base = import.meta.env.BASE_URL;

function pagePath(url: URL) {
  return url.pathname.slice(base.length).replace(/^zh-CN\/?/, "")
    .replace(/(?:^|\/)index(?:\.html)?$/, "").replace(/\.html$/, "").replace(/\/$/, "");
}

function language(url: URL) {
  const value = url.searchParams.get("lang");
  return value === "zh-CN" || (!value && url.pathname.startsWith(`${base}zh-CN/`))
    ? "zh-CN" : "en";
}

function canonical(url: URL, lang = language(url)) {
  url.pathname = `${base}${pagePath(url)}`;
  url.searchParams.set("lang", lang);
  return `${url.pathname}${url.search}${url.hash}`;
}

export function installQueryLanguage(router: Router) {
  const moduleFor = (url: URL) => pages[`../../${language(url) === "zh-CN" ? "zh-CN/" : ""}${pagePath(url) || "index"}.md`];

  // Preload before VitePress updates the route so the other language does not
  // briefly render during client-side navigation. Modules are cached by ESM.
  router.onBeforePageLoad = async (to) => {
    await moduleFor(new URL(to, location.href))?.();
  };
  router.onAfterRouteChange = async (to) => {
    const url = new URL(to, location.href);
    const load = moduleFor(url);
    if (load) {
      const page = await load() as { default: NonNullable<Router["route"]["component"]>; __pageData: Router["route"]["data"] };
      // Ignore a module if a newer navigation won the race.
      if (new URL(location.href).pathname !== url.pathname || location.search !== url.search) return;
      router.route.component = markRaw(page.default);
      router.route.data = markRaw(page.__pageData);
    }
    history.replaceState(history.state, "", canonical(url));
    await nextTick();
    rewriteLinks();
  };

  // Default-theme links (including search results and the mobile menu) are
  // generated from locale metadata. Expose only query URLs, including for
  // open-in-new-tab/copy-link, while retaining locale metadata for the theme.
  function rewriteLinks() {
    const current = new URL(location.href);
    for (const link of document.querySelectorAll<HTMLAnchorElement>("#app a[href]")) {
      const href = link.getAttribute("href")!;
      if (href.startsWith("#") || link.hasAttribute("download")) continue;
      const url = new URL(href, current);
      if (url.origin !== current.origin || !url.pathname.startsWith(base)) continue;
      if (!pages[`../../${pagePath(url) || "index"}.md`]) continue;
      const isLanguageOption = Boolean(link.closest(".VPNavBarTranslations, .VPNavScreenTranslations"))
        || ["English", "简体中文"].includes(link.textContent?.trim() || "");
      if (isLanguageOption) {
        const selected = language(url);
        const target = new URL(current);
        target.searchParams.set("lang", selected);
        const next = canonical(target, selected);
        if (href !== next) link.setAttribute("href", next);
      } else {
        const next = canonical(url, url.searchParams.has("lang") || url.pathname.startsWith(`${base}zh-CN/`) ? language(url) : language(current));
        if (href !== next) link.setAttribute("href", next);
      }
    }
  }
  const observer = new MutationObserver(rewriteLinks);
  observer.observe(document.documentElement, { childList: true, subtree: true, attributes: true, attributeFilter: ["href"] });
  if (import.meta.hot) import.meta.hot.dispose(() => observer.disconnect());
}
