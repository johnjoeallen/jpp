use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use jpp_diagnostics::{Diagnostic, JppResult, SourceLocation};
use jpp_generator::generate_java;
use jpp_parser::parse_source;

fn main() {
    if let Err(diagnostic) = run(env::args().skip(1).collect()) {
        eprintln!("{diagnostic}");
        std::process::exit(1);
    }
}

fn run(args: Vec<String>) -> JppResult<()> {
    match args.as_slice() {
        [command, input, output] if command == "generate" => {
            generate(Path::new(input), Path::new(output))
        }
        [command, output] if command == "clean" => clean(Path::new(output)),
        [command, input] if command == "validate" => validate(Path::new(input)),
        [input, output] => generate(Path::new(input), Path::new(output)),
        _ => Err(Diagnostic::new(
            "JPP0001",
            usage(),
            SourceLocation::new(None, 1, 1),
        )),
    }
}

fn usage() -> &'static str {
    "Usage:\n  jpp <input-dir> <output-dir>\n  jpp generate <input-dir> <output-dir>\n  jpp clean <output-dir>\n  jpp validate <input-dir>"
}

fn generate(input: &Path, output: &Path) -> JppResult<()> {
    for file in find_jpp_files(input)? {
        let source = read_to_string(&file)?;
        let parsed = parse_source(Some(&file), &source)?;
        let java = generate_java(&parsed)?;
        let relative = file.strip_prefix(input).unwrap_or(&file);
        let destination = output.join(relative).with_extension("java");

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|err| io_error("JPP0002", parent, err))?;
        }

        fs::write(&destination, java).map_err(|err| io_error("JPP0003", &destination, err))?;
    }

    Ok(())
}

fn validate(input: &Path) -> JppResult<()> {
    for file in find_jpp_files(input)? {
        let source = read_to_string(&file)?;
        let parsed = parse_source(Some(&file), &source)?;
        generate_java(&parsed)?;
    }

    Ok(())
}

fn clean(output: &Path) -> JppResult<()> {
    if output.exists() {
        fs::remove_dir_all(output).map_err(|err| io_error("JPP0004", output, err))?;
    }

    Ok(())
}

fn find_jpp_files(input: &Path) -> JppResult<Vec<PathBuf>> {
    let mut files = Vec::new();

    if input.is_file() {
        if input.extension().and_then(|ext| ext.to_str()) == Some("jpp") {
            files.push(input.to_path_buf());
        }
        return Ok(files);
    }

    visit(input, &mut files)?;
    files.sort();
    Ok(files)
}

fn visit(path: &Path, files: &mut Vec<PathBuf>) -> JppResult<()> {
    for entry in fs::read_dir(path).map_err(|err| io_error("JPP0005", path, err))? {
        let entry = entry.map_err(|err| io_error("JPP0005", path, err))?;
        let path = entry.path();

        if path.is_dir() {
            visit(&path, files)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("jpp") {
            files.push(path);
        }
    }

    Ok(())
}

fn read_to_string(path: &Path) -> JppResult<String> {
    fs::read_to_string(path).map_err(|err| io_error("JPP0006", path, err))
}

fn io_error(code: &'static str, path: &Path, err: std::io::Error) -> Diagnostic {
    Diagnostic::new(
        code,
        format!("{}: {err}", path.display()),
        SourceLocation::new(Some(path.to_path_buf()), 1, 1),
    )
}
