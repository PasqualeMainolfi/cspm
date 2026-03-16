use directories::ProjectDirs;
use anyhow::Result;
use std::{ fs, path };
use crate::{
    parser::{ ProjectInfo, ManageToml },
    utils::{ MessageType, log_message }
};


pub const CSPM_MANIFEST: &str = include_str!("../Cargo.toml");
pub const CSD_MAIN_TEMPLATE: &str = include_str!("../templates/main_template.csd");
pub const UDO_MAIN_TEMPLATE: &str = include_str!("../templates/main_template.udo");
pub const GITIGNORE_TEMPLATE: &str = include_str!("../templates/.gitignore");

pub const LOCK_VERSION: u32 = 1;
pub const CONFIG_VERSION: u32 = 1;

pub const CS_MODULES_CACHE_FOLDER: &str = "cs_modules_cache";
pub const CS_CACHE_INDEX: &str = ".cs_modules_cache_index.json";

pub const CS_MODULE_META: &str = "meta.json";
pub const CS_MODULES_FOLDER: &str = "cs_modules";
pub const CS_MODULES_INDEX: &str = ".cs_modules_index.json";

pub const REMOTE_REGISTRY: &str = "https://api.github.com/repos/PasqualeMainolfi/cs-modules/contents/modules";
pub const REMOTE_REGISTRY_INDEX: &str = "https://raw.githubusercontent.com/PasqualeMainolfi/cs-modules/main/cs-registry.json";

pub const LOCK_FILE: &str = "Cspm.lock";
pub const MANIFEST_FILE: &str = "Cspm.toml";
pub const DEFAULT_SRC_FOLDER: &str = "src";

pub const PROJECT_INFO_FILE: &str = ".config.toml";

#[derive(Debug)]
pub enum ProjectRootMode {
    CacheRoot,
    ModulesRoot,
    ProjectRoot
}

pub fn get_root(global: bool, mode: &ProjectRootMode) -> Result<path::PathBuf> {
    match global {
        true => {
            let mes_err = log_message(MessageType::Error("Cannot determine home directory".to_string()), None, false);
            let pdir = ProjectDirs::from("org", "csound", "cspm").expect(mes_err.as_str());
            let config_dir = pdir.config_dir();
            if !config_dir.exists() {
                log_message(
                    MessageType::Info(
                        format!("Create global cache root {}", pdir.data_dir().to_string_lossy())
                    ), None, true
                );

                fs::create_dir_all(config_dir)?;
            }

            log_message(
                MessageType::Info(
                    format!("Global cache root {}", pdir.data_dir().to_string_lossy())
                ), None, true
            );

            Ok(pdir.data_dir().to_path_buf())
        },
        false => {
            let pdir = std::env::current_dir()?;

            log_message(
                MessageType::Info(
                    format!("Local env {} (root for {:?})", pdir.to_string_lossy(), mode)
                ), None, true
            );

            Ok(pdir.to_path_buf())
        }
    }
}

pub struct ProjectRoots {
    pub project_root: path::PathBuf,
    pub modules_root: path::PathBuf,
    pub cache_root: path::PathBuf,
}

impl ProjectRoots {
    pub fn new() -> Result<Self> {
        let project_root = get_root(false, &ProjectRootMode::ProjectRoot)?;
        let cache_root = get_root(true, &ProjectRootMode::CacheRoot)?;
        Ok(Self {
            project_root,
            modules_root: path::PathBuf::new(),
            cache_root
        })
    }

    pub fn set_modules_root(&mut self) -> Result<()> {
        let pinfo = ProjectInfo::open_toml(&self.project_root.join(PROJECT_INFO_FILE))?;
        self.modules_root = get_root(pinfo.global_modules, &ProjectRootMode::ModulesRoot)?;
        Ok(())
    }
}

pub fn create_info_file(prj_root: &path::Path, global: bool) -> Result<()> {
    let prj_info_file = prj_root.join(PROJECT_INFO_FILE);
    if !prj_info_file.exists() { fs::File::create(&prj_info_file)?; }
    let prj_info = ProjectInfo { version: CONFIG_VERSION, global_modules: global };
    let prj_toml = toml::to_string_pretty::<ProjectInfo>(&prj_info)?;
    fs::write(&prj_info_file, prj_toml)?;
    Ok(())
}

pub fn create_gitignore_file(prj_root: &path::Path) -> Result<()> {
    let gitignore_path = prj_root.join(".gitignore");
    if !gitignore_path.exists() { fs::File::create(&gitignore_path)?; }
    fs::write(&gitignore_path, GITIGNORE_TEMPLATE)?;
    Ok(())
}
