import {
  addAllowlistPath,
  checkModelHealth,
  commit,
  errorMessage,
  exportStore,
  getState,
  isBackend,
  removeAllowlistPath,
  reset,
  saveModel,
  saveModelEndpoint,
  ui,
} from "../store";
import { chunk, esc, pageHead, sectionLabel, type View } from "../ui";

export function settingsView(): View {
  const state = getState();
  const backend = isBackend();
  const html = `<div class="page">
    ${pageHead("Settings", ["one machine", backend ? "local store" : "demo fixture"])}
    <div class="workspace">
      <div class="doc">
        ${ui.settingsNote ? `<p class="gate-note${ui.settingsNoteOk ? " good" : ""}">${esc(ui.settingsNote)}</p>` : ""}
        ${chunk(
          "1",
          `<div class="section-label"><h2>Model</h2><span>${esc(state.settings.model)}</span></div>
           <p class="prose">The gate and the ledger do not depend on a model. With the model off, gates still run, hints are unavailable, and the app says so.</p>
           <form class="form" data-model>
             <label class="choice"><input type="radio" name="model" value="local"${state.settings.model === "local" ? " checked" : ""} /><span><strong>Local model</strong><span class="fine">An endpoint on this machine. Read scope only; no write access.</span></span></label>
             <label class="choice"><input type="radio" name="model" value="api" disabled /><span><strong>Hosted API</strong><span class="fine">Not in this build. The model lane speaks to a local endpoint only, so no project leaves this machine.</span></span></label>
             <label class="choice"><input type="radio" name="model" value="none"${state.settings.model === "none" ? " checked" : ""} /><span><strong>Off</strong><span class="fine">No generation. Gates fail open, and the bypass is logged.</span></span></label>
             <div class="actions"><button class="btn" type="submit">Save model choice</button><span class="fine">${backend ? "Stored in SQLite and written to the event log." : "This browser fixture keeps the choice locally."}</span></div>
           </form>
           <div class="section-label spaced"><h2>Local model</h2><span>Ollama</span></div>
           <p class="prose">One endpoint, this machine. The model drafts questions and gives advice; it cannot approve, score or write.</p>
           <form class="form" data-endpoint>
             <div class="form-row">
               <div class="form-field"><label class="field-label" for="model-url">Endpoint</label><input class="input grow" id="model-url" name="url" value="${esc(state.settings.modelUrl ?? "http://127.0.0.1:11434")}" required /></div>
               <div class="form-field"><label class="field-label" for="model-name">Model</label><input class="input" id="model-name" name="name" value="${esc(state.settings.modelName ?? "qwen3.5:9b")}" required /></div>
             </div>
             <div class="actions">
               <button class="btn ghost" type="submit">Save endpoint</button>
               <button class="btn ghost" type="button" data-health>Test connection</button>
               <span class="fine">Loopback only. Any other host is refused, so a typo cannot send a project elsewhere.</span>
             </div>
             ${ui.modelHealth ? `<p class="fine"><span class="stamp ${ui.modelHealth.reachable ? "met" : "unmet"}">${ui.modelHealth.reachable ? "reachable" : "unreachable"}</span> <code>${esc(ui.modelHealth.baseUrl)}</code> · ${esc(ui.modelHealth.model)} — ${esc(ui.modelHealth.detail)}</p>` : ""}
           </form>`,
        )}
        ${chunk(
          "2",
          `${sectionLabel("Read scope", `${state.settings.allowlist.length} paths`)}
           <p class="prose">The model may read these paths and quote them. Nothing outside the list is visible to it, and no path is writable.</p>
           <p class="fine">Paths are relative to the attached project root. <code>**</code> on its own covers the whole project; either way a prompt holds at most 40 files and 40,000 characters, and build output, dotfolders and binaries are left out.</p>
           <ul class="allowlist">${state.settings.allowlist
             .map(
               (path) =>
                 `<li><code>${esc(path)}</code><button class="small-link" type="button" data-remove-path="${esc(path)}">remove</button></li>`,
             )
             .join("")}</ul>
           <form class="form inline" data-path>
             <label class="field-label" for="path">Add a path</label>
             <div class="form-row"><input class="input grow" id="path" name="path" placeholder="src/queue/**" required /><button class="btn ghost" type="submit">Add path</button></div>
           </form>
           <div class="section-label spaced"><h2>Write access</h2><span>locked</span></div>
           <label class="choice disabled"><input type="checkbox" disabled /><span><strong>Allow the model to write files</strong><span class="fine">Off in code, not a setting. An agent that can edit the record can fake the result.</span></span></label>`,
        )}
        ${chunk(
          "3",
          `${sectionLabel("Data", backend ? "SQLite" : "local")}
           <p class="prose">${backend ? "The store is one SQLite file on this machine. Markdown is a projection kept current from it. Export writes a JSON snapshot of every table; a backup is taken before each schema migration." : "This browser build is a design fixture. The desktop app keeps its records in SQLite and exports a JSON snapshot of every table."}</p>
           <dl class="facts inline">
             <div><dt>Projects</dt><dd>${state.projects.length}</dd></div>
             <div><dt>Events</dt><dd>${state.events.length}</dd></div>
             <div><dt>Storage</dt><dd>${backend ? "SQLite, one file" : "this browser, localStorage"}</dd></div>
           </dl>
           <div class="actions">
             <button class="btn ghost" type="button" data-export>Export JSON</button>
             ${backend ? "" : `<button class="btn ghost" type="button" data-reset-empty>Reset to empty</button>
             <button class="btn ghost" type="button" data-reset-demo>Restore demo data</button>`}
           </div>
           <p class="fine">${backend ? "Export is a readable snapshot, not a restore file. Backups sit beside the database and the last three are kept." : "Reset to empty is the first-run state: no projects, one honest empty screen. Restore brings the seeded project back."}</p>`,
        )}
      </div>
    </div>
  </div>`;

  return {
    html,
    wire(root) {
      root.querySelector<HTMLFormElement>("form[data-model]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        const form = event.currentTarget as HTMLFormElement;
        const value = new FormData(form).get("model");
        if (value !== "local" && value !== "api" && value !== "none") return;
        void (async () => {
          try {
            await saveModel(value);
          } catch (error) {
            ui.settingsNote = errorMessage(error);
            ui.settingsNoteOk = false;
            commit();
          }
        })();
      });
      root.querySelector<HTMLFormElement>("form[data-endpoint]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        const form = event.currentTarget as HTMLFormElement;
        const data = new FormData(form);
        const url = String(data.get("url") ?? "").trim();
        const name = String(data.get("name") ?? "").trim();
        if (!url || !name) return;
        void (async () => {
          try {
            await saveModelEndpoint(url, name);
          } catch (error) {
            ui.settingsNote = errorMessage(error);
            ui.settingsNoteOk = false;
            commit();
          }
        })();
      });
      root.querySelector<HTMLButtonElement>("[data-health]")?.addEventListener("click", () => {
        void (async () => {
          try {
            await checkModelHealth();
          } catch (error) {
            ui.settingsNote = errorMessage(error);
            ui.settingsNoteOk = false;
            commit();
          }
        })();
      });
      root.querySelector<HTMLFormElement>("form[data-path]")?.addEventListener("submit", (event) => {
        event.preventDefault();
        const form = event.currentTarget as HTMLFormElement;
        const path = String(new FormData(form).get("path") ?? "").trim();
        if (!path) return;
        void (async () => {
          try {
            await addAllowlistPath(path);
            form.reset();
          } catch (error) {
            ui.settingsNote = errorMessage(error);
            ui.settingsNoteOk = false;
            commit();
          }
        })();
      });
      root.querySelectorAll<HTMLButtonElement>("[data-remove-path]").forEach((button) => {
        button.addEventListener("click", () => {
          const path = button.dataset.removePath;
          if (!path) return;
          void (async () => {
            try {
              await removeAllowlistPath(path);
            } catch (error) {
              ui.settingsNote = errorMessage(error);
              ui.settingsNoteOk = false;
              commit();
            }
          })();
        });
      });
      root.querySelector<HTMLButtonElement>("[data-export]")?.addEventListener("click", () => {
        void (async () => {
          try {
            const json = await exportStore();
            const blob = new Blob([json], { type: "application/json" });
            const url = URL.createObjectURL(blob);
            const anchor = document.createElement("a");
            anchor.href = url;
            anchor.download = `antik-export-${new Date().toISOString().slice(0, 10)}.json`;
            anchor.click();
            URL.revokeObjectURL(url);
          } catch (error) {
            console.error("export failed", error);
          }
        })();
      });
      root.querySelector<HTMLButtonElement>("[data-reset-empty]")?.addEventListener("click", () => {
        if (window.confirm("Reset to the empty first-run state? This clears the mock data in this browser.")) {
          reset("empty");
          location.hash = "#/projects";
        }
      });
      root.querySelector<HTMLButtonElement>("[data-reset-demo]")?.addEventListener("click", () => {
        if (window.confirm("Restore the seeded demo project? Current mock changes will be lost.")) {
          reset("demo");
          location.hash = "#/projects";
        }
      });
    },
  };
}
