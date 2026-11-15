# Optional diagram refinement with Codex

Help a reader find where a system starts, understand what each part does, and follow an important flow through the code. Offer **Original** as the deterministic baseline and **Refined** as an optional, user-approved view for Packages, Layers, the three C4 views, and the seven static UML diagrams. Activity and Sequence use static code analysis. Keep the controls and renderers specific to each diagram.

Oh My Mermaid documents the same codebase from several perspectives and lets the reader open a complex node to see its own diagram and explanation. In Nodex, start with existing diagram types, make each level answer a useful question, and use the same evidence links throughout the hierarchy. [Oh My Mermaid](https://github.com/oh-my-mermaid/oh-my-mermaid#how-it-works)

## 1. Ground each refinement in project evidence

Build the inventory before applying diagram display limits. Include symbols, entry points, manifests, observed dependencies, unresolved associations, and nearby documentation. For ambiguous modules, prioritize call sites, registrations, imports and configuration over folder names. In a large project, focus the selection on the requested scope and its relevant neighbors. Report what the context budget left out, and offer a narrower preview when evidence is sparse.

Have Codex propose short operational labels, responsibility summaries, key relationships, and evidence-backed groups. Packages, Layers, and C4 Components can group related files across directories and nest these groups. Layers can reclassify a module using its actual responsibilities. C4 can select an HTTP destination from the analyzer's candidates. For example, a handler in `utils` can belong to Application when its callers and behavior support that assignment. A URL with multiple matching listeners needs a distinguishing site or configuration entry before a destination is proposed as a validated association.

Represent changes as typed operations with references to the inventory. Validate scope, references, group topology, and operation-specific evidence before showing a selectable suggestion and again after selection. Give manual declarations precedence. Keep unresolved hypotheses visible under **Needs confirmation**, with the evidence that is missing. Where a recurring hypothesis falls outside the inventory, extend the local analyzer to produce a testable candidate before adding a corresponding operation.

## 2. Make each view readable

- Start with a short summary, named responsibilities, and a manageable overview of the important relationships. Make the remaining relationships accessible in the same view. Use descriptions and progressive disclosure in the deterministic views wherever the local evidence supports them.
- Open a group repeatedly until reaching a leaf in Packages, Layers, and C4 Components. At each level, show its children, summarize dependencies that cross the boundary, and keep a breadcrumb for returning to the overview. Use the established Context → Containers → Components navigation for C4.
- Use short, specific relationship labels when supported by evidence. Show the underlying call sites, declarations, and counts in the inspector, with a link back to the code. Let a reader tell observed dependencies, accepted interpretations, and unresolved claims apart.
- Preserve operation order, signatures and relationship semantics in static UML. Generate exports locally from the selected view and scope.

## 3. Approve, run, and save

Prepare a preview showing covered files, omissions, and the exact payload. Filter credential files, redact recognized sensitive values, and let the user remove excerpts or choose a narrower focus. Bind the job to the preview digest and the open project. A new payload requires approval before sending.

Use the installed Codex CLI with its existing authentication, discovered on PATH or configured by the user. Check support for the isolated invocation. Run from a temporary directory in a read-only ephemeral session with execution tools, connectors and search disabled. Send approved context on stdin and parse a schema-constrained JSON response. Allow one active job per project, with cancellation and a timeout. Expose authenticated preview, job, save and history endpoints.

Save only the selected, validated changes and their renderable snapshot in the project's ignored `.nodex/diagram-refinements/` directory. Use atomic writes, a file lock and revision checks. Load compatible versions offline. When the source or declarations change, retain the previous snapshot as a historical view and prepare fresh evidence for a new refinement.

## 4. Verify the outcome

Use fixtures with conventional layering, misleading folder and symbol names, an ambiguous HTTP route and absent integration evidence, plus a large enough project to exercise preview omissions. For each fixture, record the expected entry points, responsibilities and relationships before comparing Original and Refined on the same scope.

- Ask a reader unfamiliar with the fixture to locate an entry point, identify the responsible component, and follow a consequential dependency to its source. Record correct answers, time, and wrong turns in both views. For the non-conventional fixture, look for a correct classification or grouping supported by code and an easier path to the relevant source.
- Verify at least two nested group levels across folders, breadcrumbs back to the root, access to secondary relationships, and traceable dependencies at each level. Check all static diagram targets and exports. Ensure no unsupported relationship renders as a fact in the fixtures.
- Exercise exact preview consent, excerpt exclusion, cancellation, project switching, concurrent writes, outdated evidence and offline reactivation. Reject invalid IDs, cyclic groups and unsafe renderer input. Run the full Rust and web suites, TypeScript checks, and production builds. Verify a real desktop generation and save using an authorized synthetic project.
