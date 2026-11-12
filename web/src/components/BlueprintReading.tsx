import type { ReactNode } from "react";
import type { ReadingSite } from "../gen/ReadingSite";
import type { Span } from "../gen/Span";
import { formatType } from "../graph/blueprintTypes";
import { readingKey } from "../graph/blueprintReading";
import { originLabel } from "../graph/rustOwnership";

const actions = {
  Copy: "Copy", Move: "Move", Borrow: "Borrow to read", BorrowMut: "Borrow to change", ByValue: "Bind value",
} as const;

export function BlueprintReading({ sites, inspect, route, choose, preview, full = false, showDefinition = false }: {
  sites: readonly ReadingSite[];
  inspect: (site: ReadingSite, span: Span) => void;
  route?: (pin: string) => string | undefined;
  choose?: (site: ReadingSite, label: string, output: string, pin?: string) => void;
  preview?: (span: Span) => ReactNode;
  full?: boolean;
  showDefinition?: boolean;
}) {
  const readingClass = full ? "bp-reading bp-reading-full" : "bp-reading";
  return sites.map((site) => {
    const { content } = site;
    const open = (span: Span) => (event: React.MouseEvent) => {
      event.stopPropagation();
      inspect(site, span);
    };
    const key = readingKey(site);
    const code = preview?.(site.span);
    if (content.kind === "Steps") {
      return (
        <section key={key} className={readingClass} aria-label={content.label}>
          <div className="reading-heading"><button type="button" className="nodrag" onClick={open(site.span)}>{content.label}</button></div>
          {code}
          <ol className="reading-fork">
            {content.steps.map(({ span, timing, action }, index) => (
              <li key={index}>
                <div className="reading-outcome"><button type="button" className="nodrag" onClick={open(span)}>{timing}</button></div>
                <div className="reading-action" title={action}>{action}</div>
              </li>
            ))}
          </ol>
        </section>
      );
    }
    if (content.kind === "Paths") {
      const label = content.timing ?? (content.deferred ? "When called" : "Paths");
      const type = content.result && formatType(content.result.shape);
      return (
        <section key={key} className={`${readingClass} bp-reading-paths`} aria-label={label}>
          <div className="reading-heading">
            <button type="button" className="nodrag" onClick={open(site.span)}>{label}</button>
            {type && <code title={type}>{type}</code>}
          </div>
          {code}
          <div className="reading-input" title={content.input}>{content.input}</div>
          <ol className="reading-fork">
            {content.branches.map((branch) => (
              <li key={branch.label}>
                <div className="reading-outcome">
                  {choose ? <button type="button" onClick={() => choose(site, branch.label, branch.output, branch.pin)}>{branch.label}</button>
                    : <strong>{branch.label}</strong>}
                  <span aria-hidden="true">→</span><code title={branch.output}>{branch.output}</code>
                </div>
                <div className="reading-action" title={branch.action}>{branch.action}</div>
              </li>
            ))}
          </ol>
        </section>
      );
    }
    if (content.kind === "Pattern") {
      const type = content.input_type && formatType(content.input_type.shape);
      const arm = content.route_label ?? (content.route && route?.(content.route));
      return (
        <section key={key} className={`${readingClass} bp-reading-pattern`} aria-label="Unpack a pattern">
          {arm && <div className="reading-arm">{arm}</div>}
          <div className="reading-heading">
            <button type="button" className="nodrag" onClick={open(site.span)} title={content.input}>{content.input}</button>
          </div>
          {code}
          {type && <code className="reading-input-type" title={type}>{type}</code>}
          <ul className="reading-parts">
            {content.parts.map((part, index) => {
              const type = part.binding && formatType(part.type_info?.shape);
              const copiedReference = part.access === "Copy" && part.type_info?.shape.kind === "Indirect" && !part.type_info.shape.pointer;
              return (
                <li key={`${part.span.start_byte}:${index}`}>
                  <div className="reading-outcome">
                    <button type="button" className="nodrag" onClick={open(part.span)}
                      title={part.binding ? "Show the binding in source" : part.condition ? "Show the comparison in source" : "Show the pattern in source"}>
                      <code className="reading-pattern-code">{part.projection}</code><span aria-hidden="true"> → </span>
                      {part.condition && <>Match <code className="reading-pattern-code">{part.condition}</code></>}
                      {part.binding && <>{part.condition && " · bind "}<code>{part.binding}</code></>}
                      {!part.condition && !part.binding && "Ignore"}
                    </button>
                    {type && <code title={type}>{type}</code>}
                  </div>
                  {part.access && <div className="reading-action">{copiedReference ? "Copy the reference" : actions[part.access]}
                    {full && part.type_info?.native && <code className="reading-native"> · {part.type_info.native}</code>}
                  </div>}
                  {!!part.borrow_sources?.length && <div className="reading-action">{originLabel(part.borrow_sources)}</div>}
                </li>
              );
            })}
          </ul>
        </section>
      );
    }
    return (
      <section key={key} className={`${readingClass} bp-reading-detail`} aria-label={content.label}>
        <div className="reading-heading">
          <button type="button" className="nodrag" title={content.text} onClick={open(site.span)}>{content.label}</button>
        </div>
        {code}
        {!!site.borrow_sources?.length && <div className="reading-action">{originLabel(site.borrow_sources)}</div>}
        {full && <>
          <p className="explain">{content.text}</p>
          {showDefinition && content.definition && <button type="button" onClick={open(content.definition)}>Show macro definition</button>}
        </>}
      </section>
    );
  });
}
