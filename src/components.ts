import type { CheckRun, Project, Task } from "./data";
import { gateResolved } from "./store";
import { checkStamp, clock, esc, relative } from "./ui";

const ORDER: Task["phase"][] = ["plan", "implement", "verify", "reflect"];

export function phaseSpine(project: Project, task: Task, active: Task["phase"]): string {
  const current = ORDER.indexOf(task.phase);
  const locked = active === "reflect" && !gateResolved(task);
  const minutes = [
    `${Math.max(1, Math.round(task.minutes * 0.22))}m`,
    `${Math.max(1, Math.round(task.minutes * 0.55))}m`,
    `${Math.max(1, Math.round(task.minutes * 0.11))}m`,
    "—",
  ];
  const steps = ORDER.map((phase, index) => {
    const isActive = phase === active;
    const isDone = index < current || (index === current && phase !== active);
    const href = `#/p/${project.id}/task/${task.id}/${phase}`;
    const cls = `step${isActive ? " active" : isDone ? " done" : index > current ? " next" : ""}${phase === "reflect" && locked ? " locked" : ""}`;
    const inner = `<span class="step-name">${phase}</span><span class="step-time">${minutes[index]}</span>`;
    if (phase === "reflect" && locked && !isActive) {
      return `<span class="${cls}" aria-disabled="true">${inner}</span>`;
    }
    return `<a class="${cls}" href="${href}">${inner}</a>`;
  }).join("");
  return `<nav class="spine" aria-label="Task phases">${steps}</nav>`;
}

/// The definition of done, with what will actually run and what ran last time.
/// The table never decides a check: it renders the runner's verdict.
export function checkTable(
  task: Task,
  run: CheckRun | null,
  running: boolean,
  note: string | null,
): string {
  const outcomes = new Map((run?.results ?? []).map((result) => [result.checkId, result]));
  const rows = task.checks
    .map((check) => {
      const outcome = outcomes.get(check.id);
      const tail = outcome && outcome.result !== "pass" ? (outcome.stderrTail || outcome.stdoutTail).trim() : "";
      return `
      <tr>
        <td class="cell-name">${esc(check.label)}<span class="cell-sub"><code>${esc(check.command || "no command stored")}</code></span></td>
        <td class="cell-detail">${esc(check.detail)}</td>
        <td class="cell-mono">${outcome ? `exit ${outcome.exitCode ?? "—"} · ${outcome.durationMs} ms` : "—"}</td>
        <td class="cell-time">${check.evaluated ? `evaluated ${clock(check.evaluated)}` : "never run"}</td>
        <td class="cell-stamp">${checkStamp(check)}</td>
        <td class="cell-stamp"><button class="btn small ghost" type="button" data-remove-check="${check.id}">Remove</button></td>
      </tr>${
        tail
          ? `<tr><td class="cell-tail" colspan="6"><pre class="output">${esc(tail.slice(-4000))}</pre></td></tr>`
          : ""
      }`;
    })
    .join("");
  const failed = (run?.results ?? []).filter((result) => result.result === "fail").length;
  const summary = run
    ? run.status === "running"
      ? "Running now. The result is written when the last process exits."
      : run.status === "done"
        ? `${run.results.length - failed} of ${run.results.length} passed.${failed ? " The failing output is under the check." : ""}`
        : run.status === "cancelled"
          ? "Cancelled. No result was written, and no check changed."
          : "The run could not finish, so no verdict was written."
    : "Nothing has run since this task was planned.";
  return `
    <table class="table">
      <thead><tr><th>Check</th><th>What it proves</th><th>Last run</th><th>Evaluated</th><th>State</th><th></th></tr></thead>
      <tbody>${rows || `<tr><td class="cell-detail" colspan="6">No checks yet. A definition of done that cannot run is a wish.</td></tr>`}</tbody>
    </table>
    <p class="fine">${esc(summary)}</p>
    ${note ? `<div class="banner">${esc(note)}</div>` : ""}
    <div class="check-actions">
      <button class="btn" type="button" data-run-checks${running ? " disabled" : ""}>${running ? "Running checks" : "Run checks"}</button>
      ${running ? `<button class="btn ghost" type="button" data-cancel-checks>Cancel</button>` : ""}
      <span class="fine">Each check is one process from the allowlist, in the project folder. The exit status decides; a timeout or a cancel writes no result.</span>
    </div>`;
}

export function relativeLine(iso: string | null): string {
  return relative(iso);
}
