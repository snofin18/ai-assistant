import type { ApprovalCardModel, AuthorizationScope } from "./approvalModel.js";
import type { ApprovalCardCopy } from "./approvalCopy.js";

interface AuthorizationScopePickerProps {
  model: ApprovalCardModel;
  selectedScope: AuthorizationScope | null;
  copy: ApprovalCardCopy;
  onSelectScope: (scope: AuthorizationScope) => void;
}

export function AuthorizationScopePicker({
  model,
  selectedScope,
  copy,
  onSelectScope,
}: AuthorizationScopePickerProps) {
  return (
    <fieldset className="border border-slate-700 p-3">
      <legend className="px-1 text-sm font-medium">{copy.authorizationScopeLabel}</legend>
      <div className="flex flex-wrap gap-3">
        {model.scopeOptions.map((scope) => (
          <label className="flex items-center gap-2 text-sm" key={scope}>
            <input
              checked={selectedScope === scope}
              name={`approval-scope-${model.requestId}`}
              onChange={() => onSelectScope(scope)}
              type="radio"
              value={scope}
            />
            <span>{copy.scopeLabels[scope]}</span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}
