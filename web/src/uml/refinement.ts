import type { ArchitectureView } from "../gen/ArchitectureView";
import type { Layer } from "../gen/Layer";
import type { Span } from "../gen/Span";
import type { UmlDiagram } from "../gen/UmlDiagram";
import type { UmlKind } from "../gen/UmlKind";
import type { UmlOptions } from "../gen/UmlOptions";
import type { UmlScope } from "../gen/UmlScope";

export type RefinementTarget =
  | { kind: "architecture"; view: ArchitectureView; scope: UmlScope; container: string | null }
  | { kind: "packages"; scope: UmlScope }
  | { kind: "uml"; diagram: UmlKind; scope: UmlScope; options: Partial<UmlOptions> };

export type RefinementExcerpt = { path: string; line: number; revision: string };

export interface RefinementPreview {
  id: string;
  digest: string;
  input: string;
  omitted: number;
  snippets: (RefinementExcerpt & { id: string; text: string })[];
  covered_files: string[];
  focus_nodes: [string, string][];
}

export interface Evidence {
  id: string;
  text: string;
  path: string | null;
  site: Span | null;
}

export interface RefinedNode {
  id: string;
  label: string;
  kind: string;
  parent: string | null;
  layer: Layer | null;
  declared: boolean;
  code: UmlScope | null;
  evidence: string[];
}

export interface RefinedEdge {
  id: string;
  from: string;
  to: string;
  label: string;
  kind: string;
  confidence: "Exact" | "Inferred" | "Unresolved";
  count: number;
  evidence: string[];
}

export interface Reviewed {
  id: number;
  suggestion: { kind: string; target: string; text: string; members: string[]; evidence: string[] };
  error: string | null;
}

export type RefinementProgress =
  | { phase: "running" | "cancelled" }
  | { phase: "failed"; message: string }
  | { phase: "ready"; suggestions: Reviewed[] };

export interface Refined {
  title: string;
  nodes: RefinedNode[];
  edges: RefinedEdge[];
  evidence: Evidence[];
  suggestions: Reviewed[];
  pending: string[];
  uml: UmlDiagram | null;
}

export interface RefinementVersion {
  id: string;
  created: number;
  basis: string;
  diagram: Refined;
}

export interface RefinementHistory {
  revision: string;
  current: string | null;
  active: string | null;
  versions: RefinementVersion[];
}

export type RefinementSave =
  | { action: "approve"; job: string; selected: number[]; revision: string }
  | { action: "activate"; target: RefinementTarget; version: string; revision: string };
