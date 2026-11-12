import type { ContractItem } from "../gen/ContractItem";
import type { Span } from "../gen/Span";
import { formatType } from "../graph/blueprintTypes";

export function RustContract({ items, inspect, compact = false }: {
  items: readonly ContractItem[];
  inspect: (span: Span) => void;
  compact?: boolean;
}) {
  return (
    <div className="bp-reading" aria-label="Rust contract">
      {items.map(({ group, text, span, type_info }, index) => {
        const type = type_info && formatType(type_info.shape);
        return (
          <div key={index} className={compact ? "reading-action" : "value-row"} title={text}>
            <strong>{group}</strong>{" "}
            <button type="button" className="link nodrag" onClick={(event) => { event.stopPropagation(); inspect(span); }}>
              {text}
            </button>
            {type && type !== "?" && <code> · {type}</code>}
            {!compact && type_info?.native && type_info.native !== type && <code> · {type_info.native}</code>}
          </div>
        );
      })}
    </div>
  );
}
