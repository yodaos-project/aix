# AIX Format Specification

| Specification metadata | Value |
| --- | --- |
| Status | Draft implementation specification |
| Format identifier | `aix` |
| Document scope | Package bytes and their normative interpretation |

This document specifies the AIX file format. It is intentionally independent
of any programming language, command-line interface, reader library, browser
binding, or host agent runtime.

## 0. Conformance Language

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHALL**, **SHALL NOT**,
**SHOULD**, **SHOULD NOT**, **RECOMMENDED**, **MAY**, and **OPTIONAL** in this
document are to be interpreted as described in RFC 2119 and RFC 8174.

An **AIX producer** creates an `.aix` archive. An **AIX consumer** parses and
uses an archive. A **conforming producer** satisfies all producer requirements;
a **conforming consumer** satisfies all consumer requirements. A package is
**valid** when its ZIP serialization and required AIX data model satisfy the
validation rules in this document. A package is **trusted** only after its
signature has been verified against a trust policy; validity alone does not
imply trust.

AIX (AI eXecutable) is a portable package format for AI agents with inspectable user interfaces. It is a ZIP-based extension of the [Open Agent Format (OAF)](https://openagentformat.com/spec.html): OAF defines the agent's identity, instructions, composition, and harness-neutral resources; AIX adds a deterministic archive, Ink Mini Program pages, page schemas, layout hints, and an agent-facing tool surface.

This document describes the format implemented by the `aiui-aix`, `aiui-aix-pack`, and `aiui-aix-web` crates. It is normative where it says **must** or **must not**.

## 0.1 Format Versioning

The string `aix` identifies the format family. The package `VERSION` entry is
an artifact/build identifier and is not a format version. The manifest
`format` field identifies the family, while the meanings of fields in this
document are versioned by the document and by future manifest extensions.
Consumers MUST reject an unsupported `format` value. Producers adding a field
MUST preserve the meaning of all existing fields; a change to an existing
field, path rule, canonical hash input, or signature domain requires a new
format identifier or an explicitly negotiated profile.

## 0.2 Data Model and Character Encoding

All path names and textual entry contents are sequences of Unicode scalar
values encoded as UTF-8. ZIP path ordering and package-id ordering operate on
the raw UTF-8 octets, not locale collation or normalized Unicode forms.
JSON documents MUST be valid JSON encoded as UTF-8. YAML in OAF `AGENTS.md`
front matter MUST be valid UTF-8 YAML. A consumer MUST NOT silently transcode
JSON, YAML, or manifest bytes while calculating a digest or signature.

## 1. OAF Baseline

OAF treats the filesystem as the source of truth for an agent. A minimal OAF agent is:

```text
agent/
└── AGENTS.md
```

The root `AGENTS.md` is the agent manifest and instruction document. Its Markdown body is the agent prompt; optional YAML front matter carries identity and composition metadata. OAF also defines optional, self-contained directories:

```text
agent/
├── AGENTS.md                 # required OAF manifest and instructions
├── skills/{name}/SKILL.md    # AgentSkills-compatible skills
├── mcp-configs/{name}/       # ActiveMCP.json and config.yaml
├── sub-agents/{name}/        # nested agents, each with AGENTS.md
├── versions/{version}/       # historical agent manifests
└── README.md, LICENSE        # human and legal metadata
```

An OAF manifest commonly declares `name`, `description`, `version`, `author`, `license`, `skills`, `packs`, `weblets`, `mcp-servers`, `sub-agents`, `model`, `memory`, and harness-specific configuration. OAF is harness-agnostic: a consumer may export the same agent to different runtimes while retaining one canonical manifest. OAF packaging may use a `PACKAGE.yaml` envelope for one or more agent directories.

### OAF and AIX boundary

An AIX package can carry OAF files verbatim. AIX does not reinterpret the Markdown instruction language, skill format, MCP configuration, or model policy. Those files remain resources for the host harness. AIX adds machine-readable application metadata in `app.json`, page files, and generated metadata under `META-INF/aix/`.

| Concern | OAF | AIX |
| --- | --- | --- |
| Agent identity and instructions | `AGENTS.md` front matter + Markdown | Preserved as package entries; interpreted by the host |
| Skills, MCP, sub-agents | Optional OAF directories | Preserved as package entries |
| Filesystem/package model | Directory or OAF `PACKAGE.yaml` | ZIP archive with normalized paths |
| Interactive UI | Not specified | Ink pages, templates, styles, and page schema |
| Agent callable surface | Harness-defined | Derived OpenAI-style tools from pages |
| Integrity | OAF packaging guidance | Optional AIX Ed25519 manifest and entry digests |

## 2. AIX Package

### 2.1 Abstract package model

An AIX package is an ordered set of entries. Each entry has a path, an
uncompressed byte sequence, and a ZIP compression representation. The ZIP
central directory is the serialization index; it is not application metadata.
The following abstract grammar describes the required names (ABNF notation is
used for readability; the actual path is UTF-8):

```text
aix-package   = zip-archive
app-entry     = "app.json"
version-entry = "VERSION"
metadata-dir  = "META-INF/aix/"
page-path     = 1*(path-char) ; logical path without a page extension
path-char     = %x21-7E / UTF8-NONASCII
```

An entry path MUST be non-empty, MUST NOT begin with `/`, MUST NOT contain `\`,
MUST NOT contain an empty, `.` or `..` segment, and MUST NOT contain a NUL
character. After ZIP path normalization, no two entries may have the same
path. A producer MUST reject paths beginning with `META-INF/aix/` supplied as
application input.

### 2.2 Entry classes

Every entry belongs to exactly one class:

| Class | Identification | Manifest treatment |
| --- | --- | --- |
| Application | Any non-directory path outside `META-INF/aix/` | MUST be listed and hashed |
| Directory | Path ending in `/` | MAY be omitted from the manifest |
| AIX metadata | Path beginning `META-INF/aix/` | MUST NOT be listed in `entries` |

The reserved metadata namespace is extensible. Unknown metadata entries MUST
be ignored by an application consumer unless a profile defines them.

### 2.3 ZIP serialization requirements

The archive MUST use ZIP entries with UTF-8 names. Consumers MUST support the
`stored` and `deflate` methods. Producers SHOULD use `deflate` for textual
entries and MAY use `stored` for already-compressed media such as PNG and JPEG.
The ZIP CRC-32 and uncompressed size are integrity hints and MUST agree with
the extracted bytes. A consumer MUST reject an entry whose extraction exceeds
its declared uncompressed size or whose CRC/size verification fails.

ZIP metadata such as timestamps, creator fields, permissions, and compression
levels are not part of the AIX semantic model and MUST NOT affect
`package_id`.

### 2.4 Required-entry processing

For a generated package, the producer MUST emit `VERSION` and `app.json`.
`VERSION` MUST be UTF-8 and MUST NOT be empty. The value SHOULD be a UUID v4
or another globally unique build identifier. A consumer MUST treat the value
as opaque except for the manifest equality check. `app.json` MUST be parsed
before page or widget metadata is interpreted.

An `.aix` file is a ZIP archive. Paths use `/` separators and are compared in byte order. A producer must reject an input named `VERSION`; it generates that entry from the build id. A generated archive contains at least:

```text
VERSION                         # UTF-8 build identifier
app.json                        # application metadata and page list
AGENTS.md                       # OAF manifest, when supplied
pages/...                       # page resources
META-INF/aix/manifest.json      # generated package manifest (recommended)
```

Directories may be present but are not logical file entries. `META-INF/aix/` is reserved for AIX metadata and is excluded from the signed application-entry set. JSON, JavaScript, Ink, templates, and stylesheets are normally deflated; PNG and JPEG entries are stored because they are already compressed. Input paths are normalized by the collector and invalid/traversal paths are rejected.

### Required and conventional entries

`VERSION` is required for a generated package. `app.json` is required for page discovery. An unsigned or legacy archive may omit `META-INF/aix/manifest.json`; such a package has no signed manifest. A signed package must contain the manifest and signature entries described in section 7.

## 3. Application Metadata

`app.json` requires `pages` to be an array of strings. The optional fields currently consumed by AIX are:

```json
{
  "pages": ["pages/index/index", "pages/settings/index"],
  "widgets": [{
    "path": "widgets/clock/index",
    "family": "1x1",
    "placement": "persistent",
    "displayName": "Clock",
    "description": "Current time"
  }],
  "window": { "navigationBarTitleText": "Example agent" }
}
```

`pages` values are logical paths without a page extension. `window.navigationBarTitleText` is the application title. Widget placement is `persistent` by default and may be `overlay`. Locale overlays can provide localized widget `displayName` and `description`.

## 4. Pages and OAF Resources

For each path in `app.json.pages`, AIX resolves metadata in one of two forms:

1. **Multi-file page:** `{path}.json`, `{path}.wxml`, and `{path}.wxss`.
2. **Single-file component:** `{path}.ink`, containing raw-text `page` or `template` markup and a `style` block.

Page JSON may contain:

```json
{
  "navigationBarTitleText": "Search",
  "description": "Search the knowledge base",
  "schema": { "data": {
    "type": "object",
    "properties": { "query": { "type": "string" } },
    "required": ["query"]
  }}
}
```

`schema.data` is copied as the page's `data_schema`. Missing schema or missing `data` yields an empty schema and a warning; it does not make the archive unreadable. Page title and description are optional. A page's `PageInfo` is `name`, `title`, `description`, `data_schema`, and `size { width, height }`.

OAF files such as `AGENTS.md`, `skills/`, and `mcp-configs/` remain ordinary readable entries. AIX does not merge their instructions into page schema or tool descriptions.

## 5. Layout Constraints

`PageAnalyzer` derives a `PageConstraint` from fixed pixel declarations in page markup and styles. It recognizes `width: Npx` and `height: Npx` in `.wxml`/`.wxss` (or the corresponding Ink blocks). Inline style values override matching external `#id` and `.class` rules.

- Width is the maximum fixed width found across root elements.
- Height is the sum of fixed heights found across root elements.
- A missing dimension uses the default: `480 × 168` pixels.
- Non-pixel values, nested layout semantics, and dynamic CSS are not evaluated.

The resulting size is advisory: hosts use it to choose a viewport or tool presentation and must still handle responsive content.

## 6. Page-to-Tool Derivation

Each page may be represented as an OpenAI-style function tool:

```json
{
  "type": "function",
  "target": "_current",
  "layout": { "width": 480, "height": 168 },
  "function": {
    "name": "pages/search/index",
    "description": "Search the knowledge base",
    "parameters": { "type": "object", "properties": { "query": { "type": "string" } } }
  }
}
```

The function name is the logical page path. Description prefers page `description`, then its title. Parameters are exactly `schema.data`; no schema conversion or validation is performed by the format.

The first declared page has special launch semantics: if its schema is null, `{}`, or an object schema with no properties, it is emitted once with `target: "_blank"` and empty parameters. Otherwise it is `_current`. Every later page is `_current`, including pages with no parameters. This rule is positional: changing `app.json.pages` order can change the initial tool target.

## 7. Manifest, Integrity, and Signatures

The generated `META-INF/aix/manifest.json` has this shape:

```json
{
  "format": "aix",
  "version": "VERSION_VALUE",
  "engine": "^0.14.0",
  "algorithm": "ed25519",
  "digest": "sha256",
  "key_id": "sha256:PUBLIC_KEY_DIGEST",
  "package_id": "sha256:ENTRY_LIST_DIGEST",
  "entries": [{ "path": "app.json", "size": 42, "sha256": "HEX_DIGEST" }]
}
```

Manifest entries must be strictly increasing by UTF-8 byte order, must exclude `META-INF/aix/*`, and must include every non-directory, non-metadata ZIP entry. `package_id` hashes, in entry order, the big-endian path length, path bytes, big-endian size, big-endian digest-string length, and digest bytes/string representation for each entry. This byte layout is compatibility-sensitive.

When a signing key is supplied, the producer also writes:

```text
META-INF/aix/signature.ed25519   # 64-byte Ed25519 signature
META-INF/aix/public-key.ed25519  # 32-byte public key
```

The signature covers the manifest with context `package-manifest` and the domain prefix `AIX-SIGNATURE\0`. Verification checks the algorithm/digest identifiers, engine range, trusted `key_id`, signature, entry ordering, every entry digest and size, absence of unsigned application entries, `manifest.version == VERSION`, and the calculated package id. A valid ZIP is not necessarily a trusted AIX package until these checks pass.

## 8. Engine Compatibility

The manifest `engine` field is a semver requirement. A bare version such as `0.14.0` is interpreted as `=0.14.0`; ranges such as `^0.14.0`, `>=0.14.0`, and `*` use normal semver matching. Invalid ranges make the package non-conformant.

## 9. Serialization and Compatibility

Producers must sort application paths by their UTF-8 byte sequence before writing the archive and manifest. `VERSION` is generated from the build identifier and input packages must not provide their own `VERSION`. Textual JSON, JavaScript, TypeScript, Ink, template, and stylesheet entries must be UTF-8 after serialization. PNG and JPEG may be optimized, but optimization must not alter the logical path or media type. `.aixignore`, when used by a producer, follows gitignore syntax and is not itself packaged.

These serialization rules define byte-level interoperability; a tool or runtime may expose any API surface, but that API is outside this format specification. OAF instructions and resources remain unchanged when serialized into AIX.

## 10. Conformance Checklist

An AIX implementation is conformant when it can:

- parse a ZIP package with normalized paths and verify entry size/CRC;
- require `app.json.pages` and `VERSION` in generated packages;
- resolve both multi-file pages and `.ink` single-file pages;
- preserve page schema data without silently inventing parameters;
- implement the default `480 × 168`, max-width, stacked-height layout algorithm;
- apply the first-page `_blank` rule exactly;
- preserve OAF resources without treating them as AIX metadata;
- reject invalid engine ranges and unsigned or mismatched signed entries;
- reproduce the manifest ordering and package-id byte layout.

## 11. Field Definitions

This section is the field-level registry. Unless a field says otherwise, unknown
fields are preserved as bytes but ignored by conforming AIX consumers. Producers
should use the names and casing shown here; consumers must not infer a value
from an unknown spelling.

### 12.1 OAF `AGENTS.md` front matter

The front matter is YAML delimited by `---`. OAF accepts a structured body or a
direct system-prompt body. If the first non-empty body line starts with `#`, it
is the structured form; otherwise the body is interpreted as the prompt of a
specialized sub-agent.

| Field | Type | Required | Constraints and meaning |
| --- | --- | --- | --- |
| `name` | string | yes | Display name, 1–100 characters. |
| `vendorKey` | string | yes | Publisher namespace, kebab-case. |
| `agentKey` | string | yes | Agent identifier, kebab-case. |
| `version` | string | yes | Semantic version. |
| `slug` | string | yes | Stable identifier; conventionally `vendorKey/agentKey`. |
| `description` | string | yes | Short purpose and capability summary. |
| `author` | string | yes | Person, organization, or handle. |
| `license` | string | yes | SPDX identifier or project-defined license name. |
| `tags` | array of strings | yes | Search and categorization labels. |
| `skills` | array of Skill objects | no | Local, registry, or well-known skill references. |
| `packs` | array of Pack objects | no | Collections of skills or tools. |
| `weblets` | array of Weblet objects | no | Web-based tools or interfaces. |
| `mcpServers` | array of MCP objects | no | MCP server declarations. |
| `agents` | array of AgentRef objects | no | Nested/sub-agent declarations. |
| `orchestration` | Orchestration object | no | Entrypoint, fallback, and event triggers. |
| `tools` | array of strings | no | Explicit tool names available to a harness. |
| `config` | Configuration object | no | Runtime limits and allow/deny policy. |
| `memory` | Memory object | no | Memory mode and named blocks. |
| `model` | string or Model object | no | Harness alias or provider/model selection. |
| `harnessConfig` | map of string to object | no | Provider-specific settings, keyed by harness name. |

OAF composition fields are descriptive and harness-neutral. AIX stores them as
ordinary archive entries and does not resolve URLs, install skills, start MCP
servers, or enforce model/configuration policy.

#### OAF composition objects

| Object | Fields | Meaning |
| --- | --- | --- |
| Skill | `name` string, `source` string, `version` string, `required` boolean | `source: local` means `./skills/{name}/`; a URL identifies a remote skill. `required` defaults to `false`. |
| Pack | `vendor` string, `pack` string, `version` string, `required` boolean | Named collection supplied by a vendor. |
| Weblet | `vendor` string, `weblet` string, `version` string, `launch` enum | `launch` is `onDemand`, `background`, or `foreground`; default is harness-defined. |
| MCP server | `vendor`, `server`, `version`, `configDir`, `required` | `configDir` points to a directory containing `ActiveMCP.json` and `config.yaml`. |
| Agent reference | `vendor`, `agent`, `version`, `role`, `delegations`, `required` | `delegations` is an array of responsibility labels. |
| Model | `provider`, `name`, `embedding` | Provider and model identifiers; `embedding` is optional. |
| Memory | `type`, `blocks` | `type` is `editable` or `read-only`; `blocks` maps names to initial values or templates. |

The OAF `config` object may contain `temperature` (number), `max_tokens`
(integer), `require_confirmation` (boolean), and nested `tools.allowed` and
`tools.denied` string arrays. OAF does not require a particular YAML parser or
ordering, but a producer must emit valid YAML and UTF-8.

### 12.2 AIX `app.json`

`app.json` is UTF-8 JSON and is required in every generated AIX package.

| Field | Type | Required | Default | Constraints and consumer behavior |
| --- | --- | --- | --- | --- |
| `pages` | array of strings | yes | none | Logical page paths, non-empty strings. Order is significant. |
| `widgets` | array of Widget | no | `[]` | Each widget must have a matching `{path}.ink` entry when widget metadata is resolved. |
| `window` | Window object | no | absent | Only `navigationBarTitleText` is currently consumed. |

`pages` must be present and have the declared shape. A consumer may reject a
package with a missing or malformed field; it must not invent page paths.

#### Widget object

| Field | Type | Required | Default | Constraints and consumer behavior |
| --- | --- | --- | --- | --- |
| `path` | string | yes | none | Extensionless logical path; widget metadata requires `{path}.ink`. |
| `family` | string | yes | none | Host-defined family such as `1x1` or `1x2`. |
| `placement` | enum string | no | `persistent` | `persistent` or `overlay`; JSON spelling is kebab-case. |
| `displayName` | string or null | no | null | User-facing label; locale overlays may replace it. |
| `description` | string or null | no | null | Short widget description; locale overlays may replace it. |

#### Window object

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `navigationBarTitleText` | string or null | no | Application title. |

Locale files named `app.{locale}.json` may contain `widgets`, a map keyed by
widget path. Locale matching normalizes `_` to `-`, ignores case, tries the
exact locale, then progressively removes subtags, then falls back to the same
primary language.

### 12.3 Page JSON and schema

For a logical path `p`, a consumer first looks for `p.ink`. If absent, it reads
`p.json`; `p.wxml` and either `p.wcss` or `p.wxss` provide layout inputs.

| Field | Type | Required | Default | Meaning |
| --- | --- | --- | --- | --- |
| `navigationBarTitleText` | string or null | no | null | Page title. |
| `description` | string or null | no | null | Natural-language description used for tool metadata. |
| `schema` | object or null | no | absent | Container for the input schema. |
| `schema.data` | any JSON value | no | `{}` | JSON Schema (or schema-compatible value) passed through as tool parameters. |

The format does not require that `schema.data` be a formal JSON Schema. A
null, empty object, or object schema whose `properties` object is empty is
treated as “no parameters” only for first-page target selection. Arrays,
strings, numbers, and non-empty objects are treated as parameterized schemas.

An Ink file uses raw-text blocks. A `script def` block contains the same page
JSON object, while `page` or `template` supplies markup and `style`
supplies CSS. A malformed optional page file does not invalidate the ZIP; the
consumer may keep defaults and emit a diagnostic.

### 12.4 Derived `PageInfo` fields

| Field | Type | Always present | Meaning |
| --- | --- | --- | --- |
| `name` | string | yes | Exact logical path from `app.json.pages`. |
| `title` | string or null | yes | Page navigation title, if declared. |
| `description` | string or null | yes | Page description, if declared. |
| `data_schema` | JSON value | yes | `schema.data`, default `{}`. |
| `size.width` | number | yes | Maximum fixed pixel width, default `480`. |
| `size.height` | number | yes | Sum of fixed pixel heights, default `168`. |

Only root XML elements are inspected for size. External selectors are applied
by id then class; inline declarations override them. Width takes the maximum;
height stacks. CSS values without a `px` suffix are ignored.

### 12.5 Derived Tool fields

| Field | Type | Required | Values and behavior |
| --- | --- | --- | --- |
| `type` | string | yes | `function`. |
| `target` | enum string | yes | `_blank` for the first no-parameter page; `_current` otherwise. |
| `layout` | PageConstraint | yes | Copied from the page's inferred `size`. |
| `function.name` | string | yes | Logical page path. |
| `function.description` | string or null | yes | Page description, falling back to title. |
| `function.parameters` | JSON value | yes | Exact page `schema.data` value, except first `_blank` tool uses `{}`. |

`target` is a rendering instruction, not a security boundary. A host decides
whether `_blank` means a new surface, isolated context, or initial navigation.

### 12.6 AIX manifest fields

| Field | Type | Required | Constraints |
| --- | --- | --- | --- |
| `format` | string | yes | Must equal `aix`. |
| `version` | string | yes | Must equal UTF-8 contents of `VERSION`. |
| `engine` | string | yes | Valid semver requirement; bare versions mean exact match. |
| `algorithm` | string | yes | Current verifier accepts `ed25519`. |
| `digest` | string | yes | Current verifier accepts `sha256`. |
| `key_id` | string | yes | `sha256:` plus lowercase hex SHA-256 of the trusted public key; empty for unsigned manifests. |
| `package_id` | string | yes | `sha256:` plus lowercase hex digest of the canonical entry sequence. |
| `entries` | array of ManifestEntry | yes | Strictly byte-sorted; excludes all `META-INF/aix/` paths. |

#### Manifest entry

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `path` | string | yes | Normalized ZIP path. |
| `size` | unsigned integer | yes | Uncompressed byte length. |
| `sha256` | lowercase hex string | yes | SHA-256 digest of the uncompressed entry bytes. |

The canonical package id encodes each entry as `u32-be(path.length)`, path
bytes, `u64-be(size)`, `u32-be(sha256.length)`, and digest-string bytes, then
SHA-256 hashes the concatenation. Implementations must not sort by locale,
Unicode collation, or case-folded order.

### 12.7 Signature records

`signature.ed25519` is exactly 64 bytes. `public-key.ed25519` is exactly 32
bytes. The signature is Ed25519 over the manifest JSON bytes using the signing
context `package-manifest`; the domain-separated message begins with the bytes
`AIX-SIGNATURE\0`, followed by a big-endian context length, context bytes, a
big-endian message length, and the manifest bytes. Verifiers must use a trusted
key supplied by the caller; the embedded public-key entry is metadata, not an
automatic trust decision.

## 12. Error and Compatibility Requirements

Readers must reject invalid ZIP paths, duplicate normalized entries, unsupported
compression, failed CRC/size verification, malformed required JSON, malformed
manifest JSON, unsorted signed entries, digest mismatches, unsigned application
entries, version mismatches, package-id mismatches, invalid engine ranges, and
untrusted key ids. Optional page metadata failures may degrade to defaults.

The format is forward-compatible only for unknown metadata fields. Existing
field meanings, enum spellings, path normalization, signature domain, and hash
encoding are compatibility contracts and must not be changed without a format
version or migration rule.

## 13. Conforming Processing Model

A consumer SHOULD process a package in the following order. This order avoids
using untrusted derived metadata before the archive has been structurally
validated:

1. Parse the ZIP central directory and normalize every entry path.
2. Reject duplicate paths, invalid paths, unsupported compression, and ZIP
   size/CRC inconsistencies.
3. Classify entries as application, directory, or reserved metadata.
4. Parse `app.json` and validate its required fields.
5. If a manifest is present, parse and validate its field types, ordering, and
   entry coverage.
6. If trust is required, verify the signature and every manifest digest before
   executing or displaying application content.
7. Resolve pages, widgets, OAF resources, schemas, and layout hints.
8. Derive presentation or tool records without changing the source bytes.

Failure at steps 1–5 makes the package invalid. Failure while resolving an
OPTIONAL page resource MAY produce a diagnostic and the specified default, but
MUST NOT cause a consumer to invent a path, schema property, or signature
claim.

## 14. Security Considerations

ZIP extraction MUST be bounded by declared and implementation resource limits.
Consumers MUST prevent path traversal and MUST NOT write an entry outside the
selected package directory. A signature authenticates the manifest and the
listed application bytes; it does not authorize execution, network access,
MCP use, model selection, or OAF instructions. Those decisions belong to the
host trust policy.

Consumers SHOULD reject packages with excessive entry counts, decompressed
sizes, nesting, or schema complexity before allocating unbounded resources.
JSON Schema values are data, not executable code. OAF Markdown, skill files,
and scripts MUST be treated as untrusted content until a host explicitly
authorizes them.

The embedded `public-key.ed25519` entry is an identifier and distribution aid,
not a trust root. A verifier MUST compare `key_id` with a caller-supplied
trusted key and MUST reject a mismatch. Implementations SHOULD use constant-time
signature and digest comparison where their cryptographic library provides it.

## 15. Interoperability Requirements

Two conforming producers given the same normalized application entries,
build-id, engine range, and signing configuration MUST produce semantically
equivalent packages: the same entry paths, manifest fields, entry digests, and
package id. ZIP timestamps and compression choices MAY differ.

An interoperability test suite SHOULD include:

- an unsigned minimal package;
- a signed package with one text and one binary entry;
- a multi-file page and an Ink single-file page;
- an empty-schema first page and a parameterized first page;
- localized widget metadata with locale fallback;
- malformed paths, duplicate normalized paths, missing `VERSION`, and invalid
  `app.json`;
- reordered manifest entries, an unsigned extra entry, a changed byte, a
  changed `VERSION`, and a wrong trusted key;
- ZIP CRC, size, and unsupported-compression failures.

## 16. Complete Minimal Example

The following is the smallest useful unsigned package model. The ZIP archive
contains exactly these application entries (the manifest is optional for an
unsigned package):

```text
VERSION                 = "550e8400-e29b-41d4-a716-446655440000"
app.json                = {"pages":["pages/index/index"]}
pages/index/index.json  = {
  "navigationBarTitleText": "Home",
  "schema": {"data": {"type": "object", "properties": {}}}
}
pages/index/index.wxml  = "<view style=\"width: 480px; height: 168px\" />"
```

The page has a `480 × 168` layout constraint. Because it is the first page and
its object schema has no properties, its derived tool record has an empty
parameter object and a `_blank` target. This conclusion is derived from the
format data; it does not require a particular API or runtime.
