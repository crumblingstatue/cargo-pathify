use {
    std::{
        path::{Path, PathBuf},
        process::{Command, ExitCode},
    },
    toml_edit::{Document, InlineTable, Item, Value},
};

fn main() -> ExitCode {
    let Some(depname) = std::env::args().nth(2) else {
        eprintln!("Need dependency name as argument");
        return ExitCode::FAILURE;
    };
    let cargo_toml = match std::fs::read_to_string("Cargo.toml") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading Cargo.toml: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut doc = match cargo_toml.parse::<Document>() {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("Oh no! Failed to parse Cargo.toml: {e}");
            return ExitCode::FAILURE;
        }
    };
    let Some((dep_key, dep_item)) = doc["dependencies"]
        .as_table_mut()
        .expect("dependencies not a table?")
        .get_key_value_mut(&depname)
    else {
        eprintln!("Could not find '{depname}' in dependencies.");
        return ExitCode::FAILURE;
    };
    if let Some(dir) = find_dep_dir(&dep_key.to_string(), get_dep_ver_item(dep_item).unwrap()) {
        let cwd = std::env::current_dir().unwrap();
        std::fs::create_dir("pathified").unwrap();
        copy_dir_all(&dir, &cwd.join(format!("pathified/{depname}")));
        eprintln!("Found dependency dir: {dir:?}");
    } else {
        eprintln!("Could not find dependency directory. Sorry.");
        return ExitCode::FAILURE;
    }
    update_toml(dep_item, dep_key, &depname);
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

fn update_toml(dep_value: &mut Item, mut dep_key: toml_edit::KeyMut<'_>, depname: &str) {
    let old_value_as_string = dep_value.to_string();
    let mut table = InlineTable::new();
    dep_key
        .leaf_decor_mut()
        .set_prefix(format!("#{old_value_as_string}\n"));
    table.insert("path", format!("pathified/{depname}").into());
    *dep_value = Item::Value(Value::InlineTable(table));
}
