import type { LanguageDetail } from "../graph/bindingDetails";

export function LanguageBadges({ details, onInspect }: {
  details: readonly LanguageDetail[];
  onInspect: (site?: string) => void;
}) {
  const [primary, ...extra] = details;
  if (!primary) return null;
  return (
    <span className="language-badges">
      <button type="button" className="pin-ptr nodrag" title={primary.explain}
        onClick={(event) => { event.stopPropagation(); onInspect(primary.site); }}>{primary.label}</button>
      {extra.length > 0 && <button type="button" className="pin-ptr nodrag"
        aria-label={`Show ${extra.length} more language details`}
        title={extra.map((detail) => detail.label).join(", ")}
        onClick={(event) => { event.stopPropagation(); onInspect(); }}>+{extra.length}</button>}
    </span>
  );
}
