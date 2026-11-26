//! #14132: `file.print` takes 1 to 9999 copies, as the reference's Copies field does; a larger count becomes 9999, a
//! smaller one 1, on every backend, and Print One Copy still prints one.

use super::*;

fn session() -> Session {
    let mut s = crate::session();
    s.execute("file.new", json!({"width": 40, "height": 30, "name": "Copies"})).unwrap();
    s
}

/// Each row: the copies asked for and the copies the job sends, in the result and to `lp -n`.
#[test]
fn print_takes_1_to_9999_copies() {
    let mut s = session();
    for (asked, kept) in [(1, 1), (999, 999), (1000, 1000), (5000, 5000), (9999, 9999), (10000, 9999), (0, 1), (-3, 1)] {
        let r = s.execute("file.print", json!({"dryRun": true, "send": true, "printer": "Office", "copies": asked})).unwrap();
        assert_eq!(r["copies"], kept, "copies {asked}");
        #[cfg(unix)]
        {
            let cmd: Vec<String> = serde_json::from_value(r["command"].clone()).unwrap();
            let n = cmd.iter().position(|a| a == "-n").and_then(|at| cmd.get(at + 1));
            assert_eq!(n, Some(&kept.to_string()), "lp -n for {asked}: {cmd:?}");
        }
    }
}

#[test]
fn the_params_doc_says_1_to_9999() {
    let _installed = session();
    let doc = crate::registry::find("file.print").map(|c| c.params).unwrap_or_default();
    assert!(doc.contains(r#""copies":1..9999=1"#), "{doc}");
}

#[test]
fn print_one_copy_prints_one_after_a_large_print() {
    let mut s = session();
    assert_eq!(s.execute("file.print", json!({"dryRun": true, "copies": 5000})).unwrap()["copies"], 5000);
    assert_eq!(s.execute("file.printOneCopy", json!({"dryRun": true})).unwrap()["copies"], 1);
}
