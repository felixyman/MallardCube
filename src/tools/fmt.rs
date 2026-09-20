/// Canonical config formatting (plan 040).
///
/// Usage:
///   mallard fmt <proxy-config.json|yaml>          rewrite canonically
///   mallard fmt --check <proxy-config.json|yaml>  exit 1 when not canonical
///   mallard fmt --to yaml|json <config>           print the effective config
///
/// The input format (JSON or YAML) is preserved, derived defaults are omitted
/// again so minimal configs stay minimal, and sections that declare a `*_file`
/// move entirely into that file. YAML comments are not preserved by a rewrite
/// (serde-based YAML has no comment model) — run `fmt` deliberately.
use std::path::Path;

pub fn run(args: Vec<String>) -> i32 {
    let check = args.iter().any(|a| a == "--check");
    let to = args
        .iter()
        .position(|a| a == "--to")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str());
    let Some(config_path) = args
        .iter()
        .skip(1)
        .find(|a| !a.starts_with("--") && *a != "yaml" && *a != "json" && *a != "yml")
    else {
        eprintln!("usage: mallard fmt [--check] [--to yaml|json] <proxy-config.json|yaml>");
        return 2;
    };
    let path = Path::new(config_path);
    let config = match crate::project::config_io::load(path) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("fmt: {e}");
            return 2;
        }
    };

    // --to: print the effective config (sections inlined) in the target format.
    if let Some(to) = to {
        let format = match to {
            "yaml" | "yml" => crate::project::config_io::ConfigFormat::Yaml,
            "json" => crate::project::config_io::ConfigFormat::Json,
            other => {
                eprintln!("fmt: unknown target format '{other}' (expected yaml or json)");
                return 2;
            }
        };
        let mut converted = config.clone();
        converted.deminimize();
        converted.dimensions_file = None;
        converted.measures_file = None;
        converted.relationships_file = None;
        converted.roles_file = None;
        return match crate::project::config_io::serialize(&converted, format) {
            Ok(text) => {
                print!("{text}");
                0
            }
            Err(e) => {
                eprintln!("fmt: {e}");
                2
            }
        };
    }

    if check {
        match crate::project::config_io::non_canonical(&config, path) {
            Ok(dirty) if dirty.is_empty() => {
                println!("fmt: {} is canonical", path.display());
                0
            }
            Ok(dirty) => {
                eprintln!("fmt: not canonical (run `mallard fmt`):");
                for file in dirty {
                    eprintln!("  {}", file.display());
                }
                1
            }
            Err(e) => {
                eprintln!("fmt: {e}");
                2
            }
        }
    } else {
        match crate::project::config_io::write_canonical(&config, path) {
            Ok(written) => {
                for file in written {
                    println!("fmt: wrote {}", file.display());
                }
                0
            }
            Err(e) => {
                eprintln!("fmt: {e}");
                2
            }
        }
    }
}
