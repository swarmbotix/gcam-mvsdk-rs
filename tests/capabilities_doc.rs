//! `docs/CAPABILITIES.md` shows code copied from `examples/capabilities.rs`, which CI compiles.
//! This test keeps the two identical in both directions.

use std::path::Path;

fn read(rel: &str) -> Option<String> {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)).ok()
}

/// Code between `// [doc:name]` and `// [/doc]` markers in the example.
fn example_regions(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut current: Option<(String, Vec<&str>)> = None;
    for line in src.lines() {
        let t = line.trim();
        if let Some(name) = t.strip_prefix("// [doc:").and_then(|r| r.strip_suffix(']')) {
            current = Some((name.to_string(), Vec::new()));
        } else if t == "// [/doc]" {
            let (name, lines) = current.take().expect("[/doc] without [doc:...]");
            out.push((name, lines.join("\n")));
        } else if let Some((_, lines)) = current.as_mut() {
            lines.push(line);
        }
    }
    assert!(current.is_none(), "unterminated [doc:...] region");
    out
}

/// Bodies of the ```rust blocks in a Markdown document.
fn rust_blocks(md: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Option<Vec<&str>> = None;
    for line in md.lines() {
        match (&mut current, line.trim_end()) {
            (None, "```rust") => current = Some(Vec::new()),
            (Some(_), "```") => out.push(current.take().unwrap().join("\n")),
            (Some(lines), _) => lines.push(line),
            _ => {}
        }
    }
    out
}

#[test]
fn every_document_snippet_is_compiled_code() {
    let (Some(doc), Some(example)) = (read("docs/CAPABILITIES.md"), read("examples/capabilities.rs")) else {
        eprintln!("skipped: docs/CAPABILITIES.md or examples/capabilities.rs not present (packaged crate)");
        return;
    };
    let regions = example_regions(&example);
    let blocks = rust_blocks(&doc);
    assert!(!blocks.is_empty(), "no rust blocks in docs/CAPABILITIES.md");
    for block in &blocks {
        assert!(
            regions.iter().any(|(_, code)| code == block),
            "this snippet in docs/CAPABILITIES.md is not a [doc:...] region of examples/capabilities.rs:\n{block}"
        );
    }
    for (name, code) in &regions {
        assert!(blocks.contains(code), "example region [doc:{name}] is missing from docs/CAPABILITIES.md");
    }
}
