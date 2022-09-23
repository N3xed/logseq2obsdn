use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

mod page;

#[derive(Parser)]
struct Args {
    file: PathBuf,
    vault: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    let page_file = std::fs::read_to_string(args.file)?;
    
    let page = page::Page::parse(&page_file)?;
    
    println!("{:#?}", page);

    Ok(())
}
