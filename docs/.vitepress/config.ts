import { defineConfig } from "vitepress";
import { searchForWorkspaceRoot } from "vite";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

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

export default defineConfig({
  title: "AIX",
  description: "Official documentation and package lab for the AIX file format.",
  base,
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
    "zh-CN": { label: "简体中文", lang: "zh-CN", themeConfig: {
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
        outline: { label: "本页目录", level: "deep" },
        docFooter: { prev: "上一页", next: "下一页" },
        footer: { message: "AIX 文件格式文档与在线体验。", copyright: "jsar-project/aix 项目" }
      }, link: "/?lang=zh-CN" }
  },
  lastUpdated: true,
  cleanUrls: true,
  head: [
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
      options: { locales: { "zh-CN": { translations: { button: { buttonText: "搜索", buttonAriaLabel: "搜索文档" } } } } }
    }
  }
});
