use anyhow::Result;
use serde_json::Value;
use std::{collections::{ HashSet, HashMap }, fs};
use crate::{parser::QueryVersion, prj_core::{ remove_helper, resolve_dependencies }};
use crate::parser::{
    RegistryMode,
    RemoteRegistryIndex,
    Manifest,
    ManageToml,
    parse_module_name,
    query_registry,
    read_internal_registry,
    resolve_module_version,
    write_internal_registry,
    remove_entry_from_registry,
    from_registry_to_list,
    compare_version
};

use crate::paths::{
    CS_MODULES_CACHE_FOLDER,
    CS_CACHE_INDEX,
    CS_MODULES_FOLDER,
    CS_MODULES_INDEX,
    REMOTE_REGISTRY_INDEX,
    MANIFEST_FILE,
    CSPM_MANIFEST,
    ProjectRootMode,
    get_root
};


pub fn get_cspm_version() -> Result<String> {
    let cspm_manifest: Value = toml::from_str(CSPM_MANIFEST)?;
    return Ok(cspm_manifest["package"]["version"].to_string())
}

pub fn search_package(module_name: &str) -> Result<()> {
    println!("[SEARCH_MOD::INFO] Search module: {}", module_name);
    let response = reqwest::blocking::get(REMOTE_REGISTRY_INDEX)?;
    let indexes: HashMap<String, RemoteRegistryIndex> = response.json()?;

    match indexes.get(module_name) {
        Some(pkg) => {
            println!("***********************");
            println!("Module found:");
            println!("  Module name: {}", module_name);
            println!("  Available versions: {:?}", pkg.version);
            println!("  Description: {}", pkg.description);
            println!("***********************");
        },
        None => println!("[SEARCH_MOD::INFO] Package {} not found in registry", module_name)
    }

    Ok(())
}

pub fn manage_cache(clean: bool, list: bool) -> Result<()> {
    let root = get_root(true, &ProjectRootMode::CacheRoot)?;
    let cache_folder = root.join(CS_MODULES_CACHE_FOLDER);

    if !cache_folder.exists() || !cache_folder.is_dir() {
        println!("[CACHE::INFO] Cache is empty. Nothing to do");
        return Ok(())
    }

    let cache_index_path = cache_folder.join(CS_CACHE_INDEX);

    if clean {
        let mut cindex = read_internal_registry(&cache_index_path, RegistryMode::CacheMode)?;
        for entry in fs::read_dir(cache_folder)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() { continue; }

            let pkg_path = entry.path();
            let pkg_name = entry.file_name().to_string_lossy().to_string();

            println!("[CACHE::INFO] Remove package {} from cache", pkg_name);
            fs::remove_dir_all(&pkg_path)?;

            println!("[CACHE::INFO] Update cache registry");
            remove_entry_from_registry(pkg_name.clone(), &mut cindex);
        }

        println!("[CACHE::INFO] Update cache registry");
        write_internal_registry(&cache_index_path, cindex)?;

        return Ok(())
    }

    // cache list
    if list {
        println!("[CACHE::INFO] Cache status:");
        println!("");
        for entry in fs::read_dir(cache_folder)? {
            let entry = entry?;
            let entry_path = entry.path();
            let entry_manifest: Manifest = Manifest::open_toml(&entry_path.join(MANIFEST_FILE))?;
            let pname = entry_manifest.package.name;
            let pversion = entry_manifest.package.version;
            let pdeps = entry_manifest.dependencies;
            let deps_format: String = pdeps.iter().map(|(d, v)| format!("{}@{}", d, v)).collect::<Vec<String>>().join(", ");

            println!("***********************");
            println!("> Module: {}@{}", pname, pversion);
            println!("> Module dependencies: [{}]", deps_format);
            println!("***********************");
        }
        println!("");
    }
    Ok(())
}

pub fn install_globally(module: String, force: bool) -> Result<()> {
    let croot = get_root(true, &ProjectRootMode::CacheRoot)?;
    let mroot = get_root(true, &ProjectRootMode::ModulesRoot)?;

    let cache_folder = croot.join(CS_MODULES_CACHE_FOLDER);
    let cache_index = cache_folder.join(CS_CACHE_INDEX);
    let modules_folder = mroot.join(CS_MODULES_FOLDER);
    let modules_index = modules_folder.join(CS_MODULES_INDEX);

    if !cache_folder.is_dir() { fs::create_dir_all(&cache_folder)?; }
    if !modules_folder.is_dir() { fs::create_dir_all(&modules_folder)?; }

    let mindex_check = read_internal_registry(&modules_index, RegistryMode::ModulesMode)?;

    let (name, version) = parse_module_name(&module);
    let version = if !version.is_empty() { Some(version) } else { None };
    let mversion = resolve_module_version(REMOTE_REGISTRY_INDEX, &name, version)?;

    let rvers = query_registry(&mindex_check, &name);
    if let Some(internal_version) = rvers {
        match compare_version(&internal_version, &mversion) {
            QueryVersion::Old | QueryVersion::Young => {
                println!("[INSTALL::INFO] Remove module {}@{} previously added", name, mversion);
                uninstall_globally(name.clone(), force)?;
            },
            QueryVersion::Same => {
                println!("[INSTALL::INFO] Module {}@{} already installed", name, mversion);
                return Ok(());
            }
        }
    }

    println!("[INSTALL::INFO] Check and resolve dependencies...");
    let mut mindex = read_internal_registry(&modules_index, RegistryMode::ModulesMode)?;
    let mut cindex = read_internal_registry(&cache_index, RegistryMode::CacheMode)?;
    let mut visited = HashSet::new();
    resolve_dependencies(
        &cache_folder,
        &modules_folder,
        &name,
        &mversion,
        &mut visited,
        &mut mindex,
        &mut cindex,
        None
    )?;

    println!("[INSTALL::INFO] Write module's registry");
    write_internal_registry(&modules_index, mindex)?;
    write_internal_registry(&cache_index, cindex)?;

    Ok(())
}

pub fn uninstall_globally(module: String, force: bool) -> Result<()> {
    let mroot = get_root(true, &ProjectRootMode::ModulesRoot)?;
    let cs_modules_path = mroot.join(CS_MODULES_FOLDER);
    let mindex_path = cs_modules_path.join(CS_MODULES_INDEX);
    let mut mindex = read_internal_registry(&mindex_path, RegistryMode::ModulesMode)?;

    let (name, _) = parse_module_name(&module);
    println!("[UNINSTALL::INFO] Remove package {} from cs_modules folder", name);

    // delete from modules (also dependencies)
    println!("[UNINSTALL::INFO] Remove package {} dependencies", name);
    remove_helper(&cs_modules_path, &name, force, &mut mindex, None)?;

    // update module's registry
    println!("[UNINSTALL::INFO] Write module's registry");
    write_internal_registry(&mindex_path, mindex)?;

    Ok(())
}

pub fn upgrade_globally(modules: Option<Vec<String>>, force: bool) -> Result<()> {
    let mroot = get_root(true, &ProjectRootMode::ModulesRoot)?;
    let cs_modules_path = mroot.join(CS_MODULES_FOLDER);
    let mindex_path = cs_modules_path.join(CS_MODULES_INDEX);
    let registry = read_internal_registry(&mindex_path, RegistryMode::ModulesMode)?;

    let mut to_update: HashSet<String> = HashSet::new();
    if let Some(mods) = &modules {
        for module in mods.iter() {
            if let Some(rvers) = query_registry(&registry, &module) {
                let latest_version = resolve_module_version(REMOTE_REGISTRY_INDEX, &module, Some(rvers.clone()))?;
                match compare_version(&rvers, &latest_version) {
                    QueryVersion::Young => println!("[UPGRADE::INFO] Module {} is up to date", &module),
                    QueryVersion::Old => { to_update.insert(format!("{}@{}", module.clone(), latest_version)); },
                    QueryVersion::Same => println!("[UPGRADE::INFO] Module {} already exists", &module),
                }
            } else {
                println!("[UPGRADE::INFO] Module {} does not exists in registry", &module);
            }
        }
    } else {
        to_update = from_registry_to_list(&registry);
    }

    for entry in to_update.iter() {
        println!("[UPGRADE::INFO] Check latest version for module {}", &entry);
        let (pkg_name, pkg_version) = parse_module_name(&entry);

        println!("[UPGRADE::INFO] Remove module {}", &pkg_name);
        uninstall_globally(entry.clone(), force)?;

        println!("[UPGRADE::INFO] Update module {} to {}", &pkg_name, &pkg_version);
        install_globally(entry.clone(), force)?;
    }

    Ok(())
}
