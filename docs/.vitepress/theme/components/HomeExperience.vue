<script setup lang="ts">
import { computed, ref } from "vue";
import { withBase } from "vitepress";
import PackageTreeDemo from "./PackageTreeDemo.vue";

const readingLayers = [
  {
    title: "Package structure",
    body: "AIX keeps the package readable as a tree of concrete entries instead of collapsing everything into an opaque blob."
  },
  {
    title: "Page metadata",
    body: "App and page config files define navigable surfaces and carry the context needed to interpret the package."
  },
  {
    title: "Schema to tools",
    body: "Schema-bearing pages can be promoted into tool-facing contracts without losing the link back to the package."
  }
];

const readingFlow = [
  {
    step: "01",
    title: "Open the archive",
    body: "Readers start with the package boundary and enumerate entries such as `VERSION`, `app.json`, page files, and assets."
  },
  {
    step: "02",
    title: "Resolve pages",
    body: "Page config and single-file components are interpreted into page summaries, descriptions, and layout hints."
  },
  {
    step: "03",
    title: "Interpret schema",
    body: "Schema data adds explicit contracts to those pages so the package becomes more than a file container."
  },
  {
    step: "04",
    title: "Derive tools",
    body: "Tool descriptions can then be generated from the same package-native structure rather than reconstructed elsewhere."
  }
];

const packageSections = [
  {
    kicker: "Core package",
    title: null,
    definition:
      "The `aix` crate defines the AIX format itself. It provides the package reading model, page analysis logic, and the structural semantics that every other tool in the repository builds on.",
    usage:
      "Use it when you need the canonical library surface for reading `.aix` packages, resolving page metadata, or deriving format-native structures in Rust.",
    command: "cargo test -p aix",
    meta: [
      { label: "Crate", value: "crates/aix" },
      { label: "Provides", value: "Format definition, package reader, page analysis" },
      { label: "Depends on", value: "Workspace foundation only" }
    ]
  },
  {
    kicker: "CLI package",
    title: null,
    definition:
      "The `aix-cli` crate turns the format model into terminal workflows. It is the command-line entry point for packaging, validating, and inspecting `.aix` artifacts.",
    usage:
      "Use it when you want to work with AIX from automation scripts, local development, or release pipelines without embedding the format library directly.",
    command: "cargo run -p aix-cli -- --help",
    meta: [
      { label: "Crate", value: "crates/aix-cli" },
      { label: "Provides", value: "Inspect, package, validate, CLI workflows" },
      { label: "Depends on", value: "aix" }
    ]
  },
  {
    kicker: "Web package",
    title: null,
    definition:
      "The `aix-web` crate makes the same format capabilities available in the browser. It exposes AIX through WASM bindings and supports interactive inspection surfaces such as the Package Lab.",
    usage:
      "Use it when AIX needs to be explored, demonstrated, or integrated into web-based environments without leaving the format model behind.",
    command: "cargo check -p aix-web --target wasm32-unknown-unknown",
    meta: [
      { label: "Crate", value: "crates/aix-web" },
      { label: "Provides", value: "WASM bindings, browser inspection, web tooling" },
      { label: "Depends on", value: "aix plus web bindings" }
    ]
  }
];

const installOptions = [
  { id: "npm", label: "npm", command: "npm install -g @yodaos-pkg/aix-cli" }
] as const;

const activeInstallId = ref<(typeof installOptions)[number]["id"]>("npm");
const copiedInstallId = ref<(typeof installOptions)[number]["id"] | null>(null);
const activeInstall = computed(
  () => installOptions.find((item) => item.id === activeInstallId.value) ?? installOptions[0]
);
const activeInstallIndex = computed(() =>
  Math.max(
    0,
    installOptions.findIndex((item) => item.id === activeInstallId.value)
  )
);

async function copyActiveInstallCommand() {
  const command = activeInstall.value.command;

  try {
    await navigator.clipboard.writeText(command);
  } catch {
    const textarea = document.createElement("textarea");
    textarea.value = command;
    textarea.setAttribute("readonly", "true");
    textarea.style.position = "absolute";
    textarea.style.left = "-9999px";
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand("copy");
    document.body.removeChild(textarea);
  }

  copiedInstallId.value = activeInstall.value.id;
  window.setTimeout(() => {
    if (copiedInstallId.value === activeInstall.value.id) {
      copiedInstallId.value = null;
    }
  }, 1600);
}

const specHref = withBase("/spec");
const packagesHref = withBase("/packages");
const playHref = withBase("/play");
</script>

<template>
  <main class="aix-doc-home">
    <section class="aix-doc-hero">
      <div class="aix-doc-container aix-doc-hero-grid">
        <div class="aix-doc-hero-copy">
          <p class="aix-doc-kicker">AIX file format</p>
          <h1 class="aix-doc-hero-title">AIX is an executable package format for AI.</h1>
          <p class="aix-doc-hero-lead">
            It packages pages, schema, and tools into a distributable artifact for AI agents.
          </p>
          <div class="aix-doc-hero-actions">
            <a class="aix-doc-button aix-doc-button-dark" :href="specHref">Specification</a>
            <a class="aix-doc-button aix-doc-button-light" :href="playHref">Play with AIX</a>
          </div>
        </div>

        <PackageTreeDemo />
      </div>
    </section>

    <section class="aix-doc-install-section">
      <div class="aix-doc-container">
        <div class="aix-doc-install-shell">
          <div class="aix-doc-install" aria-label="CLI installation commands">
            <p class="aix-doc-install-title">Start with the CLI.</p>
            <div class="aix-doc-install-tabs" role="tablist" aria-label="Install method">
              <span
                class="aix-doc-install-tab-indicator"
                aria-hidden="true"
                :style="{
                  width: `calc(${100 / installOptions.length}% - 4px)`,
                  transform: `translateX(${activeInstallIndex * 100}%)`
                }"
              />
              <button
                v-for="option in installOptions"
                :key="option.id"
                type="button"
                class="aix-doc-install-tab"
                :class="{ 'is-active': option.id === activeInstallId }"
                role="tab"
                :aria-selected="option.id === activeInstallId"
                @click="activeInstallId = option.id"
              >
                {{ option.label }}
              </button>
            </div>
            <div class="aix-doc-install-command-row">
              <pre class="aix-doc-install-command"><code>{{ activeInstall.command }}</code></pre>
              <button
                type="button"
                class="aix-doc-install-copy"
                :aria-label="`Copy ${activeInstall.label} install command`"
                @click="copyActiveInstallCommand"
              >
                <svg
                  v-if="copiedInstallId === activeInstall.id"
                  class="aix-doc-install-copy-icon"
                  viewBox="0 0 20 20"
                  fill="none"
                  aria-hidden="true"
                >
                  <path
                    d="M4.75 10.25L8.25 13.75L15.25 6.75"
                    stroke="currentColor"
                    stroke-width="1.8"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                </svg>
                <svg
                  v-else
                  class="aix-doc-install-copy-icon"
                  viewBox="0 0 20 20"
                  fill="none"
                  aria-hidden="true"
                >
                  <path
                    d="M7 5.5H5.75C5.06 5.5 4.5 6.06 4.5 6.75V14.25C4.5 14.94 5.06 15.5 5.75 15.5H13.25C13.94 15.5 14.5 14.94 14.5 14.25V13"
                    stroke="currentColor"
                    stroke-width="1.4"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                  <path
                    d="M8.75 12.5H14.25C14.94 12.5 15.5 11.94 15.5 11.25V5.75C15.5 5.06 14.94 4.5 14.25 4.5H8.75C8.06 4.5 7.5 5.06 7.5 5.75V11.25C7.5 11.94 8.06 12.5 8.75 12.5Z"
                    stroke="currentColor"
                    stroke-width="1.4"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                </svg>
              </button>
            </div>
          </div>
        </div>
      </div>
    </section>

    <section class="aix-doc-section">
      <div class="aix-doc-container">
        <div class="aix-doc-section-head">
          <p class="aix-doc-kicker">Reading model</p>
          <h2>Readable from the start.</h2>
          <p>
            The homepage should immediately tell readers what the package contains, how those files
            define pages, and how schema expands that structure into runtime-facing surfaces.
          </p>
        </div>

        <div class="aix-doc-grid aix-doc-grid-3">
          <article v-for="item in readingLayers" :key="item.title" class="aix-doc-card">
            <h3>{{ item.title }}</h3>
            <p>{{ item.body }}</p>
          </article>
        </div>
      </div>
    </section>

    <section class="aix-doc-section aix-doc-section-compact">
      <div class="aix-doc-container">
        <div class="aix-doc-section-head">
          <p class="aix-doc-kicker">How readers interpret AIX</p>
          <h2>From package to tool.</h2>
        </div>

        <div class="aix-doc-flow">
          <article v-for="item in readingFlow" :key="item.step" class="aix-doc-step">
            <span class="aix-doc-step-number">{{ item.step }}</span>
            <div>
              <h3>{{ item.title }}</h3>
              <p>{{ item.body }}</p>
            </div>
          </article>
        </div>
      </div>
    </section>

    <section v-for="pkg in packageSections" :key="pkg.kicker" class="aix-doc-section aix-doc-package-section">
      <div class="aix-doc-container">
        <div class="aix-doc-package-grid">
          <div class="aix-doc-section-head aix-doc-package-copy">
            <p class="aix-doc-kicker">{{ pkg.kicker }}</p>
            <h2 v-if="pkg.title">{{ pkg.title }}</h2>
            <div class="aix-doc-prose">
              <p>{{ pkg.definition }}</p>
              <p>{{ pkg.usage }}</p>
            </div>
            <div class="aix-doc-command-wrap">
              <span class="aix-doc-command-label">Quick command</span>
              <pre class="aix-doc-command"><code>{{ pkg.command }}</code></pre>
            </div>
          </div>

          <aside class="aix-doc-package-meta" aria-label="Package metadata">
            <dl class="aix-doc-package-meta-list">
              <div v-for="item in pkg.meta" :key="item.label" class="aix-doc-package-meta-row">
                <dt>{{ item.label }}</dt>
                <dd>{{ item.value }}</dd>
              </div>
            </dl>
          </aside>
        </div>
      </div>
    </section>

    <section class="aix-doc-section aix-doc-final">
      <div class="aix-doc-container aix-doc-final-grid">
        <p class="aix-doc-kicker">Next Step</p>
        <div class="aix-doc-hero-actions">
          <a class="aix-doc-button aix-doc-button-dark" :href="packagesHref">Explore packages</a>
          <a class="aix-doc-button aix-doc-button-light" :href="playHref">Play with AIX</a>
        </div>
      </div>
    </section>
  </main>
</template>
