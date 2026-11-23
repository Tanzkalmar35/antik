import type { Handoff } from "../data";
import { errorMessage, findProject, getState, submitRecall } from "../store";
import { chunk, emptyState, esc, pageHead, type View } from "../ui";

function matchLines(recall: string, handoff: Handoff): number {
  const words = new Set(
    recall
      .toLowerCase()
      .replace(/[^a-z0-9 ]/g, " ")
      .split(/\s+/)
      .filter((w) => w.length > 3),
  );
  const lines = [handoff.next, handoff.blocking, handoff.thinking];
  return lines.filter((line) => {
    const lineWords = line
      .toLowerCase()
      .replace(/[^a-z0-9 ]/g, " ")
      .split(/\s+/)
      .filter((w) => w.length > 3);
    return lineWords.filter((w) => words.has(w)).length >= 2;
  }).length;
}

export function sessionView(q: URLSearchParams): View {
  const state = getState();
  const requested = q.get("p");
  const project =
    findProject(requested) ??
    findProject(state.session.projectId) ??
    state.projects[0];

  if (!project) {
    return {
      html: `<div class="page"><div class="doc">${pageHead("Session start", ["no projects yet"])}${emptyState(
        "Nothing to resume",
        "Create a project first. A session begins by recalling the last handoff, so there must be a project to open.",
        '<a class="btn" href="#/projects">Go to projects</a>',
      )}</div></div>`,
    };
  }

  const handoff = project.handoff;
  const recall = state.session.projectId === project.id ? state.session.recall : null;
  const matched = recall && handoff ? matchLines(recall, handoff) : 0;
  const task = state.tasks.find((t) => t.id === project.currentTaskId);
  const pending = task?.gates.filter((g) => g.status === "due" || g.status === "failed") ?? [];
  const since = handoff
    ? state.events.filter((e) => new Date(e.at).getTime() > new Date(handoff.at).getTime()).slice(0, 6)
    : state.events.slice(0, 4);

  const recallBlock = `
    ${pageHead("Session start", [project.name, handoff ? "handoff awaiting recall" : "no handoff on record", `${state.events.length} events`])}
    <div class="workspace">
      <div class="doc">
        ${chunk(
          "1",
          `<p class="lede">Before the handoff is revealed, write what you remember: what you were doing, and what comes next. Two sentences is enough.</p>
           <form class="form" data-recall>
             <label class="field-label" for="recall">Your recall</label>
             <textarea class="textarea answer" id="recall" name="recall" rows="4" placeholder="I was ... The next thing is ..." required></textarea>
             <div class="actions">
               <button class="btn" type="submit">Submit recall</button>
               <span class="fine">The app compares your words with the handoff and records the match; it does not grade you.</span>
             </div>
           </form>`,
        )}
        ${chunk(
          "2",
          handoff
            ? `<p class="prose">The handoff stays hidden until the recall is submitted. That is the whole ritual: retrieve first, then check.</p>`
            : `<p class="prose">There is no handoff on record for ${esc(project.name)}. Submitting a recall still opens the session; the briefing below is built from the log instead.</p>`,
        )}
      </div>
    </div>`;

  const revealBlock = `
    ${pageHead("Session start", [project.name, `recall matched ${matched} of ${handoff ? 3 : 0} handoff lines`, `session open`])}
    <div class="workspace">
      <div class="doc">
        ${chunk(
          "1",
          `<div class="recall-box">
             <div class="section-label"><h2>Your recall</h2><span>${recall ? esc(recall.slice(0, 60)) + (recall.length > 60 ? "…" : "") : ""}</span></div>
             <p class="prose">${recall ? esc(recall) : "No recall recorded."}</p>
           </div>`,
        )}
        ${chunk(
          "2",
          handoff
            ? `<div class="section-label"><h2>The handoff, revealed</h2><span>${new Date(handoff.at).toLocaleString("en-GB", { day: "2-digit", month: "short", hour: "2-digit", minute: "2-digit" })}</span></div>
               <dl class="handoff">
                 <div><dt>Next</dt><dd>${esc(handoff.next)}</dd></div>
                 <div><dt>Blocking</dt><dd>${esc(handoff.blocking)}</dd></div>
                 <div><dt>Thinking</dt><dd>${esc(handoff.thinking)}</dd></div>
               </dl>
               <p class="fine">${matched === 3 ? "Three lines matched. The picture held." : matched === 2 ? "Two of three lines matched. One thing slipped." : "One line or none matched. Read the thinking line twice."}</p>`
            : `<p class="prose">No previous handoff. The briefing below is the log's own summary of where the project stands.</p>`,
        )}
        ${chunk(
          "3",
          `<div class="section-label"><h2>What the log says changed</h2><span>since the handoff</span></div>
           <table class="table">
             <thead><tr><th>Time</th><th>Kind</th><th>Entry</th></tr></thead>
             <tbody>${
               since
                 .map(
                   (e) =>
                     `<tr><td class="cell-time">${new Date(e.at).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" })}</td><td class="cell-kind">${esc(e.kind)}</td><td>${esc(e.text)}</td></tr>`,
                 )
                 .join("") || '<tr><td colspan="3" class="cell-detail">Nothing recorded since the handoff.</td></tr>'
             }</tbody>
           </table>`,
        )}
        ${chunk(
          "4",
          task
            ? `<div class="section-label"><h2>Today's task</h2><span>${esc(task.lane)} lane</span></div>
               <p class="taskline"><a href="#/p/${project.id}/task/${task.id}/${task.phase}">${esc(task.title)}</a> <span class="fine">task ${esc(task.batch)} · ${esc(task.phase)}</span></p>
               ${pending.length ? `<p class="owed">Gate 0${pending[0].n} is due. It is the first thing to clear.</p>` : `<p class="fine">No gate is due on this task.</p>`}
               <div class="actions"><a class="btn" href="#/p/${project.id}/task/${task.id}/${task.phase}">${pending.length ? "Clear the gate" : "Resume the task"}</a><a class="btn ghost" href="#/p/${project.id}">Project tasks</a></div>`
            : `<p class="prose">No current task. Plan one from the workbench.</p><div class="actions"><a class="btn" href="#/p/${project.id}">Workbench</a></div>`,
        )}
      </div>
    </div>`;

  const html = `<div class="page">${recall ? revealBlock : recallBlock}</div>`;

  return {
    html,
    wire(root) {
      const form = root.querySelector<HTMLFormElement>("form[data-recall]");
      form?.addEventListener("submit", (event) => {
        event.preventDefault();
        const data = new FormData(form);
        const answer = String(data.get("recall") ?? "").trim();
        if (!answer) return;
        const projectId = project.id;
        void (async () => {
          try {
            await submitRecall(projectId, answer);
          } catch (error) {
            console.error(errorMessage(error));
          }
        })();
      });
    },
  };
}
