//! Opt-in SQL baseline on an isolated generated library; never opens user data.
use super::*;
use serde_json::json;
use std::time::Instant;

#[test]
#[ignore = "large synthetic SQLite workload; FONICCA_LIBRARY_BENCH_OUTPUT=/tmp/library.json"]
fn library_workload_benchmark() {
    let root = tempfile::tempdir().unwrap();
    let reference = std::env::var_os("FONICCA_LIBRARY_BENCH_REFERENCE").is_some();
    let mut scenarios = Vec::new();
    for size in [32_000, 100_000] {
        let mut conn = crate::db::open(&root.path().join(format!("library-{size}.db"))).unwrap();
        crate::db::migrate(&mut conn).unwrap();
        if reference { conn.execute_batch("DROP INDEX samples_natural_name").unwrap(); }
        let transaction = conn.transaction().unwrap();
        transaction.execute("INSERT INTO sources(id,path,added_at) VALUES(1,'/fixture',0)", []).unwrap();
        {
            let mut insert = transaction.prepare("INSERT INTO samples(source_id,path,name,ext,size,mtime,duration,sample_rate,channels,category,bpm) VALUES(1,?1,?2,'wav',40000,0,0.5,44100,2,?3,120)").unwrap();
            for i in 0..size {
                let category = if i % 2 == 0 { "kick" } else { "snare" };
                insert.execute(params![format!("/fixture/Pack {}/{} {i}.wav", i % 100, category), format!("{category} {i}"), category]).unwrap();
            }
        }
        transaction.commit().unwrap();
        if std::env::var_os("FONICCA_LIBRARY_BENCH_RELATIONS").is_some() {
            conn.execute_batch("INSERT INTO tags(id,name) VALUES(1,'fixture');
                INSERT INTO sample_tags SELECT id,1 FROM samples WHERE id % 3 = 0;
                INSERT INTO sample_stems(child_id,parent_id,kind,position)
                SELECT id+1,id,'vocals',0 FROM samples WHERE id % 10 = 1 AND id < (SELECT max(id) FROM samples);").unwrap();
        }
        conn.execute_batch("ANALYZE;").unwrap();
        let filters = [
            ("all_grouped", Filter { group_stems: true, ..Filter::default() }),
            ("all_flat", Filter::default()),
            ("category", Filter { category: Some("snare".into()), group_stems: true, ..Filter::default() }),
            ("directory", Filter { source_id: Some(1), directory: Some("/fixture/Pack 2".into()), group_stems: true, ..Filter::default() }),
            ("search", Filter { query: Some("snare".into()), group_stems: true, ..Filter::default() }),
        ];
        let mut queries = Vec::new();
        for (name, mut filter) in filters {
            // Exercise the previous materialized root query with no visible
            // children. Equivalence is checked by the library regression test.
            if reference && filter.group_stems { filter.expanded_stems.push(-1); }
            let mut timings = Vec::new();
            let mut counts = Vec::new();
            let total = count(&conn, &filter).unwrap();
            for offset in [0, total / 2, (total - 200).max(0)] {
                let started = Instant::now();
                let rows = list(&conn, &filter, offset, 200).unwrap();
                timings.push(started.elapsed().as_secs_f64() * 1000.0);
                counts.push(rows.len());
            }
            let (from, args, order) = source(&filter, None);
            let sql = format!("EXPLAIN QUERY PLAN SELECT s.id {from} {order} LIMIT 200 OFFSET 0");
            let plan = conn.prepare(&sql).unwrap().query_map(params_from_iter(args_iter(&args)), |row| row.get::<_, String>(3)).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
            queries.push(json!({ "name": name, "total": total, "pageTimesMs": timings, "pageCounts": counts, "plan": plan }));
        }
        scenarios.push(json!({ "entries": size, "queries": queries }));
    }
    let report = json!({ "schema": 1, "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "relations": std::env::var_os("FONICCA_LIBRARY_BENCH_RELATIONS").is_some(),
        "reference": reference,
        "note": "Warm SQLite page timings at first/middle/last offsets, 200 rows. Query plan omits tag projection subqueries but retains filter/grouping/sort. No WebView or user database.", "scenarios": scenarios });
    let text = serde_json::to_string_pretty(&report).unwrap();
    if let Ok(path) = std::env::var("FONICCA_LIBRARY_BENCH_OUTPUT") { std::fs::write(path, &text).unwrap(); }
    println!("{text}");
}
