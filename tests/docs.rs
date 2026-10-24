// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The engine's papers in order: llms.txt in llmstxt.org's shape, every relative link in the docs
//! leads to a file, and every source file carries the licence's header.

use std::path::{Path, PathBuf};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The targets of a text's Markdown links that are files here (not web addresses or anchors).
fn local_links(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("](") {
        let after = &rest[start + 2..];
        let Some(end) = after.find(')') else { break };
        let target = after[..end].split('#').next().unwrap_or("").trim();
        if !target.is_empty() && !target.contains("://") && !target.starts_with("mailto:") {
            out.push(target.to_string());
        }
        rest = &after[end..];
    }
    out
}

/// The files a text's HTML shows (`src="…"` and `srcset="…"`) that are here, not on the web.
fn local_sources(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["src=\"", "srcset=\""] {
        let mut rest = text;
        while let Some(start) = rest.find(key) {
            let after = &rest[start + key.len()..];
            let Some(end) = after.find('"') else { break };
            let target = after[..end].split_whitespace().next().unwrap_or("");
            if !target.is_empty() && !target.contains("://") {
                out.push(target.to_string());
            }
            rest = &after[end..];
        }
    }
    out
}

#[test]
fn llms_txt_has_its_shape() {
    let text = std::fs::read_to_string(root().join("llms.txt")).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "# Polter", "a title first");
    assert!(lines[2].starts_with("> "), "then a summary in a quote");
    assert_eq!(text.matches("\n# ").count(), 0, "one title only");
    assert!(local_links(&text).len() >= 5);
}

#[test]
fn the_docs_links_lead_somewhere() {
    let mut broken = Vec::new();
    for doc in [
        "llms.txt",
        "README.md",
        "API.md",
        "SPEC.md",
        "RESEARCH.md",
        "CHANGELOG.md",
        "CONTRIBUTING.md",
    ] {
        let Ok(text) = std::fs::read_to_string(root().join(doc)) else {
            // a package carries some of them only
            continue;
        };
        for target in local_links(&text) {
            if !root().join(&target).exists() {
                broken.push(format!("{doc} → {target}"));
            }
        }
        // the pictures (a package doesn't carry the art folder)
        if root().join("art").exists() {
            for target in local_sources(&text) {
                if !root().join(&target).exists() {
                    broken.push(format!("{doc} shows {target}"));
                }
            }
        }
    }
    assert!(
        broken.is_empty(),
        "links to nothing:\n{}",
        broken.join("\n")
    );
}

#[test]
fn the_api_counts_what_the_code_has() {
    // numbers in API.md bound to the code (it once said 43 keys when there were 62)
    let Ok(api) = std::fs::read_to_string(root().join("API.md")) else {
        return;
    };
    for said in [
        format!("{} keys", polter::Key::ALL.len()),
        format!("{} buttons of its own", polter::PadButton::ALL.len()),
    ] {
        assert!(api.contains(&said), "API.md should say {said:?}");
    }
}

#[test]
fn the_readme_shows_the_example_as_it_is() {
    // as checked out: on Windows git may have turned every line's end into \r\n
    let read = |path: &str| {
        std::fs::read_to_string(root().join(path))
            .unwrap()
            .replace("\r\n", "\n")
    };
    let code = read("examples/glow.rs");
    let start =
        code.find("// readme: start\n").expect("the start mark") + "// readme: start\n".len();
    let end = code.find("// readme: end").expect("the end mark");
    let shown = format!("```rust\n{}```", &code[start..end]);
    let readme = read("README.md");
    assert!(
        readme.contains(&shown),
        "README.md should show examples/glow.rs between its marks, as it is"
    );
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() && name != "target" && !name.starts_with('.') {
            rust_files(&p, out);
        } else if name.ends_with(".rs") {
            out.push(p);
        }
    }
}

#[test]
fn every_source_file_carries_the_licence() {
    const HEADER: &str =
        "// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.";
    let mut files = Vec::new();
    for dir in ["src", "tests", "dev", "carts/lantern", "carts/testcard"] {
        rust_files(&root().join(dir), &mut files);
    }
    let without: Vec<String> = files
        .iter()
        .filter(|f| {
            !std::fs::read_to_string(f)
                .unwrap_or_default()
                .starts_with(HEADER)
        })
        .map(|f| f.strip_prefix(root()).unwrap_or(f).display().to_string())
        .collect();
    assert!(files.len() > 30, "only {} files", files.len());
    assert!(
        without.is_empty(),
        "without the MPL header:\n{}",
        without.join("\n")
    );
}
