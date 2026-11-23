import type { Check, Concept, Gate, Task } from "./data";
import { clock, relative, shortDate } from "./store";

export { clock, relative, shortDate };

export interface View {
  html: string;
  wire?: (root: HTMLElement) => void;
}

export function esc(value: string | number | null | undefined): string {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export function dash(value: string | null | undefined): string {
  return value && value.trim() ? esc(value) : "—";
}

export function phaseLabel(phase: Task["phase"]): string {
  return phase === "implement" ? "implement" : phase;
}

export function gateStamp(gate: Gate): string {
  switch (gate.status) {
    case "passed":
      return `<span class="stamp met">met · ${clock(gate.evaluated)}</span>`;
    case "overridden":
      return `<span class="stamp over">overridden · ${clock(gate.evaluated)}</span>`;
    case "failed":
      return `<span class="stamp unmet">not met</span>`;
    case "due":
      return `<span class="stamp due">due</span>`;
    default:
      return `<span class="stamp idle">not yet</span>`;
  }
}

export function checkStamp(check: Check): string {
  if (check.result === "pass" || check.result === "done") {
    return `<span class="stamp met">${check.result === "done" ? "done" : "pass"}</span>`;
  }
  if (check.result === "fail") return `<span class="stamp unmet">fail</span>`;
  return `<span class="stamp idle">not run</span>`;
}

export function conceptStamp(concept: Concept): string {
  if (concept.status === "passed") return `<span class="stamp met">passed</span>`;
  if (concept.status === "weak") return `<span class="stamp weak">weak</span>`;
  return `<span class="stamp due">due</span>`;
}

export function helpLabel(help: Concept["help"]): string {
  return help === "none" ? "none" : help === "hint" ? "hint only" : "full";
}

export function metaLine(parts: Array<string | null | undefined>): string {
  const kept = parts.filter((p): p is string => Boolean(p && p.trim()));
  return kept.map((p) => `<span>${p}</span>`).join("");
}

export function statusPill(text: string, tone: "met" | "unmet" | "due" | "weak" | "idle"): string {
  return `<span class="stamp ${tone}">${esc(text)}</span>`;
}

export function evaluatedLine(iso: string | null): string {
  return iso ? `evaluated ${clock(iso)}` : "never evaluated";
}

export function timeText(iso: string | null): string {
  return iso ? `${shortDate(iso)} ${clock(iso)}` : "—";
}

export function agoText(iso: string | null): string {
  return relative(iso);
}

export function pageHead(
  title: string,
  byline: string[],
  actions = "",
): string {
  return `
    <header class="doc-head">
      <div class="doc-head-main">
        <h1>${esc(title)}</h1>
        <p class="byline">${byline.map((b) => `<span>${b}</span>`).join("")}</p>
      </div>
      ${actions ? `<div class="head-actions">${actions}</div>` : ""}
    </header>
  `;
}

export function chunk(_key: string, body: string): string {
  return `<section class="chunk"><div class="chunk-body">${body}</div></section>`;
}

export function emptyState(title: string, body: string, action = ""): string {
  return `<div class="empty"><h3>${esc(title)}</h3><p>${body}</p>${action}</div>`;
}

export function sectionLabel(text: string, extra = ""): string {
  return `<div class="section-label"><h2>${esc(text)}</h2>${extra ? `<span>${extra}</span>` : ""}</div>`;
}
