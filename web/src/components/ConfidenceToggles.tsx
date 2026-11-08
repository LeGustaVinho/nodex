import { useUi } from "../store";

/** Which relations to show, by how sure Nodex is of them (Knowledge and UML views). */
export function ConfidenceToggles() {
  const confidences = useUi((s) => s.confidences);
  const toggle = useUi((s) => s.toggleConfidence);
  return (
    <div className="segmented" role="group" aria-label="Confidence">
      {(["Exact", "Inferred", "Unresolved"] as const).map((c) => (
        <button
          key={c}
          className={confidences.includes(c) ? "on" : ""}
          aria-pressed={confidences.includes(c)}
          title={{ Exact: "Proven by the code or a compiler", Inferred: "Deduced from a convention or a unique match", Unresolved: "Shown by the code, target unknown" }[c]}
          onClick={() => toggle(c)}
        >
          {c.toLowerCase()}
        </button>
      ))}
    </div>
  );
}
