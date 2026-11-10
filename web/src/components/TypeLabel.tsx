import type { TypeInfo } from "../gen/TypeInfo";
import { formatType } from "../graph/blueprintTypes";

export function TypeLabel({ info, fallback, onInspect }: { info?: TypeInfo; fallback?: string | null; onInspect?: () => void }) {
  const label = formatType(info?.shape);
  const title = info ? label : fallback ?? label;
  return (
    <>
      {onInspect
        ? <button type="button" className="blueprint-type nodrag" title={title} aria-label={`Inspect type ${label}`}
            onClick={(event) => { event.stopPropagation(); onInspect(); }}>{label}</button>
        : <span className="blueprint-type" title={title}>{label}</span>}
      {info?.role && <span className="blueprint-role" title={`Collection used as a ${info.role.role.toLowerCase()}`}>{info.role.role} role</span>}
    </>
  );
}
