import type { PlanItem, PlanItemKind, Gate, Phase, Project, Task } from "../data";
import {
  acceptPlanProposal,
  addCheck,
  addPlanItem,
  addTaskNote,
  answerGate,
  approvePlan,
  attachConcept,
  cancelChecks,
  checksMet,
  clock,
  closeImplement,
  commit,
  deletePlanItem,
  detachConcept,
  dismissPlanProposal,
  draftGate,
  errorMessage,
  findProject,
  findTask,
  gateResolved,
  getState,
  movePlanItem,
  overrideGate,
  pendingGate,
  recordAttempt,
  removeCheck,
  runChecks,
  sendPlanMessage,
  suggestPlan,
  togglePlanItem,
  ui,
  updatePlanItem,
  useHint,
  writeHandoff,
  writePrediction,
} from "../store";

/// The programs a check may name. Mirrors the domain allowlist; the backend
/// refuses anything else, so this list is a convenience, not the rule.
const PROGRAMS = ["cargo", "bun", "npm", "node", "tsc", "pytest", "go"];
import {
  checkStamp,
  chunk,
  emptyState,
  esc,
  gateStamp,
  pageHead,
  type View,
} from "../ui";
import { checkTable, phaseSpine } from "../components";

function arena(project: Project, task: Task, active: Phase, doc: string): string {
  return `<div class="page">
    ${pageHead(task.title, [project.name, `task ${task.batch}`, `lane ${task.lane}`], `<a class="btn ghost" href="#/p/${project.id}">Tasks</a>`)}
    ${phaseSpine(project, task, active)}
    <div class="workspace">
      <div class="doc task-doc">${doc}</div>
    </div>
  </div>`;
}

function notFound(): View {
  return {
    html: `<div class="page"><div class="doc">${pageHead("Not found", ["no such task"])}${emptyState("That task is gone", "It may have been removed when the mock was reset.", '<a class="btn" href="#/projects">All projects</a>')}</div></div>`,
  };
}

function planChildren(task: Task, sectionId: string): PlanItem[] {
  return task.planItems
    .filter((item) => item.kind === "subtask" && item.parentId === sectionId)
    .sort((a, b) => a.position - b.position);
}

function planRoot(task: Task, kind: PlanItemKind): PlanItem[] {
  return task.planItems
    .filter((item) => item.kind === kind && item.parentId === null)
    .sort((a, b) => a.position - b.position);
}

function planControls(item: PlanItem): string {
  return `<span class="plan-controls">
    <button class="plan-ctrl" type="button" data-plan-move="${esc(item.id)}" data-direction="-1" aria-label="Move up">↑</button>
    <button class="plan-ctrl" type="button" data-plan-move="${esc(item.id)}" data-direction="1" aria-label="Move down">↓</button>
    <button class="plan-ctrl" type="button" data-plan-edit-item="${esc(item.id)}">edit</button>
    <button class="plan-ctrl danger" type="button" data-plan-delete="${esc(item.id)}" aria-label="Remove">×</button>
  </span>`;
}

function planEditor(item: PlanItem): string {
  return `<form class="form plan-edit" data-plan-save="${esc(item.id)}">
    <label class="field-label" for="pe-${esc(item.id)}">${item.kind === "section" ? "Section title" : "Plan item"}</label>
    <input class="input" id="pe-${esc(item.id)}" name="title" value="${esc(item.title)}" required maxlength="200" />
    <textarea class="textarea plan-body-input" name="body" rows="2" placeholder="Outcome, constraint, or evidence (optional)">${esc(item.body)}</textarea>
    <div class="actions"><button class="btn small" type="submit">Save</button><button class="btn small ghost" type="button" data-plan-cancel>Cancel</button></div>
  </form>`;
}

function planCheckLabel(task: Task, item: PlanItem): string {
  const check = task.checks.find((entry) => entry.id === item.checkId);
  if (!check) return `<span class="plan-backing manual">manual</span>`;
  return `<span class="plan-backing check">check · ${esc(check.label)}</span> ${checkStamp(check)}`;
}

export function planView(projectId: string, taskId: string): View {
  const project = findProject(projectId);
  const task = findTask(taskId);
  if (!project || !task) return notFound();
  const modelOff = getState().settings.model === "none";
  const modelName = getState().settings.modelName ?? "the local model";
  const sections = planRoot(task, "section");
  const dods = planRoot(task, "dod");
  const dodsMet = dods.filter((item) => item.status === "done").length;
  const editingDescription = ui.planEditing === "description";
  const canEditDescription = task.phase === "plan";
  const canApprove = Boolean(task.prediction) && dods.length > 0;
  const canChat = Boolean(task.prediction) && !modelOff;
  const subtaskCount = task.planItems.filter((item) => item.kind === "subtask").length;

  const sectionHtml = sections
    .map((section, index) => {
      const subtasks = planChildren(task, section.id);
      const editing = ui.planEditing === section.id;
      return `<section class="plan-section-block" data-plan-section="${esc(section.id)}">
        <div class="plan-section-head">
          <h3><span class="plan-index">${index + 1}.</span> ${esc(section.title)}</h3>
          ${planControls(section)}
        </div>
        ${editing ? planEditor(section) : section.body ? `<p class="plan-section-body">${esc(section.body)}</p>` : ""}
        <ul class="plan-subtasks">
          ${subtasks
            .map((subtask) => {
              const subEditing = ui.planEditing === subtask.id;
              return `<li class="plan-item ${subtask.status === "done" ? "done" : ""}">
                <input class="plan-check" type="checkbox" data-plan-toggle="${esc(subtask.id)}"${subtask.status === "done" ? " checked" : ""} aria-label="Complete ${esc(subtask.title)}" />
                <div class="plan-item-main">
                  ${subEditing ? planEditor(subtask) : `<div class="plan-item-title">${esc(subtask.title)}${subtask.source === "model" ? '<span class="plan-source">model</span>' : ""}</div>${subtask.body ? `<p class="plan-item-body">${esc(subtask.body)}</p>` : ""}`}
                </div>
                ${subEditing ? "" : planControls(subtask)}
              </li>`;
            })
            .join("") || `<li class="plan-empty-line">No subtasks yet.</li>`}
        </ul>
        <form class="form plan-add-inline" data-plan-add="subtask" data-parent="${esc(section.id)}">
          <input class="input grow" name="title" required maxlength="200" placeholder="An outcome or decision, not a code step" />
          <button class="btn small ghost" type="submit">Add subtask</button>
        </form>
      </section>`;
    })
    .join("");

  const dodHtml = dods
    .map((item) => {
      const editing = ui.planEditing === item.id;
      return `<li class="plan-item dod-item ${item.status === "done" ? "done" : ""}">
        <input class="plan-check" type="checkbox" data-plan-toggle="${esc(item.id)}"${item.status === "done" ? " checked" : ""} aria-label="Met: ${esc(item.title)}" />
        <div class="plan-item-main">
          ${editing ? planEditor(item) : `<div class="plan-item-title">${esc(item.title)}${item.source === "model" ? '<span class="plan-source">model</span>' : ""}</div>${item.body ? `<p class="plan-item-body">${esc(item.body)}</p>` : ""}<div class="plan-dod-meta">${planCheckLabel(task, item)}</div>`}
        </div>
        ${editing ? "" : planControls(item)}
      </li>`;
    })
    .join("");

  const proposal = ui.planProposal
    ? `<div class="plan-proposal">
        <div class="plan-proposal-head"><h3>Suggested plan</h3><span>from <code>${esc(ui.planProposal.model || "the local model")}</code></span></div>
        <p class="fine">These stay above the code. Nothing here is part of the plan until you add it.</p>
        ${ui.planProposal.sections
          .map(
            (section) => `<div class="proposal-section"><h4>${esc(section.title)}</h4><ul>${section.subtasks.map((subtask) => `<li>${esc(subtask.title)}${subtask.body ? `<span class="fine"> — ${esc(subtask.body)}</span>` : ""}</li>`).join("")}</ul></div>`,
          )
          .join("")}
        ${ui.planProposal.dods.length ? `<div class="proposal-section"><h4>Definition of done</h4><ul>${ui.planProposal.dods.map((dod) => `<li>${esc(dod.title)}</li>`).join("")}</ul></div>` : ""}
        <form class="actions" data-accept-proposal>
          <button class="btn" type="submit">Add these suggestions</button>
          <button class="btn ghost" type="button" data-dismiss-proposal>Discard</button>
        </form>
      </div>`
    : "";

  const doc = `<article class="plan-doc">
    <header class="plan-lead">
      <p class="lede">One living document for the plan phase: the outcome-level approach, the order of the work, what done will look like, and the discussion that shaped it.</p>
      <p class="fine plan-altitude">Outcomes, boundaries and evidence only. Files, symbols, schemas and commands belong in implement.</p>
      <dl class="facts inline">
        <div><dt>Scope</dt><dd><code>${esc(task.scope)}</code></dd></div>
        <div><dt>Task</dt><dd>${esc(task.batch)}</dd></div>
        <div><dt>Lane</dt><dd>${esc(task.lane)}</dd></div>
        ${task.planApproved ? `<div><dt>Plan</dt><dd>${task.planRevised ? "approved · revised" : "approved"}</dd></div>` : `<div><dt>Plan</dt><dd>draft</dd></div>`}
      </dl>
    </header>

    <section class="plan-section" data-plan-description>
      <div class="section-label"><h2>Description</h2><span>${task.prediction ? "outcome level · before the model" : "owed"}</span>${task.prediction && canEditDescription && !editingDescription ? `<button class="plan-ctrl" type="button" data-plan-edit-description>edit</button>` : ""}</div>
      ${
        editingDescription
          ? `<form class="form plan-edit" data-plan-approach>
              <label class="field-label" for="plan-prediction">The approach, above the code</label>
              <textarea class="textarea answer" id="plan-prediction" name="prediction" rows="4" required placeholder="What should become true for the user or system, and what must stay intact?">${esc(task.prediction)}</textarea>
              <div class="actions"><button class="btn small" type="submit">Save approach</button><button class="btn small ghost" type="button" data-plan-cancel>Cancel</button></div>
            </form>`
          : task.prediction
            ? `<p class="plan-prose">${esc(task.prediction)}</p><p class="fine">Written before any model output. It stays visible through verify, so being wrong is information. It describes outcomes, not implementation.</p>`
            : canEditDescription
              ? `<form class="form" data-plan-approach>
                  <label class="field-label" for="plan-prediction">What should become true, and why?</label>
                  <textarea class="textarea answer" id="plan-prediction" name="prediction" rows="4" required placeholder="A failed delivery should be retried without creating a second record, because ..."></textarea>
                  <div class="actions"><button class="btn" type="submit">Record approach</button><span class="fine">Nothing is generated until this exists.</span></div>
                </form>`
              : `<p class="fine">No approach was recorded.</p>`
      }
    </section>

    <section class="plan-section">
      <div class="section-label"><h2>Plan</h2><span>${sections.length} sections · ${subtaskCount} subtasks</span></div>
      <p class="fine plan-altitude">Group the work by outcome or decision, not by file, layer or code step.</p>
      ${sectionHtml || `<p class="fine">No sections yet. Add one below, or ask the model for an outcome-level plan after the approach exists.</p>`}
      <form class="form plan-add" data-plan-add="section">
        <label class="field-label" for="plan-new-section">Add a section</label>
        <div class="form-row"><input class="input grow" id="plan-new-section" name="title" required maxlength="200" placeholder="For example: Agree what a retry guarantees" /><button class="btn ghost" type="submit">Add section</button></div>
      </form>
    </section>

    <section class="plan-section">
      <div class="section-label"><h2>Definition of done</h2><span>${dodsMet} of ${dods.length} met</span></div>
      <ul class="plan-subtasks dod-list">${dodHtml || `<li class="plan-empty-line">No criteria yet. A criterion is observable evidence, optionally backed by a check.</li>`}</ul>
      <form class="form plan-add" data-plan-add="dod">
        <label class="field-label" for="plan-new-dod">Add a criterion</label>
        <div class="form-row"><input class="input grow" id="plan-new-dod" name="title" required maxlength="200" placeholder="What must be true when this is done?" /><button class="btn ghost" type="submit">Add criterion</button></div>
        <div class="form-row"><input class="input grow" name="body" placeholder="How would someone confirm it? (optional)" /><select class="select" name="check"><option value="">manual</option>${task.checks.map((check) => `<option value="${esc(check.id)}">check: ${esc(check.label)}</option>`).join("")}</select></div>
      </form>
    </section>

    <section class="plan-section plan-discussion">
      <div class="section-label"><h2>Discussion</h2><span>${task.planMessages.length} turns · ${modelOff ? "model off" : esc(modelName)}</span></div>
      ${
        task.planMessages.length
          ? `<div class="plan-messages">${task.planMessages
              .map(
                (message) => `<div class="plan-message ${message.role}"><div class="plan-message-meta"><span>${message.role === "user" ? "You" : esc(message.model || "Model")}</span><span>${clock(message.at)}</span></div><div class="plan-message-body">${esc(message.content)}</div></div>`,
              )
              .join("")}</div>`
          : `<p class="fine">No discussion yet. The model is optional; the document works without it.</p>`
      }
      ${
        !task.prediction
          ? `<p class="fine">Discussion is locked until the approach exists. The model does not get the first word.</p>`
          : modelOff
            ? `<p class="fine">The model is off. You can still write the plan and the definition of done by hand.</p>`
            : `<form class="form plan-chat-form" data-plan-chat>
                 <label class="field-label" for="plan-chat">Ask about outcomes, order or risks</label>
                 <textarea class="textarea answer" id="plan-chat" name="text" rows="2" required placeholder="What outcome am I missing? What order or risk should we settle?" ${ui.planChatPending ? "disabled" : ""}></textarea>
                 <div class="actions"><button class="btn" type="submit" ${ui.planChatPending ? "disabled" : ""}>${ui.planChatPending ? "Thinking..." : "Send"}</button><button class="btn ghost" type="button" data-suggest-plan>Draft the plan with AI</button><span class="fine">Suggestions stay above the code and are reviewed before anything is added.</span></div>
               </form>
               ${ui.planNote ? `<p class="gate-note">${esc(ui.planNote)}</p>` : ""}
               ${ui.planProposalNote ? `<p class="gate-note">${esc(ui.planProposalNote)}</p>` : ""}
               ${proposal}`
      }
    </section>

    <section class="plan-section">
      <div class="section-label"><h2>Notes</h2><span>${task.notes.length}</span></div>
      ${task.notes.length ? `<ul class="notelist">${task.notes.map((note) => `<li><span class="note-time">${clock(note.at)}</span><span>${esc(note.text)}</span></li>`).join("")}</ul>` : `<p class="fine">No notes yet. A note is for the next reader, not for completeness.</p>`}
      <form class="form" data-plan-note>
        <label class="field-label" for="plan-note">Append a note to the document</label>
        <textarea class="textarea answer note-answer" id="plan-note" name="note" rows="3" required placeholder="What outcome, risk, or decision should the next reader know?"></textarea>
        <div class="actions"><button class="btn ghost" type="submit">Add note</button></div>
      </form>
    </section>

    <section class="plan-section plan-close">
      ${ui.planNote && !canChat ? `<p class="gate-note">${esc(ui.planNote)}</p>` : ""}
      ${ui.planNote && canChat && modelOff ? `<p class="gate-note">${esc(ui.planNote)}</p>` : ""}
      ${task.planApproved
        ? `<div class="banner good">The plan is approved.${task.planRevised ? " It has been revised since approval; the revision is in the log." : ""}</div><div class="actions"><a class="btn" href="#/p/${project.id}/task/${task.id}/implement">Open implement</a></div>`
        : `<div class="plan-readiness"><span class="stamp ${task.prediction ? "met" : "unmet"}">${task.prediction ? "approach written" : "approach owed"}</span><span class="stamp ${dods.length ? "met" : "unmet"}">${dods.length ? `${dods.length} criteria` : "definition of done owed"}</span><span class="fine">The plan stays editable after approval; changes are recorded.</span></div>
           <form class="form" data-approve>
             <label class="checkline"><input type="checkbox" name="confirm" required /> <span>I can state what this task will prove, and what it will not touch.</span></label>
             <div class="actions"><button class="btn" type="submit"${canApprove ? "" : " disabled"}>Approve plan</button><span class="fine">${!task.prediction ? "Write the approach first." : !dods.length ? "Add at least one definition of done item first." : "Approving opens implement. It does not run anything."}</span></div>
           </form>`}
    </section>
  </article>`;
  return {
    html: arena(project, task, "plan", doc),
    wire(root) {
      root.querySelector<HTMLButtonElement>("[data-plan-edit-description]")?.addEventListener("click", () => {
        ui.planEditing = "description";
        commit();
      });
      root.querySelectorAll<HTMLButtonElement>("[data-plan-cancel]").forEach((button) =>
        button.addEventListener("click", () => {
          ui.planEditing = null;
          commit();
        }),
      );
      root.querySelector<HTMLFormElement>("form[data-plan-approach]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        const form = event.currentTarget as HTMLFormElement;
        const value = String(new FormData(form).get("prediction") ?? "").trim();
        if (!value) return;
        const taskId = task.id;
        void (async () => {
          try {
            await writePrediction(taskId, value);
            ui.planEditing = null;
            commit();
          } catch (error) {
            ui.planNote = errorMessage(error);
            commit();
          }
        })();
      });
      root.querySelectorAll<HTMLButtonElement>("[data-plan-edit-item]").forEach((button) =>
        button.addEventListener("click", () => {
          const id = button.dataset.planEditItem;
          if (!id) return;
          ui.planEditing = id;
          commit();
        }),
      );
      root.querySelectorAll<HTMLFormElement>("form[data-plan-save]").forEach((form) =>
        form.addEventListener("submit", (event) => {
          event.preventDefault();
          const id = form.dataset.planSave;
          if (!id) return;
          const data = new FormData(form);
          const title = String(data.get("title") ?? "").trim();
          const body = String(data.get("body") ?? "").trim();
          if (!title) return;
          void (async () => {
            try {
              await updatePlanItem(id, title, body);
              ui.planEditing = null;
              commit();
            } catch (error) {
              ui.planNote = errorMessage(error);
              commit();
            }
          })();
        }),
      );
      root.querySelectorAll<HTMLFormElement>("form[data-plan-add]").forEach((form) =>
        form.addEventListener("submit", (event) => {
          event.preventDefault();
          const data = new FormData(form);
          const kind = (form.dataset.planAdd ?? "section") as PlanItemKind;
          const parentId = form.dataset.parent ?? null;
          const title = String(data.get("title") ?? "").trim();
          const body = String(data.get("body") ?? "").trim();
          const checkId = String(data.get("check") ?? "") || null;
          if (!title) return;
          void (async () => {
            try {
              await addPlanItem(task.id, { kind, parentId, title, body, checkId });
            } catch (error) {
              ui.planNote = errorMessage(error);
              commit();
            }
          })();
        }),
      );
      root.querySelectorAll<HTMLButtonElement>("[data-plan-delete]").forEach((button) =>
        button.addEventListener("click", () => {
          const id = button.dataset.planDelete;
          const item = task.planItems.find((entry) => entry.id === id);
          if (!id || !item) return;
          if (!window.confirm(`Remove "${item.title}" from the plan?`)) return;
          void deletePlanItem(id).catch((error) => {
            ui.planNote = errorMessage(error);
            commit();
          });
        }),
      );
      root.querySelectorAll<HTMLButtonElement>("[data-plan-move]").forEach((button) =>
        button.addEventListener("click", () => {
          const id = button.dataset.planMove;
          const direction = Number(button.dataset.direction ?? 0);
          if (!id || !direction) return;
          void movePlanItem(id, direction).catch((error) => {
            ui.planNote = errorMessage(error);
            commit();
          });
        }),
      );
      root.querySelectorAll<HTMLInputElement>("[data-plan-toggle]").forEach((input) =>
        input.addEventListener("change", () => {
          const id = input.dataset.planToggle;
          if (!id) return;
          void togglePlanItem(id, input.checked).catch((error) => {
            ui.planNote = errorMessage(error);
            commit();
          });
        }),
      );
      root.querySelector<HTMLFormElement>("form[data-plan-note]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        const form = event.currentTarget as HTMLFormElement;
        const value = String(new FormData(form).get("note") ?? "").trim();
        if (!value) return;
        void addTaskNote(task.id, value)
          .then(() => form.reset())
          .catch((error) => {
            ui.planNote = errorMessage(error);
            commit();
          });
      });
      root.querySelector<HTMLFormElement>("form[data-plan-chat]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        const form = event.currentTarget as HTMLFormElement;
        const value = String(new FormData(form).get("text") ?? "").trim();
        if (!value) return;
        void sendPlanMessage(task.id, value)
          .then(() => form.reset())
          .catch((error) => {
            ui.planNote = errorMessage(error);
            commit();
          });
      });
      root.querySelector<HTMLButtonElement>("[data-suggest-plan]")?.addEventListener("click", () => {
        void suggestPlan(task.id).catch((error) => {
          ui.planProposalNote = errorMessage(error);
          commit();
        });
      });
      root.querySelector<HTMLFormElement>("form[data-accept-proposal]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        if (!ui.planProposal) return;
        void acceptPlanProposal(task.id, ui.planProposal).catch((error) => {
          ui.planProposalNote = errorMessage(error);
          commit();
        });
      });
      root.querySelector<HTMLButtonElement>("[data-dismiss-proposal]")?.addEventListener("click", () => {
        dismissPlanProposal();
      });
      root.querySelector<HTMLFormElement>("form[data-approve]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        const projectId = project.id;
        const taskId = task.id;
        void (async () => {
          try {
            await approvePlan(taskId);
            location.hash = `#/p/${projectId}/task/${taskId}/implement`;
          } catch (error) {
            ui.planNote = errorMessage(error);
            commit();
          }
        })();
      });
    },
  };
}

export function implementView(projectId: string, taskId: string): View {
  const project = findProject(projectId);
  const task = findTask(taskId);
  if (!project || !task) return notFound();
  const availableConcepts = getState().concepts.filter(
    (concept) => !task.conceptIds.includes(concept.id),
  );
  const doc = `
    ${
      task.planApproved
        ? ""
        : `<div class="banner">The plan is not approved yet. Implement is open for reading, but the task is not settled. <a class="small-link" href="#/p/${project.id}/task/${task.id}/plan">Approve the plan</a></div>`
    }
    ${chunk(
      "1",
      `<p class="prose">The developer writes the code. This surface holds the attempt log, the notes worth keeping, and the zones the model must not touch.</p>
       <dl class="facts inline">
         <div><dt>Human-owned</dt><dd><code>src/outbox/policy.ts</code></dd></div>
         <div><dt>Read scope</dt><dd><code>src/outbox/**</code></dd></div>
         <div><dt>Write access</dt><dd>off, locked in code</dd></div>
       </dl>`,
    )}
    ${chunk(
      "2",
      `<div class="section-label"><h2>Working notes</h2><span>${task.notes.length}</span></div>
       ${
         task.notes.length
           ? `<ul class="notelist">${task.notes
               .map(
                 (n) =>
                   `<li><span class="note-time">${clock(n.at)}</span><span>${esc(n.text)}</span></li>`,
               )
               .join("")}</ul>`
           : `<p class="fine">No notes yet. A note is for the next session, not for the log's completeness.</p>`
       }
       <form class="form" data-note>
         <label class="field-label" for="note">Add a working note</label>
         <textarea class="textarea answer note-answer" id="note" name="note" rows="4" required placeholder="What just changed, and what it costs?"></textarea>
         <div class="actions"><button class="btn" type="submit">Add note</button><span class="fine">Attached to this task. General notes live in the <a class="small-link" href="#/p/${project.id}/notes">notebook</a>.</span></div>
       </form>`,
    )}
    ${chunk(
      "3",
      `<div class="section-label"><h2>Attempts</h2><span>${task.attempts.length} recorded</span></div>
       ${
         task.attempts.length
           ? `<ol class="attemptlist">${task.attempts
               .map(
                 (a, i) =>
                   `<li><span class="attempt-no">${String(i + 1).padStart(2, "0")}</span><span class="attempt-time">${clock(a.at)}</span><span>${esc(a.text)}</span></li>`,
               )
               .join("")}</ol>`
           : `<p class="fine">No attempt recorded. An attempt is the thing that can be wrong; record it before the fix.</p>`
       }
       <form class="form" data-attempt>
         <label class="field-label" for="attempt">Record an attempt</label>
         <div class="form-row"><input class="input grow" id="attempt" name="attempt" required placeholder="What did you try, and what happened?" /><button class="btn ghost" type="submit">Record</button></div>
       </form>`,
    )}
    ${chunk(
      "4",
      `<div class="section-label"><h2>Concepts this task touches</h2><span>${task.conceptIds.length}</span></div>
       <ul class="conceptlist">${task.conceptIds
         .map((id) => {
           const concept = getState().concepts.find((c) => c.id === id);
           return concept
             ? `<li><code>${esc(concept.id)}</code> <a class="row-link quiet" href="#/concepts">${esc(concept.name)}</a> <span class="fine">help: ${esc(concept.help)}</span><button class="small-link" type="button" data-detach-concept="${esc(concept.id)}">detach</button></li>`
             : "";
         })
         .join("")}</ul>
       ${
         availableConcepts.length
           ? `<form class="form inline" data-attach-concept>
                <label class="field-label" for="attach-concept">Attach a concept</label>
                <div class="form-row"><select class="input select grow" id="attach-concept" name="concept">${availableConcepts
                  .map(
                    (concept) =>
                      `<option value="${esc(concept.id)}">${esc(concept.id)} · ${esc(concept.name)}</option>`,
                  )
                  .join("")}</select><button class="btn ghost" type="submit">Attach</button></div>
              </form>`
           : `<p class="fine">Every concept in the ledger is already attached. <a class="small-link" href="#/concepts">Add one</a>.</p>`
       }
       <div class="actions">
         <form class="form" data-close-implement><button class="btn" type="submit">Close implement and verify</button><span class="fine">This moves the task to verify; checks decide what passes, the gate decides what you understand.</span></form>
       </div>`,
    )}`;
  return {
    html: arena(project, task, "implement", doc),
    wire(root) {
      root.querySelector<HTMLFormElement>("form[data-note]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        const form = event.currentTarget as HTMLFormElement;
        const value = String(new FormData(form).get("note") ?? "").trim();
        if (!value) return;
        const taskId = task.id;
        void (async () => {
          try {
            await addTaskNote(taskId, value);
            form.reset();
          } catch (error) {
            console.error(errorMessage(error));
          }
        })();
      });
      root.querySelector<HTMLFormElement>("form[data-attempt]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        const form = event.currentTarget as HTMLFormElement;
        const value = String(new FormData(form).get("attempt") ?? "").trim();
        if (!value) return;
        const taskId = task.id;
        void (async () => {
          try {
            await recordAttempt(taskId, value);
            form.reset();
          } catch (error) {
            console.error(errorMessage(error));
          }
        })();
      });
      root.querySelector<HTMLFormElement>("form[data-close-implement]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        const projectId = project.id;
        const taskId = task.id;
        void (async () => {
          try {
            await closeImplement(taskId);
            location.hash = `#/p/${projectId}/task/${taskId}/verify`;
          } catch (error) {
            console.error(errorMessage(error));
          }
        })();
      });
      root.querySelector<HTMLFormElement>("form[data-attach-concept]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        const form = event.currentTarget as HTMLFormElement;
        const conceptId = String(new FormData(form).get("concept") ?? "");
        if (!conceptId) return;
        void (async () => {
          try {
            await attachConcept(task.id, conceptId);
          } catch (error) {
            console.error(errorMessage(error));
          }
        })();
      });
      root.querySelectorAll<HTMLButtonElement>("[data-detach-concept]").forEach((button) => {
        button.addEventListener("click", () => {
          const conceptId = button.dataset.detachConcept;
          if (!conceptId) return;
          void (async () => {
            try {
              await detachConcept(task.id, conceptId);
            } catch (error) {
              console.error(errorMessage(error));
            }
          })();
        });
      });
    },
  };
}

function gateBlock(gate: Gate): string {
  const closed = gate.status === "passed" || gate.status === "overridden";
  const concept = gate.conceptId
    ? getState().concepts.find((entry) => entry.id === gate.conceptId)
    : undefined;
  return `<section class="gateblock" id="gate-${gate.id}">
    <div class="gate-head">
      <span class="gate-id">Gate ${gate.n}</span>
      <span class="stamp ${gate.status === "passed" ? "met" : gate.status === "overridden" ? "over" : gate.status === "failed" ? "unmet" : "due"}">${esc(gate.status)}</span>
      <span class="gate-source">${concept ? `Concept ${esc(concept.id)} · ${esc(concept.name)}` : "Scored by the app"}${gate.draftedBy === "model" ? " · question from the local model" : ""}</span>
    </div>
    <h3 class="gate-question">${esc(gate.question)}</h3>
    ${
      closed
        ? `<div class="gate-closed">
             <span class="stamp ${gate.status === "passed" ? "met" : "over"}">${gate.status === "passed" ? "met" : "overridden"}</span>
             ${gate.status === "passed" && gate.answer ? `<p class="gate-answer">${esc(gate.answer)}</p>` : ""}
             ${gate.status === "overridden" ? `<p class="gate-answer">Override reason: ${esc(gate.reason)}</p><p class="fine">The bypass is in the log, with the reason attached.</p>` : ""}
             <p class="fine">${gate.evaluated ? `Evaluated ${clock(gate.evaluated)}.` : ""} ${gate.status === "passed" ? "Reflect is unblocked." : "Reflect is unblocked by override, not by understanding."}</p>
           </div>`
        : `<form class="form" data-gate-submit data-gate="${gate.id}">
             <label class="field-label" for="ga-${gate.id}">Your answer</label>
             <textarea class="textarea answer" id="ga-${gate.id}" name="answer" rows="2" placeholder="Explain it without the notes."></textarea>
             <div class="actions">
               <button class="btn" type="submit">Submit answer</button>
               ${gate.hintUsed ? "" : '<button class="btn ghost" type="button" data-hint>Show hint</button>'}
               <span class="fine">The model drafts the question. It cannot mark this passed.</span>
             </div>
           </form>
           ${gate.hintUsed ? `<p class="hint-text"><span class="stamp weak">hint</span> ${esc(gate.hint)}</p>` : ""}
           <details class="override">
             <summary>Override with a reason</summary>
             <form class="form" data-gate-override data-gate="${gate.id}">
               <label class="field-label" for="ov-${gate.id}">Reason (written to the log)</label>
               <input class="input" id="ov-${gate.id}" name="reason" required placeholder="Say what the gate would have caught, and why you are moving on." />
               <div class="actions"><button class="btn ghost" type="submit">Override and log</button><span class="fine">A bypass is not a pass. It is recorded as one.</span></div>
             </form>
           </details>`
    }
    ${ui.gateNote ? `<p class="gate-note${ui.gateNote.startsWith("Met") ? " good" : ""}">${esc(ui.gateNote)}</p>` : ""}
    ${
      gate.attemptLog.length
        ? `<div class="attempts"><div class="section-label"><h2>Attempts on this gate</h2><span>${gate.attemptLog.length}</span></div>
             <table class="table"><thead><tr><th>Time</th><th>Answer</th><th>Result</th></tr></thead><tbody>${gate.attemptLog
               .map(
                 (a) =>
                   `<tr><td class="cell-time">${clock(a.at)}</td><td class="cell-detail">${esc(a.answer)}</td><td class="cell-stamp"><span class="stamp ${a.pass ? "met" : "unmet"}">${a.pass ? "met" : "not met"}</span></td></tr>`,
               )
               .join("")}</tbody></table></div>`
        : ""
    }
  </section>`;
}

function wireGate(root: HTMLElement, task: Task, gate: Gate): void {
  root.querySelector<HTMLFormElement>("form[data-gate-submit]")?.addEventListener("submit", (event) => {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement;
    const answer = String(new FormData(form).get("answer") ?? "").trim();
    if (!answer) {
      ui.gateNote = "Write an answer before submitting. A blank gate cannot be scored.";
      commit();
      return;
    }
    const taskId = task.id;
    const gateId = gate.id;
    void (async () => {
      try {
        await answerGate(taskId, gateId, answer);
        form.reset();
      } catch (error) {
        ui.gateNote = errorMessage(error);
        commit();
      }
    })();
  });
  root.querySelector<HTMLButtonElement>("[data-hint]")?.addEventListener("click", () => {
    void (async () => {
      try {
        await useHint(task.id, gate.id);
      } catch (error) {
        ui.gateNote = errorMessage(error);
        commit();
      }
    })();
  });
  root.querySelector<HTMLFormElement>("form[data-gate-override]")?.addEventListener("submit", (event) => {
    event.preventDefault();
    const form = event.currentTarget as HTMLFormElement;
    const reason = String(new FormData(form).get("reason") ?? "").trim();
    const taskId = task.id;
    const gateId = gate.id;
    void (async () => {
      try {
        await overrideGate(taskId, gateId, reason);
        form.reset();
      } catch (error) {
        ui.gateNote = errorMessage(error);
        commit();
      }
    })();
  });
}

export function verifyView(projectId: string, taskId: string): View {
  const project = findProject(projectId);
  const task = findTask(taskId);
  if (!project || !task) return notFound();
  const gate = pendingGate(task) ?? task.gates[task.gates.length - 1];
  const lastRun = ui.checkRun && ui.checkRun.taskId === task.id ? ui.checkRun : null;
  const property = task.checks.find((c) => c.label === "property");
  const jitter = getState().concepts.find((c) => c.id === "K-052");
  const claims = [
    {
      key: "3.1",
      text: "The cap is decided in one place.",
      met: task.planApproved,
      stamp: task.planApproved ? `<span class="stamp met">met</span>` : `<span class="stamp unmet">unmet</span>`,
    },
    {
      key: "3.2",
      text: "Attempts never exceed the cap.",
      met: property?.result === "pass",
      stamp:
        property?.result === "pass"
          ? `<span class="stamp met">met · ${clock(property.evaluated)}</span>`
          : `<span class="stamp unmet">unmet · 1 fails</span>`,
    },
    {
      key: "3.3",
      text: "You can say why five without the notes.",
      met: gate ? gate.status === "passed" : false,
      stamp: gate ? gateStamp(gate) : `<span class="stamp idle">no gate</span>`,
    },
    {
      key: "3.4",
      text: "Jitter is explained, not only applied.",
      met: jitter?.status === "passed",
      stamp: `<span class="stamp ${jitter?.status === "passed" ? "met" : "weak"}">${jitter?.status === "passed" ? "met" : "weak · 2 seen"}</span>`,
    },
  ];

  const doc = `
    ${chunk(
      "2a",
      `<p class="fine code-lede">503 retries with exponential backoff; the cap is read before the delay.</p>
       <div class="code">
         <div class="code-head"><span>policy.ts</span><span class="stamp unmet">human-written</span><span class="file">src/outbox/policy.ts</span></div>
         <div class="listing">
           <div><span class="ln">12</span><span class="kw">export const</span> cap = 5 <span class="cm">// decided before any delay</span></div>
           <div><span class="ln">13</span><span class="kw">export const</span> base = 200</div>
           <div><span class="ln">14</span><span class="kw">export const</span> jitter = 0.3</div>
           <div><span class="ln">15</span><span class="kw">export function</span> delayFor(attempt: number): number | null {</div>
           <div><span class="ln">16</span>&nbsp;&nbsp;<span class="kw">if</span> (attempt &gt;= cap) <span class="kw">return</span> null</div>
           <div><span class="ln">17</span>&nbsp;&nbsp;<span class="kw">return</span> base * 2 ** attempt * (1 + jitter * rand())</div>
           <div><span class="ln">18</span>}</div>
         </div>
       </div>
       <div class="verdict"><span class="stamp met">unit 24/24</span><span class="stamp unmet">property 1 failing</span><span class="fine">evaluated ${clock(property?.evaluated ?? null)}</span></div>`,
    )}
    ${chunk(
      "3",
      `<div class="section-label"><h2>Claims</h2><span>${claims.filter((c) => c.met).length} of ${claims.length} met</span></div>
       <div class="claims">
         ${claims
           .map(
             (c) =>
               `<div class="claim ${c.met ? "met" : "unmet"}"><span class="claim-key">${c.key}</span><span class="claim-text">${esc(c.text)}</span><span class="claim-stamp">${c.stamp}</span></div>`,
           )
           .join("")}
       </div>`,
    )}
    ${
      gate
        ? gateBlock(gate)
        : chunk(
            "3",
            `<div class="section-label"><h2>Gate</h2><span>none drafted</span></div>
             <p class="prose">A gate is a short check between verify and reflect. Drafting one prefers a concept this task touches that is due, and falls back to the task itself. The question and rubric are app-authored; no model is consulted, and scoring is deterministic.</p>
             <form class="form" data-draft-gate>
               <div class="actions"><button class="btn" type="submit">Draft a gate from this task</button><span class="fine">Reflect is open whether or not a gate exists. Familiar work earns no gate.</span></div>
             </form>`,
          )
    }
    ${chunk(
      "4",
      `<div class="section-label"><h2>Checks</h2><span>${checksMet(task)} of ${task.checks.length} met</span></div>
       ${checkTable(task, lastRun, ui.runningCheck === task.id, ui.checkNote)}
       <form class="form check-form" data-add-check>
         <div class="section-label"><h2>Add a check</h2><span>program + arguments</span></div>
         <div class="form-field"><label class="field-label" for="check-label">Check</label><input class="input" id="check-label" name="label" required maxlength="200" placeholder="property: no sixth retry" /></div>
         <div class="form-field"><label class="field-label" for="check-program">Program</label><select class="select" id="check-program" name="program">${PROGRAMS.map((program) => `<option value="${program}">${program}</option>`).join("")}</select></div>
         <div class="form-field"><label class="field-label" for="check-args">Arguments</label><input class="input" id="check-args" name="args" placeholder="test --lib" /></div>
         <div class="form-field"><label class="field-label" for="check-detail">What it proves</label><input class="input" id="check-detail" name="detail" placeholder="Fails if a sixth retry is attempted." /></div>
         <div class="actions"><button class="btn" type="submit">Add check</button><span class="fine">One program from the allowlist plus arguments. There is no shell, so no pipes, redirects or substitutions.</span></div>
       </form>
       ${gate ? (gateResolved(task) ? `<div class="banner good">The gate is resolved. Reflect is unblocked.</div><div class="actions"><a class="btn" href="#/p/${project.id}/task/${task.id}/reflect">Write the handoff</a></div>` : `<p class="fine">Reflect stays blocked until the gate passes or is overridden with a reason.</p>`) : `<p class="fine">No gate is attached to this task; reflect is open.</p>`}`,
    )}`;

  return {
    html: arena(project, task, "verify", doc),
    wire(root) {
      if (gate) wireGate(root, task, gate);
      root.querySelector<HTMLFormElement>("form[data-draft-gate]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        const taskId = task.id;
        void (async () => {
          try {
            await draftGate(taskId);
          } catch (error) {
            ui.gateNote = errorMessage(error);
            commit();
          }
        })();
      });
      root.querySelector<HTMLButtonElement>("[data-run-checks]")?.addEventListener("click", () => {
        void runChecks(task.id).catch((error) => {
          ui.checkNote = errorMessage(error);
          commit();
        });
      });
      root.querySelector<HTMLButtonElement>("[data-cancel-checks]")?.addEventListener("click", () => {
        void cancelChecks();
      });
      root.querySelectorAll<HTMLButtonElement>("[data-remove-check]").forEach((button) => {
        button.addEventListener("click", () => {
          const checkId = button.dataset.removeCheck;
          if (!checkId) return;
          void removeCheck(checkId).catch((error) => {
            ui.checkNote = errorMessage(error);
            commit();
          });
        });
      });
      root.querySelector<HTMLFormElement>("form[data-add-check]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        const form = event.currentTarget as HTMLFormElement;
        const data = new FormData(form);
        const input = {
          label: String(data.get("label") ?? "").trim(),
          detail: String(data.get("detail") ?? "").trim(),
          program: String(data.get("program") ?? "cargo"),
          args: String(data.get("args") ?? "").trim(),
        };
        if (!input.label) return;
        ui.checkNote = null;
        void addCheck(task.id, input)
          .then(() => form.reset())
          .catch((error) => {
            ui.checkNote = errorMessage(error);
            commit();
          });
      });
    },
  };
}

export function gateView(projectId: string, taskId: string, gateId: string): View {
  const project = findProject(projectId);
  const task = findTask(taskId);
  if (!project || !task) return notFound();
  const gate = task.gates.find((g) => g.id === gateId) ?? pendingGate(task) ?? task.gates[0];
  if (!gate) return notFound();
  const doc = `
    <p class="lede">One question, scored by the app, between verify and reflect. It is generated from the work just done, and the model that drafted it cannot mark it passed.</p>
    ${gateBlock(gate)}
    ${chunk(
      "1",
      `<div class="section-label"><h2>What this gate is doing</h2><span>app-owned</span></div>
       <p class="prose">A check proves the code behaves. A gate proves you can say why. The answer is compared with the intent recorded in the plan and the evidence in the checks; the result is a state change, not a score.</p>
       <p class="fine">Override is available, but it writes the reason to the log and still counts as a bypass. That is the difference between moving on and understanding.</p>`,
    )}`;
  return {
    html: arena(project, task, "verify", doc),
    wire(root) {
      wireGate(root, task, gate);
    },
  };
}

export function reflectView(projectId: string, taskId: string): View {
  const project = findProject(projectId);
  const task = findTask(taskId);
  if (!project || !task) return notFound();
  const blocked = !gateResolved(task);
  const done = task.reflected && project.handoff;
  const doc = blocked
    ? `<div class="banner">Reflect is blocked by an unresolved gate. A handoff written now would describe work you have not shown you understand. <a class="small-link" href="#/p/${project.id}/task/${task.id}/verify">Back to the gate</a></div>
       ${chunk("1", `<p class="prose">The task can still be read, but the three-line handoff stays disabled until the gate passes or is overridden with a reason.</p>`)}`
    : done
      ? `<div class="banner good">Session closed. The handoff is attached to ${esc(project.name)} and will be revealed only after the next recall.</div>
         ${chunk(
           "1",
           `<div class="section-label"><h2>Handoff</h2><span>${new Date(project.handoff?.at ?? new Date()).toLocaleString("en-GB", { day: "2-digit", month: "short", hour: "2-digit", minute: "2-digit" })}</span></div>
            <dl class="handoff">
              <div><dt>Next</dt><dd>${esc(project.handoff?.next ?? "")}</dd></div>
              <div><dt>Blocking</dt><dd>${esc(project.handoff?.blocking ?? "")}</dd></div>
              <div><dt>Thinking</dt><dd>${esc(project.handoff?.thinking ?? "")}</dd></div>
            </dl>
            <div class="actions"><a class="btn" href="#/p/${project.id}">Back to the workbench</a><a class="btn ghost" href="#/projects">All projects</a></div>`,
         )}`
      : `${chunk(
          "1",
          `<p class="lede">Three lines, written for the person who will be here next: what is next, what is blocking, and what you were thinking. They will not see the handoff until they write what they remember.</p>
           <form class="form" data-handoff>
             <div class="form-field"><label class="field-label" for="h-next">Next</label><input class="input" id="h-next" name="next" required placeholder="The very next action, not the plan." /></div>
             <div class="form-field"><label class="field-label" for="h-blocking">Blocking</label><input class="input" id="h-blocking" name="blocking" required placeholder="What is in the way, and what would unblock it?" /></div>
             <div class="form-field"><label class="field-label" for="h-thinking">Thinking</label><input class="input" id="h-thinking" name="thinking" required placeholder="The assumption you would lose if you stopped now." /></div>
             <div class="actions"><button class="btn" type="submit">Write handoff and close the session</button><span class="fine">This closes the session. The next start begins with a recall, not a briefing.</span></div>
           </form>`,
        )}
         ${chunk(
           "2",
           `<div class="section-label"><h2>What the session produced</h2><span>${checksMet(task)} checks</span></div>
            <table class="table">
              <thead><tr><th>Check</th><th>Evaluation</th><th>State</th></tr></thead>
              <tbody>${task.checks
                .map(
                  (c) =>
                    `<tr><td class="cell-name">${esc(c.label)}</td><td class="cell-time">${c.evaluated ? clock(c.evaluated) : "never"}</td><td class="cell-stamp">${checkStamp(c)}</td></tr>`,
                )
                .join("")}</tbody>
            </table>`,
         )}`;
  return {
    html: arena(project, task, "reflect", doc),
    wire(root) {
      const form = root.querySelector<HTMLFormElement>("form[data-handoff]");
      form?.addEventListener("submit", (event) => {
        event.preventDefault();
        const data = new FormData(form);
        const next = String(data.get("next") ?? "").trim();
        const blocking = String(data.get("blocking") ?? "").trim();
        const thinking = String(data.get("thinking") ?? "").trim();
        if (!next || !blocking || !thinking) return;
        const taskId = task.id;
        void (async () => {
          try {
            await writeHandoff(taskId, { next, blocking, thinking });
          } catch (error) {
            console.error(errorMessage(error));
          }
        })();
      });
    },
  };
}
