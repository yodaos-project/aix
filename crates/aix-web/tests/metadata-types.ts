import type {
  AIX, FunctionDefinition, JsonSchema, JsonValue, MetadataResolution,
  PageConstraint, PageInfo, Tool, ToolTarget, WidgetInfo, WidgetPlacement,
} from '../dist/index.js';

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends
  (<T>() => T extends B ? 1 : 2) ? true : false;
type Assert<T extends true> = T;

// Exact keys and equality guard against optional fields, broad enums, and any.
type PageFields = Assert<Equal<keyof PageInfo, 'name' | 'title' | 'description' | 'data_schema' | 'size'>>;
type PageShape = Assert<Equal<PageInfo, {
  name: string; title: string | null; description: string | null;
  data_schema: JsonSchema; size: PageConstraint;
}>>;
type Layout = Assert<Equal<PageConstraint, { width: number; height: number }>>;
type WidgetShape = Assert<Equal<WidgetInfo, {
  path: string; family: string; placement: WidgetPlacement;
  displayName: string | null; description: string | null;
}>>;
type Target = Assert<Equal<ToolTarget, '_current' | '_blank'>>;
type Placement = Assert<Equal<WidgetPlacement, 'persistent' | 'overlay'>>;
type FunctionShape = Assert<Equal<FunctionDefinition, {
  name: string; description: string | null; parameters: JsonSchema;
}>>;
type ToolShape = Assert<Equal<Tool, {
  type: string; target: ToolTarget; layout: PageConstraint; function: FunctionDefinition;
}>>;
type PagesReturn = Assert<Equal<ReturnType<AIX['getPages']>, PageInfo[]>>;
type ToolsReturn = Assert<Equal<ReturnType<AIX['getTools']>, Tool[]>>;
type WidgetsReturn = Assert<Equal<ReturnType<AIX['getWidgets']>, WidgetInfo[]>>;
type MetadataReturn = Assert<Equal<ReturnType<AIX['resolveMetadata']>, MetadataResolution>>;
type TitleReturn = Assert<Equal<ReturnType<AIX['getTitle']>, string | undefined>>;
type VersionReturn = Assert<Equal<ReturnType<AIX['getVersion']>, string | undefined>>;

const schemas: JsonSchema[] = [null, false, true, 42, 'text', [null, { default: null }],
  { type: ['object', 'null'], properties: { value: { enum: [null, true, 1, 'text'] } } }];
// @ts-expect-error JSON cannot contain undefined.
const undefinedValue: JsonValue = { value: undefined };
// @ts-expect-error JSON cannot contain functions.
const functionValue: JsonValue = () => {};
// @ts-expect-error Serialized targets do not use the deserialization alias.
const alias: ToolTarget = 'blank';
// @ts-expect-error Placement is a closed enum.
const placement: WidgetPlacement = 'floating';
// @ts-expect-error Missing nullable fields are not omitted from serialized pages.
const page: PageInfo = { name: 'home', data_schema: null, size: { width: 480, height: 168 } };
// @ts-expect-error Schema payloads require narrowing before accessing object keys.
schemas[0].properties;
