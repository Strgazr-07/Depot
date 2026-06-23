//! Headless reporting: run a scan to completion and print it, either as a
//! clean category-grouped table or as JSON for scripting.

use std::sync::mpsc;
use std::thread;

use serde::Serialize;

use crate::scan::{run_scan, Category, ScanItem, ScanMsg, ScanOptions};
use crate::util::{human_size, relative_age, unix_secs};

/// Run a scan synchronously and collect every item.
pub fn collect(opts: ScanOptions) -> Vec<ScanItem> {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || run_scan(opts, tx));

    let mut items = Vec::new();
    for msg in rx {
        match msg {
            ScanMsg::Item(it) => items.push(it),
            ScanMsg::Status(_) => {}
            ScanMsg::Done => break,
        }
    }
    let _ = handle.join();
    items.sort_by(|a, b| b.size.cmp(&a.size));
    items
}

/// Print results grouped by category, each group sorted largest-first, with
/// per-category subtotals and a grand total.
pub fn print_grouped(items: &[ScanItem]) {
    let grand: u64 = items.iter().map(|i| i.size).sum();

    for category in Category::ALL {
        let group: Vec<&ScanItem> = items.iter().filter(|i| i.category == category).collect();
        if group.is_empty() {
            continue;
        }
        let subtotal: u64 = group.iter().map(|i| i.size).sum();
        println!(
            "\n▍ {:<18} {:>12}   ({} item{})",
            category.title(),
            human_size(subtotal),
            group.len(),
            if group.len() == 1 { "" } else { "s" },
        );
        for it in group {
            let name = if it.count() > 1 {
                it.label.clone()
            } else {
                format!("{:<14} {}", it.kind, it.primary().display())
            };
            println!(
                "    {:>10}  {:<10} {}",
                human_size(it.size),
                relative_age(it.modified),
                name,
            );
        }
    }

    println!(
        "\n{} item(s) across {} categories · {} reclaimable",
        items.len(),
        Category::ALL
            .iter()
            .filter(|c| items.iter().any(|i| &i.category == *c))
            .count(),
        human_size(grand),
    );
}

#[derive(Serialize)]
struct ItemJson {
    category: String,
    kind: String,
    label: String,
    size: u64,
    size_human: String,
    count: usize,
    modified_unix: Option<u64>,
    paths: Vec<String>,
}

/// Print results as a JSON array (one object per item).
pub fn print_json(items: &[ScanItem]) {
    let out: Vec<ItemJson> = items
        .iter()
        .map(|it| ItemJson {
            category: it.category.label().to_string(),
            kind: it.kind.clone(),
            label: it.label.clone(),
            size: it.size,
            size_human: human_size(it.size),
            count: it.count(),
            modified_unix: it.modified.and_then(unix_secs),
            paths: it.paths.iter().map(|p| p.display().to_string()).collect(),
        })
        .collect();
    match serde_json::to_string_pretty(&out) {
        Ok(s) => println!("{s}"),
        Err(e) => eprintln!("failed to serialize: {e}"),
    }
}
