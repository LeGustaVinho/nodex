import type { TypeInfo } from "../gen/TypeInfo";
import { formatType } from "../graph/blueprintTypes";
import { useDebug } from "../debug";
import type { StructureIndex } from "../graph/structureGraph";
import { useBlueprintReplay } from "../views/useBlueprintReplay";
import { useUi } from "../store";

const originLabel = {
  Declared: "Declared in code",
  Compiler: "Inferred by the compiler",
  Syntax: "Inferred from the code",
} satisfies Record<TypeInfo["origin"], string>;

export function TypeDetails({ info, native, revision }: { info?: TypeInfo; native?: string | null; revision?: string }) {
  const spelling = info?.native ?? native;
  const role = info?.role;
  return (
    <div className="type-detail">
      <div className="value-row">
        <span className="muted">Type</span> <code>{formatType(info?.shape)}</code>
        {info?.shape.kind === "Named" && <span className="muted">Named type</span>}
      </div>
      {spelling && <div className="value-row"><span className="muted">Native spelling</span> <code>{spelling}</code></div>}
      {info && <div className="value-row muted">{info.shape.kind === "Unknown" ? "Type undetermined" : originLabel[info.origin]}</div>}
      {role && (
        <>
          <div className="value-row"><span className="muted">Collection role</span> <code>{role.role}</code></div>
          <div className="type-evidence">
            <button onClick={() => useUi.getState().focusCode(role.insert, revision)}>Insertion · line {role.insert.start_line}</button>
            <button onClick={() => useUi.getState().focusCode(role.remove, revision)}>Removal · line {role.remove.start_line}</button>
          </div>
        </>
      )}
    </div>
  );
}

export function ExecutionTypes({ index }: { index?: StructureIndex }) {
  const { replay } = useBlueprintReplay(index);
  const step = replay?.page.steps[replay.index];
  const scopes = useDebug((s) => s.status.state === "paused" && s.frame !== null ? s.scopes : null);
  const reported = scopes?.flatMap((scope) => scope.variables.flatMap((variable) =>
    variable.ty?.trim() ? [{ scope: scope.name, key: `${scope.reference}:${variable.name}`, variable }] : []));
  return (
    <>
      {!!step?.observations?.length && (
        <div className="insp-section type-detail">
          <div className="section-title">Observed in Run · Step {step.index + 1}</div>
          {step.observations.map((observation, i) => (
            <div className="value-row" key={i}>
              <code>{observation.name}</code>
              <code title={observation.native}>{formatType(observation.shape)}</code>
              {observation.parameter && <span className="muted">parameter</span>}
            </div>
          ))}
        </div>
      )}
      {!!reported?.length && (
        <div className="insp-section type-detail">
          <div className="section-title">Reported by debugger</div>
          {reported.map(({ scope, key, variable }) => (
            <div className="value-row" key={key}>
              <code>{variable.name}</code>
              <code title={variable.ty ?? undefined}>{variable.type_shape ? formatType(variable.type_shape) : variable.ty}</code>
              <span className="muted">{scope}</span>
            </div>
          ))}
        </div>
      )}
    </>
  );
}
