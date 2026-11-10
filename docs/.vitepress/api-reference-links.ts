import type { MarkdownRenderer } from "vitepress";

interface References {
  types: Map<string, string>;
  methods: Map<string, string>;
}

interface ReferenceEnvironment {
  relativePath?: string;
  aixReferences?: References;
}

const declaration = /^(?:export\s+)?(?:interface|type)\s+([A-Za-z_]\w*)\b/;

function referenceFor(content: string, references: References): string | undefined {
  const method = /^(?:(AIX|pkg)\.)?([A-Za-z_]\w*)\(/.exec(content);
  if (method) return references.methods.get(method[1] ? `${method[1]}.${method[2]}` : method[2]);
  for (const name of content.match(/[A-Za-z_]\w*/g) ?? []) {
    const target = references.types.get(name);
    if (target) return target;
  }
}

/** Link public symbols on each localized API page. */
export function apiReferenceLinks(md: MarkdownRenderer) {
  md.core.ruler.push("aix-api-references", (state) => {
    const env = state.env as ReferenceEnvironment;
    if (!/^(?:(?:zh-CN|ja)\/)?api\.md$/.test(env.relativePath ?? "")) return;

    const references: References = {
      types: new Map([["AIX", "#instance-methods"]]),
      methods: new Map(),
    };
    for (let index = 0; index < state.tokens.length; index++) {
      const token = state.tokens[index];
      if (token.type === "fence" && token.info.trim() === "typescript") {
        for (const line of token.content.split("\n")) {
          const name = declaration.exec(line)?.[1];
          if (name) references.types.set(name, `#type-${name.toLowerCase()}`);
        }
      }
      if (token.type === "heading_open" && token.tag === "h4") {
        const title = state.tokens[index + 1]?.content ?? "";
        const method = /^(?:(AIX|pkg)\.)?([A-Za-z_]\w*)\(/.exec(title);
        const id = token.attrGet("id");
        if (method && id) {
          references.methods.set(`${method[1] ?? "pkg"}.${method[2]}`, `#${id}`);
          references.methods.set(method[2], `#${id}`);
        }
      }
    }
    env.aixReferences = references;

    let inReference = false;
    for (let index = 0; index < state.tokens.length; index++) {
      const token = state.tokens[index];
      if (token.type === "heading_open" && token.tag === "h2") {
        inReference = token.attrGet("id") === "api-reference";
      }
      let linkDepth = 0;
      for (const child of token.children ?? []) {
        if (child.type === "link_open") linkDepth++;
        if (child.type === "code_inline" && linkDepth === 0 && (inReference || child.content !== "AIX")) {
          const href = referenceFor(child.content, references);
          if (href) child.meta = { ...child.meta, aixReference: href };
        }
        if (child.type === "link_close") linkDepth--;
      }
    }
  });

  const renderInline = md.renderer.rules.code_inline;
  md.renderer.rules.code_inline = (tokens, index, options, env, renderer) => {
    const token = tokens[index];
    const html = renderInline
      ? renderInline(tokens, index, options, env, renderer)
      : `<code>${md.utils.escapeHtml(token.content)}</code>`;
    const href = token.meta?.aixReference;
    return href ? `<a class="api-reference-link" href="${md.utils.escapeHtml(href)}">${html}</a>` : html;
  };

  const renderFence = md.renderer.rules.fence!;
  md.renderer.rules.fence = (tokens, index, options, environment, renderer) => {
    let html = renderFence(tokens, index, options, environment, renderer);
    const env = environment as ReferenceEnvironment;
    const references = env.aixReferences;
    if (!references || tokens[index].info.trim() !== "typescript") return html;

    const lines = tokens[index].content.split("\n");
    let lineIndex = 0;
    html = html.replace(/<span class="line(?: [^"]*)?"[^>]*>/g, (tag) => {
      const name = declaration.exec(lines[lineIndex++] ?? "")?.[1];
      return name ? tag.replace(/>$/, ` id="type-${name.toLowerCase()}" tabindex="-1">`) : tag;
    });
    return html.replace(/(<span\b[^>]*>)([^<>]*)(<\/span>)/g, (_, start, content, end) => {
      const linked = content.replace(/\b[A-Za-z_]\w*\b/g, (name: string) => {
        const href = references.types.get(name);
        return href ? `<a class="api-reference-link" href="${href}">${name}</a>` : name;
      });
      return start + linked + end;
    });
  };
}
