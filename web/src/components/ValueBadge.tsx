import type { ValueInfo } from "../gen/ValueInfo";
import type { PassingMode } from "../gen/PassingMode";
import type { ValueKind } from "../gen/ValueKind";
import type { Language } from "../gen/Language";

const MARK: Record<ValueKind, string> = { Value: "V", Reference: "R", Pointer: "P", Mixed: "V/R", Unknown: "?" };
const NAME: Record<ValueKind, string> = { Value: "Value", Reference: "Reference", Pointer: "Pointer", Mixed: "Value or reference", Unknown: "Unknown" };
const PASS: Record<PassingMode, string> = { ByValue: "By value", Ref: "ref", In: "in", Out: "out", RefReadonly: "ref readonly" };

function representation(info?: ValueInfo) {
  switch (info?.qualifier) {
    case "Object": return "A reference identifies an object. Copying it gives access to the same object.";
    case "Alias": return "This name provides access to another storage location.";
    case "SharedBorrow": return "A shared reference gives access to a value owned elsewhere.";
    case "ExclusiveBorrow": return "An exclusive reference permits writing through it.";
    case "Raw": return "A raw pointer holds an address. Its validity depends on the code using it.";
    case "Unique": return "This pointer owns the value it points to.";
    case "Shared": return "Strong handles share ownership of the value.";
    case "Weak": return "This pointer can observe the allocation while a strong owner keeps it alive.";
  }
  switch (info?.kind) {
    case "Value": return "The pin carries the value itself.";
    case "Mixed": return "The type admits both direct values and object references.";
    case "Reference": return "The pin provides indirect access to a value.";
    case "Pointer": return "The pin carries a pointer.";
    default: return "Available code and type information leave this classification open.";
  }
}

function transfer(info: ValueInfo, language: Language) {
  switch (info.passing) {
    case "Ref": return "A writable alias gives the receiver access to the supplied storage.";
    case "In": return "The receiver can read through the alias. The compiler may bind it to a temporary.";
    case "Out": return "The receiver must assign the storage before returning normally.";
    case "RefReadonly": return "The receiver can read the storage through this alias.";
    case "ByValue":
      if (info.qualifier === "Object") return "The receiver gets a copy of the reference and can access the same object. Reassigning the parameter changes its own binding.";
      return language === "rust" ? "The receiver takes the value. Its type determines whether this copies or moves it." : "The receiver gets its own value slot.";
    default: return undefined;
  }
}

export function ValueBadge({ info, onInspect }: { info?: ValueInfo; onInspect: () => void }) {
  const kind = info?.kind ?? "Unknown";
  const label = `${NAME[kind]}${info?.passing && info.passing !== "ByValue" ? ` · ${PASS[info.passing]}` : ""}`;
  return <button type="button" className={`pin-ptr nodrag value-kind value-${kind.toLowerCase()}`}
    aria-label={label} title={`${label} · ${representation(info)}`}
    onClick={(event) => { event.stopPropagation(); onInspect(); }}>
    {MARK[kind]}{info?.passing && info.passing !== "ByValue" && <small> {PASS[info.passing]}</small>}
  </button>;
}

export function ValueExplanation({ info, language, transferText }: { info?: ValueInfo; language: Language; transferText?: string }) {
  const kind = info?.kind ?? "Unknown";
  const passing = transferText ?? (info ? transfer(info, language) : undefined);
  return <>
    <div className="value-row"><strong>Representation</strong> <span className="badge">{NAME[kind]}</span>
      {info?.basis === "TypeAnalysis" && <span className="muted">Type analysis</span>}
    </div>
    <p className="explain">{representation(info)}</p>
    {passing && <><div className="value-row"><strong>Transfer</strong>{info?.passing && <span className="badge">{PASS[info.passing]}</span>}</div>
      <p className="explain">{passing}</p></>}
  </>;
}
