<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import TreeNodeItem, { type TreeNodeData } from "./TreeNodeItem.vue";

const props = defineProps<{ locale?: "en" | "zh-CN" }>();
const isZh = computed(() => props.locale === "zh-CN");

const tree: TreeNodeData = {
  id: "root",
  name: "example.aix",
  kind: "directory",
  role: "The packaged artifact itself. Readers start here and then walk into metadata, pages, and assets.",
  carries: ["Archive entries", "Directory hierarchy", "Packaged application boundary"],
  downstream: "Everything resolved by `aix`, `aix-cli`, and `aix-web` begins from this root artifact.",
  defaultExpanded: true,
  children: [
    {
      id: "version",
      name: "VERSION",
      kind: "file",
      role: "Carries the package version identifier generated during packaging.",
      carries: ["Version UUID", "Package identity"],
      downstream: "Shown by package readers as top-level package metadata."
    },
    {
      id: "app-json",
      name: "app.json",
      kind: "file",
      role: "Declares app-level configuration and the page list that defines the package surface.",
      carries: ["Page routes", "Window config", "Top-level package intent"],
      downstream: "Used to resolve the package title and the list of pages to inspect."
    },
    {
      id: "pages",
      name: "pages",
      kind: "directory",
      role: "Contains page-level files that define UI structure, metadata, and schema-bearing surfaces.",
      carries: ["Page config", "Layout files", "Schema sources"],
      downstream: "Page readers walk this subtree to derive page info and tool-facing contracts.",
      defaultExpanded: true,
      children: [
        {
          id: "index",
          name: "index",
          kind: "directory",
          role: "Represents a traditional multi-file page entry.",
          carries: ["Metadata", "Template", "Style"],
          downstream: "Combined into page summaries and layout constraints.",
          defaultExpanded: true,
          children: [
            {
              id: "index-json",
              name: "index.json",
              kind: "file",
              role: "Page metadata and schema source for the `index` page.",
              carries: ["navigationBarTitleText", "schema.data", "description"],
              downstream: "Used to derive page titles and tool parameters."
            },
            {
              id: "index-wxml",
              name: "index.wxml",
              kind: "file",
              role: "Template markup for the page.",
              carries: ["Layout structure", "Element tree"],
              downstream: "Feeds the page analyzer when layout constraints are computed."
            },
            {
              id: "index-wxss",
              name: "index.wxss",
              kind: "file",
              role: "Style layer for the page.",
              carries: ["Sizing", "Presentation rules"],
              downstream: "Combined with markup to estimate page width and height."
            }
          ]
        },
        {
          id: "detail-ink",
          name: "detail.ink",
          kind: "file",
          role: "Single-file component variant that can carry config, template, and style together.",
          carries: ["Inline config", "Template", "Style"],
          downstream: "Parsed as one source and still resolved into page info and layout data."
        }
      ]
    },
    {
      id: "assets",
      name: "assets",
      kind: "directory",
      role: "Static resources referenced by the packaged application.",
      carries: ["Icons", "Illustrations", "Other bundled assets"],
      downstream: "Packaged as readable entries and surfaced by file browsers in the lab.",
      children: [
        {
          id: "icon-svg",
          name: "icon.svg",
          kind: "file",
          role: "Example static asset bundled into the package.",
          carries: ["Static resource bytes"],
          downstream: "Can be listed and previewed as a concrete package entry."
        }
      ]
    }
  ]
};

function collectDefaults(node: TreeNodeData, target: Record<string, boolean>) {
  target[node.id] = Boolean(node.defaultExpanded);
  node.children?.forEach((child) => collectDefaults(child, target));
}

function flatten(node: TreeNodeData): TreeNodeData[] {
  return [node, ...(node.children?.flatMap(flatten) ?? [])];
}

const expanded = reactive<Record<string, boolean>>({});
collectDefaults(tree, expanded);

const allNodes = flatten(tree);
const selectedId = ref("app-json");

const selectedNode = computed(
  () => allNodes.find((node) => node.id === selectedId.value) ?? tree
);

const selectedDisplay = computed(() => {
  const node = selectedNode.value;
  if (!isZh.value) return node;
  const translations: Record<string, { role: string; carries: string[]; downstream: string }> = {
    root: { role: "归档本身。阅读器从这里开始，依次进入元数据、页面和资源。", carries: ["归档条目", "目录层级", "应用包边界"], downstream: "所有 AIX 解析都从这个根归档开始。" },
    version: { role: "保存打包时生成的包版本标识。", carries: ["版本 UUID", "包身份"], downstream: "作为顶层包元数据展示。" },
    "app-json": { role: "声明应用级配置和定义包界面的页面列表。", carries: ["页面路由", "窗口配置", "顶层包意图"], downstream: "用于解析应用标题和待检查的页面列表。" },
    pages: { role: "包含定义界面结构、元数据和 Schema 的页面文件。", carries: ["页面配置", "布局文件", "Schema 来源"], downstream: "页面读取器遍历此目录，生成页面信息和工具契约。" },
    index: { role: "表示一个传统的多文件页面入口。", carries: ["元数据", "模板", "样式"], downstream: "组合为页面摘要和布局约束。" },
    "index-json": { role: "index 页面的元数据和 Schema 来源。", carries: ["navigationBarTitleText", "schema.data", "description"], downstream: "用于生成页面标题和工具参数。" },
    "index-wxml": { role: "页面的模板标记。", carries: ["布局结构", "元素树"], downstream: "页面分析器据此计算布局约束。" },
    "index-wxss": { role: "页面的样式层。", carries: ["尺寸", "表现规则"], downstream: "与模板合并后估算页面宽高。" },
    "detail-ink": { role: "可同时携带配置、模板和样式的单文件组件。", carries: ["内联配置", "模板", "样式"], downstream: "作为单一来源解析为页面信息和布局数据。" },
    assets: { role: "应用引用的静态资源。", carries: ["图标", "插图", "其他资源"], downstream: "作为可读取条目打包，并在文件浏览器中展示。" },
    "icon-svg": { role: "包内的示例静态资源。", carries: ["静态资源字节"], downstream: "可以作为具体包条目列出和预览。" }
  };
  return { ...node, ...(translations[node.id] ?? {}) };
});

function toggleNode(id: string) {
  expanded[id] = !expanded[id];
}

function selectNode(node: TreeNodeData) {
  selectedId.value = node.id;
}
</script>

<template>
  <div class="aix-tree-demo">
    <div class="aix-tree-shell">
      <div class="aix-tree-header">
        <strong>{{ isZh ? "包结构示例" : "Package Structure Demo" }}</strong>
        <span>{{ isZh ? "展示 `.aix` 归档如何保持可读。" : "Interactive example of how an `.aix` artifact stays readable." }}</span>
      </div>

      <div class="aix-tree-layout">
        <div class="aix-tree-browser">
          <ul class="aix-tree-list">
            <TreeNodeItem
              :node="tree"
              :depth="0"
              :selected-id="selectedId"
              :expanded="expanded"
              @toggle="toggleNode"
              @select="selectNode"
            />
          </ul>
        </div>

        <aside class="aix-tree-detail">
          <p class="aix-tree-detail-label">{{ isZh ? "当前节点" : "Selected node" }}</p>
          <h3>{{ selectedDisplay.name }}</h3>
          <p>{{ selectedDisplay.role }}</p>

          <div class="aix-tree-detail-block">
            <strong>{{ isZh ? "包含内容" : "Carries" }}</strong>
            <ul>
              <li v-for="item in selectedDisplay.carries" :key="item">{{ item }}</li>
            </ul>
          </div>

          <div class="aix-tree-detail-block">
            <strong>{{ isZh ? "下游用途" : "Downstream usage" }}</strong>
            <p>{{ selectedDisplay.downstream }}</p>
          </div>
        </aside>
      </div>
    </div>
  </div>
</template>
