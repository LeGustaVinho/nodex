// Loads a TypeScript/JavaScript project into one or more `ts.Program`s and keeps them up to
// date. Source files are cached across programs and rebuilds, so an update only re-parses the
// files that changed.

import * as fs from "node:fs";
import * as path from "node:path";
import ts from "typescript";

const IGNORED_DIRS = new Set([
  "node_modules", ".git", "dist", "build", "out", "coverage", ".next", ".turbo", ".nodex", "bin", "obj", "target",
]);
const CODE_FILE = /\.(ts|tsx|mts|cts|js|jsx|mjs|cjs)$/;

/** Options for folders without a tsconfig.json: read everything, check nothing strictly. */
const LOOSE_OPTIONS: ts.CompilerOptions = {
  allowJs: true,
  checkJs: false,
  jsx: ts.JsxEmit.Preserve,
  target: ts.ScriptTarget.ESNext,
  module: ts.ModuleKind.ESNext,
  moduleResolution: ts.ModuleResolutionKind.Bundler,
  noEmit: true,
  skipLibCheck: true,
  strict: false,
};

interface ProgramSpec {
  /** Config file, or null for loose files. */
  config: string | null;
  rootNames: string[];
  options: ts.CompilerOptions;
  references?: readonly ts.ProjectReference[];
  program?: ts.Program;
}

export class ProjectHost {
  readonly root: string;
  readonly messages: string[] = [];
  private specs: ProgramSpec[] = [];
  private cache = new Map<string, ts.SourceFile>();

  constructor(root: string) {
    this.root = path.resolve(root);
  }

  /** Finds the configurations and builds the programs. */
  open(): void {
    this.specs = this.discover();
    this.build();
  }

  /** Re-reads `changed` files (absolute paths) and rebuilds the programs incrementally. */
  update(changed: string[]): void {
    for (const file of changed) {
      for (const key of [...this.cache.keys()]) {
        if (samePath(key, file)) this.cache.delete(key);
      }
    }
    // Configs may list new files (or lose deleted ones): parse them again.
    for (const spec of this.specs) {
      if (spec.config) {
        const parsed = this.parseConfig(spec.config);
        if (parsed) {
          spec.rootNames = parsed.fileNames;
          spec.options = parsed.options;
        }
      } else {
        spec.rootNames = this.looseFiles();
      }
    }
    this.build();
  }

  /** New programs over the same files with changed options (for emitting an instrumented copy). */
  programsWith(override: (options: ts.CompilerOptions, loose: boolean) => ts.CompilerOptions): ts.Program[] {
    return this.specs.map((spec) => {
      const options = override(spec.options, spec.config === null);
      return ts.createProgram({ rootNames: spec.rootNames, options, projectReferences: spec.references, host: this.compilerHost(options) });
    });
  }

  /** `outDir` and `rootDir` of each configuration (to map built files back to sources). */
  layouts(): { outDir?: string; rootDir?: string }[] {
    return this.specs.map((s) => ({ outDir: s.options.outDir, rootDir: s.options.rootDir }));
  }

  /** Human-readable description of what was loaded. */
  describe(): string {
    const configs = this.specs.map((s) => (s.config ? path.relative(this.root, s.config).replace(/\\/g, "/") : "loose files"));
    return `TypeScript ${ts.version} · ${configs.join(", ")} · ${this.sourceFiles().length} files`;
  }

  /** Project source files (not declaration files, not dependencies) with their program. */
  sourceFiles(): { file: ts.SourceFile; program: ts.Program }[] {
    const seen = new Set<string>();
    const out: { file: ts.SourceFile; program: ts.Program }[] = [];
    for (const spec of this.specs) {
      const program = spec.program;
      if (!program) continue;
      for (const sf of program.getSourceFiles()) {
        if (sf.isDeclarationFile || seen.has(sf.fileName) || !this.isProjectFile(sf.fileName)) continue;
        seen.add(sf.fileName);
        out.push({ file: sf, program });
      }
    }
    return out;
  }

  /** Project-relative path with `/`, or null outside the project. */
  relative(fileName: string): string | null {
    const rel = path.relative(this.root, fileName);
    if (!rel || rel.startsWith("..") || path.isAbsolute(rel)) return null;
    return rel.replace(/\\/g, "/");
  }

  isProjectFile(fileName: string): boolean {
    const rel = this.relative(fileName);
    return rel !== null && !rel.split("/").some((part) => IGNORED_DIRS.has(part));
  }

  private build(): void {
    for (const spec of this.specs) {
      const host = this.compilerHost(spec.options);
      spec.program = ts.createProgram({
        rootNames: spec.rootNames,
        options: { ...spec.options, noEmit: true },
        projectReferences: spec.references,
        host,
        oldProgram: spec.program,
      });
    }
  }

  private compilerHost(options: ts.CompilerOptions): ts.CompilerHost {
    const host = ts.createCompilerHost(options, true);
    const read = host.getSourceFile.bind(host);
    host.getSourceFile = (fileName, languageVersion, onError, shouldCreate) => {
      const cached = this.cache.get(fileName);
      if (cached && !shouldCreate) return cached;
      const sf = read(fileName, languageVersion, onError, shouldCreate);
      if (sf) this.cache.set(fileName, sf);
      return sf;
    };
    return host;
  }

  private discover(): ProgramSpec[] {
    const rootConfig = ["tsconfig.json", "jsconfig.json"].map((n) => path.join(this.root, n)).find((p) => fs.existsSync(p));
    const configs: string[] = [];
    if (rootConfig) {
      configs.push(rootConfig);
    } else {
      this.findConfigs(this.root, 0, configs);
    }
    const specs: ProgramSpec[] = [];
    const visit = (config: string, depth: number) => {
      if (specs.some((s) => s.config && samePath(s.config, config))) return;
      const parsed = this.parseConfig(config);
      if (!parsed) return;
      specs.push({ config, rootNames: parsed.fileNames, options: parsed.options, references: parsed.projectReferences });
      // Solution-style configs (`"files": []` + references) describe their projects elsewhere.
      if (depth < 3) {
        for (const ref of parsed.projectReferences ?? []) {
          const refPath = ts.resolveProjectReferencePath(ref);
          if (fs.existsSync(refPath)) visit(refPath, depth + 1);
        }
      }
    };
    for (const c of configs) visit(c, 0);
    const useful = specs.filter((s) => s.rootNames.length > 0);
    if (useful.length === 0) {
      return [{ config: null, rootNames: this.looseFiles(), options: LOOSE_OPTIONS }];
    }
    return useful;
  }

  private parseConfig(config: string): ts.ParsedCommandLine | undefined {
    const parsed = ts.getParsedCommandLineOfConfigFile(config, { noEmit: true }, {
      ...ts.sys,
      onUnRecoverableConfigFileDiagnostic: (d) => {
        this.messages.push(`${path.basename(config)}: ${ts.flattenDiagnosticMessageText(d.messageText, " ")}`);
      },
    });
    return parsed;
  }

  private findConfigs(dir: string, depth: number, out: string[]): void {
    let entries: fs.Dirent[];
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    const own = entries.find((e) => e.isFile() && (e.name === "tsconfig.json" || e.name === "jsconfig.json"));
    if (own) {
      out.push(path.join(dir, own.name));
      return;
    }
    if (depth >= 3) return;
    for (const e of entries) {
      if (e.isDirectory() && !IGNORED_DIRS.has(e.name) && !e.name.startsWith(".")) {
        this.findConfigs(path.join(dir, e.name), depth + 1, out);
      }
    }
  }

  private looseFiles(): string[] {
    const out: string[] = [];
    const walk = (dir: string) => {
      let entries: fs.Dirent[];
      try {
        entries = fs.readdirSync(dir, { withFileTypes: true });
      } catch {
        return;
      }
      for (const e of entries) {
        const full = path.join(dir, e.name);
        if (e.isDirectory()) {
          if (!IGNORED_DIRS.has(e.name) && !e.name.startsWith(".")) walk(full);
        } else if (CODE_FILE.test(e.name) && !e.name.endsWith(".min.js")) {
          out.push(full);
        }
      }
    };
    walk(this.root);
    return out;
  }
}

function samePath(a: string, b: string): boolean {
  const norm = (p: string) => path.resolve(p).replace(/\\/g, "/");
  return process.platform === "win32" ? norm(a).toLowerCase() === norm(b).toLowerCase() : norm(a) === norm(b);
}
