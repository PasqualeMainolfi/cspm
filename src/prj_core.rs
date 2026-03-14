use anyhow::Result;
use serde_json;
use colored::*;
use fs_extra::dir::{ copy, CopyOptions };
use std::{
    fs,
    path,
    collections::{
        HashMap,
        HashSet,
        VecDeque
    },
};

use crate::{
    colored_name,
    colored_name_version,
    colored_version
};

use crate::utils::{
    MessageType,
    check_risset,
    download_package,
    fetch_remote_registry_index,
    log_message,
    run_csound_script,
    run_risset
};

use crate::parser::{
    CacheMeta,
    LockChild,
    LockFile,
    MainEntry,
    MainPackage,
    ManageToml,
    Manifest,
    RegistryData,
    RegistryMode,
    QueryVersion,
    RemoteRegistryIndex,
    add_entry_to_registry,
    check_manifest_deps,
    computer_checksum,
    parse_module_name,
    read_internal_registry,
    remove_entry_from_registry,
    resolve_module_version,
    write_internal_registry,
    query_registry,
    compare_version,
    from_registry_to_list
};

use crate::paths::{
    CSD_MAIN_TEMPLATE,
    UDO_MAIN_TEMPLATE,
    LOCK_FILE,
    LOCK_VERSION,
    DEFAULT_SRC_FOLDER,
    MANIFEST_FILE,
    CS_MODULES_CACHE_FOLDER,
    CS_MODULE_META,
    CS_MODULES_FOLDER,
    REMOTE_REGISTRY,
    CS_CACHE_INDEX,
    CS_MODULES_INDEX,
    ProjectRoots,
    ProjectRootMode,
    get_root,
    create_info_file,
    read_project_info
};


pub fn create_project(p_name: String, module_flag: bool, global: bool) -> Result<()> {
    let mut dir_builder = fs::DirBuilder::new();
    dir_builder.recursive(true);
    let pfolder = format!("./{}", p_name);
    let p = path::Path::new(pfolder.as_str());

    // create src folder
    let p_src = p.join(DEFAULT_SRC_FOLDER);
    dir_builder.create(p_src.clone())?;

    // create project info file
    create_info_file(&p, global)?;

    // main script (entry point)
    let main_ext = if !module_flag { ".csd" } else { ".udo" };
    let main_script = format!("{}{}", p_name, main_ext);

    // create manifest
    log_message(MessageType::Info("Create Cspm.toml file".to_string()), Some("CREATE"), true);

    let manifest_file = p.join(MANIFEST_FILE);
    let mft = MainPackage { name: p_name, version: String::from("0.1.0"), ..Default::default() };
    let mut main_entry = MainEntry { src: DEFAULT_SRC_FOLDER.to_string(), ..Default::default() };

    let main_template;
    let entry_point = Some(format!("{}/{}", DEFAULT_SRC_FOLDER, main_script));
    match module_flag {
        true => {
            main_entry.udo = entry_point;
            main_template = UDO_MAIN_TEMPLATE;
        },
        false => {
            main_entry.csd = entry_point;
            main_template = CSD_MAIN_TEMPLATE;
        }
    }

    let mut manifest_init = Manifest { package: mft, main: main_entry, ..Default::default()};
    manifest_init.package.include.push("src".to_string()); // include src folder
    Manifest::write_toml(&manifest_file, &manifest_init)?;

    // create main .csd or .udo file
    log_message(MessageType::Info("Create src folder and entry point file".to_string()), Some("CREATE"), true);
    let src_file = p_src.join(main_script);
    fs::write(src_file, &main_template)?;

    Ok(())
}

pub fn add_package(name: &str, version: Option<String>, force: bool) -> Result<()> {
    let mut roots = ProjectRoots::new()?;
    roots.set_modules_root()?;

    let lpath = roots.project_root.join(LOCK_FILE);

    let cache_folder = roots.cache_root.join(CS_MODULES_CACHE_FOLDER);
    let cache_index = cache_folder.join(CS_CACHE_INDEX);
    let modules_folder = roots.modules_root.join(CS_MODULES_FOLDER);
    let modules_index = modules_folder.join(CS_MODULES_INDEX);

    if !cache_folder.is_dir() { fs::create_dir_all(&cache_folder)?; }
    if !modules_folder.is_dir() { fs::create_dir_all(&modules_folder)?; }

    let mversion = resolve_module_version(&name, version)?;
    let manifest_toml = Manifest::open_toml(&roots.project_root.join(MANIFEST_FILE))?;

    if let Some(internal_version) = manifest_toml.dependencies.get(name) {
        match compare_version(&internal_version, &mversion) {
            QueryVersion::Old | QueryVersion::Young => {
                log_message(
                    MessageType::Info(format!("Remove module {} previously added", colored_name_version!(name, mversion))),
                    Some("ADD"),
                    true
                );

                remove_package(name, force)?;
            },
            QueryVersion::Same => {
                log_message(
                    MessageType::Info(format!("Module {} already installed", colored_name_version!(name, mversion))),
                    Some("ADD"),
                    true
                );

                return Ok(());
            }
        }
    }

    // load lockfile
    let mut lockfile: LockFile = if !lpath.exists() {
        LockFile { version: LOCK_VERSION, ..Default::default() }
    } else {
        LockFile::open_toml(&lpath)?
    };

    log_message(
        MessageType::Info("Check and resolve dependencies...".to_string()),
        Some("ADD"),
        true
    );

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
        Some(&mut lockfile)
    )?;

    log_message(
        MessageType::Info("Write module's registry".to_string()),
        Some("ADD"),
        true
    );

    write_internal_registry(&modules_index, mindex)?;
    write_internal_registry(&cache_index, cindex)?;

    log_message(
        MessageType::Info("Update Cspm.toml file".to_string()),
        Some("ADD"),
        true
    );

    update_manifest(&name, &mversion)?;

    // update manifest in memory
    let re_manifest_toml = Manifest::open_toml(&roots.project_root.join(MANIFEST_FILE))?; // re-open after changes

    log_message(
        MessageType::Info("Update Cspm.lock file".to_string()),
        Some("ADD"),
        true
    );

    lockfile.package.retain(|p| p.name != re_manifest_toml.package.name);
    lockfile.package.push(LockChild {
        name: re_manifest_toml.package.name,
        version: re_manifest_toml.package.version,
        dependencies: re_manifest_toml.dependencies
            .iter()
            .map(|(d, v)| format!("{}@{}", d, v))
            .collect(),
        ..Default::default()
    });

    LockFile::write_toml(&lpath, &lockfile)?;

    Ok(())
}

fn update_manifest(pname: &str, version: &str) -> Result<()> {
    let mpath = get_root(false, &ProjectRootMode::ModulesRoot)?.join(MANIFEST_FILE);
    let mut manifest = Manifest::open_toml(&mpath)?;

    manifest.dependencies
        .entry(pname.to_string())
        .and_modify(|v| *v = version.to_string())
        .or_insert(version.to_string());

    Manifest::write_toml(&mpath, &manifest)?;

    Ok(())
}

pub fn resolve_dependencies(
    cfolder: &path::Path,
    mfolder: &path::Path,
    mname: &str,
    version: &str,
    visited: &mut HashSet<String>,
    mindex: &mut RegistryData,
    cindex: &mut RegistryData,
    mut lockfile: Option<&mut LockFile>
) -> Result<()>
{
    let pfull_name = format!("{}@{}", mname, version);
    if !visited.insert(pfull_name.clone()) { return Ok(()); }

    let cached_module = cfolder.join(&pfull_name);
    let local_module = mfolder.join(&pfull_name);

    let checksum: String;

    let meta_name = format!(".{}@{}_{}", mname, version, CS_MODULE_META);
    let meta_file_path = cached_module.join(meta_name);

    if !cached_module.exists() {

        log_message(
            MessageType::Info(format!("Module {} not found in cache, downloading...", colored_name_version!(mname, version))),
            Some("RESOLVE-DEPS"),
            true
        );

        download_package(&REMOTE_REGISTRY, mname, version, cfolder, &cached_module)?;
        checksum = computer_checksum(&cached_module)?;
        let cm = CacheMeta { source: REMOTE_REGISTRY.to_string(), checksum: checksum.clone() };
        let meta_json = serde_json::to_string_pretty(&cm)?;
        fs::write(meta_file_path, &meta_json)?;
        add_entry_to_registry(mname, version, cindex);
    } else {
        let meta_string = fs::read_to_string(meta_file_path)?;
        let meta_json: CacheMeta = serde_json::from_str(&meta_string)?;
        checksum = meta_json.checksum;
    }

    match mindex {
        RegistryData::ModulesRegistry(map) => {
            if let Some(v) = map.get(mname) {
                if v != &version {

                    let err_mes = log_message(
                        MessageType::Error(
                            format!(
                                "Dependency conflict: {} requested but {} is already installed",
                                colored_name_version!(mname, version),
                                colored_name_version!(mname, v)
                            )
                        ),
                        Some("RESOLVE-DEPS"),
                        false
                    );

                    return Err(anyhow::anyhow!(err_mes))
                }
            }
        },
        RegistryData::CacheRegistry(_) => { }
    }

    if !local_module.exists() {
        let mut coptions = CopyOptions::new();
        coptions.content_only = true;
        copy(&cached_module, &local_module, &coptions)?;

        log_message(
            MessageType::Info("Update module's registry".to_string()),
            Some("RESOLVE-DEPS"),
            true
        );

        add_entry_to_registry(mname, &version, mindex);
    }

    // read manifest
    let mod_manifest = Manifest::open_toml(&cached_module.join(MANIFEST_FILE))?;

    if let Some(lfile) = lockfile.as_mut() {
        // remove old dependencies and add child to lockfile

        log_message(
            MessageType::Info("Add child to Cspm.lock file and remove old dependencies".to_string()),
            Some("RESOLVE-DEPS"),
            true
        );

        lfile.package.retain(|p| !(p.name == mname && p.version == version));
        lfile.plugins = mod_manifest.plugins.clone();
        lfile.package.push(LockChild {
            name: mname.to_string(),
            version: version.to_string(),
            source: REMOTE_REGISTRY.to_string(),
            checksum,
            dependencies: mod_manifest.dependencies
                .iter()
                .map(|(d, v)| format!("{}@{}", d, v))
                .collect(),
        });
    }

    log_message(
        MessageType::Info("Resolving dependencies...".to_string()),
        Some("RESOLVE-DEPS"),
        true
    );

    for (name, version) in mod_manifest.dependencies.iter() {
        resolve_dependencies(
            cfolder,
            mfolder,
            name,
            version,
            visited,
            mindex,
            cindex,
            lockfile.as_deref_mut()
        )?;
    }

    Ok(())
}

pub fn remove_package(pname: &str, force: bool) -> Result<()> {
    let mut roots = ProjectRoots::new()?;
    roots.set_modules_root()?;

    let lpath = roots.project_root.join(LOCK_FILE);
    let manifest_path = roots.project_root.join(MANIFEST_FILE);
    let cs_modules_path = roots.modules_root.join(CS_MODULES_FOLDER);

    // read manifest for deletion
    let mut manifest_toml = Manifest::open_toml(&manifest_path)?;

    // load lockfile
    let mut lockfile: LockFile = if !lpath.exists() {
        LockFile { version: LOCK_VERSION, ..Default::default() }
    } else {
        LockFile::open_toml(&lpath)?
    };

    log_message(
        MessageType::Info(format!("Remove module {} from cs_modules folder", colored_name!(pname))),
        Some("REMOVE"),
        true
    );

    match manifest_toml.dependencies.get(pname) {
        Some(_) => { },
        None => {

            log_message(
                MessageType::Warning(format!("Undeclared module {} in Cspm.toml file", colored_name!(pname))),
                Some("REMOVE"),
                true
            );

            return Ok(())
        }
    };

    // delete from modules (also dependencies)
    log_message(
        MessageType::Info(format!("Remove module {} dependencies", colored_name!(pname))),
        Some("REMOVE"),
        true
    );

    // check version if passed
    let (_, check_version) = parse_module_name(pname);
    if !check_version.is_empty() {
        if &check_version != &manifest_toml.package.version {
            let mes_err = log_message(
                MessageType::Error(
                    format!(
                        "The provided version {} does not match the declared version {} in Cspm.toml",
                        colored_name!(check_version),
                        colored_version!(manifest_toml.package.version)
                    )
                ),
                Some("REMOVE"),
                true
            );

            return Err(anyhow::anyhow!(mes_err));
        }
    }

    let pname_full = format!("{}@{}", pname, manifest_toml.package.version);
    let mindex_path = roots.modules_root.join(CS_MODULES_FOLDER).join(CS_MODULES_INDEX);
    let mut mindex = read_internal_registry(&mindex_path, RegistryMode::ModulesMode)?;
    remove_helper(&cs_modules_path, &pname_full, force, &mut mindex, Some(&mut lockfile))?;

    // update module's registry
    log_message(
        MessageType::Info("Write module's registry".to_string()),
        Some("REMOVE"),
        true
    );

    write_internal_registry(&mindex_path, mindex)?;

    // delete from manifest
    log_message(
        MessageType::Info(format!("Remove module {} from Cspm.toml file", colored_name!(pname))),
        Some("REMOVE"),
        true
    );

    manifest_toml.dependencies.remove(pname);

    // update lockfile
    log_message(
        MessageType::Info("Update Cspm.toml and Cspm.lock file".to_string()),
        Some("REMOVE"),
        true
    );

    lockfile.package.retain(|p| p.name != manifest_toml.package.name);
    lockfile.package.push(LockChild {
        name: manifest_toml.package.name.clone(),
        version: manifest_toml.package.version.clone(),
        dependencies: manifest_toml.dependencies
            .iter()
            .map(|(d, v)| format!("{}@{}", d, v))
            .collect(),
        ..Default::default()
    });

    LockFile::write_toml(&lpath, &lockfile)?;
    Manifest::write_toml(&manifest_path, &manifest_toml)?;
    Ok(())
}

pub fn remove_helper(
    cs_modules_path: &path::Path,
    pname: &str,
    force: bool,
    mindex: &mut RegistryData,
    mut lockfile: Option<&mut LockFile>
) -> Result<()>
{
    let mut queue: VecDeque<String> = VecDeque::new();
    queue.push_back(pname.to_string());

    let mut visited = HashSet::new();

    while let Some(current) = queue.pop_front() {
        if !visited.insert(current.clone()) { continue; }

        let mut is_in = false;
        for entry in cs_modules_path.read_dir()? {
            let entry = entry?;
            let entry_path = entry.path();
            let manifest_path = entry_path.join(MANIFEST_FILE);

            if !manifest_path.exists() { continue; };

            let mtoml = Manifest::open_toml(&manifest_path)?;
            let dset: HashSet<String> = mtoml.dependencies
                .keys()
                .map(|d| d.to_string())
                .collect();

            if !force {
                if dset.contains(&current) {

                    log_message(
                        MessageType::Warning(
                            format!(
                                "Module {} removal skipped because the module {} depends on it. Use [--force or -f] if you still want to delete",
                                colored_name!(current),
                                colored_name!(mtoml.package.name)
                            )
                        ),
                        Some("REMOVE"),
                        true
                    );

                    is_in = true;
                    break;
                }
            }
        }

        if !is_in {
            let pfolder = cs_modules_path.join(&current);
            let manifest_path = pfolder.join(MANIFEST_FILE);
            if manifest_path.exists() {
                let mtoml = Manifest::open_toml(&manifest_path)?;
                for dep in mtoml.dependencies.keys() {
                    queue.push_back(dep.clone());
                }
            }

            if pfolder.exists() {

                log_message(MessageType::Info(format!("Remove module {}", colored_name!(current))),
                    Some("REMOVE"),
                    true
                );

                fs::remove_dir_all(&pfolder)?;

                log_message(MessageType::Info("Update project's modules registry".to_string()),
                    Some("REMOVE"),
                    true
                );

                remove_entry_from_registry(current.clone(), mindex);
                if let Some(lfile) = lockfile.as_mut() {
                    let (pkg_name, pkg_version) = parse_module_name(&current);
                    lfile.package.retain(|p| !(p.name == pkg_name && p.version == pkg_version));
                }
            }
        }
    }

    Ok(())
}

pub fn update_package(modules: Option<Vec<String>>, force: bool) -> Result<()> {
    let mut roots = ProjectRoots::new()?;
    roots.set_modules_root()?;
    let mindex_path = roots.modules_root.join(CS_MODULES_INDEX);
    let registry = read_internal_registry(&mindex_path, RegistryMode::ModulesMode)?;

    let manifest = Manifest::open_toml(&roots.project_root.join(MANIFEST_FILE))?;
    let installed_modules = manifest.dependencies;

    if let Some(mods) = &modules {
        for module in mods.iter() {
            if !installed_modules.contains_key(module) {

                let err_mes = log_message(
                    MessageType::Error(
                        format!("Undeclared module {} in Cspm.toml file", colored_name!(module))
                    ),
                    Some("UPDATE-MOD"),
                    false
                );

                return Err(anyhow::anyhow!(err_mes))
            }
        }
    }

    let mut to_update: HashSet<String> = HashSet::new();
    if let Some(mods) = &modules {
        for module in mods.iter() {
            if let Some(rvers) = query_registry(&registry, &module) {
                let latest_version = resolve_module_version(&module, Some(rvers.clone()))?;
                match compare_version(&rvers, &latest_version) {
                    QueryVersion::Young => {
                        log_message(
                            MessageType::Info(format!("Module {} is up to date", colored_name!(module))),
                            Some("UPDATE"),
                            true
                        );
                    },
                    QueryVersion::Old => { to_update.insert(colored_name_version!(module, latest_version)); },
                    QueryVersion::Same => {
                        log_message(
                            MessageType::Info(format!("Module {} already exists", colored_name!(module))),
                            Some("UPDATE"),
                            true
                        );
                    },
                }
            } else {
                log_message(
                    MessageType::Warning(format!("Module {} does not exists in registry", colored_name!(module))),
                    Some("UPDATE"),
                    true
                );
            }
        }
    } else {
        to_update = from_registry_to_list(&registry);
    }

    for entry in to_update.iter() {
        let (pname, pversion) = parse_module_name(entry);

        log_message(
            MessageType::Info(format!("Remove module {}", colored_name_version!(pname, pversion))),
            Some("UPDATE"),
            true
        );

        remove_package(&pname, force)?;

        log_message(
            MessageType::Info(format!("Update module {}", colored_name_version!(pname, pversion))),
            Some("UPDATE"),
            true
        );

        add_package(&pname, Some(pversion), force)?;

        log_message(
            MessageType::Info(format!("Module {} is up to date", colored_name!(pname))),
            Some("UPDATE"),
            true
        );

    }

    Ok(())
}

pub fn sync_project() -> Result<()> {
    let mut roots = ProjectRoots::new()?;
    roots.set_modules_root()?;

    let manifest_toml: Manifest = Manifest::open_toml(&roots.project_root.join(MANIFEST_FILE))?;

    let indexes: HashMap<String, RemoteRegistryIndex> = fetch_remote_registry_index()?;

    log_message(
        MessageType::Info("Check project's dependencies status".to_string()),
        Some("SYNC"),
        true
    );

    if manifest_toml.dependencies.is_empty() {

        log_message(
            MessageType::Info("Nothing to check: empty dependencies section".to_string()),
            Some("SYNC"),
            true
        );

        return Ok(())
    }

    let mregistry = read_internal_registry(
        &roots.modules_root
            .join(CS_MODULES_FOLDER)
            .join(CS_MODULES_INDEX),
        RegistryMode::ModulesMode
    )?;

    for (d, v) in manifest_toml.dependencies.iter() {
        if let Some(pkg) = indexes.get(d) {
            if let Some(latest) = pkg.versions.last() {
                if v == latest {
                    log_message(
                        MessageType::Info(format!("Module {} is up to date", colored_name!(d))),
                        Some("SYNC"),
                        true
                    );
                } else {
                    log_message(
                        MessageType::Info(
                            format!(
                                "Module {} is outdated. Latest available version: {}",
                                colored_name!(d),
                                colored_version!(latest)
                            )
                        ),
                        Some("SYNC"),
                        true
                    );
                }
            } else {
                log_message(
                    MessageType::Error(format!("Module {}: no available versions are declared in remote registry", colored_name!(d))),
                    Some("SYNC"),
                    true
                );
            }
        } else {
            log_message(
                MessageType::Warning(format!("Module {} not found in remote registry", colored_name!(d))),
                Some("SYNC"),
                true
            );
        }

        let is_in = match mregistry {
            RegistryData::ModulesRegistry(ref map) => {
                if let Some(_) = map.get(d) { true } else { false }
            },
            RegistryData::CacheRegistry(_) => false
        };

        if !is_in {
            log_message(
                MessageType::Warning(format!("Module {} declared in manifest but not available in project environment", colored_name!(d))),
                Some("SYNC"),
                true
            );
        }
    }

    Ok(())
}

pub fn build_from_manifest(global: bool) -> Result<()> { // add plugins installation from manifest when build. If not global in meta or meta does not exists spcify
    let mut roots = ProjectRoots::new()?;
    let lpath = roots.project_root.join(LOCK_FILE);

    create_info_file(&roots.project_root, global)?;
    roots.set_modules_root()?;

    let cache_folder = roots.cache_root.join(CS_MODULES_CACHE_FOLDER);
    let cache_index = cache_folder.join(CS_CACHE_INDEX);
    let modules_folder = roots.modules_root.join(CS_MODULES_FOLDER);
    let modules_index = modules_folder.join(CS_MODULES_INDEX);

    if !cache_folder.is_dir() { fs::create_dir_all(&cache_folder)?; }
    if !modules_folder.is_dir() { fs::create_dir_all(&modules_folder)?; }

    let manifest: Manifest = Manifest::open_toml(&roots.project_root.join(MANIFEST_FILE))?;

    // load lockfile
    let mut lockfile: LockFile = if !lpath.exists() {
        LockFile { version: LOCK_VERSION, ..Default::default() }
    } else {
        LockFile::open_toml(&lpath)?
    };

    let mut mindex = read_internal_registry(&modules_index, RegistryMode::ModulesMode)?;
    let mut cindex = read_internal_registry(&cache_index, RegistryMode::CacheMode)?;
    let mut visited = HashSet::new();

    log_message(
        MessageType::Info("Build dependencies from manifest".to_string()),
        Some("BUILD"),
        true
    );

    for (name, version) in manifest.dependencies.iter() {
        let mversion = resolve_module_version(name, Some(version.clone()))?;

        log_message(
            MessageType::Info("Check and resolve dependencies...".to_string()),
            Some("BUILD"),
            true
        );

        resolve_dependencies(
            &cache_folder,
            &modules_folder,
            name,
            &mversion,
            &mut visited,
            &mut mindex,
            &mut cindex,
            Some(&mut lockfile)
        )?;
    }

    log_message(
        MessageType::Info("Write module's registry".to_string()),
        Some("BUILD"),
        true
    );

    write_internal_registry(&modules_index, mindex)?;
    write_internal_registry(&cache_index, cindex)?;

    // build plugins from manifest
    log_message(
        MessageType::Info("Check for declared plugins".to_string()),
        Some("BUILD"),
        true
    );

    if manifest.plugins.is_empty() {
        log_message(
            MessageType::Info("No plugins declared in Cspm.toml file".to_string()),
            Some("BUILD"),
            true
        );

    } else {
        log_message(
            MessageType::Info("Install plugins declared in Cspm.toml file".to_string()),
            Some("BUILD"),
            true
        );

        let mut rsoptions = vec!["install".to_string()];
        rsoptions.extend(manifest.plugins.clone());
        run_risset(&rsoptions)?;
    }

    log_message(
        MessageType::Info("Update Cspm.lock file".to_string()),
        Some("BUILD"),
        true
    );

    lockfile.plugins = manifest.plugins;
    lockfile.package.retain(|p| p.name != manifest.package.name);
    lockfile.package.push(LockChild {
        name: manifest.package.name.clone(),
        version: manifest.package.version.clone(),
        dependencies: manifest.dependencies.clone()
            .iter()
            .map(|(d, v)| format!("{}@{}", d, v))
            .collect(),
        ..Default::default()
    });

    log_message(
        MessageType::Info("Write Cspm.lock file".to_string()),
        Some("BUILD"),
        true
    );

    LockFile::write_toml(&lpath, &lockfile)?;

    Ok(())
}

pub fn build_from_lock(global: bool) -> Result<()> {
    let mut roots = ProjectRoots::new()?;

    create_info_file(&roots.project_root, global)?;
    roots.set_modules_root()?;

    let mpath = roots.project_root.join(MANIFEST_FILE);
    let lpath = roots.project_root.join(LOCK_FILE);

    log_message(
        MessageType::Info("Build project from Cspm.lock file".to_string()),
        Some("BUILD"),
        true
    );

    if !lpath.exists() {
        let mes_err = log_message(
            MessageType::Error("Cspm.lock file not found".to_string()),
            Some("BUILD"),
            false
        );

        return Err(anyhow::anyhow!(mes_err));
    }

    let manifest: Manifest = Manifest::open_toml(&mpath)?;
    let lockfile: LockFile = LockFile::open_toml(&lpath)?;

    let cache_folder = roots.cache_root.join(CS_MODULES_CACHE_FOLDER);
    let modules_folder = roots.modules_root.join(CS_MODULES_FOLDER);
    let modules_index_path = modules_folder.join(CS_MODULES_INDEX);

    if !cache_folder.exists() { fs::create_dir_all(&cache_folder)?; }
    if !modules_folder.exists() { fs::create_dir_all(&modules_folder)?; }

    let mut mindex = read_internal_registry(&modules_index_path, RegistryMode::ModulesMode)?;

    log_message(
        MessageType::Info("Restoring environment exactly from Cspm.lock...".to_string()),
        Some("BUILD"),
        true
    );

    for pkg in lockfile.package.iter() {
        if pkg.name == manifest.package.name { continue; }

        let pfull_name = format!("{}@{}", pkg.name, pkg.version);
        let cached_module = cache_folder.join(&pfull_name);
        let local_module = modules_folder.join(&pkg.name);


        if !cached_module.exists() {
            log_message(
                MessageType::Info(
                    format!("Downloading exact version {}...", colored_name_version!(pkg.name, pkg.version))
                ),
                Some("BUILD"),
                true
            );

            download_package(&pkg.source, &pkg.name, &pkg.version, &cache_folder, &cached_module)?;

            let downloaded_checksum = computer_checksum(&cached_module)?;
            if downloaded_checksum != pkg.checksum {
                fs::remove_dir_all(&cached_module)?;
                let mes_err = log_message(
                    MessageType::Error(
                        format!(
                            "Checksum mismatch for {}!\n> Expected: {}\n> Got: {}",
                            colored_name_version!(pkg.name, pkg.version),
                            pkg.checksum,
                            downloaded_checksum
                        )
                    ),
                    Some("SECURITY"),
                    false
                );

                return Err(anyhow::anyhow!(mes_err));
            }

            let meta_name = format!(".{}@{}_{}", pkg.name, pkg.version, CS_MODULE_META);
            let meta_file_path = cached_module.join(meta_name);
            let cm = CacheMeta { source: pkg.source.clone(), checksum: pkg.checksum.clone() };
            let meta_json = serde_json::to_string_pretty(&cm)?;
            fs::write(meta_file_path, &meta_json)?;
        }

        if !local_module.exists() {
            log_message(
                MessageType::Info(
                    format!("Extracting {} to cs_modules...", colored_name_version!(pkg.name, pkg.version))
                ),
                Some("BUILD"),
                true
            );

            let mut coptions = CopyOptions::new();
            coptions.content_only = true;
            copy(&cached_module, &local_module, &coptions)?;

            log_message(
                MessageType::Info("Update internal registry".to_string()),
                Some("BUILD"),
                true
            );

            add_entry_to_registry(&pkg.name, &pkg.version, &mut mindex);
        }
    }

    log_message(
        MessageType::Info("Write module's registry".to_string()),
        Some("BUILD"),
        true
    );

    write_internal_registry(&modules_index_path, mindex)?;

    // rebuild plugins
    log_message(
        MessageType::Info("Check for installed plugins".to_string()),
        Some("BUILD"),
        true
    );

    if lockfile.plugins.is_empty() {
        log_message(
            MessageType::Info("No plugins declared in Cspm.lock file".to_string()),
            Some("BUILD"),
            true
        );

    } else {
        log_message(
            MessageType::Info("Install plugins declared in Cspm.lock file".to_string()),
            Some("BUILD"),
            true
        );

        let mut rsoptions = vec!["install".to_string()];
        rsoptions.extend(lockfile.plugins);
        run_risset(&rsoptions)?;
    }

    log_message(
        MessageType::Info("Environment perfectly restored!".to_string()),
        Some("BUILD"),
        true
    );

    Ok(())
}

pub fn reinstall_module(modules: Vec<String>, force: bool) -> Result<()> {
    for module in modules.iter() {
        log_message(
            MessageType::Info(format!("Remove module {}", colored_name!(module))),
            Some("REINSTALL"),
            true
        );

        remove_package(&module, force)?;

        log_message(
            MessageType::Info(format!("Reinstall module {}", colored_name!(module))),
            Some("REINSTALL"),
            true
        );

        let (mname, mversion) = parse_module_name(module);
        let version = if !mversion.is_empty() { Some(mversion) } else { None };
        add_package(&mname, version, force)?;
    }

    Ok(())
}

pub fn run_project(csoptions: &Vec<String>) -> Result<()> {
    let mut roots = ProjectRoots::new()?;
    roots.set_modules_root()?;

    let manifest: Manifest = Manifest::open_toml(&roots.project_root.join(MANIFEST_FILE))?;
    let entry_point: (String, String) = manifest.main.get_entry_point()?;

    // check deps
    log_message(
        MessageType::Info("Check dependencies status".to_string()),
        Some("RUN"),
        true
    );

    check_manifest_deps(&roots.modules_root.join(CS_MODULES_INDEX), &manifest)?;

    log_message(
        MessageType::Info("Project is in a healthy state".to_string()),
        Some("RUN"),
        true
    );

    // run csound
    log_message(
        MessageType::Info("Running csound script".to_string()),
        Some("RUN"),
        true
    );

    run_csound_script(&entry_point, csoptions)?;

    Ok(())
}

pub fn install_plugins(rstoptions: &Vec<String>) -> Result<()> {
    // check if risset is installed
    check_risset()?;

    // run risset
    log_message(
        MessageType::Info("Run plugins installation".to_string()),
        Some("RISSET"),
        true
    );

    if let Ok(()) = run_risset(rstoptions) {
        let mut to_add = HashSet::new();
        let mut to_remove = HashSet::new();
        match rstoptions.contains(&"install".to_string()) {
            true => {
                to_add = rstoptions
                    .iter()
                    .filter(|entry| matches!(entry.as_str(), "install" | "--force" | "-f"))
                    .cloned()
                    .collect();
            },
            false => {
                match rstoptions.contains(&"remove".to_string()) {
                    true => {
                        to_remove = rstoptions
                            .iter()
                            .filter(|entry| !matches!(entry.as_str(), "remove" | "--force" | "-f"))
                            .collect();
                    },
                    false => { }
                }
            }
        };

        if let Ok(proot) = get_root(false, &ProjectRootMode::ProjectRoot) {
            let manifest_path = proot.join(MANIFEST_FILE);
            let lockfile_path = proot.join(LOCK_FILE);
            if manifest_path.exists() && manifest_path.is_file() {

                log_message(
                    MessageType::Info("Update Cspm.toml".to_string()),
                    Some("RISSET"),
                    true
                );

                let mut mtoml = Manifest::open_toml(&manifest_path)?;
                mtoml.plugins.extend(to_add.clone());
                mtoml.plugins.retain(|plug| !to_remove.contains(plug));

                // load lockfile
                log_message(
                    MessageType::Info("Update Cspm.lock".to_string()),
                    Some("RISSET"),
                    true
                );

                let mut lockfile: LockFile = if !lockfile_path.exists() {
                    LockFile { version: LOCK_VERSION, ..Default::default() }
                } else {
                    LockFile::open_toml(&lockfile_path)?
                };

                lockfile.plugins.extend(to_add);
                lockfile.plugins.retain(|plug| !to_remove.contains(plug));

                log_message(
                    MessageType::Info("Write Cspm.toml".to_string()),
                    Some("RISSET"),
                    true
                );

                Manifest::write_toml(&manifest_path, &mtoml)?;

                log_message(
                    MessageType::Info("Write Cspm.lock".to_string()),
                    Some("RISSET"),
                    true
                );

                LockFile::write_toml(&lockfile_path, &lockfile)?;
            }
        }
    }

    Ok(())
}

pub fn publish_module() -> Result<()> {
    let prj_root = get_root(false, &ProjectRootMode::ProjectRoot)?;
    let mpath = prj_root.join(MANIFEST_FILE);
    let lpath = prj_root.join(LOCK_FILE);

    if !mpath.exists() {
        let mes_err = log_message(
            MessageType::Error(
                "Cspm.toml not found. Are you in a valid cspm Csound project?".to_string()
            ),
            Some("PUBLISH"),
            false
        );

        return Err(anyhow::anyhow!(mes_err));
    }

    let mtoml: Manifest = Manifest::open_toml(&mpath)?;
    let name = mtoml.package.name;
    let version = mtoml.package.version;
    let authors = mtoml.package.authors;
    let include = mtoml.package.include;
    let src_folder = mtoml.main.src;

    let mut warnings = 0;
    let mut errors = 0;

    if !lpath.exists() {
        log_message(MessageType::Warning("Cspm.lock file not found".to_string()), Some("PUBLISH"), true);
        warnings += 1;
    }

    if name.is_empty() || version.is_empty() || authors.is_empty() {
        log_message(
            MessageType::Error(
                "Name, version and authors of the module must be specified in Cspm.toml file".to_string()
            ),
            Some("PUBLISH"),
            true
        );
        errors += 1;
    }

    log_message(
        MessageType::Info(format!("Check for remote registry conflicts for {}", colored_name_version!(name, version))),
        Some("PUBLISH"),
        true
    );

    let remote_index: HashMap<String, RemoteRegistryIndex> = fetch_remote_registry_index()?;
    if let Some(entry) = remote_index.get(&name) {
        if entry.authors != authors {
            if entry.versions.contains(&version) {
                log_message(
                    MessageType::Error("Module with same name but different authors already exists in registry".to_string()),
                    Some("PUBLISH"),
                    true
                );

                errors += 1;
            }
        }

        if entry.versions.contains(&version) {
            log_message(
                MessageType::Warning(format!("Version {} already exists. This will be an update", colored_version!(version))),
                Some("PUBLISH"),
                true
            );

            warnings += 1;
        }
    }

    let spath = prj_root.join(src_folder.clone());
    if !spath.exists() {
        log_message(
            MessageType::Warning(format!("Source folder {} not found", src_folder.bold())),
            Some("PUBLISH"),
            true
        );

        warnings += 1;
    }

    for extra_file in include.iter() {
        let pfile = prj_root.join(extra_file);
        if !pfile.exists() && pfile == spath {
            log_message(
                MessageType::Error(
                    format!(
                        "Included {} file in Cspm.toml not found", (pfile.to_string_lossy().to_string()).bold())
                ),
                Some("PUBLISH"),
                true
            );

            warnings += 1;
        }
    }

    let warnings_string = format!("{} WARNINGS", warnings);
    let errors_string = format!("{} ERRORS", errors);
    log_message(
        MessageType::Info(
            format!("Check terminated with: {} and {}", warnings_string.yellow().bold(), errors_string.red().bold())
        ),
        Some("PUBLISH"),
        true
    );

    if errors >= 1 {
        log_message(
            MessageType::Info("Please fix the errors and check again".to_string()),
            Some("PUBLISH"),
            true
        );

        return  Ok(())
    }

    log_message(
        MessageType::Info("Done. To publish your module to the official cs-modules registry:".to_string()),
        Some("PUBLISH"),
        true
    );

    println!("  1. Go to <https://github.com/PasqualeMainolfi/cs-modules> and click 'Fork'");
    println!("  2. Upload the module to your forked repository");
    println!("  3. Add your module and version to the [index.json] file");
    println!("  4. Open a Pull Request to the official repository.");
    println!("Once approved, your module will be available to everyone!");
    println!("Read more about on official git hub repository");

    Ok(())
}

pub fn validate_project() -> Result<()> {
    let mut roots = ProjectRoots::new()?;

    log_message(
        MessageType::Info("Check modules folder".to_string()),
        Some("VALIDATE"),
        true
    );

    let mut empty_mfolder = false;
    if let Err(_) = roots.set_modules_root() {
        log_message(
            MessageType::Warning("Modules folder not found".to_string()),
            Some("VALIDATE"),
            true
        );
        empty_mfolder = true;
    }

    let mfolder = roots.modules_root.join(CS_MODULES_FOLDER);
    if !empty_mfolder {
        if !mfolder.exists() || !mfolder.is_dir() {
            log_message(
                MessageType::Warning("Empty modules folder".to_string()),
                Some("VALIDATE"),
                true
            );
        }

        log_message(
            MessageType::Info("Check module's registry".to_string()),
            Some("VALIDATE"),
            true
        );

    }

    let mindex_path = mfolder.join(CS_MODULES_INDEX);
    let mindex = read_internal_registry(&mindex_path, RegistryMode::ModulesMode).ok();

    log_message(
        MessageType::Info("Check Cspm.toml file".to_string()),
        Some("VALIDATE"),
        true
    );

    let manifest = roots.project_root.join(MANIFEST_FILE);
    let mtoml = match Manifest::open_toml(&manifest) {
        Ok(mnf) => mnf,
        Err(e) => {
            let mes_err = log_message(
                MessageType::Error(format!("Cspm.toml file not found:\n{}", e)),
                Some("VALIDATE"),
                false
            );

            return Err(anyhow::anyhow!(mes_err))
        }
    };

    let mut fix = false;
    if let Some(ref mregistry) = mindex {
        for (dep, ver) in mtoml.dependencies.iter() {
            match mregistry {
                RegistryData::ModulesRegistry(data) => {
                    if let Some(version) = data.get(dep) {
                        if version != ver {
                            log_message(
                                MessageType::Warning(
                                    format!(
                                        "Module {}: declared version {} not found. Found version {}",
                                        colored_name!(dep), colored_version!(ver), colored_version!(version)
                                    )
                                ),
                                Some("VALIDATE"),
                                true
                            );

                            fix = true;
                        }
                    } else {
                        log_message(
                            MessageType::Warning(format!("Module {} not found", colored_name_version!(dep, ver))),
                            Some("VALIDATE"),
                            true
                        );

                        fix = true
                    }
                }
                RegistryData::CacheRegistry(_) => {
                    let mes_err = log_message(
                        MessageType::Error("Module's registry corrupted".to_string()),
                        Some("VALIDATE"),
                        false
                    );

                    return Err(anyhow::anyhow!(mes_err))
                }
            }
        }
    }

    // rebuild
    if !mtoml.dependencies.is_empty() {
        if fix || mindex.is_none() {
            log_message(
                MessageType::Info("Repair project dependencies".to_string()),
                Some("VALIDATE"),
                true
            );

            fs::remove_dir_all(&mfolder)?;
            let lockfile = roots.project_root.join(LOCK_FILE);
            if lockfile.exists() { fs::remove_file(lockfile)?; }
            let pinfo = read_project_info()?;

            log_message(
                MessageType::Info("Rebuild project".to_string()),
                Some("VALIDATE"),
                true
            );

            build_from_manifest(pinfo.global_modules)?;
        }
    } else {
        log_message(
            MessageType::Info("No dependencies to check".to_string()),
            Some("VALIDATE"),
            true
        );
    }

    log_message(
        MessageType::Info("Now the project is in healthy status".to_string()),
        Some("VALIDATE"),
        true
    );

    Ok(())
}
