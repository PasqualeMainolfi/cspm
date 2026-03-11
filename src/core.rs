use anyhow::Result;
use tar::Builder;
use serde_json;
use toml::Value;
use flate2::{ write::GzEncoder, Compression };
use fs_extra::dir::{ copy, CopyOptions };
use std::{
    fs,
    env,
    path,
    process::Command,
    collections::{
        HashMap,
        HashSet,
        VecDeque
    }
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
    RemoteRegistryIndex,
    RegistryAnswer,
    add_entry_to_registry,
    check_manifest_deps,
    computer_checksum,
    download_package,
    parse_module_name,
    query_registry,
    read_internal_registry,
    remove_entry_from_registry,
    resolve_module_version,
    run_csound_script,
    run_risset,
    write_internal_registry,
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
    REMOTE_REGISTRY_INDEX,
    CS_CACHE_INDEX,
    CS_MODULES_INDEX,
    CSPM_MANIFEST,
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
    let src_file = p_src.join(main_script);
    fs::write(src_file, &main_template)?;

    Ok(())
}

pub fn add_package(name: &str, version: Option<String>, force: bool) -> Result<()> { // use also globally without project
    let mut roots = ProjectRoots::new()?;
    roots.set_modules_root()?;

    let lpath = roots.project_root.join(LOCK_FILE);

    let cache_folder = roots.cache_root.join(CS_MODULES_CACHE_FOLDER);
    let cache_index = cache_folder.join(CS_CACHE_INDEX);
    let modules_folder = roots.modules_root.join(CS_MODULES_FOLDER);
    let modules_index = modules_folder.join(CS_MODULES_INDEX);

    if !cache_folder.is_dir() { fs::create_dir_all(&cache_folder)?; }
    if !modules_folder.is_dir() { fs::create_dir_all(&modules_folder)?; }

    let mversion = resolve_module_version(REMOTE_REGISTRY_INDEX, &name, version)?;
    let manifest_toml = Manifest::open_toml(&roots.project_root.join(MANIFEST_FILE))?;

    if let Some(_) = manifest_toml.dependencies.get(name) {
        println!("[INFO] Remove module {} previously added...", name);
        remove_package(&name, force)?;
    }

    // load lockfile
    let mut lockfile: LockFile = if !lpath.exists() {
        LockFile { version: LOCK_VERSION, package: Vec::new() }
    } else {
        LockFile::open_toml(&lpath)?
    };

    println!("[ADD_MOD::INFO] Check and resolve dependencies...");
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

    println!("[ADD_MOD::INFO] Write module's registry");
    write_internal_registry(&modules_index, mindex)?;
    write_internal_registry(&cache_index, cindex)?;

    println!("[ADD_MOD::INFO] Update Cspm.toml file");
    update_manifest(&name, &mversion)?;

    // update manifest in memory
    let re_manifest_toml = Manifest::open_toml(&roots.project_root.join(MANIFEST_FILE))?; // re-open after changes
    println!("[ADD_MOD::INFO] Update Cspm.lock file");
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

    println!("[ADD_MOD::INFO] Done");
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

fn resolve_dependencies(
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
    let local_module = mfolder.join(&mname);

    let checksum: String;
    let source = REMOTE_REGISTRY.to_string();

    let meta_name = format!(".{}@{}_{}", mname, version, CS_MODULE_META);
    let meta_file_path = cached_module.join(meta_name);

    if !cached_module.exists() {
        println!("[RESOLVE_DEPS::INFO] Module {} not found in cache, downloading...", pfull_name);
        download_package(&source, mname, version, cfolder, &cached_module)?;
        checksum = computer_checksum(&cached_module)?;
        let cm = CacheMeta { source: source.clone(), checksum: checksum.clone() };
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
                    return Err(anyhow::anyhow!(
                        "[RESOLVE_DEPS::ERROR] Dependency conflict: {}@{} requested but {}@{} is already installed",
                        mname, version, mname, v
                    ))
                }
            }
        },
        RegistryData::CacheRegistry(_) => {}
    }

    if !local_module.exists() {
        let mut coptions = CopyOptions::new();
        coptions.content_only = true;
        copy(&cached_module, &local_module, &coptions)?;

        println!("[RESOLVE_DEPS::INFO] Update module's registry");
        add_entry_to_registry(mname, &version, mindex);
    }

    // read manifest
    let mod_manifest = Manifest::open_toml(&cached_module.join(MANIFEST_FILE))?;

    if let Some(lfile) = lockfile.as_mut() {
        // remove old dependencies and add child to lockfile
        println!("[RESOLVE_DEPS::INFO] Add child to Cspm.lock file and remove old dependencies");
        lfile.package.retain(|p| !(p.name == mname && p.version == version));
        lfile.package.push(LockChild {
            name: mname.to_string(),
            version: version.to_string(),
            source,
            checksum,
            dependencies: mod_manifest.dependencies
                .iter()
                .map(|(d, v)| format!("{}@{}", d, v))
                .collect(),
            plugins: mod_manifest.plugins
                .iter()
                .map(|(d, v)| format!("{}@{}", d, v))
                .collect()
        });
    }

    println!("[RESOLVE_DEPS::INFO] Resolving dependencies...");
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
        LockFile { version: LOCK_VERSION, package: Vec::new() }
    } else {
        LockFile::open_toml(&lpath)?
    };

    println!("[REMOVE_MOD::INFO] Remove package {} from cs_modules folder", pname);
    match manifest_toml.dependencies.get(pname) {
        Some(_) => { },
        None => {
            println!("[WARNING] Undeclared module {} in Cspm.toml file", pname);
            return Ok(())
        }
    };

    // delete from modules (also dependencies)
    println!("[REMOVE_MOD::INFO] Remove package {} dependencies", pname);
    let mindex_path = roots.modules_root.join(CS_MODULES_FOLDER).join(CS_MODULES_INDEX);
    let mut mindex = read_internal_registry(&mindex_path, RegistryMode::ModulesMode)?;
    remove_helper(&cs_modules_path, &pname, force, &mut mindex, Some(&mut lockfile))?;

    // update module's registry
    println!("[REMOVE_MOD::INFO] Write module's registry");
    write_internal_registry(&mindex_path, mindex)?;

    // delete from manifest
    println!("[REMOVE_MOD::INFO] Remove package {} from Cspm.toml file", pname);
    manifest_toml.dependencies.remove(pname);

    // update lockfile
    println!("[REMOVE_MOD::INFO] Update Cspm.lock file");
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

// TO BE OPTIMIZED -> maybe use lock to find deps
fn remove_helper(
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
                    println!(
                        "[REMOVE_MOD::WARNING] Module {} removal skipped because the module {} depends on it. Use [--force] if you still want to delete",
                        current,
                        mtoml.package.name
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
                println!("[REMOVE_MOD::INFO] Remove package {}", current.to_string());
                fs::remove_dir_all(&pfolder)?;
                println!("[REMOVE_MOD::INFO] Update project's modules registry");
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
    let prj_root = get_root(false, &ProjectRootMode::ProjectRoot)?;
    let manifest = Manifest::open_toml(&prj_root.join(MANIFEST_FILE))?;
    let installed_modules = manifest.dependencies;

    if let Some(mods) = &modules {
        for module in mods.iter() {
            if !installed_modules.contains_key(module) {
                return Err(anyhow::anyhow!("UPDATE_MOD::[ERROR] Undeclared module {} in Cspm.toml file", module));
            }
        }
    }

    let to_update: Vec<(String, String)> = match modules {
        Some(mods) => {
            installed_modules
                .iter()
                .filter(|(d, _)| mods.contains(&d))
                .map(|(d, v)| (d.to_string(), v.to_string()))
                .collect()
        },
        None => {
            installed_modules
                .iter()
                .map(|(d, v)| (d.to_string(), v.to_string()))
                .collect()
        }
    };

    for (pname, pversion) in to_update {
        println!("[UPDATE_MOD::INFO] Check latest version for module {}", &pname);
        let latest_version = resolve_module_version(REMOTE_REGISTRY_INDEX, &pname, Some(pversion.clone()))?;
        if latest_version == pversion {
            println!("[UPDATE_MOD::INFO] Module {} is up to date", pname);
            continue;
        }

        println!("[UPDATE_MOD::INFO] Remove module {}@{}", &pname, &pversion);
        remove_package(&pname, force)?;

        println!("[UPDATE_MOD::INFO] Update module {} to {}", &pname, &latest_version);
        add_package(&pname, Some(latest_version), force)?;

        println!("[UPDATE_MOD::INFO] Module {} is up to date", &pname);
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

pub fn sync_project() -> Result<()> {
    let mut roots = ProjectRoots::new()?;
    roots.set_modules_root()?;

    let manifest_toml: Manifest = Manifest::open_toml(&roots.project_root.join(MANIFEST_FILE))?;

    let response = reqwest::blocking::get(REMOTE_REGISTRY_INDEX)?;
    let indexes: HashMap<String, Vec<String>> = response.json()?;

    println!("[SYNC::INFO] Check project's dependencies status");
    if manifest_toml.dependencies.is_empty() {
        println!("[SYNC::INFO] Nothing to check: empty dependencies section");
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
            if let Some(latest) = pkg.last() {
                if v == latest {
                    println!("[SYNC::INFO] Module {} is up to date", d);
                } else {
                    println!("[SYNC::INFO] Module {} is outdated. Latest available version: {}", d, latest);
                }
            } else {
                println!("[SYNC::ERROR] Module {}: no available versions are declared in remote registry", d);
            }
        } else {
            println!("[SYNC::WARNING] Module {} not found in remote registry", d);
        }

        let is_in = match mregistry {
            RegistryData::ModulesRegistry(ref map) => {
                if let Some(_) = map.get(d) { true } else { false }
            },
            RegistryData::CacheRegistry(_) => false
        };

        if !is_in {
            println!("[SYNC::WARNING] Module {} declared in manifest but not available in project environment", d);
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
        LockFile { version: LOCK_VERSION, package: Vec::new() }
    } else {
        LockFile::open_toml(&lpath)?
    };

    let mut mindex = read_internal_registry(&modules_index, RegistryMode::ModulesMode)?;
    let mut cindex = read_internal_registry(&cache_index, RegistryMode::CacheMode)?;
    let mut visited = HashSet::new();

    println!("[BUILD::INFO] Build dependencies from manifest");
    for (name, version) in manifest.dependencies.iter() {
        let mversion = resolve_module_version(REMOTE_REGISTRY_INDEX, name, Some(version.clone()))?;

        println!("[BUILD::INFO] Check and resolve dependencies...");
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

    println!("[BUILD::INFO] Write module's registry");
    write_internal_registry(&modules_index, mindex)?;
    write_internal_registry(&cache_index, cindex)?;

    println!("[BUILD::INFO] Update lock file");
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

    println!("[BUILD::INFO] Write lockfile");
    LockFile::write_toml(&lpath, &lockfile)?;

    println!("[BUILD::INFO] Done");
    Ok(())
}

pub fn build_from_lock(global: bool) -> Result<()> { // add plugins installation from lock when build
    let mut roots = ProjectRoots::new()?;

    create_info_file(&roots.project_root, global)?;
    roots.set_modules_root()?;

    let mpath = roots.project_root.join(MANIFEST_FILE);
    let lpath = roots.project_root.join(LOCK_FILE);

    println!("[BUILD::INFO] Build project from lockfile");

    if !lpath.exists() {
        return Err(anyhow::anyhow!("[BUILD::ERROR] Lockfile not found! Run 'cspm build' or 'cspm add' first."));
    }

    let manifest: Manifest = Manifest::open_toml(&mpath)?;
    let lockfile: LockFile = LockFile::open_toml(&lpath)?;

    let cache_folder = roots.cache_root.join(CS_MODULES_CACHE_FOLDER);
    let modules_folder = roots.modules_root.join(CS_MODULES_FOLDER);
    let modules_index_path = modules_folder.join(CS_MODULES_INDEX);

    if !cache_folder.exists() { fs::create_dir_all(&cache_folder)?; }
    if !modules_folder.exists() { fs::create_dir_all(&modules_folder)?; }

    let mut mindex = read_internal_registry(&modules_index_path, RegistryMode::ModulesMode)?;

    println!("[BUILD::INFO] Restoring environment exactly from Cspm.lock...");

    for pkg in lockfile.package.iter() {
        if pkg.name == manifest.package.name { continue; }

        let pfull_name = format!("{}@{}", pkg.name, pkg.version);
        let cached_module = cache_folder.join(&pfull_name);
        let local_module = modules_folder.join(&pkg.name);


        if !cached_module.exists() {
            println!("[BUILD::INFO] Downloading exact version {}...", pfull_name);
            download_package(&pkg.source, &pkg.name, &pkg.version, &cache_folder, &cached_module)?;

            let downloaded_checksum = computer_checksum(&cached_module)?;
            if downloaded_checksum != pkg.checksum {
                fs::remove_dir_all(&cached_module)?;
                return Err(anyhow::anyhow!(
                    "[BUILD::SECURITY_ERROR] Checksum mismatch for {}!\nExpected: {}\nGot: {}",
                    pfull_name, pkg.checksum, downloaded_checksum
                ));
            }

            let meta_name = format!(".{}@{}_{}", pkg.name, pkg.version, CS_MODULE_META);
            let meta_file_path = cached_module.join(meta_name);
            let cm = CacheMeta { source: pkg.source.clone(), checksum: pkg.checksum.clone() };
            let meta_json = serde_json::to_string_pretty(&cm)?;
            fs::write(meta_file_path, &meta_json)?;
        }

        if !local_module.exists() {
            println!("[BUILD::INFO] Extracting {} to cs_modules...", pfull_name);
            let mut coptions = CopyOptions::new();
            coptions.content_only = true;
            copy(&cached_module, &local_module, &coptions)?;

            println!("[BUILD::INFO] Update internal registry");
            add_entry_to_registry(&pkg.name, &pkg.version, &mut mindex);
        }
    }

    println!("[BUILD::INFO] Write module's registry");
    write_internal_registry(&modules_index_path, mindex)?;

    println!("[BUILD::INFO] Environment perfectly restored!");
    println!("[BUILD::INFO] Done");
    Ok(())
}

pub fn reinstall_module(modules: Vec<String>, force: bool) -> Result<()> {
    for module in modules.iter() {
        println!("[REINSTALL::INFO] Remove module {}", module);
        remove_package(&module, force)?;
        println!("[REINSTALL::INFO] Reinstall module {}", module);
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
    println!("[RUN::INFO] Check dependencies status globally installed"); // check also plugins
    check_manifest_deps(&roots.modules_root.join(CS_MODULES_INDEX), &manifest)?;
    println!("[RUN::INFO] Dependencies status: the project is in a healthy state");

    // run csound
    println!("[RUN::INFO] Running csound script");
    run_csound_script(&entry_point, csoptions)?;
    Ok(())
}

pub fn install_plugins(rstoptions: &Vec<String>) -> Result<()> {
    // check if risset is installed

    if Command::new("risset").output().is_err() {
        println!("[RISSET::WARNING] risset not found. Installing...");
        match env::consts::OS {
            "linux" | "macos" => {
                println!("[RISSET::INFO] Install uv");
                Command::new("curl")
                    .args(["-LsSf", "https://astral.sh/uv/install.sh", "|", "sh"])
                    .status()?;
            },
            "windows" => {
                println!("[RISSET::INFO] Install uv");
                Command::new("powershell")
                    .args(["-ExecutionPolicy", "ByPass"])
                    .args(["-c", "irm https://astral.sh/uv/install.ps1", "|", "iex"])
                    .status()?;
            },
            _ => return Err(anyhow::anyhow!("[RISSET::ERROR] Unknown OS"))
        }

        println!("[RISSET::INFO] Install risset");
        Command::new("uv")
            .arg("tool")
            .args(["install", "risset"])
            .status()?;

        println!("[RISSET::INFO] Upgrade risset");
        Command::new("uv")
            .arg("tool")
            .args(["upgrade", "risset"])
            .status()?;
    }

    println!("[RISSET::INFO] risset has been installed");
    // run risset
    println!("[RISSET::INFO] Run plugins installation");
    run_risset(rstoptions)?;
    Ok(())
}

pub fn publish_module() -> Result<()> {
    let prj_root = get_root(false, &ProjectRootMode::ProjectRoot)?;
    let mpath = prj_root.join(MANIFEST_FILE);
    let lpath = prj_root.join(LOCK_FILE);

    if !mpath.exists() {
        return Err(anyhow::anyhow!("[PACKING_MOD::ERROR] Cspm.toml not found. Are you in a valid cspm Csound project?"));
    }

    let mtoml: Manifest = Manifest::open_toml(&mpath)?;
    let name = mtoml.package.name;
    let version = mtoml.package.version;
    let include = mtoml.package.include;

    let pkg_tar_name = format!("{}-{}.tar.gz", name, version);
    let tar_path = prj_root.join(pkg_tar_name.clone());

    println!("[PACKING_MODINFO] Packing module {} version {}", name, version);

    let tar_file = fs::File::create(&tar_path)?;
    let gz_encoder = GzEncoder::new(tar_file, Compression::default());
    let mut builder = Builder::new(gz_encoder);

    builder.append_path_with_name(mpath, MANIFEST_FILE)?;
    if lpath.exists() { builder.append_path_with_name(lpath, LOCK_FILE)?; }

    let src_folder = mtoml.main.src;
    let spath = prj_root.join(src_folder.clone());
    if spath.exists() {
        builder.append_dir_all(src_folder, &spath)?;
    } else {
        println!("[PACKING_MOD::WARNING] Source folder [{}] not found. Packing without it", src_folder);
    }

    for extra_file in include.iter() {
        let pfile = prj_root.join(extra_file);
        if pfile.exists() && pfile != spath {
            if pfile.is_file() {
                builder.append_path_with_name(&pfile, extra_file)?;
            } else {
                builder.append_dir_all(&extra_file, pfile)?;
            }
        } else {
            println!("[PACKING_MOD::WARNING] Included {} not found. Packing without it.", pfile.to_string_lossy().to_string());
        }
    }

    builder.finish()?;

    println!("[PACKING_MOD::INFO] To publish your module to the official Cs-modules registry:");
    println!("  1. Go to https://github.com/csound/modules and click 'Fork'.");
    println!("  2. Upload {} to your forked repository.", pkg_tar_name);
    println!("  3. Add your module and version to the 'index.json' file.");
    println!("  4. Open a Pull Request to the official repository.");
    println!("Once approved, your module will be available to everyone!");

    Ok(())
}

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

pub fn validate_project() -> Result<()> {
    let mut roots = ProjectRoots::new()?;

    println!("[VALIDATE::INFO] Check modules folder");
    if let Err(_) = roots.set_modules_root() {
        return Err(anyhow::anyhow!(
            "[VALIDATE::ERROR] Modules folder not found. Please run 'cspm build' globally or locally to fix this issue"
        ))
    }

    let mfolder = roots.modules_root.join(CS_MODULES_FOLDER);
    if !mfolder.exists() || !mfolder.is_dir() {
        return Err(anyhow::anyhow!(
            "[VALIDATE::ERROR] Modules folder not found. Please run 'cspm build' globally or locally to fix this issue"
        ))
    }

    println!("[VALIDATE::INFO] Check module's registry");
    let mindex_path = mfolder.join(CS_MODULES_INDEX);
    let mindex = read_internal_registry(&mindex_path, RegistryMode::ModulesMode)?;

    println!("[VALIDATE::INFO] Check Cspm.toml file");
    let manifest = roots.project_root.join(MANIFEST_FILE);
    let mtoml = match Manifest::open_toml(&manifest) {
        Ok(mnf) => mnf,
        Err(e) => {
            return Err(anyhow::anyhow!(
                "[VALIDATE::ERROR] Cspm.toml file not found: {}", e
            ))
        }
    };

    let mut fix = false;
    for (dep, ver) in mtoml.dependencies.iter() {
        match mindex {
            RegistryData::ModulesRegistry(ref data) => {
                if let Some(version) = data.get(dep) {
                    if version != ver {
                        println!(
                            "[VALIDATE::WARNING] Module {}: declared version {} not found. Found version {}",
                            dep, ver, version
                        );
                        fix = true;
                    }
                } else {
                    println!("[VALIDATE::WARNING] Module {} version {} not found", dep, ver);
                    fix = true
                }
            }
            RegistryData::CacheRegistry(_) => {
                return Err(anyhow::anyhow!("[VALIDATE::ERROR] Module's registry corrupted"))
            }
        }
    }

    // rebuild
    if fix {
        println!("[VALIDATE::INFO] Repair project dependencies");
        fs::remove_dir_all(&mfolder)?;
        let lockfile = roots.project_root.join(LOCK_FILE);
        if lockfile.exists() { fs::remove_file(lockfile)?; }
        let pinfo = read_project_info()?;
        build_from_manifest(pinfo.global_modules)?;
    }
    println!("[VALIDATE::INFO] The project is in healthy status");
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

    let mut mindex = read_internal_registry(&modules_index, RegistryMode::ModulesMode)?;
    let mut cindex = read_internal_registry(&cache_index, RegistryMode::CacheMode)?;

    let (name, version) = parse_module_name(&module);
    let version = if !version.is_empty() { Some(version) } else { None };
    let mversion = resolve_module_version(REMOTE_REGISTRY_INDEX, &name, version)?;

    match query_registry(&mindex, &name, &mversion) {
        RegistryAnswer::ExistOld | RegistryAnswer::ExistYoung => {
            println!("[INSTALL::INFO] Remove module {}@{} previously added", name, mversion);
            uninstall_globally(name.clone(), force)?;
        }
        RegistryAnswer::ExistSame => {
            println!("[INSTALL::INFO] Module {}@{} already installed", name, mversion);
            return Ok(());
        }
        RegistryAnswer::NotExist => { }
    }

    println!("[INSTALL::INFO] Check and resolve dependencies...");
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

    println!("[ADD_MOD::INFO] Write module's registry");
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
            let (name, version) = parse_module_name(module);
            match query_registry(&registry, &name, &version) {
                RegistryAnswer::ExistYoung => println!("[UPGRADE::INFO] Module {} is up to date", &name),
                RegistryAnswer::ExistOld => { to_update.insert(format!("{}@{}", name, version)); },
                RegistryAnswer::ExistSame => println!("[UPGRADE::INFO] Module {} already exists", &name),
                RegistryAnswer::NotExist => println!("[UPGRADE::INFO] Module {} does not exists. Nothing to do", &name)
            }
        }
    }

    for entry in to_update.iter() {
        println!("[UPGRADE::INFO] Check latest version for module {}", &entry);
        let (name, version) = parse_module_name(entry);
        let latest_version = resolve_module_version(REMOTE_REGISTRY_INDEX, &entry, Some(version.clone()))?;
        if latest_version == version {
            println!("[UPDATE_MOD::INFO] Module {} is up to date", &name);
        } else {
            println!("[UPDATE_MOD::INFO] Remove module {}@{}", &name, &version);
            uninstall_globally(name.clone(), force)?;

            println!("[UPDATE_MOD::INFO] Update module {} to {}", &name, &latest_version);
            install_globally(name, force)?;
        }
    }

    Ok(())
}
