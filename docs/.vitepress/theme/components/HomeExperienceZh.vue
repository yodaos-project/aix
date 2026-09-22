<script setup lang="ts">
import { withBase } from "vitepress";
import PackageTreeDemo from "./PackageTreeDemo.vue";

const specHref = withBase("/zh-CN/spec");
const apiHref = withBase("/zh-CN/api");
const playHref = withBase("/zh-CN/play");

const layers = [
  ["包结构", "AIX 将包保持为可阅读的具体条目树，而不是不可检查的二进制黑盒。"],
  ["页面元数据", "应用和页面配置定义可导航界面，并提供解释包内容所需的上下文。"],
  ["Schema 到工具", "带 Schema 的页面可以变成面向工具的契约，同时保留与原始包的关联。"]
];
const flow = [
  ["01", "打开归档", "从包边界开始，枚举 VERSION、app.json、页面文件和资源。"],
  ["02", "解析页面", "将页面配置和单文件组件解析为页面摘要、描述和布局提示。"],
  ["03", "解释 Schema", "Schema 为页面增加明确的数据契约，让包不只是文件容器。"],
  ["04", "派生工具", "从包原生结构生成工具描述，无需在其他系统中重新拼装。"]
];
const packages = [
  ["核心格式", "aix crate 定义 AIX 格式、包读取模型、页面分析和结构语义。", "在 Rust 中读取 .aix、解析页面元数据或生成格式原生结构。", "crates/aix"],
  ["npm CLI", "@yodaos-pkg/aix-cli 将格式模型变成打包、验证和检查工作流。", "在自动化脚本、开发环境或发布流程中使用 AIX。", "npm install -g @yodaos-pkg/aix-cli"],
  ["Web API", "@yodaos-pkg/aix 通过 WASM 将相同的格式能力带到浏览器。", "在浏览器中读取、演示和集成 AIX，不脱离统一的格式模型。", "npm install @yodaos-pkg/aix"]
];
</script>

<template>
  <main class="aix-doc-home">
    <section class="aix-doc-hero"><div class="aix-doc-container aix-doc-hero-grid">
      <div class="aix-doc-hero-copy">
        <p class="aix-doc-kicker">AIX 文件格式</p>
        <h1 class="aix-doc-hero-title">AIX 是面向 AI 的可执行包格式。</h1>
        <p class="aix-doc-hero-lead">将页面、Schema 和工具封装成可分发的 AI Agent 归档。</p>
        <div class="aix-doc-hero-actions"><a class="aix-doc-button aix-doc-button-dark" :href="specHref">阅读规范</a><a class="aix-doc-button aix-doc-button-light" :href="playHref">体验 AIX</a></div>
      </div><PackageTreeDemo locale="zh-CN" />
    </div></section>
    <section class="aix-doc-install-section"><div class="aix-doc-container"><div class="aix-doc-install-shell"><div class="aix-doc-install" aria-label="CLI 安装命令"><p class="aix-doc-install-title">从 CLI 开始。</p><div class="aix-doc-install-command-row"><pre class="aix-doc-install-command"><code>npm install -g @yodaos-pkg/aix-cli</code></pre><button type="button" class="aix-doc-install-copy" aria-label="复制 npm 安装命令" @click="navigator.clipboard?.writeText('npm install -g @yodaos-pkg/aix-cli')">复制</button></div></div></div></div></section>
    <section class="aix-doc-section"><div class="aix-doc-container"><div class="aix-doc-section-head"><p class="aix-doc-kicker">阅读模型</p><h2>从一开始就保持可读。</h2><p>首页展示包包含什么、文件如何定义页面，以及 Schema 如何将结构扩展为运行时界面。</p></div><div class="aix-doc-grid aix-doc-grid-3"><article v-for="item in layers" :key="item[0]" class="aix-doc-card"><h3>{{ item[0] }}</h3><p>{{ item[1] }}</p></article></div></div></section>
    <section class="aix-doc-section aix-doc-section-compact"><div class="aix-doc-container"><div class="aix-doc-section-head"><p class="aix-doc-kicker">AIX 的解析过程</p><h2>从包到工具。</h2></div><div class="aix-doc-flow"><article v-for="item in flow" :key="item[0]" class="aix-doc-step"><span class="aix-doc-step-number">{{ item[0] }}</span><div><h3>{{ item[1] }}</h3><p>{{ item[2] }}</p></div></article></div></div></section>
    <section v-for="item in packages" :key="item[0]" class="aix-doc-section aix-doc-package-section"><div class="aix-doc-container"><div class="aix-doc-package-grid"><div class="aix-doc-section-head aix-doc-package-copy"><p class="aix-doc-kicker">{{ item[0] }}</p><div class="aix-doc-prose"><p>{{ item[1] }}</p><p>{{ item[2] }}</p></div><div class="aix-doc-command-wrap"><span class="aix-doc-command-label">快速开始</span><pre class="aix-doc-command"><code>{{ item[3] }}</code></pre></div></div></div></div></section>
    <section class="aix-doc-section aix-doc-final"><div class="aix-doc-container aix-doc-final-grid"><p class="aix-doc-kicker">下一步</p><div class="aix-doc-hero-actions"><a class="aix-doc-button aix-doc-button-dark" :href="apiHref">查看 API</a><a class="aix-doc-button aix-doc-button-light" :href="playHref">体验 AIX</a></div></div></section>
  </main>
</template>
