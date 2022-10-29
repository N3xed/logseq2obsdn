use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use page::RefsFile;

use crate::page::Ref;

mod page;

#[derive(Parser)]
struct Args {
    file_or_folder: PathBuf,
    vault: PathBuf,
    #[clap(long)]
    extract_ids: bool,
}

struct Data {
    page_dir: PathBuf,
    out_vault: PathBuf,
    out_file: Option<(PathBuf, String)>,
    files: Vec<(PathBuf, PathBuf)>,
    refs_file: RefsFile,
}

impl page::Data for Data {
    fn copy_asset(&mut self, path: &str) -> String {
        let src = self.page_dir.join(path).canonicalize().unwrap();
        let dest: PathBuf = Path::new(path).components().skip(1).collect();

        let result = dest.display().to_string().replace("\\", "/");

        self.files.push((src, dest));
        result
    }

    fn page_title(&mut self, title: &str) {
        let title = title.trim();
        let vault_relative_path = title.to_owned();
        self.out_file = Some((
            self.out_vault.join(&format!("{vault_relative_path}.md")),
            vault_relative_path,
        ));
    }

    fn register_id(&mut self, id: &page::Id) {
        let obsdn_file = &self.out_file.as_ref().unwrap().1;
        if obsdn_file.is_empty() {
            return;
        }

        let obsdn_id = &id.obsdn_id;

        let hash = obsdn_id.starts_with('^').then_some("#").unwrap_or_default();

        self.refs_file.refs.insert(
            id.logseq_id.clone(),
            Ref {
                file: obsdn_file.to_owned(),
                id: format!("{hash}{obsdn_id}"),
            },
        );
    }

    fn query_id(&self, logseq_id: &str) -> Option<&Ref> {
        self.refs_file.refs.get(logseq_id)
    }
    fn curr_title(&self) -> &str {
        self.out_file
            .as_ref()
            .map(|(_, n)| n.as_str())
            .unwrap_or_default()
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    let ids_file = "ids.json";
    let refs_file = std::fs::File::open(ids_file)
        .map_err(anyhow::Error::from)
        .and_then(|f| Ok(serde_json::from_reader::<_, RefsFile>(BufReader::new(f))?))
        .unwrap_or_default();

    let mut data = Data {
        page_dir: Path::new(&args.file_or_folder).parent().unwrap().to_owned(),
        out_vault: args.vault.clone(),
        out_file: None,
        files: vec![],
        refs_file,
    };

    if args.extract_ids {
        for f in std::fs::read_dir(&args.file_or_folder)
            .with_context(|| anyhow!("Could not read dir '{}'", &args.file_or_folder.display()))?
        {
            let entry = f?;
            if !entry.path().extension().map(|e| e == "md").unwrap_or(false) {
                continue;
            }
            eprintln!("Extracting ids from '{}'", entry.path().display());
            let page_file = std::fs::read_to_string(entry.path())?;
            page::Page::parse(&entry.path(), &page_file, &mut data)?;
        }

        let ids_w = BufWriter::new(std::fs::File::create(ids_file)?);
        serde_json::to_writer_pretty(ids_w, &data.refs_file)?;
    } else {
        let page_file = std::fs::read_to_string(&args.file_or_folder)?;
        let mut page = page::Page::parse(&args.file_or_folder, &page_file, &mut data)?;
        page.transform(&mut data);

        println!("{:#?}", page);
        println!("{:#?}", data.files);

        let out_file_path = &data.out_file.as_ref().unwrap().0;
        let out_dir = out_file_path.parent().unwrap();

        let mut file = std::fs::File::create(&out_file_path)?;
        file.write_all(page.to_string().as_bytes())?;

        for (src, dest) in data.files {
            let dest = out_dir.join(dest);
            std::fs::copy(&src, &dest)
                .with_context(|| anyhow!("copy {} -> {}", src.display(), dest.display()))?;
        }
    }

    Ok(())
}
