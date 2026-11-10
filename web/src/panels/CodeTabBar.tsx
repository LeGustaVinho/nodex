import { useLayoutEffect, useRef } from "react";
import type { CloseCodeTabs } from "../codeTabs";
import { openMenu, type MenuItem } from "../components/ContextMenu";
import { absoluteFilePath, fileMenuItems } from "../fileMenu";
import type { ProjectSummary } from "../gen/ProjectSummary";
import { sidebarSymbolsInFile } from "../graph/sidebarSymbols";
import type { StructureIndex } from "../graph/structureGraph";
import { useUi } from "../store";

const CLOSE_OPTIONS: { label: string; mode: CloseCodeTabs }[] = [
  { label: "Close", mode: "one" },
  { label: "Close Other Tabs", mode: "other" },
  { label: "Close All Tabs", mode: "all" },
  { label: "Close Unmodified Tabs", mode: "unmodified" },
  { label: "Close Tabs to the Left", mode: "left" },
  { label: "Close Tabs to the Right", mode: "right" },
];

export function codeTabMenuItems(file: string, project: ProjectSummary | undefined, index: StructureIndex | undefined): MenuItem[] {
  const { codeTabs } = useUi.getState();
  const at = codeTabs.indexOf(file);
  const symbols = sidebarSymbolsInFile(index, file);
  const hasFile = !!project?.files.some((entry) => entry.path === file);
  return [
    ...CLOSE_OPTIONS.map(({ label, mode }) => ({
      label,
      disabled: (mode === "other" && codeTabs.length < 2) || (mode === "left" && at === 0) || (mode === "right" && at === codeTabs.length - 1),
      run: () => useUi.getState().closeCodeTabs(file, mode),
    })),
    ...fileMenuItems(file),
    {
      label: "Select in Files",
      disabled: !hasFile,
      run: () => useUi.getState().revealInSidebar({ tab: "files", file }),
    },
    {
      label: "Select in Symbols",
      disabled: !symbols?.size,
      children: symbols && [...symbols.values()].map(({ symbol }) => ({
        label: symbol.qualified_name,
        hint: [symbol.signature, `Line ${symbol.origins.find((origin) => origin.file === file)?.start_line}`].filter(Boolean).join(" · "),
        run: () => useUi.getState().revealInSidebar({ tab: "symbols", file, id: symbol.id }),
      })),
    },
  ];
}

export function CodeTabBar({ project, index }: { project: ProjectSummary | undefined; index: StructureIndex | undefined }) {
  const tabs = useUi((s) => s.codeTabs);
  const active = useUi((s) => s.activeCodeFile);
  const buttons = useRef(new Map<string, HTMLButtonElement>());
  const dragging = useRef<string | null>(null);
  const bar = useRef<HTMLDivElement>(null);
  const names = new Map<string, number>();
  for (const file of tabs) {
    const name = file.split("/").at(-1)!;
    names.set(name, (names.get(name) ?? 0) + 1);
  }

  useLayoutEffect(() => {
    const tab = active && buttons.current.get(active);
    const strip = bar.current;
    if (!tab || !strip) return;
    const tabRect = tab.parentElement!.getBoundingClientRect();
    const stripRect = strip.getBoundingClientRect();
    if (tabRect.left < stripRect.left) strip.scrollLeft += tabRect.left - stripRect.left;
    else if (tabRect.right > stripRect.right) strip.scrollLeft += tabRect.right - stripRect.right;
  }, [tabs, active]);

  return (
    <div className="code-tabs" role="tablist" aria-label="Open files" ref={bar}
      onDragOver={(e) => {
        if (!dragging.current) return;
        const { left, right } = e.currentTarget.getBoundingClientRect();
        if (e.clientX - left < 24) e.currentTarget.scrollLeft -= 16;
        else if (right - e.clientX < 24) e.currentTarget.scrollLeft += 16;
      }}
    >
      {tabs.map((file, at) => {
        const name = file.split("/").at(-1)!;
        const label = (names.get(name) ?? 0) > 1 ? file : name;
        const selected = file === active;
        return (
          <div className={`code-tab ${selected ? "active" : ""}`} key={file} role="presentation" draggable
            title={project?.root ? absoluteFilePath(project.root, file) : file}
            onContextMenu={(e) => openMenu(e, codeTabMenuItems(file, project, index))}
            onDragStart={(e) => {
              dragging.current = file;
              e.dataTransfer.effectAllowed = "move";
              e.dataTransfer.setData("text/plain", file);
            }}
            onDragEnd={() => { dragging.current = null; }}
            onDragOver={(e) => {
              if (!dragging.current) return;
              e.preventDefault();
              e.dataTransfer.dropEffect = "move";
            }}
            onDrop={(e) => {
              const dragged = dragging.current;
              if (!dragged) return;
              e.preventDefault();
              dragging.current = null;
              const rect = e.currentTarget.getBoundingClientRect();
              useUi.getState().moveCodeTab(dragged, file, e.clientX > rect.left + rect.width / 2);
            }}
          >
            <button
              ref={(button) => { if (button) buttons.current.set(file, button); else buttons.current.delete(file); }}
              type="button"
              className="code-tab-label"
              role="tab"
              aria-selected={selected}
              aria-controls={selected ? "source-code-panel" : undefined}
              tabIndex={selected ? 0 : -1}
              onClick={() => useUi.getState().openCodeFile(file)}
              onKeyDown={(e) => {
                const state = useUi.getState();
                if (e.key === "Delete") {
                  e.preventDefault();
                  state.closeCodeTabs(file, "one");
                  const next = useUi.getState().activeCodeFile;
                  if (next) buttons.current.get(next)?.focus();
                } else if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
                  const rect = e.currentTarget.getBoundingClientRect();
                  openMenu({
                    clientX: rect.left, clientY: rect.bottom,
                    preventDefault: () => e.preventDefault(), stopPropagation: () => e.stopPropagation(),
                  }, codeTabMenuItems(file, project, index));
                } else if (e.key === "ArrowLeft" || e.key === "ArrowRight" || e.key === "Home" || e.key === "End") {
                  e.preventDefault();
                  const right = e.key === "ArrowRight";
                  const reorder = (right || e.key === "ArrowLeft") && e.ctrlKey && e.shiftKey;
                  const target = e.key === "Home" ? 0 : e.key === "End" ? tabs.length - 1 : at + (right ? 1 : -1);
                  if (reorder) {
                    if (tabs[target]) state.moveCodeTab(file, tabs[target], right);
                  } else {
                    const next = tabs[(target + tabs.length) % tabs.length];
                    state.openCodeFile(next);
                    buttons.current.get(next)?.focus();
                  }
                }
              }}
            >{label}</button>
            <button type="button" className="code-tab-close" tabIndex={-1} aria-label={`Close ${file}`} onClick={() => useUi.getState().closeCodeTabs(file, "one")}>×</button>
          </div>
        );
      })}
    </div>
  );
}
