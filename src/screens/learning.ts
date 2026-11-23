import {
  cachedReview,
  commit,
  createConcept,
  dueConcepts,
  errorMessage,
  getState,
  loadReview,
  reviewConcept,
  reviewFailure,
  setConceptHelp,
  shortDate,
  ui,
} from "../store";
import {
  chunk,
  conceptStamp,
  emptyState,
  esc,
  helpLabel,
  pageHead,
  sectionLabel,
  type View,
} from "../ui";

export function reviewView(q: URLSearchParams): View {
  const range = q.get("range") ?? "30";
  const days = Number(range) || 30;
  const review = cachedReview();
  const head = `<div class="page">
    ${pageHead("Review", [`last ${days} days`, "computed from the event log", "SQLite"])}
    <div class="doc wide">
      <div class="seg" role="group" aria-label="Range">
        ${["7", "30", "90"]
          .map(
            (r) =>
              `<a class="seg-item${r === range ? " active" : ""}" href="#/review?range=${r}">${r} days</a>`,
          )
          .join("")}
      </div>`;

  if (!review || review.days !== days) {
    const failure = reviewFailure();
    if (!failure) void loadReview(days);
    return {
      html: `${head}
        ${
          failure
            ? emptyState(
                "Review unavailable",
                esc(failure),
                '<button class="btn ghost" type="button" data-review-retry>Try again</button>',
              )
            : emptyState(
                "Reading the log",
                "Computing the six figures from the event log.",
              )
        }
      </div>
    </div>`,
      wire(root) {
        root
          .querySelector<HTMLButtonElement>("[data-review-retry]")
          ?.addEventListener("click", () => {
            void loadReview(days);
          });
      },
    };
  }

  const rows = review.rows
    .map((task) => {
      const gate = task.gate
        ? `<span class="stamp ${task.gate.status === "passed" ? "met" : task.gate.status === "overridden" ? "over" : "due"}">${task.gate.n ? `g${task.gate.n} ` : ""}${task.gate.status}</span>`
        : '<span class="stamp idle">none</span>';
      return `<tr>
        <td class="cell-name">${esc(task.title)}<span class="cell-sub">${esc(task.project)}</span></td>
        <td class="cell-mono">${task.phase}</td>
        <td class="cell-time">${task.leadDays != null ? `${task.leadDays.toFixed(1)} d` : "open"}</td>
        <td class="cell-mono">${task.attempts}</td>
        <td>${gate}</td>
        <td class="cell-mono">${task.checksPassed}/${task.checksTotal}</td>
        <td class="cell-time">${task.closed ? shortDate(task.closed) : "in flight"}</td>
      </tr>`;
    })
    .join("");

  const lead = review.leadTimeDays;
  const rate = review.gatePassRate;
  const html = `${head}
    ${
      review.rows.length
        ? `<table class="table figures">
            <thead><tr><th>Figure</th><th>Value</th><th>Basis</th></tr></thead>
            <tbody>
              <tr><td class="cell-name">Tasks in range</td><td class="cell-value">${review.tasks}</td><td class="cell-detail">${esc(review.basis.tasks)}</td></tr>
              <tr><td class="cell-name">Lead time</td><td class="cell-value">${lead != null ? `${lead.toFixed(1)} d` : "not enough data"}</td><td class="cell-detail">${esc(review.basis.leadTime)}</td></tr>
              <tr><td class="cell-name">Rework</td><td class="cell-value">${review.rework}</td><td class="cell-detail">${esc(review.basis.rework)}</td></tr>
              <tr><td class="cell-name">Gate pass rate</td><td class="cell-value">${rate == null ? "no gates" : `${rate}%`}</td><td class="cell-detail">${esc(review.basis.gatePassRate)}</td></tr>
              <tr><td class="cell-name">Bypasses</td><td class="cell-value ${review.bypasses ? "owed-text" : ""}">${review.bypasses}</td><td class="cell-detail">${esc(review.basis.bypasses)}</td></tr>
              <tr><td class="cell-name">Failing checks now</td><td class="cell-value ${review.failingChecks ? "owed-text" : ""}">${review.failingChecks}</td><td class="cell-detail">${esc(review.basis.failingChecks)}</td></tr>
            </tbody>
          </table>
          <div class="section-label"><h2>Tasks in range</h2><span>${review.rows.length}</span></div>
          <div class="table-wrap"><table class="table">
            <thead><tr><th>Task</th><th>Phase</th><th>Lead</th><th>Attempts</th><th>Gate</th><th>Checks</th><th>Closed</th></tr></thead>
            <tbody>${rows}</tbody>
          </table></div>
          <p class="fine">No productivity score, no streak, no composite index. These are the numbers the log actually produces, with their basis named. A figure with no data says so.</p>`
        : emptyState(
            "Nothing in this range",
            "The log has no tasks in the selected window. A shorter range is honest about an empty week.",
            `<a class="btn ghost" href="#/review?range=90">Try 90 days</a>`,
          )
    }
  </div>
  </div>`;
  return { html };
}

export function conceptsView(q: URLSearchParams): View {
  const mode = q.get("mode") ?? "due";
  const reviewId = q.get("review");
  const state = getState();
  const concepts = state.concepts.filter((c) => {
    if (mode === "due") return c.status === "due";
    if (mode === "weak") return c.status === "weak";
    return true;
  });
  const reviewing = reviewId ? state.concepts.find((c) => c.id === reviewId) : undefined;
  const rows = concepts
    .map(
      (c) => `<tr>
        <td class="cell-mono">${esc(c.id)}</td>
        <td class="cell-name">${esc(c.name)}<span class="cell-sub">${esc(c.definition)}</span></td>
        <td class="cell-mono">${esc(c.area)}</td>
        <td class="cell-mono">${c.seen}</td>
        <td class="cell-mono">
          <select class="input select" data-help="${c.id}" aria-label="Help level for ${esc(c.name)}">
            ${(["full", "hint", "none"] as const)
              .map(
                (level) =>
                  `<option value="${level}"${c.help === level ? " selected" : ""}>${helpLabel(level)}</option>`,
              )
              .join("")}
          </select>
        </td>
        <td class="cell-time">${c.lastReviewed ? shortDate(c.lastReviewed) : "never"}</td>
        <td class="cell-stamp">${conceptStamp(c)}</td>
        <td class="cell-stamp"><a class="small-link" href="#/concepts?mode=${mode}&review=${c.id}">Review</a></td>
      </tr>`,
    )
    .join("");

  const reviewBlock = reviewing
    ? chunk(
        "1",
        `<div class="section-label"><h2>Explain back · ${esc(reviewing.name)}</h2><span>${esc(reviewing.id)} · help ${helpLabel(reviewing.help)}</span></div>
         <p class="prose">Write the explanation from memory, in your own words. The app checks that it is long enough to be an explanation and that it names the concept; the verdict and the next date are recorded, not generated.</p>
         <form class="form" data-concept-review data-concept="${reviewing.id}">
           <label class="field-label" for="cr-answer">Without the notes</label>
           <textarea class="textarea answer" id="cr-answer" name="answer" rows="4" required placeholder="A ${esc(reviewing.name)} is ..."></textarea>
           <div class="actions"><button class="btn" type="submit">Record explanation</button><a class="btn ghost" href="#/concepts?mode=${mode}">Not now</a><span class="fine">A pass moves the next review three weeks out. A thin answer leaves it due.</span></div>
         </form>`,
      )
    : "";

  const html = `<div class="page">
    ${pageHead("Concepts", [`${dueConcepts().length} due or weak`, `${state.concepts.length} in the ledger`])}
    <div class="workspace">
      <div class="doc">
        ${reviewBlock}
        <div class="seg" role="group" aria-label="Concept filter">
          ${[
            ["due", "Due"],
            ["weak", "Weak"],
            ["all", "All"],
          ]
            .map(
              ([key, label]) =>
                `<a class="seg-item${mode === key ? " active" : ""}" href="#/concepts?mode=${key}">${label}</a>`,
            )
            .join("")}
        </div>
        ${ui.conceptNote ? `<p class="gate-note${ui.conceptNote.startsWith("Recorded") || ui.conceptNote.startsWith("Added") ? " good" : ""}">${esc(ui.conceptNote)}</p>` : ""}
        <details class="override">
          <summary>Add a concept</summary>
          <form class="form" data-create-concept>
            <div class="form-row">
              <input class="input" name="id" placeholder="K-053" required pattern="[A-Za-z0-9_-]{1,32}" aria-label="Concept key" />
              <input class="input grow" name="name" placeholder="Name" required aria-label="Concept name" />
            </div>
            <input class="input" name="area" placeholder="Area" required aria-label="Area" />
            <textarea class="textarea answer" name="definition" rows="2" placeholder="The definition a review is scored against."></textarea>
            <div class="actions"><button class="btn ghost" type="submit">Add to the ledger</button><span class="fine">The key is stable and human-authored. New concepts start due.</span></div>
          </form>
        </details>
        ${
          concepts.length
            ? `<div class="table-wrap"><table class="table">
                 <thead><tr><th>Key</th><th>Concept</th><th>Area</th><th>Seen</th><th>Help</th><th>Last</th><th>State</th><th></th></tr></thead>
                 <tbody>${rows}</tbody>
               </table></div>`
            : emptyState(
                "Nothing due",
                "The ledger has nothing in this filter. Reviewing a concept is how it moves out of due; the schedule comes back on its own.",
                `<a class="btn ghost" href="#/concepts?mode=all">See all concepts</a>`,
              )
        }
        <p class="fine">Help level fades as a concept is explained back: full scaffolding, then a hint, then none. The ledger records what remains, not what was covered.</p>
      </div>
    </div>
  </div>`;

  return {
    html,
    wire(root) {
      const form = root.querySelector<HTMLFormElement>("form[data-concept-review]");
      form?.addEventListener("submit", (event) => {
        event.preventDefault();
        const concept = reviewing;
        if (!concept) return;
        const answer = String(new FormData(form).get("answer") ?? "").trim();
        void (async () => {
          try {
            const outcome = await reviewConcept(concept.id, answer);
            form.reset();
            if (outcome.passed) location.hash = `#/concepts?mode=${mode}`;
          } catch (error) {
            ui.conceptNote = errorMessage(error);
            commit();
          }
        })();
      });
      root.querySelectorAll<HTMLSelectElement>("select[data-help]").forEach((select) => {
        select.addEventListener("change", () => {
          const conceptId = select.dataset.help;
          if (!conceptId) return;
          const help = select.value as "full" | "hint" | "none";
          void (async () => {
            try {
              await setConceptHelp(conceptId, help);
            } catch (error) {
              ui.conceptNote = errorMessage(error);
              commit();
            }
          })();
        });
      });
      const createForm = root.querySelector<HTMLFormElement>("form[data-create-concept]");
      createForm?.addEventListener("submit", (event) => {
        event.preventDefault();
        const data = new FormData(createForm);
        const input = {
          id: String(data.get("id") ?? "").trim(),
          name: String(data.get("name") ?? "").trim(),
          area: String(data.get("area") ?? "").trim(),
          definition: String(data.get("definition") ?? "").trim(),
        };
        if (!input.id || !input.name || !input.area) return;
        void (async () => {
          try {
            await createConcept(input);
            createForm.reset();
          } catch (error) {
            ui.conceptNote = errorMessage(error);
            commit();
          }
        })();
      });
    },
  };
}

import { addDrill, runDrill } from "../store";

export function practiceView(): View {
  const state = getState();
  const skills = Array.from(new Set(state.drills.map((d) => d.skill)));
  const rows = state.drills
    .map(
      (drill) => `<tr>
        <td class="cell-name">${esc(drill.name)}<span class="cell-sub">${esc(drill.skill)}</span></td>
        <td class="cell-detail"><code>${esc(drill.command)}</code></td>
        <td class="cell-time">${drill.last ? shortDate(drill.last) : "never"}</td>
        <td class="cell-mono">${drill.attempts}</td>
        <td class="cell-stamp">${drill.passed ? '<span class="stamp met">passed</span>' : '<span class="stamp unmet">open</span>'}</td>
        <td class="cell-stamp"><button class="btn small" type="button" data-drill="${drill.id}">Run drill</button></td>
      </tr>`,
    )
    .join("");
  const html = `<div class="page">
    ${pageHead("Practice", [`${skills.length} target skills`, `${state.drills.length} drills`])}
    <div class="workspace">
      <div class="doc">
        ${chunk(
          "1",
          `<p class="lede">Practice is a separate lane, not a reward for finishing work. Two skills at a time, each with drills a machine can check, and a record that keeps the failures.</p>
           <ul class="conceptlist">${skills.map((s) => `<li>${esc(s)}</li>`).join("")}</ul>`,
        )}
        ${chunk(
          "2",
          `${sectionLabel("Drills", "run what can be checked")}
           <div class="table-wrap"><table class="table">
             <thead><tr><th>Drill</th><th>Check</th><th>Last</th><th>Attempts</th><th>State</th><th></th></tr></thead>
             <tbody>${rows}</tbody>
           </table></div>
           <p class="fine">A drill runs its command in the projects folder. The exit status is the verdict, and a failure stays in the record.</p>
           ${ui.drillNote ? `<div class="banner">${esc(ui.drillNote)}</div>` : ""}
           <form class="form check-form" data-add-drill>
             <div class="section-label"><h2>Add a drill</h2><span>one command</span></div>
             <div class="form-field"><label class="field-label" for="drill-skill">Skill</label><input class="input" id="drill-skill" name="skill" required maxlength="200" placeholder="concurrency" /></div>
             <div class="form-field"><label class="field-label" for="drill-name">Drill</label><input class="input" id="drill-name" name="name" required maxlength="200" placeholder="cancel a token twice" /></div>
             <div class="form-field"><label class="field-label" for="drill-command">Command</label><input class="input" id="drill-command" name="command" required placeholder="cargo test --test cancel" /></div>
             <div class="actions"><button class="btn" type="submit">Add drill</button><span class="fine">One allowlisted program plus arguments. A drill cannot use a shell.</span></div>
           </form>`,
        )}
      </div>
    </div>
  </div>`;
  return {
    html,
    wire(root) {
      root.querySelector<HTMLFormElement>("form[data-add-drill]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        const form = event.currentTarget as HTMLFormElement;
        const data = new FormData(form);
        const input = {
          skill: String(data.get("skill") ?? "").trim(),
          name: String(data.get("name") ?? "").trim(),
          command: String(data.get("command") ?? "").trim(),
        };
        if (!input.skill || !input.name || !input.command) return;
        ui.drillNote = null;
        void addDrill(input)
          .then(() => form.reset())
          .catch((error) => {
            ui.drillNote = errorMessage(error);
            commit();
          });
      });
      root.querySelectorAll<HTMLButtonElement>("[data-drill]").forEach((button) => {
        button.addEventListener("click", () => {
          const drillId = button.dataset.drill;
          if (!drillId) return;
          button.disabled = true;
          void (async () => {
            try {
              await runDrill(drillId);
            } catch (error) {
              ui.drillNote = errorMessage(error);
              commit();
            } finally {
              button.disabled = false;
            }
          })();
        });
      });
    },
  };
}
