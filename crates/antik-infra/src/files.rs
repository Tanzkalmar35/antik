//! Markdown projection and the four-document import lane.
//!
//! SQLite is authoritative. Each committed mutation rewrites the project's
//! documents atomically so the folder is always readable and commit-ready.
//! Brief, note and task documents are read back on open and at launch: a task
//! document that is not in the store yet becomes a task, plan tree included.
//! A folder identifies the project, so documents written by another
//! installation are handed over and claimed by the caller.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use antik_app::dto::{PlanItemKind, PlanItemStatus, ProjectDetail, TaskDetail};
use antik_app::error::AppError;
use antik_app::ports::{
    ProjectFiles, ScannedBrief, ScannedNote, ScannedPlanItem, ScannedProject, ScannedTask,
    ScannedTaskSection, ScopedFile,
};
use antik_domain::model::{FILE_MAX_BYTES, MAX_FILES, PROMPT_MAX_CHARS};
use antik_domain::scope::Pattern;

/// The folder antik keeps inside an attached repository. One dot-directory,
/// so the tree is still yours; the walker names it explicitly.
pub const ANTIK_DIR: &str = ".antik";

/// Directory names are ULIDs, never user text, so a path cannot escape the
/// root. Anything else found on disk is ignored, not guessed at.
fn safe_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 40
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub struct MarkdownFiles {
    root: PathBuf,
}

impl MarkdownFiles {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Where antik's own documents live: `.antik` inside the repository when
    /// one is attached, the app-data folder otherwise.
    fn project_dir(&self, project: &ProjectDetail) -> PathBuf {
        match project.repo_path.as_deref() {
            Some(repo) => PathBuf::from(repo).join(ANTIK_DIR),
            None => self.root.join(&project.id),
        }
    }

    fn write_project(&self, project: &ProjectDetail) -> Result<(), AppError> {
        if !safe_segment(&project.id) {
            return Err(AppError::Io("invalid project id".to_owned()));
        }
        let dir = self.project_dir(project);
        for child in ["notes", "tasks", "decisions", "questions"] {
            fs::create_dir_all(dir.join(child)).map_err(io)?;
        }
        write_atomic(
            &dir.join("brief.md"),
            &document(
                "brief",
                &project.id,
                &project.id,
                &project.created,
                &project.name,
                &project.brief,
            ),
        )?;
        write_atomic(
            &dir.join("glossary.md"),
            &document(
                "glossary",
                &project.id,
                &project.id,
                &project.created,
                "Glossary",
                "",
            ),
        )?;

        for note in &project.notes {
            if !safe_segment(&note.id) {
                continue;
            }
            write_atomic(
                &dir.join("notes").join(format!("{}.md", note.id)),
                &document(
                    "note",
                    &note.id,
                    &project.id,
                    &note.at,
                    &note.title,
                    &note.text,
                ),
            )?;
        }

        for task in &project.tasks {
            if !safe_segment(&task.id) {
                continue;
            }
            write_atomic(
                &dir.join("tasks").join(format!("{}.md", task.id)),
                &task_document(&project.id, task),
            )?;
        }
        Ok(())
    }

    /// A deleted task's document is removed here; the caller rewrites the
    /// rest of the projection so the folder keeps matching the store.
    fn remove_task_document(&self, project: &ProjectDetail, task_id: &str) -> Result<(), AppError> {
        if !safe_segment(&project.id) || !safe_segment(task_id) {
            return Ok(());
        }
        let path = self
            .project_dir(project)
            .join("tasks")
            .join(format!("{task_id}.md"));
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io(error)),
        }
    }

    fn scan(&self, project: &ProjectDetail) -> Result<ScannedProject, AppError> {
        if !safe_segment(&project.id) {
            return Err(AppError::Io("invalid project id".to_owned()));
        }
        let dir = self.project_dir(project);
        if !dir.is_dir() {
            return Ok(ScannedProject {
                brief: None,
                notes: Vec::new(),
                tasks: Vec::new(),
            });
        }

        let brief = match read_doc(&dir.join("brief.md")) {
            Ok(Some(doc)) if doc.kind == "brief" && safe_segment(&doc.project) => {
                Some(ScannedBrief {
                    project: doc.project,
                    text: doc.body,
                })
            }
            _ => None,
        };

        let mut notes = Vec::new();
        let notes_dir = dir.join("notes");
        if let Ok(entries) = fs::read_dir(&notes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                    continue;
                }
                let Ok(Some(doc)) = read_doc(&path) else {
                    continue;
                };
                if doc.kind != "note" || !safe_segment(&doc.id) || !safe_segment(&doc.project) {
                    continue;
                }
                notes.push(ScannedNote {
                    id: doc.id,
                    project: doc.project,
                    title: doc.title,
                    text: doc.body,
                });
            }
        }

        let mut tasks = Vec::new();
        let tasks_dir = dir.join("tasks");
        if let Ok(entries) = fs::read_dir(&tasks_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                    continue;
                }
                let Ok(Some(doc)) = read_doc(&path) else {
                    continue;
                };
                if doc.kind != "task" || !safe_segment(&doc.id) || !safe_segment(&doc.project) {
                    continue;
                }
                tasks.push(parse_task_document(
                    &doc.id,
                    &doc.project,
                    &doc.title,
                    &doc.body,
                ));
            }
        }

        Ok(ScannedProject {
            brief,
            notes,
            tasks,
        })
    }
}

impl ProjectFiles for MarkdownFiles {
    /// Checks run in the attached repository when there is one, and in the
    /// project's app-data folder otherwise.
    fn project_root(&self, project: &ProjectDetail) -> Option<PathBuf> {
        match project.repo_path.as_deref() {
            Some(repo) => Some(PathBuf::from(repo)),
            None if safe_segment(&project.id) => Some(self.root.join(&project.id)),
            None => None,
        }
    }

    fn sandbox_root(&self) -> Option<PathBuf> {
        Some(self.root.clone())
    }

    fn write_project(&self, project: &ProjectDetail) -> Result<(), AppError> {
        MarkdownFiles::write_project(self, project)
    }

    fn scan(&self, project: &ProjectDetail) -> Result<ScannedProject, AppError> {
        MarkdownFiles::scan(self, project)
    }

    fn remove_task_document(&self, project: &ProjectDetail, task_id: &str) -> Result<(), AppError> {
        MarkdownFiles::remove_task_document(self, project, task_id)
    }

    /// Everything the model may read: walk the project root, keep what the
    /// allowlist names and, when the batch has a scope, only what the scope
    /// names too. Every candidate is resolved and refused if it lands outside
    /// the root (a symlink is the obvious way out).
    fn read_scope(
        &self,
        project: &ProjectDetail,
        fence: &[Pattern],
        scope: &[Pattern],
    ) -> Vec<ScopedFile> {
        if fence.is_empty() {
            return Vec::new();
        }
        let Some(root) = self.project_root(project) else {
            return Vec::new();
        };
        let Ok(root) = root.canonicalize() else {
            return Vec::new();
        };

        let mut candidates = Vec::new();
        collect(&root, 0, &mut candidates);

        let mut files = Vec::new();
        let mut budget = PROMPT_MAX_CHARS;
        for path in candidates {
            if files.len() >= MAX_FILES || budget == 0 {
                break;
            }
            let Ok(resolved) = path.canonicalize() else {
                continue;
            };
            if !resolved.starts_with(&root) {
                continue;
            }
            let Ok(relative) = path.strip_prefix(&root) else {
                continue;
            };
            let relative = relative.to_string_lossy().replace('\\', "/");
            if !antik_domain::scope::allows(fence, &relative) {
                continue;
            }
            if !scope.is_empty() && !antik_domain::scope::allows(scope, &relative) {
                continue;
            }
            let Ok(metadata) = resolved.metadata() else {
                continue;
            };
            if !metadata.is_file() || metadata.len() == 0 {
                continue;
            }
            let Ok(file) = File::open(&resolved) else {
                continue;
            };
            let mut buffer = Vec::new();
            if file
                .take(FILE_MAX_BYTES as u64)
                .read_to_end(&mut buffer)
                .is_err()
            {
                continue;
            }
            // A NUL byte means this is not text, and a prompt is not a place to
            // find out what a binary file parses as.
            if buffer.contains(&0) {
                continue;
            }
            let truncated = metadata.len() > FILE_MAX_BYTES as u64;
            let text = String::from_utf8_lossy(&buffer).into_owned();
            let fitted: String = text.chars().take(budget).collect();
            budget = budget.saturating_sub(fitted.chars().count());
            files.push(ScopedFile {
                path: relative,
                text: fitted,
                truncated: truncated || budget == 0,
            });
        }
        files
    }
}

/// A bounded walk. Dotfiles and build output are skipped, because a prompt is
/// not a place to read `.git` objects or a vendored tree.
fn collect(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    if depth > 8 || out.len() >= 2_000 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= 2_000 {
            return;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if (name.starts_with('.') && name != ANTIK_DIR)
            || matches!(name.as_ref(), "node_modules" | "target" | "dist" | "logs")
        {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            collect(&path, depth + 1, out);
        } else if kind.is_file() || kind.is_symlink() {
            out.push(path);
        }
    }
}

struct Document {
    kind: String,
    id: String,
    project: String,
    title: String,
    body: String,
}

fn document(kind: &str, id: &str, project: &str, created: &str, title: &str, body: &str) -> String {
    let title = one_line(title);
    format!(
        "---\nantik: 1\nid: {id}\nkind: {kind}\nproject: {project}\ncreated: {created}\ntitle: {title}\n---\n\n{}\n",
        body.replace("\r\n", "\n").trim_end()
    )
}

fn task_document(project: &str, task: &TaskDetail) -> String {
    let mut body = String::new();
    body.push_str(&format!(
        "# {}\n\n- phase: {}\n- lane: {}\n- scope: {}\n\n",
        task.title, task.phase, task.lane, task.scope
    ));

    body.push_str("## Description\n\n");
    if task.prediction.trim().is_empty() {
        body.push_str("_Not written yet._\n\n");
    } else {
        body.push_str(task.prediction.trim());
        body.push_str("\n\n");
    }

    body.push_str("## Plan\n\n");
    let sections: Vec<_> = task
        .plan_items
        .iter()
        .filter(|item| item.kind == PlanItemKind::Section)
        .collect();
    if sections.is_empty() {
        body.push_str("_No sections yet._\n\n");
    }
    for (index, section) in sections.iter().enumerate() {
        body.push_str(&format!("### {}. {}\n\n", index + 1, section.title));
        if !section.body.trim().is_empty() {
            body.push_str(section.body.trim());
            body.push_str("\n\n");
        }
        let subtasks: Vec<_> = task
            .plan_items
            .iter()
            .filter(|item| {
                item.kind == PlanItemKind::Subtask && item.parent_id.as_deref() == Some(&section.id)
            })
            .collect();
        if subtasks.is_empty() {
            body.push_str("_No subtasks yet._\n\n");
        }
        for subtask in subtasks {
            body.push_str(&format!(
                "- [{}] {}\n",
                if subtask.status == PlanItemStatus::Done {
                    "x"
                } else {
                    " "
                },
                subtask.title
            ));
            if !subtask.body.trim().is_empty() {
                for line in subtask.body.trim().lines() {
                    body.push_str(&format!("  {line}\n"));
                }
            }
        }
        body.push('\n');
    }

    body.push_str("## Definition of done\n\n");
    let dods: Vec<_> = task
        .plan_items
        .iter()
        .filter(|item| item.kind == PlanItemKind::Dod)
        .collect();
    if dods.is_empty() {
        body.push_str("_No criteria yet._\n\n");
    }
    for dod in dods {
        let backing = dod
            .check_id
            .as_deref()
            .and_then(|id| task.checks.iter().find(|check| check.id == id))
            .map(|check| format!(" — check: {}", check.label))
            .unwrap_or_else(|| " — manual".to_owned());
        body.push_str(&format!(
            "- [{}] {}{}\n",
            if dod.status == PlanItemStatus::Done {
                "x"
            } else {
                " "
            },
            dod.title,
            backing
        ));
        if !dod.body.trim().is_empty() {
            for line in dod.body.trim().lines() {
                body.push_str(&format!("  {line}\n"));
            }
        }
    }
    body.push('\n');

    body.push_str("## Discussion\n\n");
    if task.plan_messages.is_empty() {
        body.push_str("_No discussion yet._\n\n");
    }
    for message in &task.plan_messages {
        let role = match message.role {
            antik_app::dto::PlanMessageRole::User => "You",
            antik_app::dto::PlanMessageRole::Assistant => "Model",
        };
        body.push_str(&format!("**{role} · {}**\n\n", message.at));
        body.push_str(message.content.trim());
        body.push_str("\n\n");
    }

    body.push_str("## Notes\n\n");
    if task.notes.is_empty() {
        body.push_str("_No notes yet._\n\n");
    }
    for note in &task.notes {
        body.push_str(&format!("- {} · {}\n", note.at, note.text));
    }
    body.push('\n');

    if !task.attempts.is_empty() {
        body.push_str("## Attempts\n\n");
        for attempt in &task.attempts {
            body.push_str(&format!("- {} · {}\n", attempt.at, attempt.text));
        }
        body.push('\n');
    }
    if !task.gates.is_empty() {
        body.push_str("## Gates\n\n");
        for gate in &task.gates {
            body.push_str(&format!(
                "- {} · {} · {}\n",
                gate.n, gate.question, gate.status
            ));
        }
    }
    document("task", &task.id, project, &task.started, &task.title, &body)
}

/// A task document as the projection writes it. Only the parts a person edits
/// are read back; the rest is history the app owns and rewrites.
fn parse_task_document(id: &str, project: &str, title: &str, body: &str) -> ScannedTask {
    let mut task = ScannedTask {
        id: id.to_owned(),
        project: project.to_owned(),
        title: title.trim().to_owned(),
        scope: String::new(),
        description: String::new(),
        sections: Vec::new(),
        dods: Vec::new(),
    };

    // Split on `##` headings. The chunk before the first heading carries the
    // `# title` line and the three bullets the projection writes.
    let mut chunks: Vec<(String, Vec<&str>)> = vec![(String::new(), Vec::new())];
    for line in body.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            chunks.push((heading.trim().to_owned(), Vec::new()));
        } else if let Some((_, lines)) = chunks.last_mut() {
            lines.push(line);
        }
    }

    for line in &chunks[0].1 {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            if task.title.is_empty() {
                task.title = rest.trim().to_owned();
            }
        }
        if let Some(rest) = trimmed.strip_prefix("- scope:") {
            task.scope = rest.trim().to_owned();
        }
    }

    for (heading, lines) in &chunks {
        match heading.as_str() {
            "Description" => task.description = paragraph(lines),
            "Plan" => task.sections = parse_plan(lines),
            "Definition of done" => task.dods = parse_plan_items(lines),
            _ => {}
        }
    }
    task
}

fn paragraph(lines: &[&str]) -> String {
    let mut text = String::new();
    for line in lines {
        push_line(&mut text, line);
    }
    text.trim().to_owned()
}

fn parse_plan(lines: &[&str]) -> Vec<ScannedTaskSection> {
    let mut sections: Vec<ScannedTaskSection> = Vec::new();
    let mut current: Option<ScannedTaskSection> = None;
    let mut in_items = false;

    for raw in lines {
        if let Some(rest) = raw.strip_prefix("### ") {
            if let Some(section) = current.take() {
                sections.push(finish_section(section));
            }
            current = Some(ScannedTaskSection {
                title: section_title(rest),
                body: String::new(),
                subtasks: Vec::new(),
            });
            in_items = false;
            continue;
        }

        let Some(section) = current.as_mut() else {
            continue;
        };
        let trimmed = raw.trim_end();
        if let Some(item) = plan_item(trimmed) {
            section.subtasks.push(item);
            in_items = true;
            continue;
        }
        if in_items {
            // The writer indents a subtask's detail by two spaces.
            if trimmed.starts_with("  ") {
                if let Some(last) = section.subtasks.last_mut() {
                    push_line(&mut last.body, trimmed.trim_start());
                }
            }
        } else {
            push_line(&mut section.body, trimmed);
        }
    }
    if let Some(section) = current.take() {
        sections.push(finish_section(section));
    }
    sections
}

fn finish_section(mut section: ScannedTaskSection) -> ScannedTaskSection {
    section.body = section.body.trim().to_owned();
    for item in &mut section.subtasks {
        item.title = item.title.trim().to_owned();
        item.body = item.body.trim().to_owned();
    }
    section
}

/// `### 1. Measurement` carries a number that is only position, so the number
/// is dropped and the title is what stays.
fn section_title(raw: &str) -> String {
    let raw = raw.trim();
    match raw.split_once(". ") {
        Some((number, rest))
            if !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()) =>
        {
            rest.trim().to_owned()
        }
        _ => raw.to_owned(),
    }
}

fn plan_item(line: &str) -> Option<ScannedPlanItem> {
    let rest = line.strip_prefix("- [")?;
    let (mark, rest) = rest.split_once(']')?;
    let title = rest.trim();
    if title.is_empty() {
        return None;
    }
    Some(ScannedPlanItem {
        title: title.to_owned(),
        body: String::new(),
        done: mark.trim().eq_ignore_ascii_case("x"),
    })
}

fn parse_plan_items(lines: &[&str]) -> Vec<ScannedPlanItem> {
    let mut items: Vec<ScannedPlanItem> = Vec::new();
    for raw in lines {
        let trimmed = raw.trim_end();
        if let Some(mut item) = plan_item(trimmed) {
            item.title = strip_backing(&item.title).to_owned();
            items.push(item);
            continue;
        }
        if trimmed.starts_with("  ") {
            if let Some(last) = items.last_mut() {
                push_line(&mut last.body, trimmed.trim_start());
            }
        }
    }
    for item in &mut items {
        item.body = item.body.trim().to_owned();
    }
    items
}

/// The projection writes `— check: <label>` or `— manual` after a definition of
/// done. The label is not an id, so it is dropped rather than guessed at.
fn strip_backing(title: &str) -> &str {
    match title.rsplit_once(" \u{2014} ") {
        Some((head, tail)) if tail.starts_with("check:") || tail.trim() == "manual" => {
            head.trim_end()
        }
        _ => title,
    }
}

fn push_line(target: &mut String, line: &str) {
    let line = line.trim_end();
    let bare = line.trim();
    if matches!(
        bare,
        "_Not written yet._" | "_No sections yet._" | "_No subtasks yet._" | "_No criteria yet._"
    ) {
        return;
    }
    if bare.is_empty() && target.trim().is_empty() {
        return;
    }
    if !target.is_empty() {
        target.push('\n');
    }
    target.push_str(line);
}

fn one_line(value: &str) -> String {
    value.replace(['\r', '\n'], " ").trim().to_owned()
}

fn read_doc(path: &Path) -> Result<Option<Document>, AppError> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io(error)),
    };
    let raw = raw
        .strip_prefix('\u{feff}')
        .unwrap_or(&raw)
        .replace("\r\n", "\n");
    let mut lines = raw.lines();
    if lines.next() != Some("---") {
        return Ok(None);
    }
    let mut fields = std::collections::HashMap::new();
    let mut closed = false;
    let mut consumed = 1;
    for line in raw.lines().skip(1) {
        consumed += 1;
        if line == "---" {
            closed = true;
            break;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Ok(None);
        };
        fields.insert(key.trim().to_owned(), value.trim().to_owned());
    }
    if !closed || fields.get("antik").map(String::as_str) != Some("1") {
        return Ok(None);
    }
    let body = raw
        .lines()
        .skip(consumed)
        .collect::<Vec<_>>()
        .join("\n")
        .trim_matches('\n')
        .to_owned();

    let Some(kind) = fields.remove("kind") else {
        return Ok(None);
    };
    let Some(id) = fields.remove("id") else {
        return Ok(None);
    };
    let Some(project) = fields.remove("project") else {
        return Ok(None);
    };

    Ok(Some(Document {
        kind,
        id,
        project,
        title: fields.remove("title").unwrap_or_default(),
        body,
    }))
}

fn write_atomic(path: &Path, contents: &str) -> Result<(), AppError> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Io("document has no parent".to_owned()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| AppError::Io("document has no name".to_owned()))?;
    let tmp = parent.join(format!(".{name}.tmp"));
    {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp)
            .map_err(io)?;
        file.write_all(contents.as_bytes()).map_err(io)?;
        file.sync_all().map_err(io)?;
    }
    fs::rename(&tmp, path).map_err(io)?;
    if let Ok(dir) = File::open(parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}

fn io(error: std::io::Error) -> AppError {
    AppError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_id() -> String {
        ulid::Ulid::generate().to_string()
    }

    fn empty_project(id: &str) -> ProjectDetail {
        ProjectDetail {
            id: id.to_owned(),
            name: "kestrel".to_owned(),
            brief: "A local event log.".to_owned(),
            repo_path: None,
            created: "2026-11-21T10:00:00Z".to_owned(),
            current_task_id: None,
            tasks: Vec::new(),
            notes: Vec::new(),
            decisions: Vec::new(),
            questions: Vec::new(),
            handoff: None,
            handoff_locked: false,
            recall: None,
            revision: 1,
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_read_scope_is_the_allowlist_and_nothing_outside_the_project() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "hidden").unwrap();

        let id = project_id();
        let project = dir.path().join(&id);
        fs::create_dir_all(project.join("src/queue")).unwrap();
        fs::create_dir_all(project.join("src/billing")).unwrap();
        fs::write(project.join("src/queue/flush.rs"), "fn flush() {}").unwrap();
        fs::write(project.join("src/billing/invoice.rs"), "fn bill() {}").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.txt"),
            project.join("src/queue/escape.rs"),
        )
        .unwrap();

        let files = MarkdownFiles::new(dir.path());
        let detail = empty_project(&id);
        let patterns: Vec<Pattern> = ["src/queue/**"]
            .iter()
            .filter_map(|raw| Pattern::parse(raw))
            .collect();
        let scoped = files.read_scope(&detail, &patterns, &[]);

        assert_eq!(scoped.len(), 1, "one allowlisted text file");
        assert_eq!(scoped[0].path, "src/queue/flush.rs");
        assert!(!scoped.iter().any(|file| file.text.contains("hidden")));
        assert!(
            !scoped.iter().any(|file| file.path.contains("billing")),
            "an allowlisted project folder that is not named stays out"
        );

        // An empty allowlist opens nothing at all.
        assert!(files.read_scope(&detail, &[], &[]).is_empty());
    }

    #[test]
    fn an_attached_repository_holds_the_documents_and_the_scope_stays_inside_it() {
        let data = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join("src/queue")).unwrap();
        fs::write(repo.path().join("src/queue/flush.rs"), "fn flush() {}").unwrap();
        fs::write(repo.path().join("README.md"), "outside the batch").unwrap();

        let files = MarkdownFiles::new(data.path());
        let mut project = empty_project(&project_id());
        project.repo_path = Some(repo.path().to_string_lossy().into_owned());

        files.write_project(&project).unwrap();
        assert!(repo.path().join(".antik/brief.md").is_file());
        assert!(repo.path().join(".antik/tasks").is_dir());
        assert!(
            !data.path().join(&project.id).exists(),
            "with a repository attached the app-data folder is not written"
        );

        let fence: Vec<Pattern> = ["**"]
            .iter()
            .filter_map(|raw| Pattern::parse(raw))
            .collect();
        let scope = antik_domain::scope::parse_scope("src/queue/**");
        let scoped = files.read_scope(&project, &fence, &scope);
        assert_eq!(scoped.len(), 1, "the batch scope narrows the allowlist");
        assert_eq!(scoped[0].path, "src/queue/flush.rs");

        // The documents are part of the tree, and readable when named.
        let docs: Vec<Pattern> = [".antik/**"]
            .iter()
            .filter_map(|raw| Pattern::parse(raw))
            .collect();
        let scoped = files.read_scope(&project, &docs, &[]);
        assert!(scoped.iter().any(|file| file.path == ".antik/brief.md"));
    }

    #[test]
    fn writes_a_brief_and_imports_an_external_edit() {
        let dir = tempfile::tempdir().unwrap();
        let files = MarkdownFiles::new(dir.path());
        let project = empty_project(&project_id());

        files.write_project(&project).unwrap();
        let brief = dir.path().join(&project.id).join("brief.md");
        assert!(brief.is_file());

        let text = fs::read_to_string(&brief).unwrap();
        fs::write(
            &brief,
            text.replace("A local event log.", "A different brief."),
        )
        .unwrap();

        let scanned = files.scan(&project).unwrap();
        let brief = scanned.brief.expect("a brief is read back");
        assert_eq!(brief.text, "A different brief.");
        assert_eq!(brief.project, project.id);
    }

    #[test]
    fn parses_a_note_and_ignores_documents_without_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let files = MarkdownFiles::new(dir.path());
        let project = empty_project(&project_id());
        files.write_project(&project).unwrap();

        let note_id = project_id();
        fs::write(
            dir.path()
                .join(&project.id)
                .join("notes")
                .join(format!("{note_id}.md")),
            format!(
                "---\nantik: 1\nid: {note_id}\nkind: note\nproject: {}\ncreated: 2026-11-21T10:00:00Z\ntitle: Timing\n---\n\nA note body.\n",
                project.id
            ),
        )
        .unwrap();
        fs::write(
            dir.path().join(&project.id).join("notes").join("junk.md"),
            "no frontmatter",
        )
        .unwrap();

        let scanned = files.scan(&project).unwrap();
        assert_eq!(scanned.notes.len(), 1);
        assert_eq!(scanned.notes[0].title, "Timing");
        assert_eq!(scanned.notes[0].text, "A note body.");
        assert_eq!(scanned.notes[0].project, project.id);
    }

    #[test]
    fn reads_back_a_task_document_with_its_plan() {
        let dir = tempfile::tempdir().unwrap();
        let files = MarkdownFiles::new(dir.path());
        let project = empty_project(&project_id());
        files.write_project(&project).unwrap();

        let task_id = project_id();
        let path = dir
            .path()
            .join(&project.id)
            .join("tasks")
            .join(format!("{task_id}.md"));
        fs::write(
            &path,
            format!(
                "---\nantik: 1\nid: {task_id}\nkind: task\nproject: {}\ncreated: 2026-11-21T10:00:00Z\ntitle: Drain the queue\n---\n\n# Drain the queue\n\n- phase: review\n- lane: build\n- scope: src/queue/**\n\n## Description\n\nOne worker is enough.\n\n## Plan\n\n### 1. Measurement\n\nFind the slow path.\n\n- [x] Time one flush\n  Use the dev database.\n\n## Definition of done\n\n- [x] The queue drains under a second. \u{2014} check: queue-flush\n",
                project.id
            ),
        )
        .unwrap();

        let scanned = files.scan(&project).unwrap();
        assert_eq!(scanned.tasks.len(), 1);
        let task = &scanned.tasks[0];
        assert_eq!(task.id, task_id);
        assert_eq!(task.title, "Drain the queue");
        assert_eq!(task.scope, "src/queue/**");
        assert_eq!(task.description, "One worker is enough.");
        assert_eq!(task.sections.len(), 1);
        assert_eq!(task.sections[0].title, "Measurement");
        assert_eq!(task.sections[0].body, "Find the slow path.");
        assert_eq!(task.sections[0].subtasks[0].title, "Time one flush");
        assert_eq!(task.sections[0].subtasks[0].body, "Use the dev database.");
        assert!(task.sections[0].subtasks[0].done);
        assert_eq!(task.dods[0].title, "The queue drains under a second.");
        assert!(task.dods[0].done);
    }

    #[test]
    fn rejects_project_ids_that_could_escape_the_root() {
        let dir = tempfile::tempdir().unwrap();
        let files = MarkdownFiles::new(dir.path());
        let project = empty_project("../../etc");
        assert!(files.write_project(&project).is_err());
    }
}
