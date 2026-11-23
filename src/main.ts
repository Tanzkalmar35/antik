import {
  commit,
  errorMessage,
  findProject,
  findTask,
  getState,
  hydrate,
  setCurrentTask,
  subscribe,
  ui,
} from "./store";
import { renderShell } from "./shell";
import { esc, type View } from "./ui";
import {
  briefView,
  decisionsView,
  glossaryView,
  logView,
  notesView,
  projectsView,
  workbenchView,
} from "./screens/project";
import { sessionView } from "./screens/session";
import {
  gateView,
  implementView,
  planView,
  reflectView,
  verifyView,
} from "./screens/task";
import { conceptsView, practiceView, reviewView } from "./screens/learning";
import { settingsView } from "./screens/settings";

interface PaletteItem {
  label: string;
  hint: string;
  href: string;
}

function paletteItems(): PaletteItem[] {
  const state = getState();
  const items: PaletteItem[] = [
    { label: "Projects", hint: "section", href: "#/projects" },
    { label: "Session start", hint: "recall", href: "#/session" },
    { label: "Review", hint: "section", href: "#/review" },
    { label: "Concepts", hint: "ledger", href: "#/concepts" },
    { label: "Practice", hint: "drills", href: "#/practice" },
    { label: "Settings", hint: "model and data", href: "#/settings" },
  ];
  state.projects.forEach((project) => {
    items.push({ label: project.name, hint: "project", href: `#/p/${project.id}` });
    items.push({
      label: `${project.name} notes`,
      hint: "notebook",
      href: `#/p/${project.id}/notes`,
    });
  });
  state.tasks.forEach((task) => {
    const project = state.projects.find((p) => p.id === task.projectId);
    items.push({
      label: task.title,
      hint: `${project?.name ?? ""} · ${task.phase}`,
      href: `#/p/${task.projectId}/task/${task.id}/${task.phase}`,
    });
  });
  return items;
}

let activeIndex = 0;
let filtered: PaletteItem[] = [];

function renderPaletteList(query: string): void {
  const list = document.getElementById("palette-list");
  if (!list) return;
  const needle = query.trim().toLowerCase();
  filtered = paletteItems().filter(
    (item) =>
      !needle ||
      item.label.toLowerCase().includes(needle) ||
      item.hint.toLowerCase().includes(needle),
  );
  activeIndex = 0;
  list.innerHTML = "";
  if (!filtered.length) {
    const empty = document.createElement("li");
    empty.className = "palette-item empty";
    empty.textContent = "Nothing matches.";
    list.appendChild(empty);
    return;
  }
  filtered.forEach((item, index) => {
    const li = document.createElement("li");
    li.className = `palette-item${index === 0 ? " active" : ""}`;
    const label = document.createElement("span");
    label.textContent = item.label;
    const hint = document.createElement("span");
    hint.className = "hint";
    hint.textContent = item.hint;
    li.appendChild(label);
    li.appendChild(hint);
    li.addEventListener("click", () => {
      closePalette();
      location.hash = item.href;
    });
    list.appendChild(li);
  });
}

function paletteOpen(): boolean {
  const palette = document.getElementById("palette");
  return Boolean(palette && !palette.hidden);
}

function openPalette(): void {
  const palette = document.getElementById("palette");
  const input = document.getElementById("palette-input") as HTMLInputElement | null;
  if (!palette || !input) return;
  palette.hidden = false;
  input.value = "";
  renderPaletteList("");
  input.focus();
}

function closePalette(): void {
  const palette = document.getElementById("palette");
  if (palette) palette.hidden = true;
}

function movePalette(direction: number): void {
  if (!filtered.length) return;
  activeIndex = (activeIndex + direction + filtered.length) % filtered.length;
  document.querySelectorAll<HTMLElement>(".palette-item").forEach((el, index) => {
    el.classList.toggle("active", index === activeIndex);
  });
}

type Theme = "light" | "dark";

function currentTheme(): Theme {
  return document.documentElement.dataset.theme === "dark" ? "dark" : "light";
}

function syncThemeButton(): void {
  document.querySelectorAll<HTMLButtonElement>("[data-theme-toggle]").forEach((button) => {
    const next = currentTheme() === "dark" ? "light" : "dark";
    button.textContent = next;
    const label = `Switch to ${next} theme`;
    button.setAttribute("aria-label", label);
    button.title = label;
  });
}

function bindTheme(): void {
  document.querySelector<HTMLButtonElement>("[data-theme-toggle]")?.addEventListener("click", () => {
    const next: Theme = currentTheme() === "dark" ? "light" : "dark";
    try {
      localStorage.setItem("antik:theme", next);
    } catch {
      /* the mock must survive a blocked storage quota */
    }
    document.documentElement.dataset.theme = next;
    syncThemeButton();
  });
  syncThemeButton();
}

/// Creating a project is reachable from anywhere, including the first run when
/// the sidebar is the only navigation the user has.
function bindNewProject(): void {
  document.querySelector<HTMLButtonElement>("[data-new-project-rail]")?.addEventListener("click", () => {
    ui.newProject = true;
    const onProjects = location.hash === "" || location.hash === "#" || location.hash === "#/projects";
    if (onProjects) {
      commit();
      document.querySelector<HTMLInputElement>("#np-name")?.focus();
    } else {
      location.hash = "#/projects";
    }
  });
}

function bindPalette(): void {
  const palette = document.getElementById("palette");
  const input = document.getElementById("palette-input") as HTMLInputElement | null;
  if (!palette || !input) return;
  document.querySelector<HTMLButtonElement>("[data-palette-open]")?.addEventListener("click", () => {
    openPalette();
  });
  palette.addEventListener("click", (event) => {
    if (event.target === palette) closePalette();
  });
  input.addEventListener("input", () => renderPaletteList(input.value));
  input.addEventListener("keydown", (event) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      movePalette(1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      movePalette(-1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      const chosen = filtered[activeIndex];
      if (chosen) {
        closePalette();
        location.hash = chosen.href;
      }
    }
  });
}

window.addEventListener("keydown", (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
    event.preventDefault();
    if (paletteOpen()) closePalette();
    else {
      openPalette();
      bindPalette();
    }
  } else if (event.key === "Escape" && paletteOpen()) {
    closePalette();
  }
});

interface Resolved {
  view: View;
  crumbs: string[];
  route: string;
}

/// Opening a batch is how a project's current batch changes, from the batch
/// list, the palette or a deep link alike. One attempt per task route keeps a
/// failed switch from retrying on every render it did not cause.
let syncedRoute: string | null = null;

function syncCurrentTask(projectId: string, taskId: string, route: string): void {
  if (route === syncedRoute) return;
  syncedRoute = route;
  const project = findProject(projectId);
  const task = findTask(taskId);
  if (!project || !task || task.projectId !== project.id) return;
  if (project.currentTaskId === task.id) return;
  void setCurrentTask(project.id, task.id).catch((error) => {
    console.error(errorMessage(error));
  });
}

function resolve(): Resolved {
  const raw = location.hash.replace(/^#/, "") || "/projects";
  const [pathPart, queryPart] = raw.split("?");
  const query = new URLSearchParams(queryPart ?? "");
  const seg = pathPart.split("/").filter(Boolean);
  const route = `#${raw}`;
  const state = getState();

  if (!seg.length || seg[0] === "projects") {
    return { view: projectsView(), crumbs: ["Projects"], route };
  }
  if (seg[0] === "session") {
    return { view: sessionView(query), crumbs: ["Session start"], route };
  }
  if (seg[0] === "review") {
    return { view: reviewView(query), crumbs: ["Review"], route };
  }
  if (seg[0] === "concepts") {
    return { view: conceptsView(query), crumbs: ["Concepts"], route };
  }
  if (seg[0] === "practice") {
    return { view: practiceView(), crumbs: ["Practice"], route };
  }
  if (seg[0] === "settings") {
    return { view: settingsView(), crumbs: ["Settings"], route };
  }
  if (seg[0] === "p" && seg[1]) {
    const project = findProject(seg[1]);
    const name = project?.name ?? seg[1];
    if (!seg[2]) return { view: workbenchView(seg[1]), crumbs: ["Projects", name], route };
    if (seg[2] === "brief") return { view: briefView(seg[1], query), crumbs: [name, "Brief"], route };
    if (seg[2] === "notes") return { view: notesView(seg[1]), crumbs: [name, "Notes"], route };
    if (seg[2] === "decisions")
      return { view: decisionsView(seg[1], query), crumbs: [name, "Decisions"], route };
    if (seg[2] === "glossary")
      return { view: glossaryView(seg[1], query), crumbs: [name, "Glossary"], route };
    if (seg[2] === "log") return { view: logView(seg[1], query), crumbs: [name, "Log"], route };
    if (seg[2] === "task" && seg[3]) {
      syncCurrentTask(seg[1], seg[3], route);
      const task = findTask(seg[3]);
      const taskName = task?.title ?? seg[3];
      if (seg[4] === "gate") {
        return {
          view: gateView(seg[1], seg[3], seg[5] ?? ""),
          crumbs: [name, taskName, "Gate"],
          route,
        };
      }
      if (seg[4] === "plan") return { view: planView(seg[1], seg[3]), crumbs: [name, taskName, "Plan"], route };
      if (seg[4] === "implement")
        return { view: implementView(seg[1], seg[3]), crumbs: [name, taskName, "Implement"], route };
      if (seg[4] === "verify")
        return { view: verifyView(seg[1], seg[3]), crumbs: [name, taskName, "Verify"], route };
      if (seg[4] === "reflect")
        return { view: reflectView(seg[1], seg[3]), crumbs: [name, taskName, "Reflect"], route };
    }
  }
  void state;
  return {
    view: {
      html: `<div class="page"><div class="doc"><header class="doc-head"><div class="doc-head-main"><h1>Not found</h1><p class="byline"><span>nothing lives at this address</span></p></div></header><div class="empty"><h3>That route is not part of the mock</h3><p>${esc(raw)}</p><a class="btn" href="#/projects">Back to projects</a></div></div></div>`,
    },
    crumbs: ["Not found"],
    route,
  };
}

let lastRoute = "";

function render(): void {
  const root = document.getElementById("app");
  if (!root) return;
  const resolved = resolve();
  if (lastRoute && lastRoute !== resolved.route) {
    ui.gateNote = null;
    ui.conceptNote = null;
    ui.settingsNote = null;
  }
  root.innerHTML = renderShell(resolved.view.html, {
    route: resolved.route,
    crumbs: resolved.crumbs,
  });
  resolved.view.wire?.(root);
  bindNewProject();
  bindPalette();
  bindTheme();
  const view = document.getElementById("view");
  if (view && lastRoute !== resolved.route) {
    view.scrollTo(0, 0);
    lastRoute = resolved.route;
  }
}

subscribe(render);
window.addEventListener("hashchange", render);

async function boot(): Promise<void> {
  await hydrate();
  if (!location.hash) location.hash = "#/projects";
  render();
}

void boot();
