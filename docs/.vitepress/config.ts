import { defineConfig } from "vitepress";
import { searchForWorkspaceRoot } from "vite";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";
import { apiReferenceLinks } from "./api-reference-links";

function normalizeBasePath(input: string | undefined): string {
  if (!input || input === "/") {
    return "/";
  }

  const withLeadingSlash = input.startsWith("/") ? input : `/${input}`;
  return withLeadingSlash.endsWith("/")
    ? withLeadingSlash
    : `${withLeadingSlash}/`;
}

const runtimeProcess = globalThis as typeof globalThis & {
  process?: { env?: Record<string, string | undefined> };
};

const base = normalizeBasePath(runtimeProcess.process?.env?.BASE_PATH ?? "/aix");
const docsRoot = new URL("..", import.meta.url).pathname;
const workspaceRoot = searchForWorkspaceRoot(docsRoot);
const aixWebRoot = new URL("../../crates/aix-web/", import.meta.url).pathname;

function queryLanguageBootstrap(basePath: string): string {
  return `(() => {
    // VitePress injects configured head scripts reactively in dev. Running this
    // redirect there would fight the query-language router and create a loop.
    if (document.querySelector('script[src*="/@vite/client"]')) return;
    const base = ${JSON.stringify(basePath)};
    const url = new URL(window.location.href);
    const locale = url.searchParams.get("lang");
    if (locale !== "zh-CN" && locale !== "ja") return;
    if (url.pathname.startsWith(base + locale + "/")) return;
    if (!url.pathname.startsWith(base)) return;
    const pagePath = url.pathname.slice(base.length);
    url.pathname = base + locale + "/" + pagePath;
    url.searchParams.delete("lang");
    window.location.replace(url.pathname + url.search + url.hash);
  })();`;
}

export default defineConfig({
  title: "AIX",
  description: "Official documentation and package lab for the AIX file format.",
  base,
  markdown: { config: apiReferenceLinks },
  vite: {
    plugins: [wasm(), topLevelAwait()],
    server: {
      port: 5174,
      fs: {
        allow: [workspaceRoot, aixWebRoot]
      }
    }
  },
  appearance: true,
  locales: {
    root: { label: "English", lang: "en", link: "/?lang=en" },
    "zh-CN": {
      label: "简体中文",
      lang: "zh-CN",
      themeConfig: {
        nav: [
          { text: "规范", link: "/spec?lang=zh-CN", activeMatch: "/spec" },
          { text: "命令行", link: "/cli?lang=zh-CN", activeMatch: "/cli" },
          { text: "API", link: "/api?lang=zh-CN", activeMatch: "/api" },
          { text: "体验", link: "/play?lang=zh-CN", activeMatch: "/play" },
          { text: "GitHub", link: "https://github.com/jsar-project/aix" }
        ],
        sidebar: {
          "/zh-CN/": [
            {
              text: "AIX",
              items: [
                { text: "规范", link: "/zh-CN/spec" },
                { text: "命令行", link: "/zh-CN/cli" },
                { text: "API", link: "/zh-CN/api" }
              ]
            }
          ]
        },
        langMenuLabel: "语言",
        darkModeSwitchLabel: "外观",
        lightModeSwitchTitle: "切换到浅色主题",
        darkModeSwitchTitle: "切换到深色主题",
        sidebarMenuLabel: "菜单",
        returnToTopLabel: "返回顶部",
        skipToContentLabel: "跳转到正文",
        outline: { label: "本页目录", level: "deep" },
        lastUpdated: { text: "最后更新" },
        docFooter: { prev: "上一页", next: "下一页" },
        footer: { message: "AIX 文件格式文档与在线体验。", copyright: "jsar-project/aix 项目" }
      },
      link: "/?lang=zh-CN"
    },
    ja: {
      label: "日本語",
      lang: "ja",
      themeConfig: {
        nav: [
          { text: "仕様", link: "/spec?lang=ja", activeMatch: "/spec" },
          { text: "CLI", link: "/cli?lang=ja", activeMatch: "/cli" },
          { text: "API", link: "/api?lang=ja", activeMatch: "/api" },
          { text: "プレイ", link: "/play?lang=ja", activeMatch: "/play" },
          { text: "GitHub", link: "https://github.com/jsar-project/aix" }
        ],
        sidebar: {
          "/ja/": [{ text: "AIX", items: [
            { text: "仕様", link: "/spec?lang=ja" },
            { text: "CLI", link: "/cli?lang=ja" },
            { text: "API", link: "/api?lang=ja" }
          ] }]
        },
        langMenuLabel: "言語",
        darkModeSwitchLabel: "外観",
        lightModeSwitchTitle: "ライトテーマに切り替える",
        darkModeSwitchTitle: "ダークテーマに切り替える",
        sidebarMenuLabel: "メニュー",
        returnToTopLabel: "ページ上部へ戻る",
        skipToContentLabel: "本文へ移動",
        outline: { label: "このページの内容", level: "deep" },
        lastUpdated: { text: "最終更新" },
        docFooter: { prev: "前のページ", next: "次のページ" },
        footer: { message: "AIX ファイル形式のドキュメントとオンラインツール。", copyright: "jsar-project/aix プロジェクト" }
      },
      link: "/?lang=ja"
    }
  },
  lastUpdated: true,
  cleanUrls: true,
  head: [
    ["script", {}, queryLanguageBootstrap(base)],
    ["meta", { name: "theme-color", content: "#f4f1e9" }],
    ["meta", { property: "og:title", content: "AIX" }],
    [
      "meta",
      {
        property: "og:description",
        content: "AIX is a file format for package structure, page schema, and tool surfaces."
      }
    ]
  ],
  themeConfig: {
    i18nRouting: false,
    siteTitle: '<span class="aix-brand-aiui">AIUI</span> <span class="aix-brand-aix">AIX</span>',
    nav: [
      { text: "Specification", link: "/spec" },
      { text: "CLI", link: "/cli" },
      { text: "API", link: "/api" },
      { text: "Play", link: "/play" },
      { text: "GitHub", link: "https://github.com/jsar-project/aix" }
    ],
    sidebar: [{ text: "AIX", items: [{ text: "Specification", link: "/spec" }, { text: "CLI", link: "/cli" }, { text: "API", link: "/api" }] }],
    docFooter: {
      prev: "Previous",
      next: "Next"
    },
    footer: {
      message: "AIX file format documentation and package lab.",
      copyright: "Released for the jsar-project/aix repository."
    },
    socialLinks: [
      { icon: "github", link: "https://github.com/jsar-project/aix" }
    ],
    outline: "deep",
    search: {
      provider: "local",
      options: { locales: {
        "zh-CN": { translations: { button: { buttonText: "搜索", buttonAriaLabel: "搜索文档" } } },
        ja: { translations: { button: { buttonText: "検索", buttonAriaLabel: "ドキュメントを検索" } } }
      } }
    }
  }
});
