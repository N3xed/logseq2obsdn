# Convert logseq files to obsidian

1. Install the [Rust programming language](https://www.rust-lang.org/tools/install).
2. Run 
    ```bash
    cargo run -- "<logseq-dir>/pages" "<obsidian-vault-dir>" --extract-ids
    ```
   - This will extract all IDs and create a `ids.json` file in the current directory.
3. Run 
   ```bash
   cargo run -- "<logseq-dir>/pages/<file>" "<obsidian-vault-dir>"
   ```
   for each `<file>` in the `pages` dir.
