mod parser;
mod trie_builder;
mod perf;

use parser::{DictEntry, Parser};
use std::fs;
use std::path::PathBuf;
use trie_builder::{build_trie, serialize_trie};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: dict-compiler <output.dict> <input1.dict.yaml> [input2.dict.yaml ...]");
        std::process::exit(1);
    }

    let output_path = PathBuf::from(&args[1]);
    let mut all_entries: Vec<DictEntry> = Vec::new();

    perf::init();
    perf::phase("parse_start");

    for input_path in &args[2..] {
        let content = fs::read_to_string(input_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", input_path, e));
        let entries = Parser::parse(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", input_path, e));
        all_entries.extend(entries);
    }

    perf::phase("parse_end");
    println!("Parsed {} entries from {} files", all_entries.len(), args.len() - 2);

    // Build trie
    perf::phase("build_start");
    let trie = build_trie(&all_entries);
    perf::phase("build_end");

    // Serialize to output file
    perf::phase("serialize_start");
    let mut out_file = fs::File::create(&output_path)
        .unwrap_or_else(|e| panic!("Failed to create {}: {}", output_path.display(), e));
    serialize_trie(&trie, &mut out_file, &all_entries)
        .unwrap_or_else(|e| panic!("Failed to write dict: {}", e));
    perf::phase("serialize_end");

    perf::finalize(0, all_entries.len());

    println!("Written dict to {} ({} entries)",
        output_path.display(), all_entries.len());
}
