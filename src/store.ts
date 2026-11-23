import { commands } from "./bindings";
import type {
  Bootstrap,
  ConceptSummary,
  EventSummary as WireEvent,
  ProjectDetail,
  Review as WireReview,
  Settings as WireSettings,
  TaskDetail,
} from "./bindings";
import {
  emptyState,
  seedState,
  type AppState,
  type AuditEvent,
  type Check,
  type CheckRun,
  type Concept,
  type EventKind,
  type Gate,
  type Handoff,
  type ModelHealth,
  type PlanItem,
  type PlanItemKind,
  type PlanProposal,
  type Project,
  type Task,
} from "./data";

const KEY = "antik:mock:v1";
const tauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function readLocal(): AppState {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return seedState();
    const parsed = JSON.parse(raw) as AppState;
    if (!parsed.projects || !parsed.tasks) return seedState();
    parsed.projects.forEach((project) => {
      if (!Array.isArray(project.notes)) project.notes = [];
    });
    return parsed;
  } catch {
    return seedState();
  }
}

let state: AppState = tauri ? emptyState() : readLocal();
let reviewCache: WireReview | null = null;
let reviewPending: number | null = null;
let reviewError: string | null = null;
const listeners = new Set<() => void>();

export const ui: {
  newProject: boolean;
  newBatch: boolean;
  conceptNote: string | null;
  runningCheck: string | null;
  checkRunId: string | null;
  checkRun: CheckRun | null;
  checkNote: string | null;
  drillNote: string | null;
  gateNote: string | null;
  settingsNote: string | null;
  settingsNoteOk: boolean;
  modelHealth: ModelHealth | null;
  critique: string[] | null;
  critiqueFiles: string[] | null;
  critiqueNote: string | null;
  planEditing: string | null;
  planNote: string | null;
  planChatPending: boolean;
  planProposal: PlanProposal | null;
  planProposalNote: string | null;
} = {
  newProject: false,
  newBatch: false,
  conceptNote: null,
  runningCheck: null,
  checkRunId: null,
  checkRun: null,
  checkNote: null,
  drillNote: null,
  gateNote: null,
  settingsNote: null,
  settingsNoteOk: true,
  modelHealth: null,
  critique: null,
  critiqueFiles: null,
  critiqueNote: null,
  planEditing: null,
  planNote: null,
  planChatPending: false,
  planProposal: null,
  planProposalNote: null,
};

export function getState(): AppState {
  return state;
}

export function subscribe(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

function notify(): void {
  listeners.forEach((fn) => fn());
}

/// Local cache for surfaces whose commands arrive in a later milestone
/// (gates, checks, concepts, settings). Backed data is refreshed from SQLite.
export function commit(): void {
  if (!tauri) {
    try {
      localStorage.setItem(KEY, JSON.stringify(state));
    } catch {
      /* a full quota must not break the session */
    }
  }
  notify();
}

export function reset(mode: "demo" | "empty"): void {
  state = mode === "demo" ? seedState() : emptyState();
  reviewCache = null;
  reviewError = null;
  ui.newProject = false;
  ui.conceptNote = null;
  ui.gateNote = null;
  ui.critique = null;
  ui.critiqueFiles = null;
  ui.critiqueNote = null;
  ui.planEditing = null;
  ui.planNote = null;
  ui.planChatPending = false;
  ui.planProposal = null;
  ui.planProposalNote = null;
  ui.runningCheck = null;
  ui.checkRunId = null;
  ui.checkRun = null;
  ui.checkNote = null;
  ui.drillNote = null;
  commit();
}

export function logEvent(kind: EventKind, text: string): void {
  state.events.unshift({
    id: `e-${Math.random().toString(36).slice(2, 8)}`,
    at: new Date().toISOString(),
    kind,
    text,
  });
}

export function findProject(id: string | null): Project | undefined {
  return state.projects.find((p) => p.id === id);
}

export function findTask(id: string | null): Task | undefined {
  return state.tasks.find((t) => t.id === id);
}

export function tasksOf(projectId: string): Task[] {
  return state.tasks.filter((t) => t.projectId === projectId);
}

export function conceptOf(id: string): Concept | undefined {
  return state.concepts.find((c) => c.id === id);
}

export function currentTaskOf(project: Project): Task | undefined {
  return findTask(project.currentTaskId);
}

export function pendingGate(task: Task) {
  return task.gates.find((g) => g.status === "due" || g.status === "failed");
}

export function gateResolved(task: Task): boolean {
  return !task.gates.some((g) => g.status === "due" || g.status === "failed");
}

export function checksMet(task: Task): number {
  return task.checks.filter((c) => c.result === "pass" || c.result === "done").length;
}

export function dueConcepts(): Concept[] {
  return state.concepts.filter((c) => c.status === "due" || c.status === "weak");
}

/// Browser-fixture scorer for a gate drafted from a task. It mirrors the
/// stored rubric closely enough for the design review: a mechanism word from
/// the title and a word for the cost.
function evaluateDraftAnswer(title: string, answer: string): { pass: boolean; note: string } {
  const words = answer.trim().split(/\s+/).filter(Boolean);
  if (words.length < 10) {
    return { pass: false, note: "Not met. An explanation needs at least ten words." };
  }
  const stem = (token: string): string =>
    token
      .toLowerCase()
      .replace(/[^a-z0-9]/g, "")
      .replace(/s$/, "");
  const tokens = new Set(words.map(stem).filter(Boolean));
  const mechanism = title
    .split(/[^A-Za-z0-9]+/)
    .map(stem)
    .filter((token) => token.length > 3);
  const cost = [
    "break",
    "cost",
    "downstream",
    "expensive",
    "fail",
    "failure",
    "latency",
    "leak",
    "loss",
    "rework",
    "risk",
    "stale",
    "timeout",
    "waste",
    "wrong",
  ].map(stem);
  if (!mechanism.some((term) => tokens.has(term))) {
    return { pass: false, note: "Not met. The answer never names the mechanism." };
  }
  if (!cost.some((term) => tokens.has(term))) {
    return {
      pass: false,
      note: "Not met. The answer never names the cost of getting it wrong.",
    };
  }
  return { pass: true, note: "Met. It names the mechanism and the cost." };
}

export function evaluateAnswer(answer: string): { pass: boolean; note: string } {
  const words = answer.trim().split(/\s+/).filter(Boolean);
  if (words.length < 10) {
    return { pass: false, note: "Too short to be an explanation. Name the cost the cap protects." };
  }
  const namesCap = /\b(five|5|cap|capped|attempts?)\b/i.test(answer);
  const namesCost =
    /\b(cost|leak|outlive|outlives|client|request|timeout|latency|budget|expensive|downstream|gave up|give up|gives up)\b/i.test(
      answer,
    );
  if (!namesCap) {
    return { pass: false, note: "The answer never names the cap. Say why five and not more." };
  }
  if (!namesCost) {
    return { pass: false, note: "It describes the delay but not what the cap protects. Name the cost." };
  }
  return { pass: true, note: "Met. The answer names the cap and the cost it protects." };
}

// ---------------------------------------------------------------------------
// Backend-backed store
// ---------------------------------------------------------------------------

function firstLine(text: string): string {
  return text.split(/\n/, 1)[0]?.trim() || text.trim();
}

function mapGate(gate: TaskDetail["gates"][number]): Gate {
  return {
    id: gate.id,
    conceptId: gate.conceptId ?? null,
    n: gate.n,
    question: gate.question,
    hint: gate.hint,
    status: gate.status,
    attempts: gate.attempts,
    hintUsed: gate.hintUsed,
    answer: gate.answer ?? "",
    reason: gate.reason ?? "",
    evaluated: gate.evaluatedAt ?? null,
    draftedBy: gate.draftedBy,
    attemptLog: gate.attemptLog.map((attempt) => ({
      at: attempt.at,
      answer: attempt.answer,
      pass: attempt.pass,
      note: attempt.note,
    })),
  };
}

function mapCheck(check: TaskDetail["checks"][number]): Check {
  return {
    id: check.id,
    label: check.label,
    detail: check.detail,
    command: check.command,
    result: check.result,
    evaluated: check.evaluatedAt ?? null,
  };
}

/// A run as the screens consume it. Only the runner writes these results; the
/// view renders them and never decides one.
function mapRun(run: Awaited<ReturnType<typeof commands.getRun>>): CheckRun {
  return {
    id: run.id,
    taskId: run.taskId,
    status: run.status,
    started: run.started,
    finished: run.finished ?? null,
    results: run.results.map((result) => ({
      checkId: result.checkId,
      label: result.label,
      result: result.result,
      exitCode: result.exitCode ?? null,
      durationMs: result.durationMs,
      stdoutTail: result.stdoutTail,
      stderrTail: result.stderrTail,
      truncated: result.truncated,
    })),
  };
}

function mapTask(task: TaskDetail): Task {
  return {
    id: task.id,
    projectId: task.projectId,
    title: task.title,
    lane: task.lane,
    phase: task.phase,
    started: task.started,
    finished: task.finished ?? null,
    minutes: task.minutes,
    batch: task.batch,
    scope: task.scope,
    prediction: task.prediction,
    planApproved: task.planApproved,
    planRevised: task.planRevised,
    planItems: task.planItems.map((item) => ({
      id: item.id,
      parentId: item.parentId ?? null,
      kind: item.kind,
      title: item.title,
      body: item.body,
      position: item.position,
      status: item.status,
      source: item.source,
      checkId: item.checkId ?? null,
    })),
    planMessages: task.planMessages.map((message) => ({
      id: message.id,
      at: message.at,
      role: message.role,
      content: message.content,
      model: message.model,
    })),
    notes: task.notes.map((note) => ({ at: note.at, text: note.text })),
    attempts: task.attempts.map((attempt) => ({ at: attempt.at, text: attempt.text })),
    checks: task.checks.map(mapCheck),
    gates: task.gates.map(mapGate),
    conceptIds: task.conceptIds,
    reflected: task.reflected,
  };
}

function mapHandoff(handoff: NonNullable<ProjectDetail["handoff"]>): Handoff {
  return {
    at: handoff.at,
    next: handoff.next,
    blocking: handoff.blocking,
    thinking: handoff.thinking,
  };
}

function mapProject(detail: ProjectDetail): { project: Project; tasks: Task[] } {
  const tasks = detail.tasks.map(mapTask);
  return {
    project: {
      id: detail.id,
      name: detail.name,
      brief: firstLine(detail.brief),
      briefBody: detail.brief,
      repoPath: detail.repoPath ?? null,
      created: detail.created,
      currentTaskId: detail.currentTaskId ?? "",
      handoff: detail.handoff ? mapHandoff(detail.handoff) : null,
      recall: detail.recall ?? null,
      questions: detail.questions.map((question) => ({
        id: question.id,
        at: question.at,
        text: question.text,
        answer: question.answer ?? null,
      })),
      notes: detail.notes.map((note) => ({
        id: note.id,
        at: note.at,
        title: note.title,
        text: note.text,
      })),
      revision: detail.revision,
    },
    tasks,
  };
}

function mapConcept(concept: ConceptSummary): Concept {
  return {
    id: concept.id,
    name: concept.name,
    area: concept.area,
    definition: concept.definition,
    seen: concept.seen,
    help: concept.help,
    status: concept.status,
    lastReviewed: concept.lastReviewedAt ?? null,
    dueOn: concept.dueOn ?? null,
    taskIds: [],
  };
}

const coarseKinds: Record<string, EventKind> = {
  project_created: "decision",
  brief_edited: "decision",
  note_added: "note",
  note_deleted: "note",
  task_planned: "decision",
  task_switched: "decision",
  task_deleted: "decision",
  prediction_written: "note",
  plan_approved: "decision",
  plan_item_added: "note",
  plan_item_updated: "note",
  plan_item_moved: "note",
  plan_item_completed: "note",
  plan_item_removed: "note",
  plan_message_added: "note",
  attempt_recorded: "note",
  implement_closed: "scope",
  gate_drafted: "gate",
  gate_attempted: "gate",
  gate_overridden: "gate",
  hint_used: "gate",
  check_added: "check",
  check_removed: "check",
  check_run: "check",
  check_result: "check",
  drill_added: "check",
  drill_run: "check",
  handoff_written: "handoff",
  recall_stored: "recall",
  session_opened: "session",
  session_closed: "session",
  decision_recorded: "decision",
  question_recorded: "decision",
  question_answered: "decision",
  read_scope_changed: "scope",
};

function mapEvent(event: WireEvent): AuditEvent {
  return {
    id: event.id,
    at: event.at,
    kind: coarseKinds[event.kind] ?? "note",
    text: event.summary,
  };
}

async function refreshEvents(): Promise<void> {
  try {
    const events = await commands.listEvents(null, 100);
    state.events = events.map(mapEvent);
  } catch (error) {
    console.error("events refresh failed", error);
  }
}

async function applyProject(detail: ProjectDetail): Promise<Project> {
  const { project, tasks } = mapProject(detail);
  state.projects = [project, ...state.projects.filter((entry) => entry.id !== project.id)].sort(
    (a, b) => a.name.localeCompare(b.name),
  );
  state.tasks = [...state.tasks.filter((task) => task.projectId !== project.id), ...tasks];
  // Decisions are append-only and project-scoped; the stored ones replace the
  // cached rows for this project, and nothing else is touched.
  state.decisions = [
    ...detail.decisions.map((decision) => ({
      id: decision.id,
      at: decision.at,
      title: decision.title,
      why: decision.why,
      cost: decision.cost,
      taskId: decision.taskId ?? null,
      projectId: decision.projectId,
    })),
    ...state.decisions.filter((decision) => decision.projectId !== detail.id),
  ];
  await refreshEvents();
  notify();
  return project;
}

async function refreshProject(projectId: string): Promise<Project> {
  return applyProject(await commands.getProject(projectId));
}

/// Applies one task mutation without refetching the project. Events are
/// refreshed because the log is the source of the screen's history.
async function commitTaskDetail(detail: TaskDetail, note?: string): Promise<Task> {
  const task = mapTask(detail);
  state.tasks = state.tasks.map((entry) => (entry.id === task.id ? task : entry));
  if (note !== undefined) ui.gateNote = note;
  await refreshEvents();
  notify();
  return task;
}

/// Load the shell from SQLite. In a browser (no Tauri) the synthetic fixture is
/// used so the design can still be reviewed.
export async function hydrate(): Promise<void> {
  reviewCache = null;
  reviewError = null;
  if (!tauri) {
    state = readLocal();
    notify();
    return;
  }
  try {
    const boot: Bootstrap = await commands.bootstrap();
    const projects: Project[] = [];
    const tasks: Task[] = [];
    for (const summary of boot.projects) {
      const detail = await commands.getProject(summary.id);
      const mapped = mapProject(detail);
      projects.push(mapped.project);
      tasks.push(...mapped.tasks);
    }
    state = {
      projects,
      tasks,
      concepts: boot.concepts.map(mapConcept),
      decisions: [],
      events: boot.events.map(mapEvent),
      drills: boot.drills.map((drill) => ({
        id: drill.id,
        skill: drill.skill,
        name: drill.name,
        command: drill.command,
        last: drill.lastRunAt ?? null,
        attempts: drill.attempts,
        passed: drill.passed,
      })),
      settings: {
        model: boot.settings.model,
        allowlist: boot.settings.allowlist,
        writeAccess: false,
        modelUrl: boot.settings.modelUrl,
        modelName: boot.settings.modelName,
      },
      session: {
        started: boot.session.startedAt ?? null,
        projectId: boot.session.projectId ?? null,
        recall: null,
      },
    };
    notify();
  } catch (error) {
    console.error("bootstrap failed", error);
    state = emptyState();
    notify();
  }
}

export function errorMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return error instanceof Error ? error.message : "Something went wrong.";
}

function requireTauri(): void {
  if (!tauri) throw new Error("The backend is only available in the desktop app.");
}

export async function createProject(
  name: string,
  brief: string,
): Promise<{ project: Project; task: Task }> {
  requireTauri();
  const detail = await commands.createProject({ name, brief });
  const project = await applyProject(detail);
  const task = state.tasks.find((entry) => entry.projectId === project.id);
  if (!task) throw new Error("The new project has no first task.");
  return { project, task };
}

export async function saveBrief(
  projectId: string,
  body: string,
  expectedRevision: number | null,
): Promise<Project> {
  requireTauri();
  return applyProject(await commands.updateBrief(projectId, body, expectedRevision));
}

/// Attaches the repository a project's checks run in and the model reads from.
/// An empty path clears it and returns the project to app data.
export async function setProjectPath(
  projectId: string,
  path: string | null,
): Promise<void> {
  if (!tauri) {
    const project = findProject(projectId);
    if (project) project.repoPath = path && path.trim() ? path.trim() : null;
    ui.settingsNote = "Project path saved in this browser fixture.";
    ui.settingsNoteOk = true;
    logEvent("scope", path ? `project path set · ${path}` : "project path cleared");
    commit();
    return;
  }
  await applyProject(await commands.setProjectPath(projectId, path));
}

/// Switching the current task moves only the project's pointer. The task that
/// was current keeps its phase, notes, attempts and gates, so switching back
/// resumes it instead of starting over.
export async function setCurrentTask(projectId: string, taskId: string): Promise<void> {
  if (!tauri) {
    const project = findProject(projectId);
    if (!project) throw new Error("No such project.");
    project.currentTaskId = taskId;
    logEvent("decision", `current task switched · ${findTask(taskId)?.title ?? taskId}`);
    commit();
    return;
  }
  await applyProject(await commands.setCurrentTask(projectId, taskId));
}

export async function addProjectNote(
  projectId: string,
  title: string,
  text: string,
): Promise<Project> {
  requireTauri();
  return applyProject(await commands.addNote(projectId, title, text));
}

export async function deleteProjectNote(projectId: string, noteId: string): Promise<Project> {
  requireTauri();
  return applyProject(await commands.deleteNote(projectId, noteId));
}

export async function createTask(
  projectId: string,
  title: string,
  scope: string,
  prediction: string,
): Promise<Task> {
  if (!tauri) {
    const project = findProject(projectId);
    if (!project) throw new Error("No such project.");
    const count = tasksOf(projectId).length;
    const task: Task = {
      id: `t${Date.now().toString(36)}`,
      projectId,
      title: title.trim(),
      lane: "learn",
      phase: "plan",
      started: new Date().toISOString(),
      finished: null,
      minutes: 0,
      batch: `${count + 1} of ${count + 1}`,
      scope: scope.trim(),
      prediction: prediction.trim(),
      planApproved: false,
      planRevised: false,
      planItems: [],
      planMessages: [],
      notes: [],
      attempts: [],
      checks: [],
      gates: [],
      conceptIds: [],
      reflected: false,
    };
    state.tasks = [...state.tasks, task];
    project.currentTaskId = task.id;
    logEvent("decision", `task planned · ${task.title}`);
    commit();
    return task;
  }
  const task = await commands.createTask(projectId, { title, scope, prediction });
  await refreshProject(projectId);
  return mapTask(task);
}

export async function deleteTask(taskId: string): Promise<void> {
  const task = findTask(taskId);
  if (!task) throw new Error("No such task.");
  const project = findProject(task.projectId);
  if (!project) throw new Error("No such project.");
  if (!tauri) {
    state.tasks = state.tasks.filter((entry) => entry.id !== taskId);
    if (project.currentTaskId === taskId) {
      const next = tasksOf(project.id).sort(
        (a, b) => new Date(b.started).getTime() - new Date(a.started).getTime(),
      )[0];
      project.currentTaskId = next?.id ?? "";
    }
    logEvent("decision", `task deleted · ${task.title}`);
    commit();
    return;
  }
  await applyProject(await commands.deleteTask(taskId));
}

export async function writePrediction(taskId: string, text: string): Promise<void> {
  const task = findTask(taskId);
  if (!task) throw new Error("No such task.");
  if (!tauri) {
    const trimmed = text.trim();
    if (!trimmed) return;
    task.prediction = trimmed;
    logEvent("note", "prediction written in the browser fixture");
    commit();
    return;
  }
  await commands.writePrediction(taskId, text);
  await refreshProject(task.projectId);
}

// ---------------------------------------------------------------------------
// Plan document
// ---------------------------------------------------------------------------

function nextPlanPosition(task: Task, kind: PlanItemKind, parentId: string | null): number {
  return (
    task.planItems
      .filter((item) => item.kind === kind && (item.parentId ?? null) === parentId)
      .reduce((highest, item) => Math.max(highest, item.position), 0) + 1
  );
}

function planOwner(itemId: string): { task: Task; item: PlanItem } | null {
  for (const task of state.tasks) {
    const item = task.planItems.find((entry) => entry.id === itemId);
    if (item) return { task, item };
  }
  return null;
}

export async function addPlanItem(
  taskId: string,
  input: {
    kind: PlanItemKind;
    parentId: string | null;
    title: string;
    body: string;
    checkId: string | null;
  },
): Promise<void> {
  const task = findTask(taskId);
  if (!task) throw new Error("No such task.");
  if (!tauri) {
    const title = input.title.trim();
    if (!title) return;
    task.planItems.push({
      id: `pi-${Math.random().toString(36).slice(2, 8)}`,
      parentId: input.parentId,
      kind: input.kind,
      title,
      body: input.body.trim(),
      position: nextPlanPosition(task, input.kind, input.parentId),
      status: "open",
      source: "user",
      checkId: input.checkId,
    });
    ui.planEditing = null;
    ui.planNote = null;
    logEvent("note", `plan item added · ${title}`);
    commit();
    return;
  }
  await commitTaskDetail(await commands.addPlanItem(taskId, input));
}

export async function updatePlanItem(
  itemId: string,
  title: string,
  body: string,
): Promise<void> {
  const found = planOwner(itemId);
  if (!found) throw new Error("No such plan item.");
  if (!tauri) {
    const trimmed = title.trim();
    if (!trimmed) return;
    found.item.title = trimmed;
    found.item.body = body.trim();
    ui.planEditing = null;
    ui.planNote = null;
    logEvent("note", `plan item edited · ${trimmed}`);
    commit();
    return;
  }
  await commitTaskDetail(await commands.updatePlanItem(itemId, title, body));
}

export async function togglePlanItem(itemId: string, done: boolean): Promise<void> {
  const found = planOwner(itemId);
  if (!found) throw new Error("No such plan item.");
  if (!tauri) {
    found.item.status = done ? "done" : "open";
    logEvent("note", `${done ? "completed" : "reopened"} plan item · ${found.item.title}`);
    commit();
    return;
  }
  await commitTaskDetail(await commands.setPlanItemStatus(itemId, done));
}

export async function movePlanItem(itemId: string, direction: number): Promise<void> {
  const found = planOwner(itemId);
  if (!found) throw new Error("No such plan item.");
  if (!tauri) {
    const siblings = found.task.planItems
      .filter(
        (item) =>
          item.kind === found.item.kind &&
          (item.parentId ?? null) === (found.item.parentId ?? null),
      )
      .sort((a, b) => a.position - b.position);
    const index = siblings.findIndex((item) => item.id === itemId);
    const target = index + direction;
    if (index < 0 || target < 0 || target >= siblings.length) return;
    [siblings[index], siblings[target]] = [siblings[target], siblings[index]];
    siblings.forEach((item, position) => {
      item.position = position + 1;
    });
    logEvent("note", `plan item reordered · ${found.item.title}`);
    commit();
    return;
  }
  await commitTaskDetail(await commands.movePlanItem(itemId, direction));
}

export async function deletePlanItem(itemId: string): Promise<void> {
  const found = planOwner(itemId);
  if (!found) throw new Error("No such plan item.");
  if (!tauri) {
    const removed = new Set([itemId]);
    let changed = true;
    while (changed) {
      changed = false;
      for (const item of found.task.planItems) {
        if (item.parentId && removed.has(item.parentId) && !removed.has(item.id)) {
          removed.add(item.id);
          changed = true;
        }
      }
    }
    found.task.planItems = found.task.planItems.filter((item) => !removed.has(item.id));
    ui.planEditing = null;
    logEvent("note", `plan item removed · ${found.item.title}`);
    commit();
    return;
  }
  await commitTaskDetail(await commands.deletePlanItem(itemId));
}

export async function sendPlanMessage(taskId: string, text: string): Promise<void> {
  const task = findTask(taskId);
  if (!task) throw new Error("No such task.");
  const trimmed = text.trim();
  if (!trimmed) return;
  if (!tauri) {
    const at = new Date().toISOString();
    task.planMessages.push({
      id: `pm-${Math.random().toString(36).slice(2, 8)}`,
      at,
      role: "user",
      content: trimmed,
      model: "",
    });
    task.planMessages.push({
      id: `pm-${Math.random().toString(36).slice(2, 8)}`,
      at,
      role: "assistant",
      content:
        "This browser fixture has no model lane. In the desktop app this turn is sent to the local model, and its reply is stored in the same discussion.",
      model: "fixture",
    });
    ui.planNote = "No model was reached; the fixture wrote a placeholder reply.";
    logEvent("note", "plan discussion · your turn");
    commit();
    return;
  }
  ui.planChatPending = true;
  ui.planNote = null;
  notify();
  try {
    const reply = await commands.sendPlanMessage(taskId, trimmed);
    await commitTaskDetail(reply.task);
    ui.planNote = reply.note;
  } catch (error) {
    ui.planNote = errorMessage(error);
    throw error;
  } finally {
    ui.planChatPending = false;
    notify();
  }
}

export async function suggestPlan(taskId: string): Promise<void> {
  if (!tauri) {
    ui.planProposal = null;
    ui.planProposalNote = "The browser fixture has no model lane. Write the plan by hand.";
    commit();
    return;
  }
  ui.planProposal = null;
  ui.planProposalNote = "Asking the local model for a proposal...";
  notify();
  try {
    ui.planProposal = await commands.suggestPlan(taskId);
    ui.planProposalNote = null;
  } catch (error) {
    ui.planProposalNote = errorMessage(error);
  }
  notify();
}

export async function acceptPlanProposal(
  taskId: string,
  proposal: PlanProposal,
): Promise<void> {
  const task = findTask(taskId);
  if (!task) throw new Error("No such task.");
  if (!tauri) {
    let rootPosition = nextPlanPosition(task, "section", null);
    for (const section of proposal.sections) {
      const sectionId = `pi-${Math.random().toString(36).slice(2, 8)}`;
      task.planItems.push({
        id: sectionId,
        parentId: null,
        kind: "section",
        title: section.title,
        body: "",
        position: rootPosition++,
        status: "open",
        source: "model",
        checkId: null,
      });
      section.subtasks.forEach((subtask, index) => {
        task.planItems.push({
          id: `pi-${Math.random().toString(36).slice(2, 8)}`,
          parentId: sectionId,
          kind: "subtask",
          title: subtask.title,
          body: subtask.body,
          position: index + 1,
          status: "open",
          source: "model",
          checkId: null,
        });
      });
    }
    let dodPosition = nextPlanPosition(task, "dod", null);
    for (const dod of proposal.dods) {
      task.planItems.push({
        id: `pi-${Math.random().toString(36).slice(2, 8)}`,
        parentId: null,
        kind: "dod",
        title: dod.title,
        body: dod.body,
        position: dodPosition++,
        status: "open",
        source: "model",
        checkId: null,
      });
    }
    ui.planProposal = null;
    ui.planProposalNote = null;
    ui.planNote = "The suggested plan was added. Edit it like any other plan.";
    logEvent("note", `plan proposal accepted · ${proposal.sections.length} sections`);
    commit();
    return;
  }
  const detail = await commands.acceptPlanProposal(taskId, {
    sections: proposal.sections,
    dods: proposal.dods,
  });
  await commitTaskDetail(detail);
  ui.planProposal = null;
  ui.planProposalNote = null;
  ui.planNote = "The suggested plan was added. Edit it like any other plan.";
  notify();
}

export function dismissPlanProposal(): void {
  ui.planProposal = null;
  ui.planProposalNote = null;
  commit();
}

export async function approvePlan(taskId: string): Promise<void> {
  const task = findTask(taskId);
  if (!task) throw new Error("No such task.");
  if (!tauri) {
    if (!task.prediction.trim()) throw new Error("Write the approach first.");
    if (!task.planItems.some((item) => item.kind === "dod")) {
      throw new Error("Add at least one definition of done item first.");
    }
    task.planApproved = true;
    task.phase = "implement";
    logEvent("decision", `plan approved · ${task.title}`);
    commit();
    return;
  }
  await commands.approvePlan(taskId);
  await refreshProject(task.projectId);
}

export async function addTaskNote(taskId: string, text: string): Promise<void> {
  const task = findTask(taskId);
  if (!task) throw new Error("No such task.");
  if (!tauri) {
    const trimmed = text.trim();
    if (!trimmed) return;
    task.notes.unshift({ at: new Date().toISOString(), text: trimmed });
    logEvent("note", `note added · ${trimmed.slice(0, 80)}`);
    commit();
    return;
  }
  await commands.addTaskNote(taskId, text);
  await refreshProject(task.projectId);
}

export async function recordAttempt(taskId: string, text: string): Promise<void> {
  requireTauri();
  const task = findTask(taskId);
  if (!task) throw new Error("No such task.");
  await commands.recordAttempt(taskId, text);
  await refreshProject(task.projectId);
}

export async function closeImplement(taskId: string): Promise<void> {
  requireTauri();
  const task = findTask(taskId);
  if (!task) throw new Error("No such task.");
  await commands.closeImplement(taskId);
  await refreshProject(task.projectId);
}

export async function writeHandoff(
  taskId: string,
  input: { next: string; blocking: string; thinking: string },
): Promise<void> {
  requireTauri();
  const task = findTask(taskId);
  if (!task) throw new Error("No such task.");
  const at = new Date().toISOString();
  await commands.writeHandoff(taskId, input);
  state.session = { started: null, projectId: null, recall: null };
  await refreshProject(task.projectId);
  // The backend withholds the handoff from the next session until recall. The
  // person who just wrote it still gets the confirmation on this screen.
  const project = findProject(task.projectId);
  if (project) project.handoff = { at, ...input };
  notify();
}

export async function startSession(projectId: string): Promise<void> {
  requireTauri();
  const session = await commands.startSession(projectId);
  state.session = {
    started: session.startedAt,
    projectId: session.projectId,
    recall: state.session.projectId === projectId ? state.session.recall : null,
  };
  notify();
}

export async function submitRecall(projectId: string, text: string): Promise<void> {
  requireTauri();
  await commands.startSession(projectId);
  const outcome = await commands.submitRecall(projectId, text);
  state.session = {
    started: state.session.started ?? new Date().toISOString(),
    projectId,
    recall: text,
  };
  await refreshProject(projectId);
  if (outcome.handoff) {
    const project = findProject(projectId);
    if (project) project.handoff = mapHandoff(outcome.handoff);
  }
  notify();
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

function applySettings(settings: WireSettings): void {
  state.settings = {
    model: settings.model,
    allowlist: [...settings.allowlist],
    writeAccess: false,
    modelUrl: settings.modelUrl,
    modelName: settings.modelName,
  };
}

export async function saveModel(model: "local" | "api" | "none"): Promise<void> {
  if (!tauri) {
    state.settings.model = model;
    ui.settingsNote = `Model set to ${model} in this browser fixture.`;
    ui.settingsNoteOk = true;
    logEvent("decision", `model set to ${model}`);
    commit();
    return;
  }
  applySettings(await commands.setModel(model));
  ui.settingsNote = `Model set to ${model}. The choice is in SQLite and in the log.`;
  ui.settingsNoteOk = true;
  ui.modelHealth = null;
  notify();
}

export async function saveModelEndpoint(baseUrl: string, modelName: string): Promise<void> {
  if (!tauri) {
    state.settings.modelUrl = baseUrl.trim();
    state.settings.modelName = modelName.trim();
    ui.settingsNote = "Endpoint saved in this browser fixture.";
    ui.settingsNoteOk = true;
    ui.modelHealth = null;
    commit();
    return;
  }
  applySettings(await commands.setModelEndpoint(baseUrl.trim(), modelName.trim()));
  ui.settingsNote = "Local model saved. It applies to the next request, not the next launch.";
  ui.settingsNoteOk = true;
  ui.modelHealth = null;
  notify();
}

/// Health is a report, not a failure: "off" and "unreachable" render the same
/// way the backend describes them.
export async function checkModelHealth(): Promise<void> {
  if (!tauri) {
    ui.modelHealth = {
      configured: state.settings.model !== "none",
      reachable: false,
      baseUrl: state.settings.modelUrl ?? "http://127.0.0.1:11434",
      model: state.settings.modelName ?? "qwen3.5:9b",
      detail: "This browser fixture reaches no model. The desktop app asks Ollama on this machine.",
    };
    commit();
    return;
  }
  ui.modelHealth = await commands.modelHealth();
  notify();
}

export async function askCritique(taskId: string): Promise<void> {
  ui.critiqueNote = null;
  if (!tauri) {
    ui.critique = null;
    ui.critiqueFiles = null;
    ui.critiqueNote = "Nothing was sent: the browser fixture has no model lane.";
    commit();
    return;
  }
  try {
    const critique = await commands.critiquePlan(taskId);
    ui.critique = critique.points;
    ui.critiqueFiles = critique.files;
  } catch (error) {
    ui.critique = null;
    ui.critiqueFiles = null;
    ui.critiqueNote = errorMessage(error);
  }
  notify();
}

export async function addAllowlistPath(path: string): Promise<void> {
  const trimmed = path.trim();
  if (!trimmed) return;
  if (!tauri) {
    if (!state.settings.allowlist.includes(trimmed)) {
      state.settings.allowlist.push(trimmed);
    }
    ui.settingsNote = "Read scope widened in this browser fixture.";
    ui.settingsNoteOk = true;
    logEvent("scope", `read scope widened · ${trimmed}`);
    commit();
    return;
  }
  applySettings(await commands.addAllowlistPath(trimmed));
  ui.settingsNote = "Read scope saved. Those are the only paths the model may read.";
  ui.settingsNoteOk = true;
  notify();
}

export async function removeAllowlistPath(path: string): Promise<void> {
  if (!tauri) {
    state.settings.allowlist = state.settings.allowlist.filter((entry) => entry !== path);
    ui.settingsNote = "Read scope narrowed in this browser fixture.";
    ui.settingsNoteOk = true;
    logEvent("scope", `read scope narrowed · ${path}`);
    commit();
    return;
  }
  applySettings(await commands.removeAllowlistPath(path));
  ui.settingsNote = "Read scope saved.";
  ui.settingsNoteOk = true;
  notify();
}

export function isBackend(): boolean {
  return tauri;
}

// ---------------------------------------------------------------------------
// Gates
// ---------------------------------------------------------------------------

/// Drafts a gate from the task. The question and the rubric are written by the
/// app, never by the model, so the scoring rule exists before the answer does.
export async function draftGate(taskId: string): Promise<Gate> {
  const task = findTask(taskId);
  if (!task) throw new Error("No such task.");

  if (!tauri) {
    const due = task.conceptIds
      .map((id) => conceptOf(id))
      .filter((concept): concept is Concept => concept !== undefined)
      .find(
        (concept) =>
          concept.status === "due" ||
          concept.status === "weak" ||
          (concept.dueOn !== null && new Date(concept.dueOn).getTime() <= Date.now()),
      );
    const n = task.gates.reduce((highest, gate) => Math.max(highest, gate.n), 0) + 1;
    const gate: Gate = {
      id: `g-${Math.random().toString(36).slice(2, 8)}`,
      conceptId: due?.id ?? null,
      n,
      question: due
        ? `Explain "${due.name}" as it applies to this task: what is it, and what goes wrong without it?`
        : `What does "${task.title}" change, and what would break if it were done the naive way?`,
      hint: due
        ? `Start from the failure "${due.name}" prevents${due.area ? ` in ${due.area}` : ""}.`
        : task.scope
          ? `Start from \`${task.scope}\`: what does it control, and what fails without it?`
          : "Name the decision you made and the failure it prevents.",
      status: "due",
      attempts: 0,
      hintUsed: false,
      answer: "",
      reason: "",
      evaluated: null,
      attemptLog: [],
    };
    task.gates.push(gate);
    ui.gateNote = "Gate drafted. In the demo fixture the app writes the rubric.";
    logEvent("gate", `gate drafted · ${task.title}`);
    commit();
    return gate;
  }

  const detail = await commands.draftGate(taskId);
  const mapped = mapTask(detail);
  const gate = mapped.gates[mapped.gates.length - 1];
  await commitTaskDetail(detail, "Gate drafted from this task. The rubric is stored with it.");
  return gate;
}

export async function answerGate(taskId: string, gateId: string, answer: string): Promise<Gate> {
  const task = findTask(taskId);
  if (!task) throw new Error("No such task.");
  const gate = task.gates.find((entry) => entry.id === gateId);
  if (!gate) throw new Error("No such gate.");

  if (!tauri) {
    const subject = gate.conceptId
      ? (conceptOf(gate.conceptId)?.name ?? task.title)
      : task.title;
    const result = gate.id.startsWith("g-")
      ? evaluateDraftAnswer(subject, answer)
      : evaluateAnswer(answer);
    const now = new Date().toISOString();
    gate.answer = answer;
    gate.attempts += 1;
    gate.attemptLog.unshift({ at: now, answer, pass: result.pass, note: result.note });
    gate.status = result.pass ? "passed" : "failed";
    gate.evaluated = now;
    if (result.pass && gate.conceptId) {
      const concept = conceptOf(gate.conceptId);
      if (concept) {
        concept.seen += 1;
        concept.status = "passed";
        concept.lastReviewed = now;
        concept.dueOn = new Date(Date.now() + 21 * 86_400_000).toISOString();
      }
    }
    ui.gateNote = result.note;
    logEvent(
      "gate",
      result.pass
        ? `gate 0${gate.n} passed · ${task.title}`
        : `gate 0${gate.n} not met · ${result.note}`,
    );
    commit();
    return gate;
  }

  const detail = await commands.answerGate(gateId, answer);
  const mapped = mapTask(detail);
  const scored = mapped.gates.find((entry) => entry.id === gateId);
  if (!scored) throw new Error("The gate disappeared while it was scored.");
  const note =
    scored.attemptLog[0]?.note ??
    (scored.status === "passed" ? "Met." : "Not met. The answer misses the rubric.");
  await commitTaskDetail(detail, note);
  return scored;
}

export async function useHint(taskId: string, gateId: string): Promise<void> {
  const task = findTask(taskId);
  if (!task) throw new Error("No such task.");
  if (!tauri) {
    const gate = task.gates.find((entry) => entry.id === gateId);
    if (gate) gate.hintUsed = true;
    logEvent("gate", `hint used · ${task.title}`);
    ui.gateNote = "Hint shown. Using it is recorded on the gate.";
    commit();
    return;
  }
  const detail = await commands.useHint(gateId);
  await commitTaskDetail(detail, "Hint shown. Using it is recorded on the gate.");
}

export async function overrideGate(
  taskId: string,
  gateId: string,
  reason: string,
): Promise<void> {
  const task = findTask(taskId);
  if (!task) throw new Error("No such task.");
  if (!tauri) {
    if (reason.length < 12) {
      throw new Error("A reason needs at least a sentence. What would this gate have caught?");
    }
    const gate = task.gates.find((entry) => entry.id === gateId);
    if (!gate) throw new Error("No such gate.");
    gate.status = "overridden";
    gate.reason = reason;
    gate.evaluated = new Date().toISOString();
    ui.gateNote = "Overridden. The bypass and the reason are in the log.";
    logEvent("gate", `gate 0${gate.n} overridden · ${reason}`);
    commit();
    return;
  }
  const detail = await commands.overrideGate(gateId, reason);
  await commitTaskDetail(
    detail,
    "Overridden. The bypass and the reason are in the log.",
  );
}

// ---------------------------------------------------------------------------
// Concepts
// ---------------------------------------------------------------------------

function replaceConcept(summary: ConceptSummary): Concept {
  const concept = mapConcept(summary);
  state.concepts = state.concepts
    .map((entry) => (entry.id === concept.id ? concept : entry))
    .sort((a, b) => a.name.localeCompare(b.name));
  return concept;
}

export async function createConcept(input: {
  id: string;
  name: string;
  area: string;
  definition: string;
}): Promise<Concept> {
  if (!tauri) {
    const concept: Concept = {
      id: input.id.trim(),
      name: input.name.trim(),
      area: input.area.trim(),
      definition: input.definition.trim(),
      seen: 0,
      help: "full",
      status: "due",
      lastReviewed: null,
      dueOn: null,
      taskIds: [],
    };
    state.concepts.push(concept);
    logEvent("note", `concept added · ${concept.name} · ${concept.id}`);
    ui.conceptNote = `Added ${concept.id}. It is due until it is explained back.`;
    commit();
    return concept;
  }
  const concept = replaceConcept(await commands.createConcept(input));
  ui.conceptNote = `Added ${concept.id}. It is due until it is explained back.`;
  notify();
  return concept;
}

/// The app judges an explanation pass or thin and sets the next due date. The
/// returned note says which, and why.
export async function reviewConcept(
  conceptId: string,
  answer: string,
): Promise<{ passed: boolean; note: string }> {
  const concept = conceptOf(conceptId);
  if (!concept) throw new Error("No such concept.");

  if (!tauri) {
    const thin = answer.trim().length < 40;
    concept.seen += 1;
    concept.lastReviewed = new Date().toISOString();
    if (thin) {
      concept.status = "weak";
      concept.dueOn = new Date(Date.now() + 86_400_000).toISOString();
      ui.conceptNote = "Recorded, but thin. It stays due: an explanation needs a cause and a consequence.";
      logEvent("note", `concept review thin · ${concept.name}`);
      commit();
      return { passed: false, note: ui.conceptNote };
    }
    concept.status = "passed";
    concept.dueOn = new Date(Date.now() + 21 * 86_400_000).toISOString();
    ui.conceptNote = "Recorded. Next review is three weeks out.";
    logEvent("note", `concept explained back · ${concept.name} · ${concept.id}`);
    commit();
    return { passed: true, note: ui.conceptNote };
  }

  const outcome = await commands.reviewConcept(conceptId, answer);
  replaceConcept(outcome.concept);
  ui.conceptNote = outcome.note;
  notify();
  return { passed: outcome.passed, note: outcome.note };
}

export async function setConceptHelp(
  conceptId: string,
  help: "full" | "hint" | "none",
): Promise<void> {
  if (!tauri) {
    const concept = conceptOf(conceptId);
    if (!concept) return;
    concept.help = help;
    logEvent("note", `help level ${help} · ${concept.name}`);
    commit();
    return;
  }
  replaceConcept(await commands.setHelp(conceptId, help));
  notify();
}

export async function attachConcept(taskId: string, conceptId: string): Promise<void> {
  const task = findTask(taskId);
  if (!task) throw new Error("No such task.");
  if (!tauri) {
    if (!task.conceptIds.includes(conceptId)) task.conceptIds.push(conceptId);
    const concept = conceptOf(conceptId);
    logEvent("note", `concept attached · ${concept?.name ?? conceptId}`);
    commit();
    return;
  }
  const detail = await commands.attachConcept(taskId, conceptId);
  await commitTaskDetail(detail);
}

export async function detachConcept(taskId: string, conceptId: string): Promise<void> {
  const task = findTask(taskId);
  if (!task) throw new Error("No such task.");
  if (!tauri) {
    task.conceptIds = task.conceptIds.filter((id) => id !== conceptId);
    commit();
    return;
  }
  const detail = await commands.detachConcept(taskId, conceptId);
  await commitTaskDetail(detail);
}

/// Re-reads one task after a run so the check states come from storage rather
/// than from the view's optimism.
export async function refreshTask(taskId: string): Promise<void> {
  if (!tauri) return;
  await commitTaskDetail(await commands.getTask(taskId));
}

export async function addCheck(
  taskId: string,
  input: { label: string; detail: string; program: string; args: string },
): Promise<void> {
  requireTauri();
  await commitTaskDetail(await commands.addCheck(taskId, input));
}

export async function removeCheck(checkId: string): Promise<void> {
  requireTauri();
  await commitTaskDetail(await commands.removeCheck(checkId));
}

/// Starts a run and waits for it, polling `get_run`. The command returns as
/// soon as the run is reserved, so the window stays responsive while a suite
/// takes minutes.
const POLL_MS = 400;
const POLL_LIMIT = 1_800;

export async function runChecks(taskId: string): Promise<void> {
  requireTauri();
  if (ui.runningCheck) return;
  ui.runningCheck = taskId;
  ui.checkNote = null;
  commit();
  try {
    const runId = await commands.runChecks(taskId);
    ui.checkRunId = runId;
    for (let attempt = 0; attempt < POLL_LIMIT; attempt += 1) {
      const run = await commands.getRun(runId);
      ui.checkRun = mapRun(run);
      commit();
      if (run.status !== "running") {
        ui.checkNote =
          run.status === "cancelled"
            ? "The run was cancelled. No result was written."
            : run.status === "failed"
              ? "The run stopped before a verdict. Every check is unchanged."
              : null;
        await refreshTask(taskId);
        return;
      }
      await new Promise((resolve) => window.setTimeout(resolve, POLL_MS));
    }
    ui.checkNote = "Stopped waiting for the run. Its state is still in the log.";
  } finally {
    ui.runningCheck = null;
    ui.checkRunId = null;
    commit();
  }
}

export async function cancelChecks(): Promise<void> {
  if (!tauri || !ui.checkRunId) return;
  await commands.cancelRun(ui.checkRunId);
}

export async function addDecision(
  projectId: string,
  input: { title: string; why: string; cost: string; taskId: string | null },
): Promise<void> {
  requireTauri();
  await applyProject(await commands.addDecision(projectId, input));
}

export async function addQuestion(projectId: string, text: string): Promise<void> {
  requireTauri();
  await applyProject(await commands.addQuestion(projectId, { text }));
}

export async function answerQuestion(questionId: string, answer: string): Promise<void> {
  requireTauri();
  await applyProject(await commands.answerQuestion(questionId, answer));
}

export async function addDrill(input: {
  skill: string;
  name: string;
  command: string;
}): Promise<void> {
  requireTauri();
  const drill = await commands.addDrill(input);
  state.drills = [
    ...state.drills,
    {
      id: drill.id,
      skill: drill.skill,
      name: drill.name,
      command: drill.command,
      last: drill.lastRunAt ?? null,
      attempts: drill.attempts,
      passed: drill.passed,
    },
  ].sort((a, b) => a.skill.localeCompare(b.skill) || a.name.localeCompare(b.name));
  await refreshEvents();
  notify();
}

/// A drill's exit status is its verdict, and a failure stays in the record.
export async function runDrill(drillId: string): Promise<void> {
  requireTauri();
  const outcome = await commands.runDrill(drillId);
  state.drills = state.drills.map((drill) =>
    drill.id === drillId
      ? {
          id: outcome.drill.id,
          skill: outcome.drill.skill,
          name: outcome.drill.name,
          command: outcome.drill.command,
          last: outcome.drill.lastRunAt ?? null,
          attempts: outcome.drill.attempts,
          passed: outcome.drill.passed,
        }
      : drill,
  );
  ui.drillNote = outcome.passed
    ? `${outcome.drill.name} passed in ${outcome.durationMs} ms.`
    : `${outcome.drill.name} failed${outcome.exitCode === null ? "" : ` (exit ${outcome.exitCode})`}. The attempt stays in the record.`;
  await refreshEvents();
  notify();
}

export function cachedReview(): WireReview | null {
  return reviewCache;
}

export function reviewFailure(): string | null {
  return reviewError;
}

/// The six M2 figures. The desktop app reads the event log; the browser fixture
/// derives the same shape from the local records so the screen still renders.
export async function loadReview(days: number): Promise<void> {
  if (reviewCache?.days === days || reviewPending === days) return;
  reviewPending = days;
  reviewError = null;
  try {
    if (!tauri) {
      await Promise.resolve();
      reviewCache = localReview(days);
    } else {
      const result = await commands.review(days);
      if (reviewPending !== days) return;
      reviewCache = result;
    }
    notify();
  } catch (error) {
    reviewError = errorMessage(error);
    notify();
  } finally {
    if (reviewPending === days) reviewPending = null;
  }
}

function localReview(days: number): WireReview {
  const cutoff = Date.now() - days * 86_400_000;
  const tasks = state.tasks.filter(
    (task) => new Date(task.finished ?? task.started).getTime() >= cutoff,
  );
  const finished = tasks.filter((task) => task.finished);
  const lead = finished.length
    ? finished.reduce((sum, task) => {
        const ms = new Date(task.finished ?? task.started).getTime() - new Date(task.started).getTime();
        return sum + Math.max(0, ms);
      }, 0) /
      finished.length /
      86_400_000
    : null;
  const rework = tasks.reduce((sum, task) => sum + task.attempts.length, 0);
  const gates = tasks
    .flatMap((task) => task.gates)
    .filter((gate) => gate.status !== "due" && gate.status !== "not-yet");
  const passed = gates.filter((gate) => gate.status === "passed").length;
  const overrides = gates.filter((gate) => gate.status === "overridden").length;
  const failing = tasks.flatMap((task) => task.checks).filter((check) => check.result === "fail").length;
  const rate = gates.length ? Math.round((passed / gates.length) * 100) : null;

  const rows = tasks.map((task) => {
    const project = state.projects.find((entry) => entry.id === task.projectId);
    const gate =
      task.gates.find((entry) => entry.status === "due" || entry.status === "failed") ??
      task.gates.find((entry) => entry.status === "passed" || entry.status === "overridden");
    const daysOpen = task.finished
      ? Math.max(
          0,
          new Date(task.finished).getTime() - new Date(task.started).getTime(),
        ) / 86_400_000
      : null;
    return {
      taskId: task.id,
      projectId: task.projectId,
      project: project?.name ?? "",
      title: task.title,
      phase: task.phase,
      leadDays: daysOpen,
      attempts: task.attempts.length,
      gate: gate ? { n: gate.n, status: gate.status } : null,
      checksPassed: task.checks.filter(
        (check) => check.result === "pass" || check.result === "done",
      ).length,
      checksTotal: task.checks.length,
      closed: task.finished,
    };
  });

  return {
    days,
    from: new Date(cutoff).toISOString(),
    tasks: tasks.length,
    leadTimeDays: lead,
    rework,
    gatePassRate: rate,
    resolvedGates: gates.length,
    passedGates: passed,
    bypasses: overrides,
    failingChecks: failing,
    basis: {
      tasks: `local fixture · ${tasks.length} tasks in range`,
      leadTime: finished.length
        ? `mean started→finished · ${finished.length} finished`
        : "no handoff in range",
      rework: `recorded attempts · ${rework}`,
      gatePassRate: gates.length
        ? `${passed} passed / ${gates.length} resolved`
        : "no scored gate attempts in range",
      bypasses: `overrides · ${overrides}`,
      failingChecks: `current failing checks · ${failing}`,
    },
    rows,
  };
}

export async function exportStore(): Promise<string> {
  if (!tauri) return JSON.stringify(state, null, 2);
  return commands.exportStore();
}

export function relative(iso: string | null): string {
  if (!iso) return "never";
  const diff = Date.now() - new Date(iso).getTime();
  const minutes = Math.round(diff / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.round(hours / 24);
  return `${days}d ago`;
}

export function shortDate(iso: string | null): string {
  if (!iso) return "—";
  return new Date(iso).toLocaleDateString("en-GB", { day: "2-digit", month: "short" });
}

export function clock(iso: string | null): string {
  if (!iso) return "—";
  return new Date(iso).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });
}

export function secondsStamp(date = new Date()): string {
  return date.toLocaleTimeString("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function sessionLength(): string {
  if (!state.session.started) return "no session";
  const ms = Date.now() - new Date(state.session.started).getTime();
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  return h > 0 ? `${h}h ${String(m).padStart(2, "0")}m` : `${m}m`;
}
