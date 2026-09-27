import type { CSSProperties } from "react";

import type { PickerCopy } from "./pickerCopy.js";
import {
  getElementById,
  getHighlightRect
} from "./pickerModel.js";
import { PropertyPanel } from "./PropertyPanel.js";
import { SelectorCandidateList } from "./SelectorCandidateList.js";
import { useElementPicker } from "./useElementPicker.js";

interface ElementPickerProps {
  model: unknown;
  copy: PickerCopy;
  onSelectElement?: (elementId: string) => void;
}

export function ElementPicker({ model, copy, onSelectElement }: ElementPickerProps) {
  const [state, dispatch] = useElementPicker(model);
  if (!state.parseResult.ok) {
    return (
      <div className="border border-red-500 bg-red-950/40 p-3 text-sm text-red-100" role="alert">
        <strong>{copy.validationErrorLabel}</strong>
        <ul className="mt-2 list-disc pl-5">
          {state.parseResult.errors.map((error) => (
            <li key={error}>{error}</li>
          ))}
        </ul>
      </div>
    );
  }
  const snapshot = state.parseResult.snapshot;
  const hoveredElement = state.hoveredElementId === null
    ? null
    : getElementById(snapshot, state.hoveredElementId);
  const selectedElement = state.selectedElementId === null
    ? null
    : getElementById(snapshot, state.selectedElementId);
  const highlightedElement = hoveredElement ?? selectedElement;
  const highlight = highlightedElement === null
    ? null
    : getHighlightRect(highlightedElement, snapshot.workspaceBounds);
  return (
    <section className="grid gap-3" aria-label={copy.heading}>
      <h2 className="text-lg font-semibold">{copy.heading}</h2>
      <div className="grid gap-3 lg:grid-cols-[minmax(0,2fr)_minmax(18rem,1fr)]">
        <div className="relative min-h-72 border border-slate-600 bg-slate-950" aria-label={copy.workspaceLabel}>
          {highlight === null ? null : (
            <div
              className="pointer-events-none absolute border-2 border-cyan-400 bg-cyan-400/10"
              data-highlighted-element={highlightedElement?.elementId}
              style={highlightStyle(highlight)}
            />
          )}
          <div className="absolute inset-x-0 bottom-0 max-h-44 overflow-auto border-t border-slate-700 bg-slate-900/95 p-2">
            <h3 className="sr-only">{copy.elementListLabel}</h3>
            <ul className="grid gap-1">
              {[snapshot.window, ...snapshot.elements].map((element) => (
                <li key={element.elementId}>
                  <button
                    aria-pressed={state.selectedElementId === element.elementId}
                    className="w-full border border-transparent px-2 py-1 text-left text-xs hover:border-cyan-500 hover:bg-slate-800 focus:border-cyan-500"
                    onClick={() => {
                      dispatch({ type: "select_element", elementId: element.elementId });
                      onSelectElement?.(element.elementId);
                    }}
                    onFocus={() =>
                      dispatch({ type: "hover_element", elementId: element.elementId })
                    }
                    onMouseEnter={() =>
                      dispatch({ type: "hover_element", elementId: element.elementId })
                    }
                    onMouseLeave={() => dispatch({ type: "hover_element", elementId: null })}
                    type="button"
                  >
                    <span className="font-mono text-cyan-200">{element.role}</span>
                    <span className="ml-2 text-slate-300">
                      {element.name || element.automationId || element.elementId}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        </div>
        <div className="grid content-start gap-3">
          <PropertyPanel copy={copy} element={selectedElement ?? hoveredElement} />
          <button
            className="border border-cyan-600 px-3 py-2 text-sm disabled:opacity-40"
            disabled={selectedElement === null}
            onClick={() => dispatch({ type: "generate_candidates" })}
            type="button"
          >
            {copy.generateLabel}
          </button>
          <SelectorCandidateList candidates={state.candidates} copy={copy} />
          {state.errorKey === null ? null : (
            <p className="text-sm text-red-200" role="alert">
              {copy.errorMessages[state.errorKey] ?? state.errorKey}
            </p>
          )}
        </div>
      </div>
    </section>
  );
}

function highlightStyle(rect: {
  leftPercent: number;
  topPercent: number;
  widthPercent: number;
  heightPercent: number;
}): CSSProperties {
  return {
    left: `${rect.leftPercent}%`,
    top: `${rect.topPercent}%`,
    width: `${rect.widthPercent}%`,
    height: `${rect.heightPercent}%`
  };
}
