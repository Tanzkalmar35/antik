//! Performance guard for the M2 exit criterion: a 90-day review over 100k
//! events. Run explicitly in release:
//! `cargo test --release -p antik-infra --test review_scale -- --ignored --nocapture`

use std::time::Instant;

use antik_app::ports::Store;
use antik_infra::Db;

#[test]
#[ignore = "performance budget; run in release"]
fn review_reads_100k_events_under_budget() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("antik.db");
    let db = Db::open(&path).unwrap();

    let span = 90i64 * 86_400_000;
    let cutoff = 1_000_000i64;
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        tx.execute(
            "INSERT INTO projects (id, slug, name, root_path, created_at)
             VALUES ('p1', 'p1', 'perf', '/tmp/perf', 0)",
            [],
        )
        .unwrap();
        {
            let mut task = tx
                .prepare(
                    "INSERT INTO tasks (id, project_id, title, lane, phase, started_at)
                     VALUES (?1, 'p1', ?2, 'learn', 'verify', ?3)",
                )
                .unwrap();
            for i in 0..200 {
                task.execute(rusqlite::params![
                    format!("t{i}"),
                    format!("task {i}"),
                    cutoff + i
                ])
                .unwrap();
            }
        }
        {
            let mut event = tx
                .prepare(
                    "INSERT INTO events (id, at, kind, actor, project_id, task_id, payload, summary)
                     VALUES (?1, ?2, ?3, 'user', 'p1', ?4, ?5, 'perf')",
                )
                .unwrap();
            let kinds = [
                "task_planned",
                "handoff_written",
                "attempt_recorded",
                "gate_attempted",
            ];
            for i in 0..100_000i64 {
                let kind = kinds[(i as usize) % kinds.len()];
                let payload = if kind == "gate_attempted" {
                    "{\"pass\":true}"
                } else {
                    "{}"
                };
                event
                    .execute(rusqlite::params![
                        format!("e{i}"),
                        cutoff + i * span / 100_000,
                        kind,
                        format!("t{}", i / 500),
                        payload
                    ])
                    .unwrap();
            }
        }
        tx.commit().unwrap();
    }

    // Warm the page cache and the prepared read model. The budget is a warm
    // p95, not the first read through a freshly written WAL.
    db.review(90, cutoff).unwrap();
    let start = Instant::now();
    let review = db.review(90, cutoff).unwrap();
    let elapsed = start.elapsed();
    eprintln!("review over 100k events: {elapsed:?}");

    assert_eq!(review.tasks, 200);
    assert_eq!(review.rows.len(), 200);
    assert_eq!(review.rework, 25_000);
    assert_eq!(review.resolved_gates, 25_000);
    assert_eq!(review.gate_pass_rate, Some(100));
    assert!(review.lead_time_days.is_some());
    assert!(review.from.ends_with('Z'));

    if !cfg!(debug_assertions) {
        assert!(
            elapsed < std::time::Duration::from_millis(15),
            "90-day review took {elapsed:?}, budget is 15ms"
        );
    }
}
