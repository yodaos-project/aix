<script setup lang="ts">
import { computed, ref, shallowRef } from "vue";
import type { AixInputFile, PackResult, ProgressiveAssetUploadReceipt } from "@yodaos-pkg/aix";

const props = defineProps<{ locale: "en" | "zh-CN" | "ja" }>();
const emit = defineEmits<{ inspect: [data: Uint8Array, entryName: string] }>();

const sourcePath = "assets/clip.mp4";
const indexPath = "META-INF/assets/progressive_assets/index.json";
const sampleUrl = new URL("../../../../crates/aix-pack/tests/data/pattern-tone-flat.mp4", import.meta.url).href;
const samplePage = `<script setup>
import clip from '../assets/clip.mp4' with { type: 'video', progressive: 'prefer' };
export default { data: { clip } };
<\/script>
<page><video src="{{clip}}" preload="metadata" /></page>`;

const copy = {
  en: {
    title: "Progressive Assets",
    description: "Package a sample MP4 with a simulated upload. Everything runs in this browser.",
    run: "Run mock upload",
    idle: "Ready to prepare the sample",
    preparing: "Preparing media fragments",
    uploading: "Simulating fragment upload",
    packaging: "Validating the receipt and building the AIX package",
    done: "Mock upload complete. The AIX package is ready to inspect.",
    verified: "Verified for mock receipt",
    pending: "Pending",
    fragments: "Fragments verified",
    mediaBytes: "Fragment bytes",
    packageSize: "AIX package size",
    inspect: "Inspect demo package",
    note: "The package contains placeholder URLs for inspection. Playback requires real hosted fragments.",
  },
  "zh-CN": {
    title: "渐进资源",
    description: "模拟上传示例 MP4 的分片并构建 AIX 包。整个过程在当前浏览器中完成。",
    run: "运行模拟上传",
    idle: "准备开始处理示例",
    preparing: "正在准备媒体分片",
    uploading: "正在模拟上传分片",
    packaging: "正在校验回执并构建 AIX 包",
    done: "模拟上传完成，可以检查生成的 AIX 包。",
    verified: "已校验，可写入模拟回执",
    pending: "待处理",
    fragments: "已校验分片",
    mediaBytes: "分片大小",
    packageSize: "AIX 包大小",
    inspect: "检查示例包",
    note: "包中的地址仅供检查。播放需要实际托管的媒体分片。",
  },
  ja: {
    title: "段階配信アセット",
    description: "サンプル MP4 のアップロードを模擬し、AIX パッケージを作成します。処理はすべてブラウザー内で完結します。",
    run: "模擬アップロードを実行",
    idle: "サンプルの準備を開始できます",
    preparing: "メディアを分割しています",
    uploading: "分割データのアップロードを模擬しています",
    packaging: "受領証を検証し、AIX パッケージを作成しています",
    done: "模擬アップロードが完了しました。AIX パッケージを検査できます。",
    verified: "模擬受領証の検証済み",
    pending: "処理待ち",
    fragments: "検証済みフラグメント",
    mediaBytes: "フラグメントのサイズ",
    packageSize: "AIX のサイズ",
    inspect: "デモパッケージを検査",
    note: "パッケージ内の URL は検査用です。再生には実際にホストされた分割データが必要です。",
  },
};
const text = computed(() => copy[props.locale]);
const phase = ref<"idle" | "preparing" | "uploading" | "packaging" | "done">("idle");
const running = computed(() => phase.value !== "idle" && phase.value !== "done");
const fragments = shallowRef<{ id: string; size: number }[]>([]);
const verified = ref(0);
const result = shallowRef<PackResult | null>(null);
const error = ref("");
const fragmentBytes = computed(() => fragments.value.reduce((sum, fragment) => sum + fragment.size, 0));

function bytes(size: number) {
  return `${size.toLocaleString(props.locale)} B`;
}

async function runDemo() {
  phase.value = "preparing";
  fragments.value = [];
  verified.value = 0;
  result.value = null;
  error.value = "";

  try {
    const [{ AIX }, sample] = await Promise.all([import("@yodaos-pkg/aix"), fetch(sampleUrl)]);
    if (!sample.ok) throw new Error(`Sample MP4 could not be loaded (HTTP ${sample.status})`);
    const encoder = new TextEncoder();
    const files: AixInputFile[] = [
      { path: "app.json", data: encoder.encode(JSON.stringify({
        pages: ["pages/index"],
        window: { navigationBarTitleText: "Progressive Asset demo" },
      })) },
      { path: "pages/index.ink", data: encoder.encode(samplePage) },
      { path: sourcePath, data: new Uint8Array(await sample.arrayBuffer()) },
    ];

    result.value = await AIX.packFromSource(files, {
      async onProgressiveAsset(candidate, { prepare }) {
        if (!candidate.progressiveSupport.supported) {
          throw new Error(candidate.progressiveSupport.reason ?? "Sample media profile is unsupported");
        }
        const plan = await prepare();
        fragments.value = plan.remoteObjects.map(({ id, size }) => ({ id, size }));
        phase.value = "uploading";

        // Verify the real fragment bytes and return inspection-only locators.
        // The delay gives each simulated upload a visible step in Play.
        const objects: ProgressiveAssetUploadReceipt["objects"] = [];
        for (const { id, size, sha256, bytes: data } of plan.remoteObjects) {
          const digest = await crypto.subtle.digest("SHA-256", Uint8Array.from(data));
          const hash = Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
          if (data.byteLength !== size || hash !== sha256) {
            throw new Error(`Fragment integrity check failed for ${id}`);
          }
          await new Promise((resolve) => setTimeout(resolve, 180));
          objects.push({ id, size, sha256, url: `https://playground.invalid/objects/${id}` });
          verified.value++;
        }
        phase.value = "packaging";
        return {
          delivery: "progressive",
          planId: plan.id,
          receipt: { uploadId: "play-mock", assetVersion: plan.assetVersion, objects },
        };
      },
    });
    phase.value = "done";
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : String(cause);
    phase.value = "idle";
  }
}
</script>

<template>
  <section class="lab-builder-panel progressive-demo">
    <div class="demo-header">
      <div>
        <h2>{{ text.title }}</h2>
        <p>{{ text.description }}</p>
        <code>{{ sourcePath }}</code>
      </div>
      <button class="lab-build-button" type="button" :disabled="running" @click="runDemo">{{ text.run }}</button>
    </div>

    <p role="status">{{ text[phase] }}</p>
    <p v-if="error" class="lab-error" role="alert">{{ error }}</p>

    <template v-if="fragments.length">
      <progress :max="fragments.length" :value="verified" :aria-label="text.fragments" />
      <p>{{ text.fragments }} · {{ verified }} / {{ fragments.length }} · {{ text.mediaBytes }} {{ bytes(fragmentBytes) }}</p>
      <ul class="demo-fragments">
        <li v-for="(fragment, index) in fragments" :key="fragment.id">
          <code :title="fragment.id">{{ fragment.id.slice(0, 12) }}…</code>
          <span>{{ bytes(fragment.size) }}</span>
          <span>{{ index < verified ? text.verified : text.pending }}</span>
        </li>
      </ul>
    </template>

    <div v-if="result" class="demo-result">
      <strong>{{ text.packageSize }} · {{ bytes(result.data.byteLength) }}</strong>
      <p>{{ text.note }}</p>
      <button class="lab-build-button" type="button" @click="emit('inspect', result.data, indexPath)">{{ text.inspect }}</button>
    </div>
  </section>
</template>

<style scoped>
.progressive-demo {
  padding: clamp(16px, 4vw, 28px);
}

.demo-header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.progressive-demo h2 {
  margin: 0;
  padding: 0;
  border: 0;
}

.progressive-demo p {
  margin: 8px 0;
  color: var(--aix-muted);
}

.progressive-demo progress {
  width: 100%;
  accent-color: var(--vp-c-brand-1);
}

.demo-fragments {
  padding: 0;
  list-style: none;
}

.demo-fragments li {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px 16px;
  padding: 8px 0;
  border-bottom: 1px solid var(--vp-c-divider);
}

.demo-result {
  margin-top: 20px;
}
</style>
