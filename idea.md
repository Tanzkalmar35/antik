# A personal coding-agent harness that keeps you learning

*Research brief, 18 November 2026*

**Scope and assumption.** By harness I mean the scaffolding around an LLM coding agent: instructions, tools, feedback checks, memory, permissions. I do not mean a test harness. I researched your five topics in order, preferring peer-reviewed papers, official engineering posts, standards bodies and industry surveys. Section 6 lists what I could not verify and where the evidence is thin. Sections 5.2 to 5.5 were added later and cover the platform design that grew out of the discussion: a scoped AI, phase gates, a productivity layer, and model and cost options.

## Short answer

- The research does not show that using AI makes you worse by default. It shows that skills erode when AI removes the effortful parts of the work (attempting, recalling, explaining, debugging) while you are still acquiring a skill.
- The design that came out of this is one platform, not a separate harness: project folders with notes and tasks, a plan phase with a definition of done, short knowledge checks between phases, and a scoped AI that reads only allowlisted paths and advises rather than writing your code (Section 5.2).
- Productivity is expected to come mostly from the ordinary parts: small well-defined tasks, a resumption briefing, curated project memory, delegating toil where checks are cheap, and a weekly review of your own numbers (Section 5.4). The checks cost time, so they attach to concepts and not to every task.
- Deliberate practice (Ericsson and Pool, Peak) fits as a practice lane in which the platform acts as your coach and feedback source. Strictly, that framework assumes a teacher and objective performance measures that software only partly has (Section 5.3).
- The platform keeps its logic outside any one AI agent, so the model is a configuration choice: a local model on a 16 GB GPU, a free tier, or a cheap paid API such as DeepSeek (Section 5.5).
- I found no published evidence that a platform like this restores no-AI-level understanding or raises productivity for a solo developer. Treat the design as a hypothesis, and test it on yourself.
- Being as fast as an unrestricted AI user and understanding everything as if you had written it by hand are in tension. Expect to trade some speed for understanding, mostly in areas that are new to you.

## 1. How people learn and get better

### 1.1 Learning is not performing

Bjork and Bjork separate how well you do something right now (performance) from a lasting change in ability (learning), which only shows up after a delay. Conditions that make practice feel harder, such as spacing, mixing problem types and testing yourself instead of rereading, can lower immediate performance and raise long-term retention. Only difficulties that trigger effortful encoding and retrieval count as desirable; many difficulties are simply obstacles. ([Bjork & Bjork chapter](https://www.unh.edu/teaching-learning-resource-hub/sites/default/files/media/2023-06/itow-introducing-desirable-difficulties-into-practice-and-instruction-bjork-and-bjork.pdf))

*Implication:* an agent that returns correct code instantly maximizes your performance and can starve learning.

### 1.2 Which study techniques work

Dunlosky and colleagues (2013) reviewed ten techniques. Practice testing and distributed practice earned the highest utility rating. Interleaved practice, elaborative interrogation and self-explanation were moderate. Summarization, highlighting, keyword mnemonics, imagery for text and rereading were low. ([APS summary](https://www.psychologicalscience.org/news/releases/which-study-strategies-make-the-grade.html), DOI 10.1177/1529100612453266)

*Caveat:* the review covers techniques students can use alone, mostly in classroom-style tasks. Transfer to professional programming is plausible but less directly tested.

*My inference:* reading an agent's diff and nodding is closer to rereading than to practice testing.

### 1.3 Attempt first, even if you fail

Richland, Kornell and Kao (2009) found that asking people questions before they studied a text improved later performance on those items, even for items they got wrong in the pretest, compared with extra study time. ([abstract](https://pubmed.ncbi.nlm.nih.gov/19751074/)) The related generation effect points the same way.

*Implication:* writing your own approach before seeing the agent's should help you process its answer, even when your approach is wrong.

### 1.4 Guidance should fade as you gain expertise

Cognitive load theory predicts that worked examples help novices. Kalyuga and colleagues showed an expertise reversal effect: as learners gain knowledge, the same explanations become redundant and can hurt, and problem solving becomes better. ([Kalyuga, Ayres, Chandler & Sweller](https://typeset.io/pdf/the-expertise-reversal-effect-5y3mqyx7oo.pdf))

*Implication:* the amount of help should depend on the topic. Heavy scaffolding in areas new to you, light in areas you know.

### 1.5 Practice quality beats practice hours

Ericsson's deliberate practice requires individualized tasks, specific goals and informative feedback, ideally from a coach. A 2014 meta-analysis (Macnamara et al.) found deliberate practice explained a modest share of performance variance across domains; Ericsson and Harwell (2019) argue that analysis used a much looser definition than the original. ([Ericsson & Harwell](https://pmc.ncbi.nlm.nih.gov/articles/PMC6824411), [Macnamara & Maitra](https://pmc.ncbi.nlm.nih.gov/articles/PMC6731745))

*Take:* the dispute is about how much practice explains. Nobody disputes that targeted practice with feedback matters, so do not rely on hours logged.

**What Peak adds.** Ericsson and Pool's popular account (*Peak*, 2016) sorts practice into three kinds. Naive practice is repetition without a plan. Purposeful practice has specific goals, focus, feedback and work outside your comfort zone. Deliberate practice adds a teacher and a field with established training methods and clear performance differences. Its central claim is that practice works by building better mental representations: the internal models that let experts plan, notice problems and correct themselves. I read summaries of the book, not the book itself.

**Software evidence.** A survey-based theory of software development expertise (335 developers; [summary](https://blog.acolyer.org/2018/12/21/towards-a-theory-of-software-development-expertise/)) concludes that experience alone does not make an expert. Deliberate practice with monitoring, feedback and reflection does, and individual differences also matter. A position paper argues that programming knowledge ages quickly ([PPIG 2017](https://www.ppig.org/papers/2017-ppig-28th-parnin)), which suggests practicing durable mental models over tool details. Both are modest evidence.

### 1.6 The old warning about automation

Bainbridge (1983) argued that automation leaves humans two jobs, monitoring and manual takeover, and that automation erodes the skills both need. Unused skills decay, and the more reliable the automation, the rarer the practice. ([paper](https://gwern.net/doc/sociology/technology/1983-bainbridge.pdf))

*Implication:* reviewing agent output while rarely writing or debugging yourself is exactly this pattern.

### 1.7 What the generative-AI evidence says

- **Anthropic RCT (Shen & Tamkin, Jan 2026).** 52 mostly junior engineers learned an unfamiliar Python library (Trio). The AI group averaged 50% on a follow-up quiz versus 67% for hand-coders (Cohen's d 0.74), with the biggest gap on debugging. The speedup was not statistically significant. How people used AI mattered: wholesale delegation, progressive reliance and iterative AI debugging scored low, while asking conceptual questions, requesting explanations, or using AI to check your own understanding scored better. ([Anthropic summary](https://www.anthropic.com/research/AI-assistance-coding-skills), [preprint](https://www.arxiv.org/pdf/2601.20245)) *Limits:* short task, one library, a learning scenario, and the authors work at an AI vendor.
- **Bastani et al., PNAS 2025.** About 1,000 high-school math students. Unguarded GPT-4 raised practice scores by 48%, then students scored 17% worse than controls once access was removed. A tutor variant with guardrails raised practice by 127% and largely avoided the harm. ([PNAS record](https://papers.ssrn.com/abstract=4895486)) *Takeaway:* tool design changes the learning outcome.
- **Lee et al., CHI 2025.** A survey of 319 knowledge workers: higher confidence in AI went with less critical thinking, higher self-confidence with more. Self-reported and correlational. ([paper](https://www.microsoft.com/en-us/research/uploads/prod/2025/01/lee_2025_ai_critical_thinking_survey.pdf))
- **METR.** In a 2025 RCT, 16 experienced open-source developers took 19% longer with AI on their own repositories while believing they were about 20% faster. ([METR](https://metr.org/blog/2025-07-10-early-2025-ai-experienced-os-dev-study/)) A February 2026 update says newer data hint at a speedup, but developers withholding tasks they did not want to do without AI made the estimate unreliable, so METR is changing its design. ([update](https://metr.substack.com/p/2026-02-24-uplift-update)) *Takeaway:* there is no stable productivity number, but self-perception can be badly wrong.
- **DORA 2025** (about 5,000 respondents). AI use is near 90%, now correlates with higher delivery throughput, and acts as an amplifier of existing team strengths and weaknesses. Gains show up in loosely coupled architectures with fast feedback loops. ([Google Cloud summary](https://cloud.google.com/blog/products/ai-machine-learning/announcing-the-2025-dora-report))

### 1.8 Gating progress on checks (mastery learning)

Mastery learning makes you pass a check before moving on to new material; failing leads to extra help and a retest. A 1990 meta-analysis of 108 controlled evaluations found positive effects on exam performance ([Kulik, Kulik & Bangert-Drowns](https://typeset.io/pdf/effectiveness-of-mastery-learning-programs-a-meta-analysis-45bgwh1byb.pdf)), and an ASCD summary puts the average mastery student at about the 70th percentile of a conventional class ([ASCD](https://ascd.org/el/articles/research-on-obe-what-we-know-and-dont-know)). Treat that as moderate evidence. One toolkit summary lists pooled effects from 0.04 to 0.60 with no synthesis from the last decade ([summary](https://educaixa.org/es/-/apendice-tecnico-mastery-learning)), the studies are classroom-based, and mastery learning adds instruction time ([Evidence for Learning](https://evidenceforlearning.org.au/education-evidence/australasian-research-summaries/teaching-and-learning-toolkit/mastery-learning)). A later analysis found that removing the mastery testing from such courses substantially reduced their effectiveness ([Bangert-Drowns et al. 1991](https://www.gwern.net/docs/spaced-repetition/1991-bangertdrowns.pdf)). Typical pass thresholds are around 80%.

*Implication:* the check is probably the active ingredient, so use short, sharp gates only where they pay for their time, and attach them to concepts, not to every task.

### 1.9 Design requirements this section implies

1. Keep you doing the retrieval, generation and debugging while you are learning something.
2. Make attempts come before answers.
3. Make guidance depend on your competence in each topic, and fade it.
4. Test understanding later, at a delay, not only during the task.
5. Do not trust your feeling of productivity or understanding; measure.
6. Reserve some practice with feedback for no-AI work on your weak spots.

## 2. Distributed systems and systems development today

This is a broad field. I covered the slice that bears on building a harness, not the whole discipline.

**Infrastructure baseline.** The CNCF 2025 survey (628 respondents) reports 98% of organizations using cloud native techniques and 82% of container users running Kubernetes in production, up from 66% in 2023. ([CNCF](https://www.cncf.io/reports/the-cncf-annual-survey-2025/))

**Correctness practice.** AWS describes a spectrum: formal specification (TLA+, the P language) for designs, plus lightweight methods used widely: property-based testing, fuzzing, and deterministic simulation testing, where a distributed system runs on a single-threaded simulator that controls scheduling, timing and message ordering. Production logs can also be checked against a formal spec. ([ACM / AWS](https://cacm.acm.org/practice/systems-correctness-practices-at-amazon-web-services))

**LLMs and formal specs.** SysMoBench evaluates LLMs writing TLA+ models of real systems. A SIGOPS write-up of it reports specs that pass the syntax checker yet are semantically wrong, for example by merging multi-step operations into one atomic step so real behaviors become impossible. ([SysMoBench](https://arxiv.org/pdf/2509.23130v2), [SIGOPS](https://sigops.org/2026/can-llms-model-real-world-systems-in-tla)) *Implication:* output that looks valid is not necessarily correct. You need checks that exercise behavior.

**Agent systems are distributed systems.** Anthropic's Managed Agents post splits an agent into a session (an append-only event log), a harness (the loop that calls the model and routes tool calls) and a sandbox. The harness is stateless and replaceable; after a crash a new one reloads the log and resumes. Containers are disposable, and credentials are kept out of reach of the sandbox where generated code runs. ([Anthropic](https://www.anthropic.com/research/managed-agents)) Practitioner reports describe the same failure modes you would expect: non-deterministic steps, retries that repeat side effects, and the need for idempotency keys and durable execution. These are engineering blogs, not peer-reviewed.

**Standards.** In December 2025 the Linux Foundation formed the Agentic AI Foundation, housing Anthropic's Model Context Protocol, Block's goose and OpenAI's AGENTS.md. ([report](https://campustechnology.com/articles/2025/12/15/newly-launched-agentic-ai-foundation-brings-together-tech-giants-for-open-source-ai-development.aspx)) Tool connections and instruction files are becoming portable across agents.

**What this means for a personal harness**

- Keep a durable, append-only record of every run, and keep the harness process itself disposable.
- Make tools idempotent, and checkpoint (git commit) before side effects.
- Prefer deterministic semantic checks (tests, property-based tests, simulation for concurrent code) over trusting generated code or generated specs.
- Work in small batches. DORA reports this matters more with AI, since AI makes large risky changes easy.

## 3. What a harness is, and how to build one

**Definition.** The term has come to mean everything in an AI agent except the model: Agent = Model + Harness. For coding agents, Birgitta Böckeler (Thoughtworks, on martinfowler.com) separates two layers: the builder harness built into the tool (system prompt, code retrieval, orchestration) and the outer or user harness you add for your codebase. ([Böckeler](https://www.martinfowler.com/articles/exploring-gen-ai/harness-engineering.html)) Anthropic's simple working definition of an agent is an LLM using tools in a loop. ([context engineering post](https://www.anthropic.com/research/effective-context-engineering-for-ai-agents)) Context engineering (what the model sees) is a subset of harness engineering, which also covers tools, feedback loops and the repository itself.

**What it is used for.** Models are fallible, forgetful across sessions, and prone to declaring victory early. The harness supplies what the model lacks: memory across context windows, verification, boundaries and a way for you to steer.

**Components.**

- Instructions and context: system prompt, CLAUDE.md or AGENTS.md, skills.
- Tools and the agent-computer interface.
- Context management: compaction, note files, subagents.
- Guides (feedforward) and sensors (feedback), computational or inferential.
- Session state, permissions, sandbox.
- Evals, to see whether the harness is improving.

**A worked example (Anthropic's long-running harness).** An initializer session creates an init script, a progress file and a first git commit, plus a structured feature list (over 200 items for their demo) with every item marked failing. Each later session reads the progress file, git log and feature list, picks one feature, implements it, tests it end to end, commits, and updates the progress notes. The harness targeted three observed failure modes: trying to do everything at once, declaring the job done too early, and marking features complete without end-to-end testing. ([Anthropic](https://www.anthropic.com/research/effective-harnesses-for-long-running-agents))

**Three ways to build your own**

1. *Configure an existing agent (start here).* Claude Code supports instruction files, skills, subagents, hooks and output styles. Its built-in Learning style asks you to implement small pieces of code yourself via TODO(human) markers, and the Explanatory style adds explanatory notes. Custom styles are plain markdown files. ([docs](https://code.claude.com/docs/en/output-styles))
2. *Build on an agent SDK.* Use the Claude Agent SDK or similar when you need your own orchestration, gates or logging.
3. *Write the loop yourself.* A model call, a tool router, a log and a sandbox. Most flexible, most maintenance, and rarely needed for personal use.

*Updated recommendation:* the later design (Sections 5.2 and 5.5) keeps the learning logic outside any agent, so the agent stays swappable. Claude Code is one option among several, not a requirement.

## 4. Best practices: what makes one harness better

1. **Interface design matters.** SWE-agent (NeurIPS 2024) showed that a purpose-built agent-computer interface, without changing model weights, improved results by about 64% relative to a shell-only agent on SWE-bench Lite (18% vs 11%, GPT-4 Turbo), with a linting edit command the most important piece. ([paper](https://arxiv.org/abs/2405.15793)) The models are dated; the principle is not.
2. **Context is a scarce resource.** Anthropic recommends the smallest set of high-signal tokens, minimal non-overlapping tool sets (if a human cannot say which tool applies, neither can the agent), just-in-time retrieval, and for long tasks compaction, note-taking and subagents. ([post](https://www.anthropic.com/research/effective-context-engineering-for-ai-agents))
3. **Verify outside the generator.** Agents tend to praise their own work. Anthropic found that separating the doer from a skeptical evaluator was far easier to tune than making the generator self-critical, and that end-to-end checks (driving the running app) caught bugs unit tests missed. It also found the evaluator pays off only at the edge of what the model can do alone, and a full multi-agent run cost about $200 over 6 hours versus $9 for a 20-minute solo run on the same prompt. ([post](https://www.anthropic.com/research/harness-design-long-running-apps))
4. **Guides plus sensors, cheap ones first.** Böckeler: guides alone never learn whether rules worked; sensors alone repeat mistakes. Prefer fast deterministic sensors (types, linters, tests, structural checks) run left, before commit; run slower inferential checks (LLM review, mutation testing) later. Whenever a failure repeats, improve the harness. She also warns that behavior is the weakest area: leaning on AI-written tests is not trustworthy enough yet, and correctness is outside any sensor's reach if the human never specified it. ([Böckeler](https://www.martinfowler.com/articles/exploring-gen-ai/harness-engineering.html))
5. **Every component encodes an assumption that goes stale.** Anthropic added context resets for a model that wrapped up early, then found the newer model no longer needed them. Remove components one at a time when a new model arrives and see what is still load-bearing. ([Managed Agents](https://www.anthropic.com/research/managed-agents), [harness design](https://www.anthropic.com/research/harness-design-long-running-apps))
6. **Make it durable and safe.** Stateless harness, durable log, tool-as-interface, credentials unreachable from generated code, git as checkpoint.
7. **Evaluate the harness.** Anthropic's eval guidance: define tasks and graders (code-based, model-based, human), run multiple trials, and read transcripts to catch broken graders. Separate capability evals from regression evals. ([guide](https://www.anthropic.com/research/demystifying-evals-for-ai-agents), I read only excerpts.)
8. **Keep it simple.** Anthropic's repeated advice is the simplest thing that works, adding complexity only when measured need appears.

**What makes one harness better than another, operationally:** pass rate on your tasks, cost per task, review time per change, how legible failures are, and, for your goal, how well you still understand and can reproduce the work without the agent.

## 5. A harness for staying sharp while moving faster

### 5.1 Design principles, tied to the evidence

| Mechanism | Why (evidence) | Cost to you |
| --- | --- | --- |
| **Two lanes, chosen per task.** Learn lane for concepts or code you cannot yet write unaided; Delegate lane for familiar work. | Harm concentrated in skill acquisition (Shen & Tamkin); expertise reversal (Kalyuga). | A 10-second classification. |
| **Predict-first gate (Learn lane).** Write a 3-5 line approach before the agent proposes anything. | Pretesting and generation effects (Richland et al.). | A few minutes per task. |
| **Explain-back gate (Delegate lane).** Before merging a non-trivial diff, answer 2-4 questions generated from it (invariant, failure case, why this approach, predict a test result). | Practice testing and self-explanation (Dunlosky); better outcomes for users who verified understanding (Shen & Tamkin). | 3-5 minutes per change; scale with risk. |
| **Human-owned zones.** Mark core logic, concurrency and consistency-critical code, and skills you want to keep as yours to write; the agent scaffolds around them. | Learning style precedent; Bainbridge. | Slower in those zones, by design. |
| **You hypothesize first when debugging.** Log your suspected cause before the agent investigates. | Debugging showed the largest gap (Shen & Tamkin). | Minutes; large learning value. |
| **Spec and acceptance checks written by you; deterministic sensors first.** | Behavior harness is the weak spot (Böckeler); feature-list pattern (Anthropic). | Upfront effort, then faster review. |
| **Learning ledger plus spaced review.** The harness logs concepts you asked about or got wrong, and resurfaces them as short retrieval prompts on a schedule. | Spacing and practice testing rated highest (Dunlosky); learning appears at a delay (Bjork). | 5-10 minutes a day. |
| **Fading help.** Track per-topic explain-back scores; reduce scaffolding as scores rise. | Expertise reversal. | Bookkeeping, automated. |
| **Calibration and blind trials.** Randomly assign some tasks as AI-allowed or AI-off, and log time, defects and your own estimates. | Perception gap (METR); confidence effect (Lee et al.). | Some tasks done slower. |
| **Practice lane.** Stretch goals from your skill map, no-AI sessions, trusted feedback (Section 5.3). | Deliberate and purposeful practice (Ericsson & Pool, Peak). | 2-3 sessions a week (a starting point). |

### 5.2 Architecture: one platform with a scoped AI

The design has merged into one application, not a separate harness plus a learning service. It runs on your Pi. Each project is a folder of markdown files in git (notes, tasks, a one-page brief, a decision log, a glossary), with SQLite alongside for events, gate state and the concept map. It calls a model through a narrow interface. The model can run on your desktop GPU, a free or cheap hosted tier, or a paid API, chosen by configuration (Section 5.5). It starts as a tutor: the AI reads and advises, and you write the code.

- **Projects:** a folder per project with notes, tasks, a one-page brief, a decision log, a glossary and open questions. Plain markdown, so any editor (Obsidian included) can open it and the data stays portable.
- **Lanes:** each task is routed by two questions, how much you would learn and how cheaply the result can be verified (Section 5.4). Learn and practice lanes run tutor-style. A delegate lane for toil is optional and comes later.
- **Task flow:** each task moves through phases, for example plan, implement, verify, reflect. The AI proposes the phases and you approve them. The plan phase produces a small-batch breakdown and a definition of done.
- **Gates:** a short knowledge check between phases, attached to concepts rather than tasks. The app owns gate state. The AI drafts questions and hints but cannot mark a gate as passed.
- **Scoped AI:** read-only access to allowlisted paths per project, enforced by the app in code and not by the prompt. Write access is off by default. If you add it later, limit it to a scratch folder or branch.
- **Sensors:** tests, linters, type checks, property-based tests and, for concurrent code, simulation. Executable checks anchor grading wherever possible, because AI graders tend to be lenient.
- **Records:** an append-only event log (task started, prediction submitted, check answered with score, AI-off trial finished, gate overridden with a reason), written locally first and synced to the Pi.
- **Resumption briefing:** at session start, shows your last three-line handoff after you try to recall it (Section 5.4).
- **Review scheduler:** spaced prompts drawn from the concept map and the learning ledger.
- **Weekly review:** a short report built from the event log (Section 5.7).
- **Model layer:** one configuration value selects the provider and model. Keep instructions in AGENTS.md and tools behind MCP so any agent can be swapped in.
- **Safety:** sandboxed execution if you ever let code run, scoped permissions, secrets kept out of the AI's reach, git checkpoints.
- **Failure rule:** if the model or the Pi is unreachable, the harness writes events locally and gates fail open, with the bypass logged.

### 5.3 The practice lane, built on Peak

The harness can supply the coaching and feedback that deliberate practice needs, but only if your goals are narrow and your practice time is protected.

**Choose narrow targets.** Deliberate practice needs specific goals, and becoming the best engineer is not one. Pick one or two sub-skills for about three months, such as diagnosing failures in concurrent or distributed code, or writing and checking invariants. Prefer skills that stay valuable when AI writes most of the code: debugging, specifying behavior and verifying results. Debugging showed the biggest learning gap for AI users (Shen & Tamkin), and Lee et al. found that critical thinking under AI shifts toward verification, integration and stewardship.

**How Peak's elements map to the harness**

| Peak element | What the harness does | Main risk |
| --- | --- | --- |
| Specific stretch goal | Keeps a skill map built from explain-back scores, quiz results and blind trials; proposes the next target just beyond your current ability. | Misjudged difficulty. Review the map weekly. |
| Focus | Practice sessions are no-AI by default and time-boxed; the agent joins only after you submit your attempt. | Deadlines squeeze practice out. Put sessions on the calendar. |
| Immediate, informative feedback | Deterministic first: tests, property-based tests, simulation for concurrent code. The LLM comments on your reasoning. | LLM judges are lenient and can be wrong. Spot-check against tests. |
| Mental representations | You write the model first (state machine, invariants, failure modes). The harness compares it with the code, an expert source (a paper, postmortem or formal spec) and the agent's own model, and logs the gaps. | The agent's model may be wrong. Trust expert sources over it. |
| Teacher | A stronger engineer reviews a sample of your practice work every few weeks. | An AI coach is unvalidated for programming. |
| Progress monitoring | Per-skill scores on repeatable drills, tracked over time. | Drills drift away from real work. Refresh them. |

**Drills.** Seeded-bug debugging (the agent plants a bug that a failing test proves; you diagnose it without AI), predicting the behavior of unfamiliar code before running it, redesigning from a postmortem, and implementing a small protocol piece checked by simulation. Every generated exercise must ship with an executable check you run before using it. Drills derived from your real work (a variant of the hardest part of a task you just finished) are my own idea and untested.

**Time.** Start with two or three sessions of 45 to 60 minutes a week. That figure is my starting point, not an evidence-based dose. Summaries of Peak, such as [this one](https://www.mindtools.com/aqzv9sh/peak/), describe deliberate practice as effortful and usually not enjoyable, so cap it and schedule it rather than relying on motivation.

### 5.4 The productivity layer

The tutor and the gates mostly add time. DORA's 2025 research found that the capabilities that amplify AI's benefit are ordinary engineering practices: connecting AI to your own internal data, strong version control, working in small batches and a user-centric focus, among others ([DORA](https://cloud.google.com/blog/products/ai-machine-learning/from-adoption-to-impact-putting-the-dora-ai-capabilities-model-to-work)). These ideas move the platform's center of gravity from quizzing toward making your work well-defined, resumable and measurable.

1. **Make the plan phase the core.** Each task needs a small-batch breakdown and a definition of done, ideally as executable checks. You write them. The AI critiques them for ambiguities, missing edge cases and a quick pre-mortem. This guards against building the wrong thing faster, a risk one [review of the DORA report](https://www.splunk.com/en_us/blog/learn/state-of-devops.html) highlights. Anthropic's long-running harness uses the same pattern: a feature list whose items start as failing and flip only when verified.
2. **Gate concepts, not tasks.** Keep a concept map. A task lists the concepts it touches. You are checked only on concepts you have not passed recently, and each concept returns on a spaced schedule. Familiar work gets no gate, which fits the expertise reversal effect (Section 1.4) and limits the time cost that mastery learning carries (Section 1.8).
3. **Make checks produce something.** Your explain-back answers become the decision-log entry, the commit message or a README paragraph. Generate questions from your actual diff and decisions, and cap them at two or three.
4. **Add a resumption briefing.** In a study of 10,000 sessions from about 86 programmers, only 10% resumed coding within a minute of returning, and programmers relied heavily on notes ([Parnin & Rugaber](https://sites.cc.gatech.edu/reverse/repository/resumptionstrategies.pdf)). At session end you write three lines: what is next, what is blocking, what you were thinking. At the next start you try to recall them first, then see them. It needs no AI.
5. **Route by two questions.** How much would you learn, and how cheap is the result to verify? Boilerplate, config, scaffolding, test setup and migrations with good tests go to an optional delegate lane under strict checks. You drive the high-learning work. I found no evidence that AI speeds up experienced developers on familiar code (Section 1.7, METR), so log time per lane and let your own data decide.
6. **Instrument yourself like a team.** From the event log, a weekly review of four or five numbers: lead time per task, rework (tasks reopened), review time, estimate versus actual, gate overrides, and AI-on versus AI-off outcomes.
7. **Keep project memory small.** A one-page brief, decision log, glossary and open questions. The AI reads those plus allowlisted code. DORA found that connecting AI to internal data boosts individual effectiveness and code quality, and Anthropic's context-engineering advice favors a small set of high-signal context over a large pile of notes.

These are my inferences from DORA, METR, the interruption research and the design logic. None has been tested for a solo developer. Item 6 is how you find out.

### 5.5 Models, cost and free options

The platform should not depend on one provider. Keep the model as a configuration value, and keep the valuable logic (gates, notes, concept map, event log) outside any agent. Prices and free-tier terms below were checked in November 2026 and change often.

**Cheap paid.** DeepSeek's official docs list deepseek-flash at $0.15 input and $0.60 output per million tokens off-peak (double at peak), and deepseek-v4-pro at $0.66 and $1.98 off-peak ([pricing](https://api-docs.deepseek.com/quick_start/pricing/)). Claude Sonnet 5.5 is $2 and $10, Opus 5.5 is $4 and $20, and Haiku 5.5 is $0.10 and $0.50 for prompts up to 100K tokens, five times that beyond. Most of the Claude figures come from secondary sources quoting [Anthropic's pricing page](https://platform.claude.com/docs/en/about-claude/pricing), which I could not open directly. For one request with 100K tokens in and 20K out, my arithmetic (no caching, no reasoning tokens) gives about $0.02 on Haiku 5.5, $0.03 on DeepSeek Flash off-peak, $0.11 on DeepSeek V4 Pro off-peak, $0.40 on Sonnet 5.5 and $0.80 on Opus 5.5. I did not verify quality for agentic coding on any of them, so compare on your own task set.

**Lock-in.** DeepSeek documents pointing Claude Code at its Anthropic-compatible endpoint ([docs](https://api-docs.deepseek.com/quick_start/agent_integrations/claude_code.md)), so Claude Code does not force a Claude subscription, although a subscription does not route through that setup. Per one comparison, Claude Code itself is not open source (its public repository is a docs-only tracker). OpenCode is MIT-licensed with 75+ providers, and Aider and goose are Apache-2.0. CLAUDE.md, hooks and skills only work in Claude Code. Because the platform keeps its logic outside the agent, you can swap models and, later, agents.

**DeepSeek caveats.** Names and prices changed several times in 2026, and older model names were retired, so do not hard-code either. Peak pricing doubles the rates: one snapshot of the pricing page listed 01:00 to 04:00 and 06:00 to 10:00 UTC on weekdays, which would cover German mornings, so check the current hours. On privacy, the hosted service stores data in China, and Berlin's data protection commissioner called its transfers of user data unlawful ([heise](https://heise.de/-10462419)). That concerns personal data. One German legal analysis rates plain code generation as lower risk but flags IP exposure separately ([analysis](https://compound.law/en-DE/tools/deepseek/)). Do not send employer code, secrets or personal data.

**Free hosted tiers.** These are thin for agent work and unstable ([overview](https://www.requesty.ai/blog/free-llm-apis)). OpenRouter's free models allow 20 requests a minute and 50 a day (1,000 a day after buying $10 of credits), and Cerebras offers 1 million tokens a day. Free-tier prompts on Google's Gemini API can be used to improve Google products and read by human reviewers, and the sources disagree on whether that tier applies in the EU. They also disagree on Mistral's free API access. Assume free-tier prompts may be logged, and check each provider's terms before use.

**Local models on your RX 7800 XT (16 GB).** The card is on AMD's [ROCm Linux support list](https://rocm.docs.amd.com/en/latest/release/gpu_os_support.html) and Ollama's [supported-GPU list](https://docs.ollama.com/gpu.md). On Windows, ROCm officially supports only RDNA 4, so use the Vulkan backend. Phoronix found Vulkan in llama.cpp still beating ROCm in its 2026 tests, though not on your card ([Phoronix](https://www.phoronix.com/review/lemonade-vulkan-rocm/2)). For 16 GB, guides suggest gpt-oss-20b or Devstral Small ([guide](https://bishalkshah.com.np/blog/best-local-llm-for-coding)), and a repo-sized context can exceed what a 16 GB card holds ([guide](https://www.orcarouter.ai/blog/best-local-llm-for-coding)). That fits a tutor with small contexts better than autonomous coding. Local quality for writing and grading questions is untested, so spot-check it and keep a hosted fallback. The app and state live on the Pi and the model runs on the desktop, reached over your network or a VPN. Gates fail open if the desktop is off.

**Suggested setup.** A priority list: the local model for routine work, a free tier only for content that is not private, and a few dollars of DeepSeek or a small Claude model as the fallback for hard cases. Decide with data by running your Phase 0 task set through two models.

### 5.6 Build order

1. **Week 0, baseline and targets.** Take 5-10 representative tasks done without AI and a short quiz on areas you care about. Record times and scores. Pick one or two target skills for the practice lane. You need the baseline to know whether anything changed.
2. **Phase 1, the no-AI core.** Project folders, a plan phase with a definition of done, the decision log, the three-line handoff with the resumption briefing, and the local event log. Run it for about four weeks against your baseline.
3. **Phase 2, weekly review.** Add the few numbers from Section 5.4, item 6.
4. **Phase 3, tutor and gates.** The scoped read-only AI, the concept map, short gates, and explain-back answers saved as notes. Start with whichever model setup in Section 5.5 is cheapest to try.
5. **Phase 4, practice lane.** A skill map, a small drill library where every drill has an executable check, calendared sessions, and a human review of your practice work every few weeks.
6. **Phase 5, delegate lane, only if your data say AI helps.** Toil such as boilerplate, config and migrations, under strict checks.
7. **Ongoing.** Re-run your task set whenever the model changes, and drop any feature that moves none of your numbers by month three.

### 5.7 How you will know it works

- No-AI quiz and kata scores over time (the main answer to your question).
- Drill results on your one or two target skills (timed no-AI debugging, predicting the behavior of unfamiliar code), and how your written models compare with an expert reference.
- Explain-back scores per concept.
- Lead time and review time per task compared with your baseline, rework (tasks reopened), and defects found after merge.
- Estimate versus actual time, and the gap between your estimate and measured time on blind trials. Pre-commit your rules for what gets randomized, because METR's experience shows people withhold tasks they want AI for.
- Resumption time after breaks: how long until your first productive step.
- Gate overrides, and what they were for.

## 6. What I am unsure about

- **The evidence that AI hurts learning is small.** One vendor RCT (52 people, short task, a new library), one high-school math field experiment, and one self-report survey. How far this carries to an experienced engineer in a familiar domain is unknown.
- **I found no study of a harness like the one in Section 5** in professional software work. The closest analog is Bastani's guarded tutor, in math. Everything in Section 5 is derived from general learning science plus these studies, and is a hypothesis.
- **No evidence yet that an AI coach gives deliberate-practice-quality feedback on programming.** The practice lane in Section 5.3 is a hypothesis too.
- **Learning-science effects come mostly from lab and classroom tasks.** Transfer to programming is plausible, not proven.
- **Productivity numbers are unstable.** I would not quote any single METR or vendor figure.
- **Deliberate practice is contested** in how much it explains, and its strict conditions (a teacher, objective performance measures) are only partly met in software. The software-specific evidence I found is one survey-based theory and a position paper.
- **Peak is second-hand here.** I read summaries of the book, not the book, so what I say about its criteria comes from those summaries.
- **Several sources are vendor or practitioner posts** (Anthropic, OpenAI, Thoughtworks, practitioner blogs). They are first-hand accounts, not independent evaluations.
- **Gaps in what I read.** I could not open OpenAI's harness-engineering post directly (it blocked the fetch), so I relied on Böckeler's summary of it. For Anthropic's evals guide I read excerpts only. My distributed-systems coverage is a slice, not a survey.

* **Mastery-learning evidence is old and classroom-based.** The main syntheses predate 1990 and pooled effects vary widely. Transfer to a solo adult learner is unknown, and gates add time.
* **The platform design is untested.** The productivity ideas in Section 5.4 are my inferences from DORA, METR and the interruption research. None has been tested for a solo developer.
* **Prices and free tiers move fast.** The figures in Section 5.5 come from DeepSeek's docs and secondary sources, and free-tier terms change without notice. I did not verify model quality for agentic coding or for writing and grading questions, local or hosted.
* **Local-model fit on the RX 7800 XT** comes from vendor documentation and third-party guides, not from my own tests.

## Sources

*Peer-reviewed or archival:* Dunlosky et al. 2013 (Psychological Science in the Public Interest); Richland, Kornell & Kao 2009 (J. Exp. Psychology: Applied); Kalyuga et al. 2003 (Educational Psychologist); Ericsson & Harwell 2019 and Macnamara & Maitra 2019; Bainbridge 1983 (Automatica); Bastani et al. 2025 (PNAS); Lee et al. 2025 (CHI); Yang et al. 2024, SWE-agent (NeurIPS); Baltes & Diehl 2018, software development expertise (ESEC/FSE, read via a summary); ACM Queue / CACM on AWS correctness practices.

*Preprints:* Shen & Tamkin 2026; SysMoBench.

*Official or institutional reports:* DORA 2025; CNCF 2025 annual survey; METR posts.

*Vendor and practitioner engineering posts:* Anthropic (effective harnesses, harness design, Managed Agents, context engineering, evals, Claude Code docs); OpenAI harness engineering post (not fetched); Böckeler on martinfowler.com; SIGOPS blog.

*Book and position paper:* Ericsson & Pool 2016, Peak (read via summaries only); PPIG 2017 position paper on programming expertise.

*Added with the platform design:* DORA AI Capabilities Model (2025); Kulik, Kulik & Bangert-Drowns 1990 and Bangert-Drowns et al. 1991 on mastery learning and mastery testing; Parnin & Rugaber 2009 on resumption after interruptions; DeepSeek API documentation (pricing, Anthropic-compatible endpoint, Claude Code integration); Anthropic API pricing (via secondary sources); AMD ROCm and Ollama GPU support documentation; third-party free-tier and local-model guides (treat as volatile).
