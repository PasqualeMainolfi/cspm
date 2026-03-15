pub mod cli;
pub mod parser;
pub mod paths;
pub mod glb_core;
pub mod prj_core;
pub mod utils;
pub mod macros;

use clap::Parser;
use cli::{ CsCli, CsCommands };
use crate::utils::{ MessageType, log_message };
use crate::glb_core::{
    get_cspm_version,
    search_package,
    manage_cache,
    install_globally,
    uninstall_globally,
    upgrade_globally
};

use prj_core::{
    create_project,
    add_package,
    remove_package,
    update_package,
    sync_project,
    build_from_manifest,
    build_from_lock,
    reinstall_module,
    run_project,
    install_plugins,
    validate_project,
    publish_module
};

fn main() {
    colored::control::set_override(true);

    let c = CsCli::parse();

    match c.command {
        // init project_name [--m -module] [--p -project]
        CsCommands::Init { global, name, module_flag, project_flag } => {
            if name.is_empty() {
                log_message(MessageType::Error("Missing project or module name!".to_string()), None, true);
                return
            }

            let mflag = if !(module_flag ^ project_flag) || !module_flag { false } else { true };
            match mflag {
                false => {
                    log_message(MessageType::Info(format!("Creating new project: {}", name)), None, true);
                },
                true => {
                    log_message(MessageType::Info(format!("Creating new module: {}", name)), None, true);
                }
            }

            if let Err(e) = create_project(name, mflag, global) {
                log_message(MessageType::Error(format!("Failes to create the project folder:\n{}", e)), None, true);
                return
            }
        },
        // add module[@version]
        CsCommands::Add { module, force } => {
            log_message(MessageType::Info(format!("Check new module {:?}", module)), None, true);
            for module_name in module.iter() {
                let msplit: Vec<&str> = module_name.split('@').collect();
                if msplit.len() > 2 || msplit.len() <= 0 {
                    log_message(MessageType::Error("Bad module name syntax. Specify <module_name@version> or <module_name>".to_string()), None, true);
                    return
                }
                let mname = msplit[0].to_string();
                let version: Option<String> = if msplit.len() == 2 { Some(msplit[1].to_string()) } else { None };
                if let Err(e) = add_package(&mname.clone(), version.clone(), force) {
                    log_message(MessageType::Error(format!("Failed to add the module:\n{}", e)), None, true);
                    return
                }
            }
        },
        // reinstall module
        CsCommands::Reinstall { module, force } => {
            log_message(MessageType::Info("Reinstall module".to_string()), None, true);
            if let Err(e) = reinstall_module(module, force) {
                log_message(MessageType::Error(format!("Failed to reinstall the module the module:\n{}", e)), None, true);
                return
            }
        },
        // remove module
        CsCommands::Remove { module, force } => {
            log_message(MessageType::Info("Remove module".to_string()), None, true);
            for module_name in module.iter() {
                if let Err(e) = remove_package(&module_name, force) {
                    log_message(MessageType::Error(format!("Failed to remove the module:\n{}", e)), None, true);
                    return
                }
            }
        },
        // update module
        CsCommands::Update { module, force } => {
            log_message(MessageType::Info("Update project dependencies".to_string()), None, true);
            if let Err(e) = update_package(module, force) {
                log_message(MessageType::Error(format!("Failed to update the module:\n{}", e)), None, true);
                return
            }
        },
        // install modules globally
        CsCommands::Install { module, force } => {
            log_message(MessageType::Info("Install module".to_string()), None, true);
            for m in module.iter() {
                if let Err(e) = install_globally(m.clone(), force) {
                    log_message(MessageType::Error(format!("Failed to install the module:\n{}", e)), None, true);
                    return
                }
            }
        },
        // uninstall modules globally
        CsCommands::Uninstall { module, force } => {
            log_message(MessageType::Info("Uninstall module".to_string()), None, true);
            for m in module {
                if let Err(e) = uninstall_globally(m, force) {
                    log_message(MessageType::Error(format!("Failed to uninstall the module:\n{}", e)), None, true);
                    return
                }
            }
        },
        // upgrade modules globally
        CsCommands::Upgrade { module, force } => {
            log_message(MessageType::Info("Upgrade module".to_string()), None, true);
            if let Err(e) = upgrade_globally(module, force) {
                log_message(MessageType::Error(format!("Failed to upgrade the module:\n{}", e)), None, true);
                return
            }
        },
        // manage cache
        CsCommands::Cache { clean, list } => {
            log_message(MessageType::Info("Manage cspm cache".to_string()), None, true);
            if let Err(e) = manage_cache(clean, list) {
                log_message(MessageType::Error(format!("Failed to manage the cache:\n{}", e)), None, true);
                return
            }
        },
        // check the env dependencies
        CsCommands::Sync => {
            log_message(MessageType::Info("Check project's environment status".to_string()), None, true);
            if let Err(e) = sync_project() {
                log_message(MessageType::Error(format!("Failed to sync the project:\n{}", e)), None, true);
                return
            }
        },
        // build project from manifest or lock file
        CsCommands::Build { from_lock, global }=> {
            match from_lock {
                true => {
                    log_message(MessageType::Info("Read Cspm.lock file and build project".to_string()), None, true);
                    if let Err(e) = build_from_lock(global) {
                        log_message(MessageType::Error(format!("Failed to build the project from Cspm.lock file:\n{}", e)), None, true);
                        return
                    }
                },
                false => {
                    log_message(MessageType::Info("Read Cspm.toml file and build project".to_string()), None, true);
                    if let Err(e) = build_from_manifest(global) {
                        log_message(MessageType::Error(format!("Failed to build the project from Cspm.toml file:\n{}", e)), None, true);
                        return
                    }
                }
            }
        },
        // run csuound project
        CsCommands::Run { csoptions } => {
            log_message(MessageType::Info("Run Csound project".to_string()), None, true);
            if let Err(e) = run_project(&csoptions) {
                log_message(MessageType::Error(format!("Failed to run the project:\n{}", e)), None, true);
                return
            }
        },
        // validate project
        CsCommands::Validate => {
            log_message(MessageType::Info("Check Cspm.toml file and fixes issues automatically".to_string()), None, true);
            if let Err(e) = validate_project() {
                log_message(MessageType::Error(format!("Failed to validate the project:\n{}", e)), None, true);
                return
            }
        },
        // use risset for plugins installation
        CsCommands::Risset { rstoptions } => {
            log_message(MessageType::Info("Install plugins using risset".to_string()), None, true);
            if let Err(e) = install_plugins(&rstoptions) {
                log_message(MessageType::Error(format!("Failed to uninstall the module:\n{}", e)), None, true);
                return
            }
        },
        // validate module structure and metadata before creating a pull request
        CsCommands::Publish => {
            log_message(MessageType::Info("Validate module structure and metadata before creating a Pull Request".to_string()), None, true);
            if let Err(e) = publish_module() {
                log_message(MessageType::Error(format!("Failed to validate:\n{}", e)), None, true);
                return
            }
        },
        // display module info
        CsCommands::Search { module } => {
            log_message(MessageType::Info("Display module info".to_string()), None, true);
            if let Err(e) = search_package(&module) {
                log_message(MessageType::Error(format!("Failed to search the module:\n{}", e)), None, true);
                return
            }
        },
        // display cspm version
        CsCommands::Version => {
            match get_cspm_version() {
                Ok(version) => {
                    log_message(MessageType::Info(format!("cspm: Csound Package Manager v{}", version)), None, true);
                },
                Err(_) => {
                    log_message(MessageType::Error("Version not found".to_string()), None, true);
                }
            }
        }
    }

}
