mod parser;
mod trie_builder;

use parser::{DictEntry, Parser};
use std::fs;
use std::path::PathBuf;
use trie_builder::DoubleArrayTrie;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: dict-compiler <output.dict> <input1.dict.yaml> [input2.dict.yaml ...]");
        std::process::exit(1);
    }

    let output_path = PathBuf::from(&args[1]);
    let mut all_entries: Vec<DictEntry> = Vec::new();

    for input_path in &args[2..] {
        let content = fs::read_to_string(input_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", input_path, e));
        let entries = Parser::parse(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", input_path, e));
        all_entries.extend(entries);
    }

    println!("Parsed {} entries from {} files", all_entries.len(), args.len() - 2);

    // Build trie
    let trie = DoubleArrayTrie::build(&all_entries);

    // Serialize to output file
    let mut out_file = fs::File::create(&output_path)
        .unwrap_or_else(|e| panic!("Failed to create {}: {}", output_path.display(), e));
    trie.serialize(&mut out_file, &all_entries)
        .unwrap_or_else(|e| panic!("Failed to write dict: {}", e));

    println!("Written dict to {} ({} entries, {} trie nodes)",
        output_path.display(), all_entries.len(), trie.len());
}
