import type { Decision, Project, Task } from "../data";
import {
  addProjectNote,
  checksMet,
  commit,
  createProject,
  createTask,
  deleteProjectNote,
  deleteTask,
  errorMessage,
  findProject,
  getState,
  pendingGate,
  saveBrief,
  setProjectPath,
  tasksOf,
  ui,
} from "../store";
import {
  agoText,
  chunk,
  clock,
  emptyState,
  esc,
  gateStamp,
  pageHead,
  sectionLabel,
  shortDate,
  type View,
} from "../ui";

function lastTouched(project: Project, task: Task | undefined): string | null {
  const stamps = [
    task?.notes[0]?.at,
    task?.attempts[0]?.at,
    project.handoff?.at,
    task?.started,
    project.created,
  ].filter((stamp): stamp is string => Boolean(stamp));
  return stamps.sort((a, b) => new Date(b).getTime() - new Date(a).getTime())[0] ?? null;
}

export function projectsView(): View {
  const state = getState();
  const awaiting = state.projects.filter((p) => p.handoff && !p.recall).length;
  const due = state.tasks.filter((t) => pendingGate(t)).length;

  const rows = state.projects
    .map((project) => {
      const task = state.tasks.find((t) => t.id === project.currentTaskId);
      const gate = task ? pendingGate(task) : undefined;
      const handoff = project.handoff
        ? project.recall
          ? "recalled"
          : "awaiting recall"
        : "none";
      return `
        <tr>
          <td class="cell-name"><a class="row-link" href="#/p/${project.id}">${esc(project.name)}</a><span class="cell-sub">${esc(project.brief)}</span></td>
          <td>${task ? `<a class="row-link quiet" href="#/p/${project.id}/task/${task.id}/${task.phase}">${esc(task.title)}</a> <span class="fine">${esc(task.phase)}</span>` : "—"}</td>
          <td>${gate ? gateStamp(gate) : '<span class="stamp idle">none</span>'}</td>
          <td class="cell-mono ${handoff === "awaiting recall" ? "owed-text" : ""}">${handoff}</td>
          <td class="cell-time">${agoText(lastTouched(project, task))}</td>
          <td class="cell-stamp"><a class="small-link" href="#/p/${project.id}">open</a></td>
        </tr>`;
    })
    .join("");

  const createForm = ui.newProject
    ? `<form class="form inline-form" data-new-project>
         <div class="form-row">
           <div class="form-field"><label class="field-label" for="np-name">Project name</label><input class="input" id="np-name" name="name" required placeholder="marsh" /></div>
           <div class="form-field grow"><label class="field-label" for="np-brief">One-line brief</label><input class="input" id="np-brief" name="brief" required placeholder="What is this project for?" /></div>
         </div>
         <div class="form-row">
           <div class="form-field grow"><label class="field-label" for="np-path">Repository path <span class="label-opt">optional</span></label><input class="input" id="np-path" name="path" placeholder="/home/you/code/repo" /></div>
         </div>
         <div class="actions"><button class="btn" type="submit">Create project</button><button class="btn ghost" type="button" data-cancel-project>Cancel</button><span class="fine">A project opens with one unapproved task in the plan phase. With a repository, checks run there and documents live in <code>.antik/</code>.</span></div>
       </form>`
    : "";

  const html = `<div class="page">
    ${pageHead("Projects", [`${state.projects.length} projects`, `${awaiting} handoff${awaiting === 1 ? "" : "s"} awaiting recall`, `${due} gate${due === 1 ? "" : "s"} due`], `<button class="btn" type="button" data-new-project>New project</button>`)}
    <div class="doc wide">
      ${createForm}
      ${
        state.projects.length
          ? `<div class="table-wrap"><table class="table projects-table">
              <thead><tr><th>Project</th><th>Current task</th><th>Gate</th><th>Handoff</th><th>Touched</th><th></th></tr></thead>
              <tbody>${rows}</tbody>
            </table></div>`
          : emptyState(
              "No projects on this machine",
              "antik opens into real work. Create a project, then the plan phase starts with a prediction you write before the model says anything.",
              ui.newProject
                ? ""
                : '<button class="btn" type="button" data-new-project>Create the first project</button>',
            )
      }
      <div class="page-note">
        <span>Local store. Projects and their documents live in SQLite and on disk.</span>
        <span><a class="small-link" href="#/session">Session start</a> · <a class="small-link" href="#/settings">Data</a></span>
      </div>
    </div>
  </div>`;

  return {
    html,
    wire(root) {
      // `button[data-new-project]`, not `[data-new-project]`: the form itself
      // carries that attribute for the submit handler, and a click on one of its
      // inputs bubbles to the form, which used to toggle the form shut.
      root.querySelectorAll<HTMLButtonElement>("button[data-new-project]").forEach((el) =>
        el.addEventListener("click", () => {
          ui.newProject = !ui.newProject;
          commit();
          if (ui.newProject) root.querySelector<HTMLInputElement>("#np-name")?.focus();
        }),
      );
      root.querySelector<HTMLButtonElement>("[data-cancel-project]")?.addEventListener("click", () => {
        ui.newProject = false;
        commit();
      });
      const form = root.querySelector<HTMLFormElement>("form[data-new-project]");
      form?.addEventListener("submit", (event) => {
        event.preventDefault();
        const data = new FormData(form);
        const name = String(data.get("name") ?? "").trim();
        const brief = String(data.get("brief") ?? "").trim();
        const path = String(data.get("path") ?? "").trim();
        if (!name || !brief) return;
        ui.newProject = false;
        void (async () => {
          try {
            const made = await createProject(name, brief);
            if (path) await setProjectPath(made.project.id, path);
            location.hash = `#/p/${made.project.id}/task/${made.task.id}/plan`;
          } catch (error) {
            ui.newProject = true;
            console.error(errorMessage(error));
            commit();
          }
        })();
      });
    },
  };
}

export function workbenchView(projectId: string): View {
  const project = findProject(projectId);
  if (!project) {
    return { html: `<div class="page"><div class="doc">${pageHead("Not found", ["no such project"])}${emptyState("That project is gone", "It may have been removed when the mock was reset.", '<a class="btn" href="#/projects">All projects</a>')}</div></div>` };
  }
  const tasks = tasksOf(project.id).sort(
    (a, b) => new Date(b.started).getTime() - new Date(a.started).getTime(),
  );
  const dueGates = tasks.filter((t) => pendingGate(t)).length;
  const openQuestions = project.questions.filter((q) => !q.answer).length;
  // Opening a task is how the pointer moves, so the list itself is the
  // switcher: the current row is marked, every other row switches on click.
  const rows = tasks
    .map((t) => {
      const gate = pendingGate(t);
      const current = t.id === project.currentTaskId;
      return `<li class="batch-item">
          <a class="batch-row${current ? " current" : ""}" href="#/p/${project.id}/task/${t.id}/${t.phase}"${current ? ' aria-current="true"' : ""}>
            <span class="batch-main">
              <span class="batch-title">${esc(t.title)}</span>
              <span class="batch-meta"><span class="batch-phase">${esc(t.phase)}</span><span>task ${esc(t.batch)}</span><span>${checksMet(t)} of ${t.checks.length} checks</span><span>${t.minutes}m</span></span>
            </span>
            <span class="batch-side">${current ? '<span class="batch-flag">current</span>' : ""}${gate ? gateStamp(gate) : '<span class="stamp idle">no gate due</span>'}</span>
          </a>
          <button class="note-del batch-del" type="button" data-delete-task="${esc(t.id)}" aria-label="Delete task">delete</button>
        </li>`;
    })
    .join("");

  const html = `<div class="page">
    ${pageHead(project.name, [esc(project.brief), `${tasks.length} task${tasks.length === 1 ? "" : "s"}`, `${dueGates} gate${dueGates === 1 ? "" : "s"} due`], `<button class="btn" type="button" data-new-batch>New task</button><a class="btn ghost" href="#/session?p=${project.id}">${project.handoff ? "Recall and start" : "Start a session"}</a>`)}
    <div class="workspace">
      <div class="doc workbench-doc">
        <section class="side-block">
        <div class="section-label"><h2>Repository</h2><span>${project.repoPath ? "attached" : "not attached"}</span></div>
        <p class="fine">${project.repoPath ? `Checks run in <code>${esc(project.repoPath)}</code>. Documents live in <code>.antik/</code>, and the model sees only what this task's scope names inside the read scope.` : "No repository attached. Documents live in the app's data folder and checks run there. Attach one to run checks against real code."}</p>
        <form class="form inline" data-repo>
          <label class="field-label" for="repo-path">${project.repoPath ? "New path" : "Project path"}</label>
          <div class="form-row"><input class="input grow" id="repo-path" name="path" value="${esc(project.repoPath ?? "")}" placeholder="/home/you/code/repo" /><button class="btn ghost" type="submit">${project.repoPath ? "Update" : "Attach"}</button></div>
          <span class="fine">Absolute path, canonicalised on save. An empty path detaches the repository.</span>
        </form>
        </section>
        ${chunk(
          "1",
          `<div class="section-label"><h2>Tasks</h2><span>${tasks.length} in this project${dueGates ? ` · ${dueGates} gate${dueGates === 1 ? "" : "s"} due` : ""}</span></div>
           ${
             tasks.length
               ? `<ul class="batch-list">${rows}</ul>`
               : emptyState(
                   "No tasks yet",
                   "A task is one small change with a prediction and a definition of done. Plan the first one; it opens in plan before the model sees anything.",
                   `<button class="btn" type="button" data-new-batch>Plan the first task</button>`,
                 )
           }
           ${
             ui.newBatch
               ? `<form class="form inline-form batch-form" data-new-batch-form>
                    <div class="form-row">
                      <div class="form-field grow"><label class="field-label" for="nb-title">Task title</label><input class="input" id="nb-title" name="title" required placeholder="Idempotency keys for delivery" /></div>
                      <div class="form-field grow"><label class="field-label" for="nb-scope">Scope <span class="label-opt">paths this task may touch</span></label><input class="input" id="nb-scope" name="scope" placeholder="src/outbox/deliver.ts" /></div>
                    </div>
                    <div class="actions"><button class="btn" type="submit">Plan task</button><button class="btn ghost" type="button" data-cancel-batch>Cancel</button><span class="fine">The new task becomes current and opens in plan. The prediction is written there.</span></div>
                  </form>`
               : ""
           }
           <p class="fine stack-top">${openQuestions} open question${openQuestions === 1 ? "" : "s"} · <a class="small-link" href="#/p/${project.id}/glossary">Glossary</a> · <a class="small-link" href="#/p/${project.id}/brief">Brief</a></p>`,
        )}
        ${chunk(
          "2",
          `<div class="section-label"><h2>What the last session left</h2><span>${project.handoff ? (project.recall ? "recalled" : "not yet recalled") : "none"}</span></div>
           ${
             project.handoff
               ? `<dl class="handoff compact">
                    <div><dt>Next</dt><dd>${esc(project.handoff.next)}</dd></div>
                    <div><dt>Blocking</dt><dd>${esc(project.handoff.blocking)}</dd></div>
                    <div><dt>Thinking</dt><dd>${esc(project.handoff.thinking)}</dd></div>
                  </dl>
                  <p class="fine">Written ${agoText(project.handoff.at)}. ${project.recall ? "Recalled in this session." : "The recall screen reveals it only after you write what you remember."}</p>`
               : `<p class="prose">No handoff yet. One is written in the reflect phase, in three lines: next, blocking, thinking.</p>`
           }`,
        )}
        <section class="side-block">
          <div class="section-label"><h2>Notebook</h2><span>${project.notes.length} notes</span></div>
          ${
            project.notes.length
              ? `<ul class="notelist notebook-peek">${project.notes
                  .slice(0, 2)
                  .map(
                    (note) =>
                      `<li><span class="note-time">${agoText(note.at)}</span><span>${note.title ? `<strong>${esc(note.title)}</strong> — ` : ""}${esc(note.text)}</span></li>`,
                  )
                  .join("")}</ul>`
              : `<p class="fine">The notebook is empty. It takes anything the task does not: a measurement, a quote, an idea for later.</p>`
          }
          <div class="actions"><a class="small-link" href="#/p/${project.id}/notes">Open the notebook</a></div>
        </section>
      </div>
    </div>
  </div>`;

  return {
    html,
    wire(root) {
      root.querySelector<HTMLFormElement>("form[data-repo]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        const form = event.currentTarget as HTMLFormElement;
        const path = String(new FormData(form).get("path") ?? "").trim();
        const projectId = project.id;
        void (async () => {
          try {
            await setProjectPath(projectId, path || null);
          } catch (error) {
            console.error(errorMessage(error));
          }
        })();
      });
      root.querySelectorAll<HTMLButtonElement>("[data-new-batch]").forEach((button) =>
        button.addEventListener("click", () => {
          ui.newBatch = !ui.newBatch;
          commit();
          if (ui.newBatch) root.querySelector<HTMLInputElement>("#nb-title")?.focus();
        }),
      );
      root.querySelector<HTMLButtonElement>("[data-cancel-batch]")?.addEventListener("click", () => {
        ui.newBatch = false;
        commit();
      });
      root.querySelectorAll<HTMLButtonElement>("[data-delete-task]").forEach((button) => {
        button.addEventListener("click", () => {
          const taskId = button.dataset.deleteTask;
          if (!taskId) return;
          const task = tasks.find((entry) => entry.id === taskId);
          const title = task?.title ?? taskId;
          if (
            !window.confirm(
              `Delete task "${title}"? Its notes, attempts, checks and gates go with it. The event log keeps the record.`,
            )
          ) {
            return;
          }
          void deleteTask(taskId).catch((error) => console.error(errorMessage(error)));
        });
      });
      root
        .querySelector<HTMLFormElement>("form[data-new-batch-form]")
        ?.addEventListener("submit", (event) => {
          event.preventDefault();
          const form = event.currentTarget as HTMLFormElement;
          const data = new FormData(form);
          const title = String(data.get("title") ?? "").trim();
          const scope = String(data.get("scope") ?? "").trim();
          if (!title) return;
          const submit = form.querySelector<HTMLButtonElement>('button[type="submit"]');
          if (submit) submit.disabled = true;
          void (async () => {
            try {
              const made = await createTask(project.id, title, scope, "");
              ui.newBatch = false;
              location.hash = `#/p/${project.id}/task/${made.id}/plan`;
            } catch (error) {
              if (submit) submit.disabled = false;
              console.error(errorMessage(error));
            }
          })();
        });
    },
  };
}

export function briefView(projectId: string, q: URLSearchParams): View {
  const project = findProject(projectId);
  if (!project) return { html: emptyState("No project", "The brief belongs to a project.", '<a class="btn" href="#/projects">Projects</a>') };
  const editing = q.get("edit") === "1";
  const html = `<div class="page">
    ${pageHead("Brief", [project.name, `created ${shortDate(project.created)}`], `<a class="btn ghost" href="#/p/${project.id}/brief${editing ? "" : "?edit=1"}">${editing ? "Read" : "Edit brief"}</a>`)}
    <div class="workspace">
      <div class="doc">
        ${
          editing
            ? `<form class="form" data-brief>
                 <label class="field-label" for="brief-body">Project brief</label>
                 <textarea class="textarea prose-input" id="brief-body" name="body" rows="16">${esc(project.briefBody)}</textarea>
                 <div class="actions"><button class="btn" type="submit">Save brief</button><a class="btn ghost" href="#/p/${project.id}/brief">Cancel</a><span class="fine">Editing the brief writes a decision to the log.</span></div>
               </form>`
            : `<div class="prose">${project.briefBody
                .split("\n\n")
                .map((p) => `<p>${esc(p)}</p>`)
                .join("")}</div>
               <div class="page-note"><span>${esc(project.brief)}</span><span><a class="small-link" href="#/p/${project.id}/decisions">Decisions</a></span></div>`
        }
      </div>
    </div>
  </div>`;
  return {
    html,
    wire(root) {
      const form = root.querySelector<HTMLFormElement>("form[data-brief]");
      form?.addEventListener("submit", (event) => {
        event.preventDefault();
        const body = String(new FormData(form).get("body") ?? "").trim();
        if (!body) return;
        const projectId = project.id;
        void (async () => {
          try {
            await saveBrief(projectId, body, project.revision ?? null);
            location.hash = `#/p/${projectId}/brief`;
          } catch (error) {
            console.error(errorMessage(error));
          }
        })();
      });
    },
  };
}

function paragraphs(text: string): string {
  return text
    .split(/\n{2,}/)
    .map((part) => `<p>${esc(part)}</p>`)
    .join("");
}

export function notesView(projectId: string): View {
  const project = findProject(projectId);
  if (!project) {
    return {
      html: `<div class="page"><div class="doc">${pageHead("No project", ["the notebook belongs to a project"])}${emptyState("No project", "Notes live with the work they belong to.", '<a class="btn" href="#/projects">Projects</a>')}</div></div>`,
    };
  }
  const notes = project.notes ?? [];

  const html = `<div class="page">
    ${pageHead("Notes", [esc(project.name), `${notes.length} note${notes.length === 1 ? "" : "s"}`], `<a class="btn ghost" href="#/p/${project.id}">Tasks</a>`)}
    <div class="workspace">
      <div class="doc">
        ${chunk(
          "1",
          `           <p class="lede">The notebook takes what the task does not: a measurement worth trusting, a quote, a question you are not ready to record as a decision.</p>
           <p class="fine">Nothing here is gated, scored or summarised. It stays with the project and the export.</p>`,
        )}
        ${chunk(
          "2",
          `<div class="section-label"><h2>New note</h2><span>plain text</span></div>
           <form class="form note-form" data-note-add>
             <div class="form-field"><label class="field-label" for="note-title">Title <span class="label-opt">optional</span></label><input class="input" id="note-title" name="title" placeholder="A handle for later" /></div>
             <div class="form-field"><label class="field-label" for="note-text">Note</label><textarea class="textarea notepad" id="note-text" name="text" rows="10" required placeholder="Write it in your own words. Link it to a check or a decision if that helps."></textarea></div>
             <div class="actions"><button class="btn" type="submit">Save note</button><span class="fine">Saved to this project. It never enters a gate.</span></div>
           </form>`,
        )}
        ${chunk(
          "3",
          `<div class="section-label"><h2>Notebook</h2><span>${notes.length} entr${notes.length === 1 ? "y" : "ies"}</span></div>
           ${
             notes.length
               ? `<div class="notebook">${notes
                   .map(
                     (note) => `<article class="note-item">
                       <span class="note-time">${shortDate(note.at)}<br />${clock(note.at)}</span>
                       <div class="note-main">${note.title ? `<h3>${esc(note.title)}</h3>` : ""}<div class="note-text">${paragraphs(note.text)}</div></div>
                       <button class="note-del" type="button" data-note-del="${esc(note.id)}" aria-label="Delete note">delete</button>
                     </article>`,
                   )
                   .join("")}</div>`
               : emptyState("The notebook is empty", "A note is not an attempt and not a decision. It is the thing you would otherwise lose between sessions.", "")
           }`,
        )}
      </div>
    </div>
  </div>`;

  return {
    html,
    wire(root) {
      root.querySelector<HTMLFormElement>("form[data-note-add]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        const form = event.currentTarget as HTMLFormElement;
        const data = new FormData(form);
        const title = String(data.get("title") ?? "").trim();
        const text = String(data.get("text") ?? "").trim();
        if (!text) return;
        const projectId = project.id;
        void (async () => {
          try {
            await addProjectNote(projectId, title, text);
            form.reset();
          } catch (error) {
            console.error(errorMessage(error));
          }
        })();
      });
      root.querySelectorAll<HTMLButtonElement>("[data-note-del]").forEach((button) => {
        button.addEventListener("click", () => {
          const id = button.dataset.noteDel;
          if (!id) return;
          const projectId = project.id;
          void (async () => {
            try {
              await deleteProjectNote(projectId, id);
            } catch (error) {
              console.error(errorMessage(error));
            }
          })();
        });
      });
    },
  };
}

import { addDecision, addQuestion, answerQuestion } from "../store";

export function decisionsView(projectId: string, q: URLSearchParams): View {
  const project = findProject(projectId);
  if (!project) return { html: emptyState("No project", "Decisions belong to a project.", '<a class="btn" href="#/projects">Projects</a>') };
  const creating = q.get("new") === "1";
  const decisions = getState().decisions.filter(
    (d) => !d.projectId || d.projectId === project.id,
  );
  const rows = decisions
    .map(
      (d: Decision) => `
      <tr>
        <td class="cell-time">${shortDate(d.at)}</td>
        <td class="cell-name"><details><summary>${esc(d.title)}</summary><div class="detail-body"><p><strong>Why.</strong> ${esc(d.why)}</p><p><strong>Cost.</strong> ${esc(d.cost)}</p></div></details></td>
        <td class="cell-mono">${d.taskId ? esc(d.taskId) : "project"}</td>
      </tr>`,
    )
    .join("");
  const html = `<div class="page">
    ${pageHead("Decisions", [project.name, `${decisions.length} entries`, "append only"], `<a class="btn" href="#/p/${project.id}/decisions?new=1">Record a decision</a>`)}
    <div class="doc wide">
      ${
        creating
          ? `<form class="form inline-form" data-decision>
               <div class="form-row">
                 <div class="form-field grow"><label class="field-label" for="d-title">Decision</label><input class="input" id="d-title" name="title" required placeholder="What was decided?" /></div>
                 <div class="form-field"><label class="field-label" for="d-task">Task</label><input class="input" id="d-task" name="task" placeholder="t14" /></div>
               </div>
               <div class="form-field"><label class="field-label" for="d-why">Why</label><input class="input" id="d-why" name="why" required placeholder="The reason, in one sentence." /></div>
               <div class="form-field"><label class="field-label" for="d-cost">Cost</label><input class="input" id="d-cost" name="cost" required placeholder="What it makes harder." /></div>
               <div class="actions"><button class="btn" type="submit">Record</button><a class="btn ghost" href="#/p/${project.id}/decisions">Cancel</a><span class="fine">A decision with a named cost is harder to reverse quietly.</span></div>
             </form>`
          : ""
      }
      ${
        decisions.length
          ? `<table class="table decisions-table"><thead><tr><th>Date</th><th>Decision</th><th>Scope</th></tr></thead><tbody>${rows}</tbody></table>`
          : emptyState("No decisions recorded", "The first one is usually a constraint you chose, not a feature you added.", `<a class="btn" href="#/p/${project.id}/decisions?new=1">Record the first</a>`)
      }
    </div>
  </div>`;
  return {
    html,
    wire(root) {
      const form = root.querySelector<HTMLFormElement>("form[data-decision]");
      form?.addEventListener("submit", (event) => {
        event.preventDefault();
        const data = new FormData(form);
        const title = String(data.get("title") ?? "").trim();
        const why = String(data.get("why") ?? "").trim();
        const cost = String(data.get("cost") ?? "").trim();
        if (!title || !why || !cost) return;
        void (async () => {
          try {
            await addDecision(project.id, {
              title,
              why,
              cost,
              taskId: String(data.get("task") ?? "").trim() || null,
            });
            location.hash = `#/p/${project.id}/decisions`;
          } catch (error) {
            console.error(error);
          }
        })();
      });
    },
  };
}

export function glossaryView(projectId: string, q: URLSearchParams): View {
  const project = findProject(projectId);
  if (!project) return { html: emptyState("No project", "The glossary belongs to a project.", '<a class="btn" href="#/projects">Projects</a>') };
  const asking = q.get("ask") === "1";
  const concepts = getState().concepts;
  const rows = concepts
    .map(
      (c) => `<tr>
        <td class="cell-mono">${esc(c.id)}</td>
        <td class="cell-name">${esc(c.name)}</td>
        <td class="cell-detail">${esc(c.definition)}</td>
        <td class="cell-mono">${esc(c.area)}</td>
        <td class="cell-stamp">${c.status === "passed" ? '<span class="stamp met">passed</span>' : c.status === "weak" ? '<span class="stamp weak">weak</span>' : '<span class="stamp due">due</span>'}</td>
        <td class="cell-time">${c.lastReviewed ? shortDate(c.lastReviewed) : "never"}</td>
      </tr>`,
    )
    .join("");
  const questions = project.questions
    .map(
      (question) => `<tr>
        <td class="cell-time">${shortDate(question.at)}</td>
        <td>${esc(question.text)}</td>
        <td class="cell-stamp">${
          question.answer
            ? `<span class="stamp met">answered</span><p class="fine">${esc(question.answer)}</p>`
            : `<span class="stamp due">open</span><form class="form inline-form" data-answer-question data-id="${question.id}"><div class="form-row"><input class="input grow" name="answer" required placeholder="What settled it?" /><button class="btn small" type="submit">Answer</button></div></form>`
        }</td>
      </tr>`,
    )
    .join("");
  const html = `<div class="page">
    ${pageHead("Glossary and questions", [project.name, `${concepts.length} concepts`, `${project.questions.filter((x) => !x.answer).length} open`], `<a class="btn" href="#/p/${project.id}/glossary?ask=1">Add a question</a>`)}
    <div class="doc wide">
      ${chunk("1", `${sectionLabel("Concepts", "shared vocabulary")}
        <div class="table-wrap"><table class="table">
          <thead><tr><th>Key</th><th>Concept</th><th>Definition</th><th>Area</th><th>State</th><th>Last</th></tr></thead>
          <tbody>${rows}</tbody>
        </table></div>`)}
      ${chunk("2", `${sectionLabel("Open and answered questions", "the project's unknowns")}
        ${asking
          ? `<form class="form inline-form" data-question>
               <label class="field-label" for="q-text">Question</label>
               <div class="form-row"><input class="input grow" id="q-text" name="text" required placeholder="What is still unknown?" /><button class="btn" type="submit">Record</button><a class="btn ghost" href="#/p/${project.id}/glossary">Cancel</a></div>
               <p class="fine">An open question is a real artifact; answering it is a decision, not a deletion.</p>
             </form>`
          : ""}
        ${questions ? `<table class="table"><thead><tr><th>Since</th><th>Question</th><th>State</th></tr></thead><tbody>${questions}</tbody></table>` : emptyState("No questions yet", "The ones worth keeping are the ones you cannot answer without opening the code.", "")}`)}
    </div>
  </div>`;
  return {
    html,
    wire(root) {
      const form = root.querySelector<HTMLFormElement>("form[data-question]");
      form?.addEventListener("submit", (event) => {
        event.preventDefault();
        const text = String(new FormData(form).get("text") ?? "").trim();
        if (!text) return;
        void (async () => {
          try {
            await addQuestion(project.id, text);
            location.hash = `#/p/${project.id}/glossary`;
          } catch (error) {
            console.error(error);
          }
        })();
      });
      root.querySelectorAll<HTMLFormElement>("form[data-answer-question]").forEach((answerForm) => {
        answerForm.addEventListener("submit", (event) => {
          event.preventDefault();
          const questionId = answerForm.dataset.id;
          const answer = String(new FormData(answerForm).get("answer") ?? "").trim();
          if (!questionId || !answer) return;
          void (async () => {
            try {
              await answerQuestion(questionId, answer);
              location.hash = `#/p/${project.id}/glossary`;
            } catch (error) {
              console.error(error);
            }
          })();
        });
      });
    },
  };
}

export function logView(projectId: string, q: URLSearchParams): View {
  const project = findProject(projectId);
  if (!project) return { html: emptyState("No project", "The log belongs to a project.", '<a class="btn" href="#/projects">Projects</a>') };
  const kind = q.get("kind") ?? "all";
  const kinds = ["all", "gate", "check", "handoff", "decision", "note", "recall", "session", "scope"];
  const events = getState().events.filter((e) => kind === "all" || e.kind === kind);
  const html = `<div class="page">
    ${pageHead("Log", [project.name, "append only", `${getState().events.length} events`])}
    <div class="doc wide">
      <div class="seg" role="group" aria-label="Filter by kind">
        ${kinds
          .map(
            (k) =>
              `<a class="seg-item${k === kind ? " active" : ""}" href="#/p/${project.id}/log${k === "all" ? "" : `?kind=${k}`}">${k}</a>`,
          )
          .join("")}
      </div>
      ${
        events.length
          ? `<table class="table log-table"><thead><tr><th>Time</th><th>Kind</th><th>Entry</th></tr></thead><tbody>${events
              .map(
                (e) =>
                  `<tr><td class="cell-time">${shortDate(e.at)} ${clock(e.at)}</td><td class="cell-kind">${esc(e.kind)}</td><td>${esc(e.text)}</td></tr>`,
              )
              .join("")}</tbody></table>`
          : emptyState("Nothing under this filter", "The log is append-only, so an empty filter means that kind of event has not happened yet.", `<a class="btn ghost" href="#/p/${project.id}/log">Show all</a>`)
      }
    </div>
  </div>`;
  return { html };
}
