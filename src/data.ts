export type Phase = "plan" | "implement" | "verify" | "reflect";
export type GateStatus = "due" | "passed" | "failed" | "overridden" | "not-yet";
export type CheckResult = "pass" | "fail" | "pending" | "done";
export type HelpLevel = "full" | "hint" | "none";
export type ConceptStatus = "passed" | "weak" | "due";
export type PlanItemKind = "section" | "subtask" | "dod";
export type PlanItemStatus = "open" | "done";
export type PlanSource = "user" | "model";
export type PlanMessageRole = "user" | "assistant";

export interface Check {
  id: string;
  label: string;
  detail: string;
  /** The exact command line the runner executes; empty when none is stored. */
  command: string;
  result: CheckResult;
  evaluated: string | null;
}

/** One process in a run, as the UI reports it. */
export interface RunResult {
  checkId: string;
  label: string;
  result: CheckResult;
  exitCode: number | null;
  durationMs: number;
  stdoutTail: string;
  stderrTail: string;
  truncated: boolean;
}

export interface CheckRun {
  id: string;
  taskId: string;
  status: "running" | "done" | "failed" | "cancelled";
  started: string;
  finished: string | null;
  results: RunResult[];
}

export interface GateAttempt {
  at: string;
  answer: string;
  pass: boolean;
  note: string;
}

export interface Gate {
  id: string;
  conceptId: string | null;
  n: number;
  question: string;
  hint: string;
  status: GateStatus;
  attempts: number;
  hintUsed: boolean;
  answer: string;
  reason: string;
  evaluated: string | null;
  attemptLog: GateAttempt[];
  /** Who phrased the question. The rubric is the app's either way. */
  draftedBy?: "app" | "model";
}

export interface Note {
  at: string;
  text: string;
}

export interface ProjectNote {
  id: string;
  at: string;
  title: string;
  text: string;
}

export interface PlanItem {
  id: string;
  parentId: string | null;
  kind: PlanItemKind;
  title: string;
  body: string;
  position: number;
  status: PlanItemStatus;
  source: PlanSource;
  checkId: string | null;
}

export interface PlanMessage {
  id: string;
  at: string;
  role: PlanMessageRole;
  content: string;
  model: string;
}

export interface PlanProposalItem {
  title: string;
  body: string;
}

export interface PlanProposalSection {
  title: string;
  subtasks: PlanProposalItem[];
}

export interface PlanProposal {
  model: string;
  sections: PlanProposalSection[];
  dods: PlanProposalItem[];
}

export interface Task {
  id: string;
  projectId: string;
  title: string;
  lane: "learn" | "verify";
  phase: Phase;
  started: string;
  finished: string | null;
  minutes: number;
  batch: string;
  scope: string;
  prediction: string;
  planApproved: boolean;
  planRevised: boolean;
  planItems: PlanItem[];
  planMessages: PlanMessage[];
  notes: Note[];
  attempts: Note[];
  checks: Check[];
  gates: Gate[];
  conceptIds: string[];
  reflected: boolean;
}

export interface Handoff {
  at: string;
  next: string;
  blocking: string;
  thinking: string;
}

export interface Question {
  id: string;
  at: string;
  text: string;
  answer: string | null;
}

export interface Project {
  id: string;
  name: string;
  brief: string;
  briefBody: string;
  /** Absolute path of the attached repository; null when none. */
  repoPath?: string | null;
  created: string;
  currentTaskId: string;
  handoff: Handoff | null;
  recall: string | null;
  questions: Question[];
  notes: ProjectNote[];
  revision?: number;
}

export interface Concept {
  id: string;
  name: string;
  area: string;
  definition: string;
  seen: number;
  help: HelpLevel;
  status: ConceptStatus;
  lastReviewed: string | null;
  dueOn: string | null;
  taskIds: string[];
}

export interface Decision {
  id: string;
  at: string;
  title: string;
  why: string;
  cost: string;
  taskId: string | null;
  /** Set on decisions read from storage; the demo fixture omits it. */
  projectId?: string;
}

export type EventKind =
  | "gate"
  | "check"
  | "handoff"
  | "decision"
  | "note"
  | "recall"
  | "session"
  | "scope";

export interface AuditEvent {
  id: string;
  at: string;
  kind: EventKind;
  text: string;
}

export interface Drill {
  id: string;
  skill: string;
  name: string;
  command: string;
  last: string | null;
  attempts: number;
  passed: boolean;
}

export interface Settings {
  model: "local" | "api" | "none";
  allowlist: string[];
  writeAccess: false;
  /** Loopback endpoint for the local model. */
  modelUrl?: string;
  modelName?: string;
}

/// What the Settings screen shows about the model lane.
export interface ModelHealth {
  configured: boolean;
  reachable: boolean;
  baseUrl: string;
  model: string;
  detail: string;
}

export interface Session {
  started: string | null;
  projectId: string | null;
  recall: string | null;
}

export interface AppState {
  projects: Project[];
  tasks: Task[];
  concepts: Concept[];
  decisions: Decision[];
  events: AuditEvent[];
  drills: Drill[];
  settings: Settings;
  session: Session;
}

const stamp = Date.now();
const ago = (minutes: number): string =>
  new Date(stamp - minutes * 60_000).toISOString();
const uid = (prefix: string): string =>
  `${prefix}-${Math.random().toString(36).slice(2, 8)}`;

const check = (
  id: string,
  label: string,
  detail: string,
  result: CheckResult,
  minutesAgo: number | null,
  command = "",
): Check => ({
  id,
  label,
  detail,
  command,
  result,
  evaluated: minutesAgo === null ? null : ago(minutesAgo),
});

const gate = (
  id: string,
  n: number,
  question: string,
  hint: string,
  status: GateStatus,
  minutesAgo: number | null,
): Gate => ({
  id,
  conceptId: null,
  n,
  question,
  hint,
  status,
  attempts: status === "passed" || status === "overridden" ? 1 : 0,
  hintUsed: false,
  answer: "",
  reason: "",
  evaluated: minutesAgo === null ? null : ago(minutesAgo),
  attemptLog: [],
});

export function seedState(): AppState {
  const projects: Project[] = [
    {
      id: "kestrel",
      name: "kestrel",
      brief: "A local event log for one developer's tools, with a replayable outbox.",
      briefBody:
        "kestrel records what a tool did, in the order it happened, so a failed run can be replayed instead of guessed at.\n\nThe first version is one machine, one SQLite file, and no network. The outbox exists because the log and the delivery of what it describes must never disagree: a row is written in the same transaction as the state change, and a separate flush delivers it.\n\nNon-goals for now: multi-user sync, a hosted collector, and any read of a repository outside the allowlist.",
      created: ago(60 * 24 * 18),
      currentTaskId: "t14",
      recall: null,
      handoff: {
        at: ago(60 * 19),
        next: "Pass gate 03, then close the task and write the handoff.",
        blocking:
          "The property test still fails when the timer fires twice inside one backoff window.",
        thinking:
          "The cap should be counted before the delay is computed. Five attempts is a request-shaped number, not a backoff-shaped one.",
      },
      questions: [
        {
          id: uid("q"),
          at: ago(60 * 26),
          text: "Does the outbox need a dead-letter table, or is a capped retry enough?",
          answer: null,
        },
        {
          id: uid("q"),
          at: ago(60 * 50),
          text: "What does jitter actually protect here — the downstream, or this machine's timer?",
          answer: null,
        },
        {
          id: uid("q"),
          at: ago(60 * 70),
          text: "Should the flush own its timer, or read the clock the log already keeps?",
          answer: "Read the log's clock. One source of time keeps replay honest.",
        },
      ],
      notes: [
        {
          id: uid("pn"),
          at: ago(95),
          title: "Timing note",
          text: "The p99 of 3.8s is from the staging box, not this laptop. Re-measure before defending five in a gate.\n\nIf the laptop is slower, five still holds because the cap is about the request, not the machine — but the decision should say which measurement it uses.",
        },
        {
          id: uid("pn"),
          at: ago(60 * 31),
          title: "",
          text: "Replay could print the log as a diff: one line per event, delivered state in the margin. Park it until the task after the outbox.",
        },
      ],
    },
    {
      id: "plumb",
      name: "plumb",
      brief: "Replay a captured trace locally and watch each decision the parser makes.",
      briefBody:
        "plumb takes a trace file and replays it against a parser, printing every branch it took and every branch it skipped.\n\nThe mock only carries the planning surface: a task that has not been approved, a prediction the developer owes, and a definition of done that still has holes.",
      created: ago(60 * 24 * 31),
      currentTaskId: "p08",
      recall: null,
      handoff: {
        at: ago(60 * 24 * 2),
        next: "Write the prediction before asking the model to critique the task.",
        blocking: "The parser's error positions are off by one line.",
        thinking: "A replay is a reading, not a run. The output should say where it looked, not only what it found.",
      },
      questions: [
        {
          id: uid("q"),
          at: ago(60 * 24 * 3),
          text: "Should a replay be able to write files, or only report?",
          answer: null,
        },
      ],
      notes: [
        {
          id: uid("pn"),
          at: ago(60 * 24 * 4),
          title: "",
          text: "The trace format has no line numbers. Positions will be reconstructed from byte offsets; check what the error message promises before choosing 0- or 1-based.",
        },
      ],
    },
    {
      id: "saltmarsh",
      name: "saltmarsh",
      brief: "Reading notes on distributed systems, kept as claims with sources.",
      briefBody:
        "saltmarsh is a reading project, not a codebase: each note is a claim, a source, and the sentence in the source that supports it.\n\nThe current task is a verify pass on quorum reads, where the gate has already been passed and the handoff is still open.",
      created: ago(60 * 24 * 52),
      currentTaskId: "s03",
      recall: "Quorum reads and the cost of a stale replica.",
      handoff: null,
      questions: [],
      notes: [
        {
          id: uid("pn"),
          at: ago(60 * 24 * 5),
          title: "Kleppmann, ch. 5",
          text: "A stale read is cheap until the read is a decision. The book stops at latency; the cost is what happens downstream of a wrong answer.",
        },
      ],
    },
    {
      id: "foundry",
      name: "foundry",
      brief: "A two-week drill on property-based testing.",
      briefBody:
        "foundry is the practice lane's first target: write generators that describe the fault, not the happy path.\n\nNo production code. Drills have executable checks, and a drill that fails is recorded, not deleted.",
      created: ago(60 * 24 * 61),
      currentTaskId: "f01",
      recall: null,
      handoff: null,
      questions: [],
      notes: [],
    },
  ];

  const tasks: Task[] = [
    {
      id: "t14",
      projectId: "kestrel",
      title: "Retry policy for outbox flush",
      lane: "learn",
      phase: "verify",
      started: ago(60 * 24 * 12),
      finished: null,
      minutes: 56,
      batch: "3 of 6",
      scope: "src/outbox/policy.ts, src/outbox/flush.ts, tests/outbox/**",
      prediction:
        "Four attempts should be enough; the fifth is there because the downstream's p99 is just under four seconds.",
      planApproved: true,
      planRevised: false,
      planItems: [],
      planMessages: [],
      notes: [
        { at: ago(52), text: "Moved the cap into policy.ts; the flush now reads it instead of computing it." },
        { at: ago(38), text: "The timer can fire twice inside one window. That is what the property test catches." },
      ],
      attempts: [
        { at: ago(64), text: "Counted the cap after the delay. Last attempt waited longest and failed." },
        { at: ago(41), text: "Counted the cap before the delay. One property case still fails." },
      ],
      checks: [
        check("ck1", "unit", "outbox.retry.spec · 24 cases", "pass", 41),
        check("ck2", "property", "attempts never exceed cap · 1,000 seeds", "fail", 18),
        check("ck3", "typecheck", "tsc --noEmit · project refs", "pass", 41),
        check("ck4", "hand-run", "flush under 503 for 90s", "done", 33),
      ],
      gates: [
        gate(
          "g3",
          3,
          "Why cap attempts at five instead of letting backoff run to exhaustion? Name the cost the cap protects.",
          "Think about the client that started the request and has already given up.",
          "due",
          null,
        ),
        gate(
          "g2",
          2,
          "Why is the cap applied before the delay is computed?",
          "The last clause of the sentence is the answer.",
          "passed",
          41,
        ),
      ],
      conceptIds: ["K-041", "K-018", "K-004", "K-052", "K-033", "K-037"],
      reflected: false,
    },
    {
      id: "t12",
      projectId: "kestrel",
      title: "Outbox table schema and indexes",
      lane: "learn",
      phase: "reflect",
      started: ago(60 * 24 * 19),
      finished: ago(60 * 24 * 7),
      minutes: 94,
      batch: "2 of 6",
      scope: "migrations/004_outbox.sql",
      prediction: "One index on (delivered_at, id) is enough for the flush.",
      planApproved: true,
      planRevised: false,
      planItems: [],
      planMessages: [],
      notes: [
        { at: ago(60 * 24 * 8), text: "Partial index on undelivered rows cut the flush scan from 41ms to 3ms." },
      ],
      attempts: [
        { at: ago(60 * 24 * 8), text: "Indexed delivered_at alone; the planner still scanned the table." },
      ],
      checks: [
        check("ck5", "unit", "migration round-trip", "pass", 60 * 24 * 7),
        check("ck6", "hand-run", "10k rows, flush under 20ms", "pass", 60 * 24 * 7),
      ],
      gates: [
        gate(
          "g1",
          1,
          "Why does the flush index lead with delivered_at rather than created_at?",
          "The query filters on one column first.",
          "passed",
          60 * 24 * 7,
        ),
      ],
      conceptIds: ["K-004", "K-033"],
      reflected: true,
    },
    {
      id: "t11",
      projectId: "kestrel",
      title: "Idempotency keys for delivery",
      lane: "learn",
      phase: "plan",
      started: ago(60 * 24 * 3),
      finished: null,
      minutes: 12,
      batch: "4 of 6",
      scope: "src/outbox/deliver.ts",
      prediction: "",
      planApproved: false,
      planRevised: false,
      planItems: [],
      planMessages: [],
      notes: [],
      attempts: [],
      checks: [
        check("ck7", "unit", "duplicate delivery writes once", "pending", null),
        check("ck8", "property", "same key, any order, one row", "pending", null),
      ],
      gates: [
        gate(
          "g4",
          4,
          "What makes a key safe to reuse across a retry?",
          "It is about what the key is derived from.",
          "not-yet",
          null,
        ),
      ],
      conceptIds: ["K-041", "K-011", "K-027"],
      reflected: false,
    },
    {
      id: "p08",
      projectId: "plumb",
      title: "Error positions in the trace parser",
      lane: "verify",
      phase: "plan",
      started: ago(60 * 24 * 2),
      finished: null,
      minutes: 8,
      batch: "1 of 3",
      scope: "src/parse/position.ts",
      prediction: "",
      planApproved: false,
      planRevised: false,
      planItems: [],
      planMessages: [],
      notes: [],
      attempts: [],
      checks: [
        check("ck9", "unit", "position is 1-based at the error", "pending", null),
      ],
      gates: [
        gate(
          "g1",
          1,
          "Where should the position point: the byte read, or the byte that broke the rule?",
          "Check what the error message promises.",
          "not-yet",
          null,
        ),
      ],
      conceptIds: ["K-037"],
      reflected: false,
    },
    {
      id: "s03",
      projectId: "saltmarsh",
      title: "Quorum reads: the stale replica cost",
      lane: "learn",
      phase: "verify",
      started: ago(60 * 24 * 16),
      finished: null,
      minutes: 44,
      batch: "2 of 4",
      scope: "notes/quorum-reads.md",
      prediction: "One stale read is cheaper than one extra round trip, until the read is a decision.",
      planApproved: true,
      planRevised: false,
      planItems: [],
      planMessages: [],
      notes: [{ at: ago(60 * 24 * 5), text: "Kleppmann's example does not cover a read that feeds a write." }],
      attempts: [],
      checks: [
        check("ck10", "source", "every claim has a page reference", "pass", 60 * 24 * 5),
        check("ck11", "cross-check", "one contradicting source named", "fail", 60 * 24 * 5),
      ],
      gates: [
        gate(
          "g2",
          2,
          "When does the extra round trip buy more than it costs?",
          "Name the read that cannot tolerate a stale answer.",
          "passed",
          60 * 24 * 5,
        ),
      ],
      conceptIds: ["K-011"],
      reflected: false,
    },
    {
      id: "f01",
      projectId: "foundry",
      title: "Generator for duplicate deliveries",
      lane: "learn",
      phase: "implement",
      started: ago(60 * 24 * 9),
      finished: null,
      minutes: 29,
      batch: "1 of 2",
      scope: "drills/gen_duplicates.ts",
      prediction: "Generating from the fault outward finds the case in under 200 samples.",
      planApproved: true,
      planRevised: false,
      planItems: [],
      planMessages: [],
      notes: [{ at: ago(60 * 24 * 9), text: "Shrinking is the hard part; the failure order is what matters." }],
      attempts: [],
      checks: [check("ck12", "drill", "shrinker reports a minimal case", "fail", 60 * 24 * 9)],
      gates: [],
      conceptIds: ["K-037"],
      reflected: false,
    },
  ];

  const concepts: Concept[] = [
    {
      id: "K-041",
      name: "idempotency key",
      area: "messaging",
      definition: "A value that makes a repeated delivery land on the same row instead of a new one.",
      seen: 4,
      help: "none",
      status: "passed",
      lastReviewed: ago(60 * 24 * 7),
      dueOn: ago(-60 * 24 * 21),
      taskIds: ["t14", "t11"],
    },
    {
      id: "K-018",
      name: "exponential backoff",
      area: "reliability",
      definition: "Each retry waits twice as long as the one before, which is useful while the far side is cold.",
      seen: 7,
      help: "hint",
      status: "due",
      lastReviewed: ago(60 * 24 * 12),
      dueOn: ago(60 * 6),
      taskIds: ["t14"],
    },
    {
      id: "K-004",
      name: "outbox pattern",
      area: "messaging",
      definition: "Write the message and the state change in one transaction; a separate flush delivers it.",
      seen: 9,
      help: "none",
      status: "passed",
      lastReviewed: ago(60 * 24 * 12),
      dueOn: ago(-60 * 24 * 40),
      taskIds: ["t14", "t12"],
    },
    {
      id: "K-052",
      name: "jitter",
      area: "reliability",
      definition: "A random offset on each wait so many clients do not wake at the same moment.",
      seen: 2,
      help: "full",
      status: "weak",
      lastReviewed: null,
      dueOn: null,
      taskIds: ["t14"],
    },
    {
      id: "K-033",
      name: "attempt cap",
      area: "reliability",
      definition: "The line past which a retry stops being patience and becomes a leak.",
      seen: 3,
      help: "full",
      status: "due",
      lastReviewed: ago(60 * 24 * 4),
      dueOn: ago(60 * 2),
      taskIds: ["t14", "t12"],
    },
    {
      id: "K-037",
      name: "property test",
      area: "testing",
      definition: "A check over generated cases rather than one example, with a shrinker for the failure.",
      seen: 5,
      help: "hint",
      status: "due",
      lastReviewed: ago(60 * 24 * 9),
      dueOn: ago(60 * 24),
      taskIds: ["t14", "p08", "f01"],
    },
    {
      id: "K-009",
      name: "flush",
      area: "messaging",
      definition: "The pass that reads pending rows and delivers them, on a timer or a scheduled retry.",
      seen: 6,
      help: "none",
      status: "passed",
      lastReviewed: ago(60 * 24 * 12),
      dueOn: ago(-60 * 24 * 30),
      taskIds: ["t14"],
    },
    {
      id: "K-002",
      name: "policy boundary",
      area: "design",
      definition: "One file owns the numbers; everything else reads them and stays ignorant of the value.",
      seen: 4,
      help: "none",
      status: "passed",
      lastReviewed: ago(60 * 24 * 2),
      dueOn: ago(-60 * 24 * 50),
      taskIds: ["t14"],
    },
    {
      id: "K-011",
      name: "at-least-once delivery",
      area: "messaging",
      definition: "A delivery guarantee that permits duplicates, so the consumer must be the one that copes.",
      seen: 8,
      help: "hint",
      status: "due",
      lastReviewed: ago(60 * 24 * 20),
      dueOn: ago(60 * 12),
      taskIds: ["t14", "s03"],
    },
    {
      id: "K-027",
      name: "dedupe window",
      area: "messaging",
      definition: "How long a key is remembered; after that, a duplicate is indistinguishable from a new message.",
      seen: 1,
      help: "full",
      status: "weak",
      lastReviewed: null,
      dueOn: null,
      taskIds: ["t11"],
    },
  ];

  const decisions: Decision[] = [
    {
      id: uid("d"),
      at: ago(60 * 14),
      title: "Cap the outbox at five attempts",
      why: "The downstream's p99 is 3.8s; a sixth wait outlives the request that started it.",
      cost: "A transient outage now fails loudly instead of quietly retrying for a minute.",
      taskId: "t14",
    },
    {
      id: uid("d"),
      at: ago(60 * 27),
      title: "Count the cap before computing the delay",
      why: "The attempt number decides whether there is a next attempt at all.",
      cost: "The delay function now needs to know the cap, which widens policy.ts.",
      taskId: "t14",
    },
    {
      id: uid("d"),
      at: ago(60 * 24 * 8),
      title: "Index undelivered rows, not delivered ones",
      why: "The flush only ever asks for rows that are not delivered yet.",
      cost: "Partial indexes need a migration when the delivery states change.",
      taskId: "t12",
    },
    {
      id: uid("d"),
      at: ago(60 * 24 * 12),
      title: "Keep the event log in SQLite, not a file per session",
      why: "One file makes a replay a query instead of a directory walk.",
      cost: "The log is opaque without the app; export is a real feature, not a nice-to-have.",
      taskId: null,
    },
    {
      id: uid("d"),
      at: ago(60 * 24 * 20),
      title: "No write access for the model, ever",
      why: "The learning state is the point; an agent that edits the record can fake the result.",
      cost: "The developer types every change, including the boring ones.",
      taskId: null,
    },
    {
      id: uid("d"),
      at: ago(60 * 24 * 6),
      title: "Gate 02 passes on the explanation, not the code",
      why: "The code was already correct; the sentence about order was not.",
      cost: "A passing check no longer implies a passed gate.",
      taskId: "t14",
    },
    {
      id: uid("d"),
      at: ago(60 * 24 * 31),
      title: "Read scope is a path list, not a permission system",
      why: "One developer, one machine; a list is honest and readable.",
      cost: "Nothing stops a broader path from being added by hand.",
      taskId: null,
    },
  ];

  const events: AuditEvent[] = [
    { id: uid("e"), at: ago(6), kind: "check", text: "property check evaluated · 1 failing case (timer fires twice)" },
    { id: uid("e"), at: ago(18), kind: "scope", text: "scope narrowed to src/outbox/policy.ts and flush.ts" },
    { id: uid("e"), at: ago(41), kind: "gate", text: "gate 02 passed · explanation named the order of evaluation" },
    { id: uid("e"), at: ago(52), kind: "note", text: "note added · cap moved into policy.ts" },
    { id: uid("e"), at: ago(60), kind: "session", text: "session opened · kestrel · task 14" },
    { id: uid("e"), at: ago(60 * 19), kind: "handoff", text: "handoff written · 3 lines · awaiting recall" },
    { id: uid("e"), at: ago(60 * 20), kind: "check", text: "hand-run completed · flush under 503 for 90s" },
    { id: uid("e"), at: ago(60 * 24 * 2), kind: "decision", text: "decision recorded · read scope is a path list" },
    { id: uid("e"), at: ago(60 * 24 * 7), kind: "gate", text: "gate 01 passed · index order explained" },
    { id: uid("e"), at: ago(60 * 24 * 8), kind: "note", text: "note added · partial index cut the flush scan to 3ms" },
    { id: uid("e"), at: ago(60 * 24 * 12), kind: "decision", text: "decision recorded · event log stays in SQLite" },
    { id: uid("e"), at: ago(60 * 24 * 14), kind: "gate", text: "gate 02 retried after a first answer was not met" },
    { id: uid("e"), at: ago(60 * 24 * 19), kind: "handoff", text: "handoff written · task 12 closed" },
    { id: uid("e"), at: ago(60 * 24 * 26), kind: "recall", text: "recall matched 2 of 3 handoff lines" },
  ];

  const drills: Drill[] = [
    {
      id: uid("dr"),
      skill: "property-based testing",
      name: "Generator from the fault, not the happy path",
      command: "bun test drills/gen_duplicates.test.ts",
      last: ago(60 * 24 * 9),
      attempts: 3,
      passed: false,
    },
    {
      id: uid("dr"),
      skill: "property-based testing",
      name: "Shrinker reports a minimal case",
      command: "bun test drills/shrink.test.ts",
      last: ago(60 * 24 * 11),
      attempts: 2,
      passed: false,
    },
    {
      id: uid("dr"),
      skill: "distributed systems",
      name: "Quorum read: name the stale case",
      command: "bun test drills/quorum.test.ts",
      last: ago(60 * 24 * 15),
      attempts: 4,
      passed: true,
    },
    {
      id: uid("dr"),
      skill: "distributed systems",
      name: "At-least-once: write the duplicate test",
      command: "bun test drills/duplicates.test.ts",
      last: null,
      attempts: 0,
      passed: false,
    },
  ];

  return {
    projects,
    tasks,
    concepts,
    decisions,
    events,
    drills,
    settings: {
      model: "local",
      allowlist: ["src/outbox/**", "src/queue/**", "tests/outbox/**"],
      writeAccess: false,
      modelUrl: "http://127.0.0.1:11434",
      modelName: "qwen3.5:9b",
    },
    session: { started: null, projectId: null, recall: null },
  };
}

export function emptyState(): AppState {
  return {
    projects: [],
    tasks: [],
    concepts: [],
    decisions: [],
    events: [],
    drills: [],
    settings: {
      model: "local",
      allowlist: [],
      writeAccess: false,
      modelUrl: "http://127.0.0.1:11434",
      modelName: "qwen3.5:9b",
    },
    session: { started: null, projectId: null, recall: null },
  };
}

export function makeProject(name: string, brief: string): { project: Project; task: Task } {
  const projectId = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || uid("p");
  const project: Project = {
    id: projectId,
    name,
    brief,
    briefBody: brief,
    repoPath: null,
    created: new Date().toISOString(),
    currentTaskId: "t1",
    handoff: null,
    recall: null,
    questions: [],
    notes: [],
  };
  const task: Task = {
    id: "t1",
    projectId,
    title: "First task",
    lane: "learn",
    phase: "plan",
    started: new Date().toISOString(),
    finished: null,
    minutes: 0,
    batch: "1 of 1",
    scope: "not set",
    prediction: "",
    planApproved: false,
    planRevised: false,
    planItems: [],
    planMessages: [],
    notes: [],
    attempts: [],
    checks: [check("ck1", "check", "write the first executable check", "pending", null)],
    gates: [
      gate("g1", 1, "What will tell you this task is done?", "It has to be something a machine can answer.", "not-yet", null),
    ],
    conceptIds: [],
    reflected: false,
  };
  return { project, task };
}
