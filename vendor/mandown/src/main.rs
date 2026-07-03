use std::io::Read;
use std::io::Write;
use std::path::Path;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args();
    let prog = args.next();
    let prog = prog.as_deref().unwrap_or("mandown");
    let source = args.next();
    let title = args.next();
    let section = args.next();
    let path_tmp;

    let (markdown, title) = match &source {
        Some(path) if !path.starts_with('-') => {
            let path = Path::new(path);
            let title = match title.as_deref() {
                None => match path.file_stem().and_then(|f| f.to_str()) {
                    Some("README") => {
                        path_tmp = path.canonicalize()?;
                        path_tmp.parent().and_then(|p| p.file_name()?.to_str())
                    },
                    x => x,
                },
                x => x,
            };

            (std::fs::read_to_string(path).map_err(|e| format!("Can't load markdown from {}: {}", path.display(), e))?, title)
        },
        Some(path) if path == "-" => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            (s, None)
        },
        _ => {
            println!("Usage: {prog} path-to-markdown.md [title] [manpage section]\n");
            println!("e.g. {prog} README.md MYCOOLPROGRAM 1 > out.1 && man ./out.1");
            println!("The path can be \"-\" to read from stdin.");
            return Ok(());
        },
    };

    let section = match section {
        Some(num) => num.parse().map_err(|e| format!("The section argument must be a number: {e}"))?,
        None => 1,
    };

    std::io::stdout().write_all(
        mandown::convert(&markdown, title.unwrap_or(""), section).as_bytes(),
    )?;
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
