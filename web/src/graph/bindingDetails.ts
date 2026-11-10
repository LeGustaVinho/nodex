import type { Binding } from "../gen/Binding";
import type { Language } from "../gen/Language";

export interface LanguageDetail {
  label: string;
  explain: string;
  site?: string;
}

export function bindingDetails(binding: Binding, language: Language): LanguageDetail[] {
  const { form, reassignable, has_initializer, shadows } = binding;
  if (form === "RefReadonly") return [
    { label: "Read-only alias", explain: "You can read the referenced value. You cannot assign a value through this alias." },
    { label: "Can rebind", explain: "Use ref assignment (alias = ref other) to point this alias at another variable." },
  ];
  const rule: LanguageDetail = reassignable
    ? {
        label: "Can reassign",
        explain: language === "rust"
          ? "The mut keyword lets you assign another value to this name."
          : "You can assign another value to this name.",
      }
    : {
        label: "Assign once",
        explain: language === "rust"
          ? "This name can receive its initial value once. Access through a reference has its own rules."
          : "You cannot assign another value to this name. Properties on the object may still be writable.",
      };

  const details = [rule];
  if (shadows) details.unshift({
    label: "New binding",
    explain: "This declaration introduces a new variable with the same name. Earlier uses still refer to the previous variable.",
  });
  if (form === "Inferred") details.unshift({
    label: "Inferred type", explain: "The compiler uses the initializer to determine the local's type.",
  });
  if (form === "Var") details.unshift({
    label: "var scope", explain: "Inside a function, var belongs to the function. A new var binding starts as undefined. A redeclaration without an initializer preserves the current value.",
  });
  if (form === "Let" && !has_initializer) details.push(language === "rust"
    ? { label: "Initialized later", explain: "This declaration has no initial value. Every path must assign it before reading it." }
    : { label: "Starts undefined", explain: "After this declaration runs, the variable holds undefined until an assignment gives it another value." });
  if (form === "Const" && language === "csharp") details[0] = {
    label: "Compile-time value",
    explain: "The compiler evaluates this constant. Its value is fixed wherever it is used.",
  };
  if (form === "Using") details.push({
    label: "Disposed at scope exit",
    explain: "The resource is cleaned up when control leaves this scope, including early exits.",
  });
  return details;
}
