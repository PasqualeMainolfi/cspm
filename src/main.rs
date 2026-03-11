pub mod cli;
pub mod parser;
pub mod paths;
pub mod core;

use clap::Parser;
use cli::{ CsCli, CsCommands };
use core::{
    create_project,
    add_package,
    remove_package,
    update_package,
    manage_cache,
    sync_project,
    build_from_manifest,
    build_from_lock,
    reinstall_module,
    run_project,
    install_plugins,
    get_cspm_version,
    search_package,
    validate_project,
    install_globally,
    uninstall_globally,
    upgrade_globally
};

fn main() {

    println!("Hello, from cspm!");

    let c = CsCli::parse();

    match c.command {
        // init project_name [--m -module] [--p -project]
        CsCommands::Init { global, name, module_flag, project_flag } => {
            if name.is_empty() {
                eprintln!("[ERROR] Missing project or module name!");
                return;
            }

            let mflag = if !(module_flag ^ project_flag) || !module_flag { false } else { true };
            match mflag {
                false => println!("[INFO] Create new project: {name}"),
                true => println!("[INFO] Create new module: {name}")
            }

            if let Err(e) = create_project(name, mflag, global) {
                eprintln!("[ERROR] Something went wrong while creating project folder: {e}");
                return;
            }
        },
        // add module[@version]
        CsCommands::Add { module, force } => {
            println!("[INFO] Check new module {:?}", module);
            for module_name in module.iter() {
                let msplit: Vec<&str> = module_name.split('@').collect();
                if msplit.len() > 2 || msplit.len() <= 0 {
                    eprintln!("[ERROR] Bad module name syntax. Specify <module_name@version> or <module_name>");
                    return;
                }
                let mname = msplit[0].to_string();
                let version: Option<String> = if msplit.len() == 2 { Some(msplit[1].to_string()) } else { None };
                if let Err(e) = add_package(&mname.clone(), version.clone(), force) {
                    eprintln!("[ERROR] An error occurred while adding the package: {e}");
                    return;
                }
            }
        },
        // reinstall module
        CsCommands::Reinstall { module, force } => {
            println!("[INFO] Reinstall modules {:?}", module);
            if let Err(e) = reinstall_module(module, force) {
                eprintln!("[ERROR] An error occurred while removing the package: {e}");
                return;
            }
        },
        // remove module
        CsCommands::Remove { module, force } => {
            println!("[INFO] Removed module {:?}", module);
            for module_name in module.iter() {
                if let Err(e) = remove_package(&module_name, force) {
                    eprintln!("[ERROR] An error occurred while removing the package: {e}");
                    return;
                }
            }
        },
        // update module
        CsCommands::Update { module, force } => {
            println!("[INFO] Update the project's dependencies {:?}", module);
            if let Err(e) = update_package(module, force) {
                eprintln!("[ERROR] An error occurred while updating the package: {e}");
                return;
            }
        },
        // install modules globally
        CsCommands::Install { module, force } => {
            println!("[INFO] Install modules globally");
            for m in module.iter() {
                if let Err(e) = install_globally(m.clone(), force) {
                    eprintln!("[ERROR] An error occurred while installing globally: {e}");
                    return;
                }
            }
        },
        // uninstall modules globally
        CsCommands::Uninstall { module, force } => {
            println!("[INFO] Install modules globally");
            for m in module {
                if let Err(e) = uninstall_globally(m, force) {
                    eprintln!("[ERROR] An error occurred while uninstalling globally: {e}");
                    return;
                }
            }
        },
        // upgrade modules globally
        CsCommands::Upgrade { module, force } => {
            println!("[INFO] Upgrade modules globally");
            if let Err(e) = upgrade_globally(module, force) {
                eprintln!("[ERROR] An error occurred while upgrading globally: {e}");
                return;
            }
        },
        // manage cache
        CsCommands::Cache { clean, list } => {
            println!("[INFO] Manage cspm cache");
            if let Err(e) = manage_cache(clean, list) {
                eprintln!("[ERROR] An error occurred during cache management: {e}");
                return;
            }
        },
        // check the env dependencies
        CsCommands::Sync => {
            println!("[INFO] Check project's environment status");
            if let Err(e) = sync_project() {
                eprintln!("[ERROR] An error occurred during sync: {e}");
                return;
            }
        },
        // build project from manifest or lock file
        CsCommands::Build { from_lock, global }=> {
            match from_lock {
                true => {
                    println!("[INFO] Read Cspm.lock file and build project");
                    if let Err(e) = build_from_lock(global) {
                        eprintln!("[ERROR] An error occurred building project from Cspm.toml file: {e}");
                        return;
                    }
                },
                false => {
                    println!("[INFO] Read Cspm.toml file and build project");
                    if let Err(e) = build_from_manifest(global) {
                        eprintln!("[ERROR] An error occurred building project from Cspm.lock file: {e}");
                        return;
                    }
                }
            }
        },
        // run csuound project
        CsCommands::Run { csoptions } => {
            println!("[INFO] Run Csound project");
            if let Err(e) = run_project(&csoptions) {
                eprintln!("[ERROR] An error occurred running project: {e}");
                return;
            }
        },
        // run csuound project
        CsCommands::Validate => {
            println!("[INFO] Check Cspm.toml file and fixes issues automatically");
            if let Err(e) = validate_project() {
                eprintln!("[ERROR] An error occurred running project: {e}");
                return;
            }
        },
        // use risset for plugins installation
        CsCommands::Risset { rstoptions } => {
            println!("[INFO] Install plugins using risset");
            if let Err(e) = install_plugins(&rstoptions) {
                eprintln!("[ERROR] An error occurred installing plugins: {e}");
                return;
            }
        },
        // pack module for publish
        CsCommands::Publish => {
            println!("[INFO] Publish Csound module")
        },
        // display module info
        CsCommands::Search { module } => {
            println!("[INFO] Display module info");
            if let Err(e) = search_package(&module) {
                eprintln!("[INFO] Something went wrong while searching package: {}", e);
                return
            }
        },
        // display cspm version
        CsCommands::Version => {
            match get_cspm_version() {
                Ok(version) => println!("[INFO] cspm: Csound Package Manager v{}", version),
                Err(_) => eprintln!("[INFO] Something went wrong: version not found in manifest")
            }
        }
    }

}
