<script setup lang="ts">
import { computed, ref } from "vue";
import type { AIX as AixClass, AixEntry, AixInputFile, OptimizeReport } from "@yodaos-pkg/aix";

interface LabPageInfo {
  name: string;
  title?: string;
  description?: string;
  data_schema?: Record<string, unknown>;
  size: {
    width: number;
    height: number;
  };
}

interface LabTool {
  type: string;
  target: string;
  layout?: string;
  function: {
    name: string;
    description?: string;
    parameters: Record<string, unknown>;
  };
}

type SecondaryTab = "meta" | "pages" | "tools";
type LabMode = "inspect" | "build";

function formatBytes(value: number): string {
  if (value < 1024) {
    return `${value} B`;
  }

  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KB`;
  }

  return `${(value / (1024 * 1024)).toFixed(2)} MB`;
}

const entries = ref<AixEntry[]>([]);
const version = ref<string | null>(null);
const title = ref<string | null>(null);
const pages = ref<LabPageInfo[]>([]);
const tools = ref<LabTool[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const selectedFile = ref<string | null>(null);
const fileContent = ref<string | null>(null);
const aixInstance = ref<AixClass | null>(null);
const currentTab = ref<SecondaryTab>("meta");
const mode = ref<LabMode>("inspect");
const directoryFiles = ref<File[]>([]);
const directoryName = ref("");
const building = ref(false);
const buildError = ref<string | null>(null);
const buildReport = ref<OptimizeReport | null>(null);
const optimizeBuild = ref(true);
const optimizeLevel = ref<1 | 2 | 3>(2);
const locale = ref<"en" | "zh-CN">("en");

const text = computed(() => locale.value === "zh-CN" ? {
  play: "体验",
  hint: "在浏览器中检查和构建 AIX 包。",
  inspect: "检查",
  build: "构建",
  reading: "正在读取包…",
  replacePackage: "替换包",
  uploadPackage: "上传包",
  replaceDirectory: "替换目录",
  chooseDirectory: "选择目录",
  error: "错误：",
  noDirectory: "未选择目录",
  directory: "目录",
  files: "文件",
  sourceSize: "源文件大小",
  optimize: "优化资源",
  level: "级别",
  building: "正在构建包…",
  buildDownload: "构建并下载",
  downloaded: "包已下载",
  saved: "已节省",
  noPackage: "尚未加载包",
  compressed: "压缩后",
  preview: "预览",
  notPreviewable: "该文件不是可预览的 UTF-8 文本。",
  noPreview: "未选择可预览的文件。",
  details: "包详情",
  meta: "元数据",
  pages: "页面",
  tools: "工具",
  untitled: "未命名页面",
  schema: "Schema",
  noSchema: "无 Schema",
  noPages: "没有页面。",
  noTools: "没有工具。",
  title: "标题",
  version: "版本",
  entries: "条目",
  unknown: "未知"
} : {
  play: "Play",
  hint: "Inspect and build AIX packages in the browser.",
  inspect: "Inspect",
  build: "Build",
  reading: "Reading package…",
  replacePackage: "Replace package",
  uploadPackage: "Upload package",
  replaceDirectory: "Replace directory",
  chooseDirectory: "Choose directory",
  error: "Error:",
  noDirectory: "No directory selected",
  directory: "Directory",
  files: "Files",
  sourceSize: "Source size",
  optimize: "Optimize resources",
  level: "Level",
  building: "Building package…",
  buildDownload: "Build and download",
  downloaded: "Package downloaded",
  saved: "saved",
  noPackage: "No package loaded",
  compressed: "compressed",
  preview: "Preview",
  notPreviewable: "This file is not previewable as UTF-8 text.",
  noPreview: "No previewable file was selected.",
  details: "Package details",
  meta: "Meta",
  pages: "Pages",
  tools: "Tools",
  untitled: "Untitled page",
  schema: "Schema",
  noSchema: "No schema",
  noPages: "No pages.",
  noTools: "No tools.",
  title: "Title",
  version: "Version",
  entries: "Entries",
  unknown: "Unknown"
});

const TEXT_FILE_PATTERN =
  /\.(md|txt|json|js|ts|jsx|tsx|css|html|xml|yaml|yml|toml|ini|cfg|ink|wxml|wxss|wcss|svg)$/i;

const hasPackage = computed(() => entries.value.length > 0);
const selectedEntry = computed(() => entries.value.find((entry) => entry.name === selectedFile.value) ?? null);
const packageLabel = computed(() => title.value ?? "No package loaded");
const packageStats = computed(() => [
  { label: text.value.entries, value: String(entries.value.length) },
  { label: text.value.pages, value: String(pages.value.length) },
  { label: text.value.tools, value: String(tools.value.length) },
  { label: text.value.version, value: version.value ?? text.value.unknown }
]);
const metadataRows = computed(() => [
  { label: text.value.title, value: title.value ?? text.value.untitled },
  { label: text.value.version, value: version.value ?? text.value.unknown },
  { label: text.value.entries, value: String(entries.value.length) },
  { label: text.value.pages, value: String(pages.value.length) },
  { label: text.value.tools, value: String(tools.value.length) }
]);
const directorySize = computed(() => directoryFiles.value.reduce((total, file) => total + file.size, 0));
const hasDirectory = computed(() => directoryFiles.value.length > 0);

if (typeof window !== "undefined") {
  locale.value = new URLSearchParams(window.location.search).get("lang") === "zh-CN"
    || window.location.pathname.includes("/zh-CN/") ? "zh-CN" : "en";
}

function resetState() {
  error.value = null;
  entries.value = [];
  version.value = null;
  title.value = null;
  pages.value = [];
  tools.value = [];
  fileContent.value = null;
  selectedFile.value = null;
}

function decodeEntryContent(aix: AixClass, fileName: string): string | null {
  const content = aix.readFile(fileName);
  const looksTextual = TEXT_FILE_PATTERN.test(fileName) || !fileName.includes(".");

  if (!looksTextual && content.includes(0)) {
    return null;
  }

  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(content);
  } catch {
    return null;
  }
}

function selectInitialFile(aix: AixClass, nextEntries: AixEntry[]) {
  for (const entry of nextEntries) {
    const decoded = decodeEntryContent(aix, entry.name);
    if (decoded !== null) {
      selectedFile.value = entry.name;
      fileContent.value = decoded;
      return;
    }
  }

  selectedFile.value = nextEntries[0]?.name ?? null;
  fileContent.value = null;
}

async function loadAixModule() {
  return import("@yodaos-pkg/aix");
}

async function handleFileUpload(event: Event) {
  const input = event.target as HTMLInputElement | null;
  const file = input?.files?.[0];

  if (!file) {
    return;
  }

  loading.value = true;
  resetState();

  try {
    const { AIX } = await loadAixModule();
    const aix = await AIX.From(file);
    const nextEntries = aix.list();

    aixInstance.value = aix;
    entries.value = nextEntries;
    version.value = aix.getVersion() ?? "Unknown";
    title.value = aix.getTitle() ?? null;
    pages.value = aix.getPages() as unknown as LabPageInfo[];
    tools.value = aix.getTools() as unknown as LabTool[];
    currentTab.value = "meta";
    selectInitialFile(aix, nextEntries);
  } catch (err) {
    console.error("Error parsing AIX:", err);
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    loading.value = false;

    if (input) {
      input.value = "";
    }
  }
}

function handleDirectorySelection(event: Event) {
  const input = event.target as HTMLInputElement | null;
  const files = Array.from(input?.files ?? []);

  buildError.value = null;
  buildReport.value = null;
  directoryFiles.value = files;
  directoryName.value = files[0]?.webkitRelativePath.split("/")[0] || "package";
}

function packagePath(file: File): string {
  const sourcePath = file.webkitRelativePath || file.name;
  const parts = sourcePath.split("/").filter(Boolean);
  return parts.length > 1 ? parts.slice(1).join("/") : sourcePath;
}

function downloadPackage(data: Uint8Array, fileName: string) {
  const bytes = new Uint8Array(data);
  const blob = new Blob([bytes.buffer], { type: "application/zip" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = fileName;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

async function buildAndDownload() {
  if (!hasDirectory.value || building.value) {
    return;
  }

  building.value = true;
  buildError.value = null;
  buildReport.value = null;

  try {
    const { AIX } = await loadAixModule();
    const files = await Promise.all(
      directoryFiles.value
        .filter((file) => packagePath(file) !== "VERSION")
        .map(async (file): Promise<AixInputFile> => ({
          path: packagePath(file),
          data: new Uint8Array(await file.arrayBuffer())
        }))
    );
    const result = await AIX.pack(files, {
      optimize: optimizeBuild.value
        ? {
            level: optimizeLevel.value,
            json: true,
            png: true,
            jpeg: true
          }
        : false
    });
    const safeName = directoryName.value.replace(/[^a-zA-Z0-9._-]+/g, "-") || "bundle";
    downloadPackage(result.data, `${safeName}.aix`);
    buildReport.value = result.report;
  } catch (err) {
    console.error("Error building AIX:", err);
    buildError.value = err instanceof Error ? err.message : String(err);
  } finally {
    building.value = false;
  }
}

function viewFile(fileName: string) {
  if (!aixInstance.value) {
    return;
  }

  try {
    selectedFile.value = fileName;
    fileContent.value = decodeEntryContent(aixInstance.value, fileName);
  } catch (err) {
    console.error("Error reading file:", err);
    error.value = `Failed to read ${fileName}: ${String(err)}`;
  }
}

function formatTool(tool: LabTool): string {
  return JSON.stringify(
    {
      type: tool.type,
      target: tool.target,
      layout: tool.layout,
      parameters: tool.function.parameters
    },
    null,
    2
  );
}
</script>

<template>
  <div class="lab-shell">
    <section class="lab-topbar">
      <div class="lab-topbar-copy">
        <strong class="lab-topbar-title">{{ text.play }}</strong>
        <span class="lab-topbar-hint">{{ text.hint }}</span>
      </div>

      <div class="lab-mode-switch">
        <button type="button" :class="{ 'is-active': mode === 'inspect' }" @click="mode = 'inspect'">
          {{ text.inspect }}
        </button>
        <button type="button" :class="{ 'is-active': mode === 'build' }" @click="mode = 'build'">
          {{ text.build }}
        </button>
      </div>

      <label v-if="mode === 'inspect'" class="lab-upload-button" for="file-input">
        <input
          id="file-input"
          type="file"
          accept=".aix"
          class="lab-hidden-input"
          @change="handleFileUpload"
        />
        <span>{{ loading ? text.reading : hasPackage ? text.replacePackage : text.uploadPackage }}</span>
      </label>

      <label v-else class="lab-upload-button" for="directory-input">
        <input
          id="directory-input"
          type="file"
          webkitdirectory
          multiple
          class="lab-hidden-input"
          @change="handleDirectorySelection"
        />
        <span>{{ hasDirectory ? text.replaceDirectory : text.chooseDirectory }}</span>
      </label>
    </section>

    <template v-if="mode === 'build'">
      <div v-if="buildError" class="lab-error"><strong>{{ text.error }}</strong> {{ buildError }}</div>

      <section v-if="!hasDirectory" class="lab-empty-stage">
        <div class="lab-empty-stage-card">
            <strong>{{ text.noDirectory }}</strong>
        </div>
      </section>

      <section v-else class="lab-builder-panel">
        <div class="lab-builder-summary">
          <div>
            <span>{{ text.directory }}</span>
            <strong>{{ directoryName }}</strong>
          </div>
          <div>
            <span>{{ text.files }}</span>
            <strong>{{ directoryFiles.length }}</strong>
          </div>
          <div>
            <span>{{ text.sourceSize }}</span>
            <strong>{{ formatBytes(directorySize) }}</strong>
          </div>
        </div>

        <div class="lab-builder-controls">
          <label class="lab-check-control">
            <input v-model="optimizeBuild" type="checkbox" />
            <span>{{ text.optimize }}</span>
          </label>

          <label class="lab-level-control">
            <span>{{ text.level }}</span>
            <select v-model="optimizeLevel" :disabled="!optimizeBuild">
              <option :value="1">1</option>
              <option :value="2">2</option>
              <option :value="3">3</option>
            </select>
          </label>

          <button class="lab-build-button" type="button" :disabled="building" @click="buildAndDownload">
            {{ building ? text.building : text.buildDownload }}
          </button>
        </div>

        <div v-if="buildReport" class="lab-build-result" role="status">
          <strong>{{ text.downloaded }}</strong>
          <span>{{ formatBytes(buildReport.output_size) }}</span>
          <span>{{ formatBytes(buildReport.saved_bytes) }} {{ text.saved }}</span>
        </div>
      </section>
    </template>

    <template v-else>
    <div v-if="error" class="lab-error"><strong>{{ text.error }}</strong> {{ error }}</div>

    <section v-if="!hasPackage" class="lab-empty-stage">
      <div class="lab-empty-stage-card">
      <strong>{{ loading ? text.reading : text.noPackage }}</strong>
      </div>
    </section>

    <template v-else>
      <section class="lab-stats">
        <article v-for="item in packageStats" :key="item.label" class="lab-stat-card">
          <span>{{ item.label }}</span>
          <strong>{{ item.value }}</strong>
        </article>
      </section>

      <section class="lab-workspace">
        <aside class="lab-sidebar">
          <div class="lab-panel-head">
            <h2>Files</h2>
            <span class="lab-count">{{ entries.length }}</span>
          </div>
          <div class="lab-file-list">
            <button
              v-for="entry in entries"
              :key="entry.name"
              :class="['lab-file-card', selectedFile === entry.name ? 'is-active' : '']"
              type="button"
              @click="viewFile(entry.name)"
            >
              <h3>{{ entry.name }}</h3>
              <div class="lab-file-meta">
                <span>{{ formatBytes(entry.size) }}</span>
                <span>{{ text.compressed }} {{ formatBytes(entry.compressed_size) }}</span>
              </div>
            </button>
          </div>
        </aside>

        <section class="lab-preview-panel">
          <div class="lab-panel-head">
            <div>
              <h2>{{ selectedFile ?? text.preview }}</h2>
              <p v-if="selectedEntry" class="lab-panel-subtitle">
                {{ formatBytes(selectedEntry.size) }}
              </p>
            </div>
          </div>
          <pre v-if="selectedFile && fileContent !== null" class="lab-preview-code">{{ fileContent }}</pre>
          <div v-else class="lab-empty lab-empty-compact">
            <p>
              {{
                selectedFile
                  ? text.notPreviewable
                  : text.noPreview
              }}
            </p>
          </div>
        </section>
      </section>

      <section class="lab-secondary">
        <div class="lab-secondary-tabs" role="tablist" :aria-label="text.details">
          <button
            type="button"
            class="lab-tab"
            :class="{ 'is-active': currentTab === 'meta' }"
            @click="currentTab = 'meta'"
          >
            {{ text.meta }}
          </button>
          <button
            type="button"
            class="lab-tab"
            :class="{ 'is-active': currentTab === 'pages' }"
            @click="currentTab = 'pages'"
          >
            {{ text.pages }}
          </button>
          <button
            type="button"
            class="lab-tab"
            :class="{ 'is-active': currentTab === 'tools' }"
            @click="currentTab = 'tools'"
          >
            {{ text.tools }}
          </button>
        </div>

        <div v-if="currentTab === 'meta'" class="lab-secondary-panel lab-meta-grid">
          <article v-for="item in metadataRows" :key="item.label" class="lab-meta-card">
            <span>{{ item.label }}</span>
            <strong>{{ item.value }}</strong>
          </article>
        </div>

        <div v-else-if="currentTab === 'pages'" class="lab-secondary-panel lab-pages-grid">
          <article v-for="page in pages" :key="page.name" class="lab-page-card">
            <div class="lab-page-head">
              <h3>{{ page.title || text.untitled }}</h3>
              <span class="lab-chip">{{ page.data_schema && Object.keys(page.data_schema).length > 0 ? text.schema : text.noSchema }}</span>
            </div>
            <p>{{ page.name }}</p>
            <div class="lab-page-meta">
              <span>{{ page.size.width.toFixed(0) }}w</span>
              <span>{{ page.size.height.toFixed(0) }}h</span>
            </div>
          </article>
          <div v-if="pages.length === 0" class="lab-empty lab-empty-compact">
            <p>{{ text.noPages }}</p>
          </div>
        </div>

        <div v-else class="lab-secondary-panel lab-tools-grid">
          <article v-for="tool in tools" :key="`${tool.function.name}-${tool.target}`" class="lab-tool-card">
            <div class="lab-tool-header">
              <h3>{{ tool.function.name }}</h3>
              <span class="lab-chip">{{ tool.target }}</span>
            </div>
            <pre class="lab-code">{{ formatTool(tool) }}</pre>
          </article>
          <div v-if="tools.length === 0" class="lab-empty lab-empty-compact">
            <p>{{ text.noTools }}</p>
          </div>
        </div>
      </section>
    </template>
    </template>
  </div>
</template>
