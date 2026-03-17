use anyhow::Result;
use serde_json::Value;
use colored::*;
use std::{ collections::{ HashSet, HashMap }, fs };
use crate::{
    colored_name,
    colored_name_version,
    colored_version,
    build_dir
};

use crate::{
    prj_core::{ remove_helper, resolve_dependencies },
    utils::{ MessageType, fetch_remote_registry_index, log_message },
    confres::{
        ProjectPaths,
        ProjectRoots,
        CSPM_MANIFEST,
        MANIFEST_FILE,
        REMOTE_MREGISTRY_INDEX,
        REMOTE_PREGISTRY_INDEX
    },
    parser::{
        ManageToml,
        Manifest,
        ModuleTools,
        Registry,
        RegistryMode,
        RemoteRegistryIndex,
        Version,
        VersionStatus
    }
};


pub fn get_cspm_version() -> Result<String> {
    let cspm_manifest: Value = toml::from_str(CSPM_MANIFEST)?;
    return Ok(cspm_manifest["package"]["version"].to_string())
}

pub fn search_package(module_name: &str) -> Result<()> {
    log_message(MessageType::Info(format!("Search module: {}", colored_name!(module_name))), Some("SEARCH"), true);

    let mindexes: HashMap<String, RemoteRegistryIndex> = fetch_remote_registry_index(REMOTE_MREGISTRY_INDEX)?;
    let pindexes: HashMap<String, RemoteRegistryIndex> = fetch_remote_registry_index(REMOTE_PREGISTRY_INDEX)?;

    log_message(
        MessageType::Warning("Look at modules registry...".to_string()),
        Some("SEARCH"),
        true
    );

    match mindexes.get(module_name) {
        Some(pkg) => {
            println!();
            println!("📦 {}", colored_name!(module_name));
            println!("  ├─ Versions: {}", pkg.versions.join(", "));
            println!("  └─ Description: {}", pkg.description);
            println!();
        },
        None => {
            log_message(
                MessageType::Warning(format!("No module named {} was found", colored_name!(module_name))),
                Some("SEARCH"),
                true
            );
        }
    }

    log_message(
        MessageType::Warning("Look at projects registry...".to_string()),
        Some("SEARCH"),
        true
    );

    match pindexes.get(module_name) {
        Some(pkg) => {
            println!();
            println!("📁 {}", colored_name!(module_name));
            println!("  ├─ Versions: {}", pkg.versions.join(", "));
            println!("  └─ Description: {}", pkg.description);
            println!();
        },
        None => {
            log_message(
                MessageType::Warning(format!("No project named {} was found", colored_name!(module_name))),
                Some("SEARCH"),
                true
            );
        }
    }

    Ok(())
}

pub fn manage_cache(clean: bool, list: bool) -> Result<()> {
    let roots = ProjectRoots::new(false)?;
    let pths = ProjectPaths::new(&roots);

    if !pths.cache_folder.exists() || !pths.cache_folder.is_dir() {
        log_message(MessageType::Info("Cache is empty. Nothing to do".to_string()), Some("CACHE"), true);
        return Ok(())
    }

    if clean {
        let mut cache_registry = Registry::new(&pths.cache_registry, RegistryMode::CacheMode);
        cache_registry.read_internal_registry()?;
        for entry in fs::read_dir(pths.cache_folder)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() { continue; }

            let pkg_path = entry.path();
            let pkg_name = entry.file_name().to_string_lossy().to_string();

            log_message(MessageType::Info(format!("Remove module {} from cache", colored_name!(pkg_name))), Some("CACHE"), true);
            fs::remove_dir_all(&pkg_path)?;

            cache_registry.remove_entry_from_registry(pkg_name.clone());
        }

        log_message(MessageType::Info("Update cache registry".to_string()), Some("CACHE"), true);
        cache_registry.write_internal_registry()?;

        return Ok(())
    }

    // cache list
    if list {
        log_message(MessageType::Info("Cache status".to_string()), Some("CACHE"), true);
        for entry in fs::read_dir(pths.cache_folder)? {
            let entry = entry?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                let entry_manifest: Manifest = Manifest::open_toml(&entry_path.join(MANIFEST_FILE))?;
                let pname = entry_manifest.package.name;
                let pversion = entry_manifest.package.version;
                let pdeps = entry_manifest.dependencies;
                let deps_format: String = pdeps.iter().map(|(d, v)| colored_name_version!(d, v)).collect::<Vec<String>>().join(", ");

                println!();
                println!("🗂️  {}", colored_name_version!(pname, pversion));
                println!("  ├─ Dependencies: {}", deps_format);
                println!();
            }
        }
        println!("");
    }

    Ok(())
}

pub fn install_globally(module: String, force: bool) -> Result<()> {
    let mut roots = ProjectRoots::new(false)?;
    roots.set_modules_root(Some(true))?;
    let pths = ProjectPaths::new(&roots);

    build_dir!(&pths.cache_folder);
    build_dir!(&pths.modules_folder);

    let mut module_registry = Registry::new(&pths.modules_registry, RegistryMode::ModulesMode);
    module_registry.read_internal_registry()?;

    let (name, version) = ModuleTools::parse_module_name(&module);
    let version = if !version.is_empty() { Some(version) } else { None };
    let mversion = ModuleTools::resolve_module_version(&name, version)?;

    let rvers = module_registry.query_registry(&name);
    if let Some(internal_version) = rvers {
        let parsed_internal_version = Version::parse(&internal_version)?;
        let parsed_mversion = Version::parse(&mversion)?;

        match parsed_internal_version.compare(&parsed_mversion) {
            VersionStatus::Old | VersionStatus::Young => {
                log_message(
                    MessageType::Info(format!("Remove module {} previously added", colored_name_version!(name, mversion))),
                    Some("INSTALL"),
                    true
                );

                uninstall_globally(name.clone(), force)?;
            },
            VersionStatus::Same => {
                log_message(
                    MessageType::Info(format!("Module {} already installed", colored_name_version!(name, mversion))),
                    Some("INSTALL"),
                    true
                );

                return Ok(());
            }
        }
    }

    log_message(
        MessageType::Info("Check and resolve dependencies...".to_string()),
        Some("INSTALL"),
        true
    );

    // read updated registry
    module_registry.read_internal_registry()?;

    let mut cache_registry = Registry::new(&pths.cache_registry, RegistryMode::CacheMode);
    cache_registry.read_internal_registry()?;

    let mut visited = HashSet::new();
    resolve_dependencies(
        &pths.cache_folder,
        &pths.modules_folder,
        &name,
        &mversion,
        &mut visited,
        &mut module_registry,
        &mut cache_registry,
        None
    )?;

    log_message(
        MessageType::Info("Write registry index".to_string()),
        Some("INSTALL"),
        true
    );

    module_registry.write_internal_registry()?;
    cache_registry.write_internal_registry()?;

    Ok(())
}

pub fn uninstall_globally(module: String, force: bool) -> Result<()> {
    let mut roots = ProjectRoots::new(false)?;
    roots.set_modules_root(Some(true))?;
    let pths = ProjectPaths::new(&roots);

    let mut mregistry = Registry::new(&pths.modules_registry, RegistryMode::ModulesMode);
    mregistry.read_internal_registry()?;

    let (name, mut version) = ModuleTools::parse_module_name(&module);
    if version.is_empty() {
        match mregistry.query_registry(&name) {
            Some(v) => version = v,
            None => {
                let mes_err = log_message(
                    MessageType::Error("Failed to read the registry. Specify the version <name@version>".to_string()),
                    Some("UNINSTALL"),
                    false
                );

                return Err(anyhow::anyhow!(mes_err))
            }
        }
    }

    log_message(
        MessageType::Info(format!("Remove module {} from cs_modules folder", colored_name!(name))),
        Some("UNINSTALL"),
        true
    );

    log_message(
        MessageType::Info(format!("Remove module {} dependencies", colored_name!(name))),
        Some("UNINSTALL"),
        true
    );

    // delete from modules (also dependencies)
    let full_name = format!("{}@{}", name, version);
    remove_helper(&pths.modules_folder, &full_name, force, &mut mregistry, None)?;

    // update registry index
    log_message(
        MessageType::Info("Write registry index".to_string()),
        Some("UNINSTALL"),
        true
    );

    mregistry.write_internal_registry()?;

    Ok(())
}

pub fn upgrade_globally(modules: Option<Vec<String>>, force: bool) -> Result<()> {
    let mut roots = ProjectRoots::new(false)?;
    roots.set_modules_root(Some(true))?;
    let pths = ProjectPaths::new(&roots);

    let mut mregistry = Registry::new(&pths.modules_registry, RegistryMode::ModulesMode);
    mregistry.read_internal_registry()?;

    let mut to_update: HashSet<String> = HashSet::new();
    if let Some(mods) = &modules {
        for module in mods.iter() {
            if let Some(rvers) = mregistry.query_registry(&module) {
                let parsed_registry_version = Version::parse(&rvers)?;
                let latest_version = ModuleTools::resolve_module_version(&module, Some(rvers.clone()))?;
                match parsed_registry_version.compare(&Version::parse(&latest_version)?) {
                    VersionStatus::Young => {
                        log_message(
                            MessageType::Info(format!("Module {} is up to date", colored_name!(module))),
                            Some("UPGRADE"),
                            true
                        );

                    },
                    VersionStatus::Old => {
                        to_update.insert(format!("{}@{}", module.clone(), latest_version));
                    },
                    VersionStatus::Same => {
                        log_message(
                            MessageType::Info(format!("Module {} already exists", colored_name!(module))),
                            Some("UPGRADE"),
                            true
                        );

                    }
                }
            } else {
                log_message(
                    MessageType::Warning(format!("Module {} does not exists in registry", colored_name!(module))),
                    Some("UPGRADE"),
                    true
                );

            }
        }
    } else {
        to_update = mregistry.from_registry_to_list();
    }

    for entry in to_update.iter() {
        log_message(
            MessageType::Info(format!("Check latest version for module {}", colored_name!(entry))),
            Some("UPGRADE"),
            true
        );

        let (pkg_name, pkg_version) = ModuleTools::parse_module_name(&entry);

        log_message(
            MessageType::Info(format!("Remove module {}", colored_name!(pkg_name))),
            Some("UPGRADE"),
            true
        );

        uninstall_globally(entry.clone(), force)?;

        log_message(
            MessageType::Info(format!("Update module {} to {}", colored_name!(pkg_name), colored_version!(pkg_version))),
            Some("UPGRADE"),
            true
        );

        install_globally(entry.clone(), force)?;
    }

    Ok(())
}
