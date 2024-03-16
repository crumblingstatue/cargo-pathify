use {
    clap::Parser,
    std::{
        path::{Path, PathBuf},
        process::{Command, ExitCode},
    },
    toml_edit::{DocumentMut, InlineTable, Item, Value},
};

#[derive(clap::Parser)]
struct Args {
    /// Name of the dependency to pathify
    dep_name: String,
    /// Point to an already existing directory instead of copying over from `$CARGO_HOME`
    #[arg(long = "path")]
    existing: Option<String>,
}

fn main() -> ExitCode {
    let args = Args::parse_from(std::env::args_os().skip(1));
    let cargo_toml = match std::fs::read_to_string("Cargo.toml") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading Cargo.toml: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut doc = match cargo_toml.parse::<DocumentMut>() {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("Oh no! Failed to parse Cargo.toml: {e}");
            return ExitCode::FAILURE;
        }
    };
    let Some((dep_key, dep_item)) = doc["dependencies"]
        .as_table_mut()
        .expect("dependencies not a table?")
        .get_key_value_mut(&args.dep_name)
    else {
        eprintln!("Could not find '{}' in dependencies.", args.dep_name);
        return ExitCode::FAILURE;
    };
    let pathified_path = match args.existing {
        Some(path) => path,
        None => {
            if let Some(dir) =
                find_dep_dir(&dep_key.to_string(), get_dep_ver_item(dep_item).unwrap())
            {
                let cwd = std::env::current_dir().unwrap();
                std::fs::create_dir("pathified").unwrap();
                let destination_path = cwd.join(format!("pathified/{}", args.dep_name));
                match destination_path.to_str() {
                    Some(destination_path) => {
                        copy_dir_all(&dir, destination_path.as_ref());
                        eprintln!("Found dependency dir: {dir:?}");
                        destination_path.to_string()
                    }
                    None => {
                        eprintln!("Sorry, the calculated path isn't valid UTF-8. Giving up.");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                eprintln!("Could not find dependency directory. Sorry.");
                return ExitCode::FAILURE;
            }
        }
    };
    update_toml(dep_item, dep_key, &pathified_path);
    std::fs::write("Cargo.toml", doc.to_string().as_bytes()).unwrap();
    ExitCode::SUCCESS
}

fn get_dep_ver_item(item: &Item) -> Option<&str> {
    match item {
        Item::Value(val) => get_dep_ver_val(val),
        Item::Table(tbl) => get_dep_ver_item(&tbl["version"]),
        _ => None,
    }
}

fn get_dep_ver_val(val: &Value) -> Option<&str> {
    match val {
        Value::String(s) => Some(s.value()),
        Value::InlineTable(tbl) => get_dep_ver_val(&tbl["version"]),
        _ => None,
    }
}

fn copy_dir_all(src: &Path, dst: &Path) {
    Command::new("cp")
        .arg("-r")
        .arg(src)
        .arg(dst)
        .status()
        .unwrap();
}

fn find_dep_dir(dep_key: &str, dep_ver: &str) -> Option<PathBuf> {
    let cargo_home = std::env::var_os("CARGO_HOME").expect("No CARGO_HOME?");
    let registry_index = Path::new(&cargo_home).join("registry/src");
    match std::fs::read_dir(registry_index).unwrap().find(|en| {
        en.as_ref()
            .unwrap()
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("index.crates.io-")
    }) {
        Some(en) => {
            let depdir_name = format!("{dep_key}-{dep_ver}");
            let final_path = en.unwrap().path().join(depdir_name);
            if !final_path.exists() {
                eprintln!("Cannot find {final_path:?}");
                None
            } else {
                Some(final_path)
            }
        }
        None => None,
    }
}

fn update_toml(dep_value: &mut Item, mut dep_key: toml_edit::KeyMut<'_>, dep_path: &str) {
    let old_value_as_string = dep_value.to_string();
    let mut table = InlineTable::new();
    dep_key
        .leaf_decor_mut()
        .set_prefix(format!("#{old_value_as_string}\n"));
    table.insert("path", dep_path.into());
    *dep_value = Item::Value(Value::InlineTable(table));
}
