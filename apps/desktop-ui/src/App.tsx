import { IntentLauncher } from "./features/intent/IntentLauncher.js";

export function App() {
  return (
    <main className="min-h-screen bg-slate-950 p-8 text-slate-100">
      <section className="mx-auto max-w-2xl border border-slate-700 p-6">
        <h1 className="text-xl font-semibold">Assistant Host</h1>
        <p className="mt-2 text-sm text-slate-300">
          Describe a task; the assistant plans and executes it under policy.
        </p>
      </section>
      <div className="mx-auto mt-6 max-w-2xl">
        <IntentLauncher />
      </div>
    </main>
  );
}
