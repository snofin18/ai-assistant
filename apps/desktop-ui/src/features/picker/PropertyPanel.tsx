import type { PickerCopy } from "./pickerCopy.js";
import type { PickerElement } from "./pickerModel.js";

interface PropertyPanelProps {
  copy: PickerCopy;
  element: PickerElement | null;
}

export function PropertyPanel({ copy, element }: PropertyPanelProps) {
  if (element === null) {
    return (
      <section className="border border-slate-700 p-3 text-sm text-slate-400">
        {copy.selectLabel}
      </section>
    );
  }
  return (
    <section className="border border-slate-700 p-3" aria-label={copy.propertiesHeading}>
      <h3 className="text-sm font-semibold">{copy.propertiesHeading}</h3>
      <dl className="mt-3 grid gap-2 text-xs sm:grid-cols-2">
        <Field label={copy.roleLabel} value={element.role} />
        <Field label={copy.nameLabel} value={element.name || copy.emptyValue} />
        <Field
          label={copy.automationIdLabel}
          value={element.automationId ?? copy.emptyValue}
        />
        <Field label={copy.classNameLabel} value={element.className ?? copy.emptyValue} />
        <Field label={copy.runtimeIdLabel} value={element.runtimeId ?? copy.emptyValue} />
        <Field
          label={copy.stateLabel}
          value={[
            `${copy.enabledLabel}: ${String(element.isEnabled)}`,
            `${copy.focusedLabel}: ${String(element.isFocused)}`,
            `${copy.keyboardFocusableLabel}: ${String(element.isKeyboardFocusable)}`
          ].join(" | ")}
        />
        <Field
          label={copy.actionsLabel}
          value={element.actions.join(", ") || copy.emptyValue}
        />
        <Field
          label={copy.patternsLabel}
          value={element.patterns.join(", ") || copy.emptyValue}
        />
        <Field
          label={copy.boundsLabel}
          value={`${element.bounds.x},${element.bounds.y} ${element.bounds.width}x${element.bounds.height}`}
        />
        <Field
          label={copy.parentPathLabel}
          value={
            element.parentPath
              .map((ancestor) => `${ancestor.role}:${ancestor.name}`)
              .join(" > ") || copy.emptyValue
          }
        />
      </dl>
    </section>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="font-medium text-slate-400">{label}</dt>
      <dd className="break-words font-mono text-slate-100">{value}</dd>
    </div>
  );
}
