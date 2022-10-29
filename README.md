# Convert logseq files to obsidian
**Use at your own risk!**

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

## Notes

The script gets the name of the converted file from the `title:: <Title>` property at the beginning of the logseq file (this property only exists if the file is in a namespace), or from the file name otherwise. It copies all assets of the logseq file into the `assets` subdirectory of the destination folder. **The assets folder must be created manually, otherwise the script will fail to copy the assets.**

A block that is tagged with `#.self-border` (see [logtools](https://github.com/cannibalox/logtools)) will be converted to a callout (using the obsidian Admonition extension) in the form:

````
```ad-def
**<Tagged block>**

<sub blocks>
```

````
