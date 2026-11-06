import { markRaw, nextTick } from "vue";
import type { Router } from "vitepress";
import { restoreApiReferenceTarget } from "./api-reference-navigation";

// Markdown remains the single source of truth. Both languages are compiled
// by VitePress; query navigation selects a module, never another public route.
const pages = import.meta.glob(["../../*.md", "../../zh-CN/*.md", "../../ja/*.md"]);
const base = import.meta.env.BASE_URL;

function pagePath(url: URL) {
  return url.pathname.slice(base.length).replace(/^(?:zh-CN|ja)\/?/, "")
    .replace(/(?:^|\/)index(?:\.html)?$/, "").replace(/\.html$/, "").replace(/\/$/, "");
}

function language(url: URL) {
  const value = url.searchParams.get("lang");
  if (value === "zh-CN" || value === "ja") return value;
  if (url.pathname.startsWith(`${base}zh-CN/`)) return "zh-CN";
  if (url.pathname.startsWith(`${base}ja/`)) return "ja";
  return "en";
}

function canonical(url: URL, lang = language(url)) {
  url.pathname = `${base}${pagePath(url)}`;
  url.searchParams.set("lang", lang);
  return `${url.pathname}${url.search}${url.hash}`;
}

export function installQueryLanguage(router: Router) {
  const moduleFor = (url: URL) => {
    const locale = language(url);
    const localized = pages[`../../${locale === "en" ? "" : `${locale}/`}${pagePath(url) || "index"}.md`];
    return localized ?? pages[`../../${pagePath(url) || "index"}.md`];
  };

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
      const locale = language(url);
      const pageData = locale === "ja" && !page.__pageData.relativePath.startsWith("ja/")
        ? { ...page.__pageData, relativePath: `ja/${pagePath(url) || "index"}.md` }
        : page.__pageData;
      router.route.data = markRaw(pageData);
    }
    history.replaceState(history.state, "", canonical(url));
    await nextTick();
    restoreApiReferenceTarget();
    rewriteLinks();
    rewriteLocaleLabels();
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
        || ["English", "简体中文", "日本語"].includes(link.textContent?.trim() || "");
      if (isLanguageOption) {
        const selected = language(url);
        const target = new URL(current);
        target.searchParams.set("lang", selected);
        const next = canonical(target, selected);
        if (href !== next) link.setAttribute("href", next);
        // A locale change must start from the matching static HTML document.
        // GitHub Pages cannot select that document from a query parameter, so
        // force a document navigation and let the bootstrap script resolve it.
        link.setAttribute("target", "_self");
      } else {
        const next = canonical(url, url.searchParams.has("lang") || /\/(?:zh-CN|ja)\//.test(url.pathname) ? language(url) : language(current));
        if (href !== next) link.setAttribute("href", next);
      }
    }
  }

  // VitePress 1.6 still hard-codes a few English accessibility labels and the
  // code-copy tooltip. Keep those details aligned with the selected locale.
  function rewriteLocaleLabels() {
    const locale = language(new URL(location.href));
    if (locale === "en") return;

    const labels = locale === "zh-CN" ? {
      copy: "复制代码", main: "主导航", sidebar: "侧边栏导航", pager: "文档分页",
      mobile: "移动端导航", extra: "更多导航", permalink: (heading: string) => `“${heading}”的永久链接`
    } : {
      copy: "コードをコピー", main: "メインナビゲーション", sidebar: "サイドバーナビゲーション", pager: "ページ送り",
      mobile: "モバイルナビゲーション", extra: "その他のナビゲーション", permalink: (heading: string) => `「${heading}」へのパーマリンク`
    };

    for (const button of document.querySelectorAll<HTMLButtonElement>("button.copy[title]")) {
      if (button.title !== labels.copy) button.title = labels.copy;
    }

    const textLabels = [
      ["#main-nav-aria-label", labels.main], ["#sidebar-aria-label", labels.sidebar], ["#doc-footer-aria-label", labels.pager]
    ] as const;
    for (const [selector, label] of textLabels) {
      const element = document.querySelector<HTMLElement>(selector);
      if (element && element.textContent?.trim() !== label) element.textContent = label;
    }

    const attributeLabels = [
      [".VPNavBarHamburger", labels.mobile], [".VPNavBarExtra > button", labels.extra]
    ] as const;
    for (const [selector, label] of attributeLabels) {
      const element = document.querySelector<HTMLElement>(selector);
      if (element?.getAttribute("aria-label") !== label) element?.setAttribute("aria-label", label);
    }

    for (const anchor of document.querySelectorAll<HTMLAnchorElement>(".header-anchor[aria-label]")) {
      const heading = anchor.parentElement?.textContent?.trim();
      const label = heading ? labels.permalink(heading) : labels.permalink("");
      if (anchor.getAttribute("aria-label") !== label) anchor.setAttribute("aria-label", label);
    }
  }

  const observer = new MutationObserver(() => {
    rewriteLinks();
    rewriteLocaleLabels();
  });
  observer.observe(document.documentElement, { childList: true, subtree: true, attributes: true, attributeFilter: ["href"] });
  if (import.meta.hot) import.meta.hot.dispose(() => observer.disconnect());
}
