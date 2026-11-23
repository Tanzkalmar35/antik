import { findProject, getState, pendingGate, sessionLength } from "./store";
import { esc } from "./ui";

export interface ShellContext {
  route: string;
  crumbs: string[];
}

function railLink(href: string, label: string, active: boolean): string {
  return `<a class="rail-link${active ? " active" : ""}" href="${href}"${active ? ' aria-current="page"' : ""}>${esc(label)}</a>`;
}

function isRoute(route: string, prefix: string): boolean {
  return route === prefix || route.startsWith(`${prefix}/`) || route.startsWith(`${prefix}?`);
}

export function renderShell(viewHtml: string, ctx: ShellContext): string {
  const state = getState();
  const route = ctx.route;
  const [, ...parts] = route.split("?")[0].split("/");
  const projectId = parts[0] === "p" ? parts[1] : null;
  const project = findProject(projectId);
  const sessionProject = findProject(state.session.projectId);

  const projectLinks = project
    ? `<div class="rail-group">
        <div class="rail-head">This project</div>
        ${railLink(`#/p/${project.id}`, "Tasks", route === `#/p/${project.id}`)}
        ${railLink(`#/p/${project.id}/notes`, "Notes", isRoute(route, `#/p/${project.id}/notes`))}
        ${railLink(`#/p/${project.id}/brief`, "Brief", isRoute(route, `#/p/${project.id}/brief`))}
        ${railLink(`#/p/${project.id}/decisions`, "Decisions", isRoute(route, `#/p/${project.id}/decisions`))}
        ${railLink(`#/p/${project.id}/glossary`, "Glossary", isRoute(route, `#/p/${project.id}/glossary`))}
        ${railLink(`#/p/${project.id}/log`, "Log", isRoute(route, `#/p/${project.id}/log`))}
      </div>`
    : "";

  const projectRows = state.projects
    .map((p) => {
      const task = state.tasks.find((t) => t.id === p.currentTaskId);
      const due = task ? Boolean(pendingGate(task)) : false;
      return `<a class="rail-project${p.id === projectId ? " current" : ""}" href="#/p/${p.id}" title="${esc(p.brief)}"${p.id === projectId ? ' aria-current="page"' : ""}>
        <span class="rail-project-name">${esc(p.name)}</span>
        <span class="rail-project-meta">${due ? '<span class="dot" aria-label="gate due"></span>' : ""}</span>
      </a>`;
    })
    .join("");

  const theme = document.documentElement.dataset.theme === "dark" ? "dark" : "light";
  const sessionLabel = sessionProject
    ? `${sessionProject.name} · ${sessionLength()}`
    : "No session";

  return `
  <div class="app">
    <header class="topbar">
      <a class="wordmark" href="#/projects">antik</a>
      <div class="crumbs">${ctx.crumbs
        .map((c, i) => `<span class="${i === ctx.crumbs.length - 1 ? "here" : ""}">${esc(c)}</span>`)
        .join('<span class="slash">/</span>')}</div>
      <div class="topmeta">
        <span class="session">${esc(sessionLabel)}</span>
        <button class="kbd-btn" type="button" data-theme-toggle aria-label="Switch theme" title="Switch theme">${theme === "dark" ? "Light" : "Dark"}</button>
        <button class="kbd-btn" type="button" data-palette-open><span>Search</span><kbd>⌘K</kbd></button>
      </div>
    </header>

    <nav class="rail" aria-label="Sections">
      <div class="rail-group">
        <div class="rail-head has-action"><span>Projects</span><button class="rail-add" type="button" data-new-project-rail aria-label="New project" title="New project">+</button></div>
        ${projectRows || '<p class="rail-empty">None yet</p>'}
      </div>
      ${projectLinks}
      <div class="rail-group">
        <div class="rail-head">Across projects</div>
        ${railLink("#/review", "Review", isRoute(route, "#/review"))}
        ${railLink("#/concepts", "Concepts", isRoute(route, "#/concepts"))}
        ${railLink("#/practice", "Practice", isRoute(route, "#/practice"))}
        ${railLink("#/settings", "Settings", isRoute(route, "#/settings"))}
      </div>
      <div class="rail-foot">
        <span>Local · ${state.events.length} events logged</span>
      </div>
    </nav>

    <main class="view" id="view">${viewHtml}</main>

    <div class="palette" id="palette" hidden>
      <div class="palette-panel" role="dialog" aria-label="Jump to a section">
        <input class="palette-input" id="palette-input" type="text" placeholder="Jump to a project, section or task" autocomplete="off" />
        <ul class="palette-list" id="palette-list"></ul>
        <div class="palette-foot"><span>Enter to open</span><span>Esc to close</span></div>
      </div>
    </div>
  </div>`;
}
