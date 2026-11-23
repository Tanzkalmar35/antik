---
name: antik
description: A local-first learning harness that keeps the developer's own record of the work.
colors:
  page: "#fbfbf9"
  surface: "#ffffff"
  surface-muted: "#f6f6f3"
  rail: "#f4f4f1"
  ink: "#1c1c1f"
  ink-secondary: "#56565d"
  ink-tertiary: "#76767e"
  ink-hover: "#33333a"
  rule: "#e7e6e1"
  rule-strong: "#d7d6d0"
  owed: "#b23a27"
  owed-wash: "#fbeeea"
  wash: "#f1f0ec"
  overlay: "rgba(20, 20, 22, 0.32)"
typography:
  display:
    fontFamily: "Geist, ui-sans-serif, system-ui, sans-serif"
    fontSize: "25px"
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: "-0.022em"
  phone:
    fontFamily: "Geist, ui-sans-serif, system-ui, sans-serif"
    fontSize: "22px"
    fontWeight: 600
    lineHeight: 1.2
  title:
    fontFamily: "Geist, ui-sans-serif, system-ui, sans-serif"
    fontSize: "19px"
    fontWeight: 600
    lineHeight: 1.35
  heading:
    fontFamily: "Geist, ui-sans-serif, system-ui, sans-serif"
    fontSize: "17px"
    fontWeight: 600
    lineHeight: 1.45
  lede:
    fontFamily: "Geist, ui-sans-serif, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 400
    lineHeight: 1.6
  prose:
    fontFamily: "Geist, ui-sans-serif, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.68
  body:
    fontFamily: "Geist, ui-sans-serif, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.55
  control:
    fontFamily: "Geist, ui-sans-serif, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 500
    lineHeight: 1.4
  meta:
    fontFamily: "Geist, ui-sans-serif, system-ui, sans-serif"
    fontSize: "11.5px"
    fontWeight: 400
    lineHeight: 1.6
  label:
    fontFamily: "Geist, ui-sans-serif, system-ui, sans-serif"
    fontSize: "11px"
    fontWeight: 600
    letterSpacing: "0.07em"
  micro:
    fontFamily: "Geist Mono, ui-monospace, monospace"
    fontSize: "10.5px"
    fontWeight: 400
    lineHeight: 1.6
  data:
    fontFamily: "Geist Mono, ui-monospace, monospace"
    fontSize: "11.5px"
    fontWeight: 400
    lineHeight: 1.6
rounded:
  sm: "6px"
  md: "8px"
  lg: "10px"
  xl: "12px"
  pill: "99px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "18px"
  xl: "22px"
  page: "40px"
components:
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.surface}"
    rounded: "{rounded.md}"
    height: "32px"
    padding: "0 12px"
    fontSize: "12px"
    fontWeight: 500
  button-secondary:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink-secondary}"
    borderColor: "{colors.rule-strong}"
    rounded: "{rounded.md}"
    height: "32px"
  input:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    borderColor: "{colors.rule-strong}"
    rounded: "{rounded.md}"
    height: "34px"
    padding: "0 10px"
  stamp:
    backgroundColor: "{colors.surface-muted}"
    textColor: "{colors.ink-secondary}"
    rounded: "{rounded.pill}"
    height: "20px"
    padding: "0 8px"
    fontSize: "10.5px"
    fontWeight: 500
  card:
    backgroundColor: "{colors.surface}"
    borderColor: "{colors.rule-strong}"
    rounded: "{rounded.lg}"
    padding: "18px 20px"
    boxShadow: "0 1px 2px rgba(24, 24, 20, 0.06)"
  nav-active:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
    boxShadow: "0 1px 2px rgba(24, 24, 20, 0.06)"
---

# Design System: antik

## Overview

**Creative North Star: "The Quiet Ledger"**

antik is a tool used between sessions, at a desk, under office light, for ten to forty minutes at a time. The interface should disappear into that work: a calm neutral ground, one clear next action, and state written as a short pill on the exact thing it describes. It is a working surface, not a document about the work.

The predecessor system set the app as a letterpress document — serif headlines, tracked mono labels, numbered sections, uncoated paper. It read as a design study. This system keeps the honesty of that idea (one record, state on the thing itself, nothing inflated into a dashboard) and trades the paper metaphor for the composure of daily software: one sans face, hairline structure, rounded controls, a single raised card where attention belongs.

**Key Characteristics:**
- Warm-neutral page, white raised surfaces, one signal red for what is owed.
- Geist for every interface voice; Geist Mono only for machine-authored values.
- 6–10px radii, 1px hairlines, one small shadow, no gradients.
- Compact rows and tables over card grids; the gate is the only raised block.
- Light and dark grounds both fully themed, including selection, caret, scrollbar and focus.

## Colors

### Primary
- **Ink** (#1c1c1f): all primary text, the primary button, the active nav label, code keywords.
- **Signal** (#b23a27): a gate that is due, a claim that is unmet, a failing check, an override, a project with a due gate. Never decorative.

### Neutral
- **Page** (#fbfbf9): the work surface behind everything.
- **Surface** (#ffffff): the top bar, the gate card, inputs, the palette, active nav pills.
- **Surface Muted** (#f6f6f3): state pills, code grounds, neutral banners.
- **Rail** (#f4f4f1): the navigation column, one shade off the page.
- **Secondary Ink** (#56565d) and **Tertiary Ink** (#76767e): body-adjacent text and meta, both at or above 4.5:1 on the page.
- **Rule** (#e7e6e1) and **Strong Rule** (#d7d6d0): row dividers and the boundaries of real controls.
- **Wash** (#f1f0ec): hover fills and the phase spine track.

### Dark Ground
Page #0f0f11, surface #17171a, rail #121214, muted #1e1e21, ink #ecedef, secondary #b4b4bb, tertiary #94949c, rules #26262a and #33333a, signal #e9856f. The toggle persists locally; a first run follows the system.

### Named Rules
**The Signal Rule.** The red is only ever owed, failing, due or overridden: a due gate, a not-met claim, a bypass, a project carrying a due gate. A passed check is ink on muted grey. If something needs emphasis without state, it gets weight, spacing or a rule.

**The Four To One Rule.** Every ink and every state pill is checked on both grounds; secondary text is never lighter than #76767e on the page.

## Typography

**Interface Face:** Geist (with ui-sans-serif, system-ui, sans-serif)
**Data Face:** Geist Mono (with ui-monospace, monospace)

**Character:** One workhorse sans carries headings, prose, controls and labels, so the interface has a single voice. Mono is reserved for values a machine wrote or checked: paths, timestamps, keys, counts, code.

### Hierarchy
- **Display** (600, 25px, 1.2, -0.022em): the page title; one per screen. 22px at phone width.
- **Title** (600, 19px, 1.35): the current task line.
- **Heading** (600, 17px, 1.45): the gate question and the one large figure in a metrics table.
- **Lede** (400, 15px, 1.6, secondary ink): the opening sentence of a section.
- **Prose** (400, 14px, 1.68, secondary ink): briefs, explanations, 64–68ch measure.
- **Body** (400, 13px, 1.55): table cells, list rows, answers, note text.
- **Control** (500, 12px): buttons, navigation, segmented items, small links.
- **Section label** (600, 11px, 0.07em, uppercase, tertiary ink): the head of a region, with a count or state at the right.
- **Meta** (400, 11.5px, tertiary ink): bylines, helper text, field labels.
- **Micro** (400, 10.5px, mono, tabular): timestamps, keys, durations, stamps.
- **Data** (400, 11.5px, mono, tabular): paths, code listings, figures.

### Named Rules
**The Machine-Value Rule.** Mono appears only where a value is machine-authored or machine-checked. Interface labels, helper sentences and navigation are always Geist.

## Layout

A 224px rail and a 48px top bar frame a single content column: 780px for a task document, 1080px for tables and the glossary, capped at 1180px overall with 40px page padding. Content scrolls as one page; the rail is chrome, not a second viewport.

Regions are separated by 22px of vertical padding and a 1px rule, never by boxes. The phase spine is a four-segment control on a wash track. At 900px the rail becomes a horizontal strip and the crumbs leave the top bar; at 760px the gate is ordered first on the task document; at 640px tables tighten and the delete affordance on notes becomes permanently visible.

## Elevation & Depth

One shadow scale, used only where an element is genuinely raised: the active nav pill, the gate card, the inline create form, and the search palette. Everything else is flat: paper tone plus a hairline. No gradients, no glass except a 2px backdrop blur under the palette overlay.

## Shapes

Controls, inputs and pills are rounded: 6px for small nav and chips, 8px for buttons and fields, 10px for cards and the phase track, fully round for state pills and the project dot. Rules are 1px; focus is a 2px ink outline with a 2px offset, and fields show a 3px soft ring instead.

## Components

### Buttons
- **Primary:** ink fill, white label, 32px high, 8px radius, 12.5px/500 Geist. Hover lightens the fill.
- **Secondary:** white fill, strong rule, secondary ink; hover fills with wash.
- **Small:** 27px for table-row actions.
- **Links:** underlined only on hover, with a 3px offset; `.small-link` keeps a hairline underline because it is an action in prose.

### Inputs
- White ground, 1px strong rule, 8px radius, 34px high, 13px Geist.
- Focus replaces the border with ink plus a 3px soft ring; placeholders are tertiary ink.
- Textareas keep 13.5px/1.6 and a 64px minimum for answers.

### Navigation
- 30px rows at 12.5px, quiet secondary ink, no numbering, no tracked caps. The active row is a white pill with a small shadow; the current project is the same pill.
- A 6px signal dot at the right edge of a project means that project has a due gate.

### Stamps
- 20px pills, 10.5px/500, no uppercase, a 5px dot before the word.
- Ink on muted for met; signal on signal wash for unmet/due; dashed border for weak/overridden; tertiary outline for idle.

### Gate
- The only raised card in the document: white, strong rule, 10px radius, 18/20px padding, one soft shadow.
- Anatomy: gate number, status pill and "Scored by the app" on one line; 17px question; answer field; primary action; override disclosure. The evaluated result renders inside the card, never as a toast.

### Tables
- Header: 10.5px/600 uppercase tertiary, strong rule below. Rows: 13px, hairline rules, white hover fill.
- Numbers are tabular; `cell-value` is the one place a figure reaches 17px.

## Do's and Don'ts

### Do:
- **Do** put state on the sentence or row it describes, as a pill with a time where one exists.
- **Do** keep the gate's action in the first viewport at 1440×900 and 390×844.
- **Do** use spacing and hairlines for structure; save the shadow for raised surfaces.
- **Do** label synthetic data where the mock could be mistaken for real output.
- **Do** check both grounds, including selection, caret, scrollbar and focus.
- **Do** keep the notebook free of gates, scoring and model output.
- **Do** use one accent: signal red, and only for what is owed.

### Don't:
- **Don't** reintroduce numbered sections, tracked mono labels or serif interface text.
- **Don't** make the red mean anything other than owed, failing, due or overridden.
- **Don't** add a second accent colour, gradients, glass, or decoration in place of content.
- **Don't** stack cards or nest them; the gate is the only raised block on a work document.
- **Don't** set interface copy in mono, or use mono as a costume for "technical".
- **Don't** add a productivity score, streak, progress ring or badge count.
- **Don't** put the model's output above the developer's attempt, or let a generated pane become the centre of the screen.
