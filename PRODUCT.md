# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

**Primary: the owner.** A solo developer working on his own machine, deliberately practicing and staying sharp while using AI to move faster. He works in his own repositories, keeps notes, and is willing to spend minutes on a check when a concept is new to him and none when it is not. He is the person who set the rules of the research brief and is the first test subject for them.

**Secondary: a few developers he knows.** Personal-first, but the thing has to be understandable and installable by other developers without a support apparatus or a tutorial written for strangers.

## Product Purpose

A harness for coding agents that keeps the developer learning what the agent does for them. It organizes work into project folders with notes and tasks, makes the plan phase carry a definition of done, puts short knowledge checks between phases, and gives the developer a scoped AI that reads and advises rather than writing the code.

It exists because the evidence the owner gathered says skills erode when AI removes the effortful parts of the work — attempting, recalling, explaining, debugging — and because the available productivity numbers are unstable and self-perception is unreliable. Success is therefore two numbers moving together against a no-AI baseline: retained understanding (no-AI quiz and drill scores, explain-back scores per concept) and actual delivery measures (lead time, review time, rework, estimate versus actual). Neither alone counts as success.

## Positioning

The learning and productivity logic lives outside any AI agent. Projects, phases, gates, the concept map, the learning ledger and the append-only event log are owned by the application; the model is a configuration value behind one narrow interface, so a local model, a free tier or a paid API can be swapped without changing the product's behavior.

That separation is the part a neighboring coding tool could not truthfully copy while its core value is the agent's autonomy: here the agent's authority is deliberately bounded — reads limited to allowlisted paths and enforced in code rather than the prompt, gate state owned by the app, and the AI unable to mark a gate as passed.

## Operating Context

- **Local-first desktop application.** A Tauri 2 shell with a vanilla TypeScript/Vite frontend, used on one Linux machine. Project data lives on that machine; there is no server to keep running and no sync to depend on.
- **Projects are plain markdown folders in git** — notes, tasks, a one-page brief, a decision log, a glossary, open questions — openable in any editor, including Obsidian. SQLite sits alongside for events, gate state and the concept map.
- **The work is routed by two questions per task:** how much would the developer learn, and how cheaply can the result be verified. Learn and practice lanes run tutor-style; a delegate lane for toil is optional and comes later.
- **Tasks move through phases** — plan, implement, verify, reflect — that the AI proposes and the developer approves. The plan phase produces a small-batch breakdown and a definition of done, ideally as executable checks the developer writes.
- **Gates attach to concepts, not tasks.** A task lists the concepts it touches; the developer is checked only on concepts not passed recently, and passed concepts return on a spaced schedule.
- **Sessions are bracketed by recall.** At session end the developer writes three lines (what is next, what is blocking, what he was thinking); at the next start he tries to recall them before seeing them. A weekly review reports a few numbers from the event log.
- **Models are undecided by intent.** Hosted API or a local model on a 16 GB AMD GPU are both plausible; the product must not depend on that choice.

## Capabilities and Constraints

Confirmed capabilities from the owner's design:

- Project folders with notes, tasks, brief, decision log, glossary and open questions; markdown as the storage format so data stays portable.
- Lanes assigned per task; task phases with an approved plan and a definition of done; an owner-written spec and acceptance checks where possible.
- App-owned concept map and gate state; short knowledge checks between phases; spaced resurfacing of concepts; explain-back answers saved as notes, decision-log entries or commit messages.
- Scoped, read-only AI by default; write access off and, if ever added, limited to a scratch folder or branch.
- Deterministic sensors first — tests, linters, type checks, property-based tests, simulation for concurrent code — with AI judgment only after them.
- Append-only local event log (task started, prediction submitted, check answered and scored, AI-off trial finished, gate overridden with a reason).
- Resumption briefing gated on a recall attempt; weekly review; calibration through blind AI-on/AI-off trials.
- Model as one configuration value behind a narrow interface; events written locally and gates fail open with the bypass logged when the model or machine is unreachable.

Constraints and current state:

- **This round is a frontend mock of the whole platform surface.** No backend, no persistence, no AI integration is being built yet. The design work targets screens and states that a mock can show.
- **Local-first is decided.** The earlier Pi-hosted design in the brief does not apply to the first version; anything about server-side sync or remote state is undecided and out of scope until the owner raises it.
- **Nothing beyond the Tauri template exists yet** — the project folders, event log, gates and model layer are designed, not implemented.
- Explicitly undecided: model provider and routing, event-log schema and retention, whether the delegate lane ever ships, backup beyond git, and how other developers would install it.

## Brand Commitments

The product name is **antik** (Tauri product name and bundle identifier `com.fabian.antik`). No visual identity, voice, logo or brand assets exist beyond the framework template defaults; none are binding yet.

## Evidence on Hand

- `idea.md` — the owner's research brief (18 November 2026): the learning-science and harness-engineering sources behind the design, the five platform sections, the six-phase build order, and a candid uncertainty section stating that the platform is an untested hypothesis and that the evidence for AI harming learning is small.
- That brief's own caveats are part of the product record: no study of a harness like this exists, mastery-learning evidence is old and classroom-based, and no AI coach has been validated for programming feedback.
- **Absences that future work must not fabricate:** no users, testimonials, case studies, measured results, benchmarks, pricing, or published efficacy claims exist. Marketing-style proof for this product would have to be real data the owner collects.

## Product Principles

1. **Learning leads; speed is a variable, not the goal.** Every piece of help is routed by how much the developer would learn and how cheaply the result can be verified.
2. **Attempt before answer.** Prediction, recall and self-explanation come before the agent's output; reading a diff is not practice.
3. **The app owns the learning state; the model is replaceable.** Gates, concepts, notes and events belong to the product, and no provider gets to define them.
4. **Small batches, explicit definitions of done, deterministic checks first.** Semantic checks written in code outrank generated code and generated judgment.
5. **Measure, do not self-report.** Numbers from the event log decide what stays; a component that moves no number is removed by month three.
