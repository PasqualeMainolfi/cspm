use directories::ProjectDirs;
use anyhow::Result;
use std::{ fs, path };
use crate::parser::ProjectInfo;
use crate::utils::{ MessageType, log_message };

pub const CSPM_MANIFEST: &str = include_str!("../Cargo.toml");
pub const CSD_MAIN_TEMPLATE: &str = include_str!("../templates/main_template.csd");
pub const UDO_MAIN_TEMPLATE: &str = include_str!("../templates/main_template.udo");

pub const LOCK_VERSION: u32 = 1;

pub const CS_MODULES_CACHE_FOLDER: &str = ".cs_modules_cache";
pub const CS_CACHE_INDEX: &str = ".cs_modules_cache_index.json";

pub const CS_MODULE_META: &str = "meta.json";
pub const CS_MODULES_FOLDER: &str = "cs_modules";
pub const CS_MODULES_INDEX: &str = ".cs_modules_index.json";

pub const REMOTE_REGISTRY: &str = ""; // registry
pub const REMOTE_REGISTRY_INDEX: &str = ""; // package indexes

pub const LOCK_FILE: &str = "Cspm.lock";
pub const MANIFEST_FILE: &str = "Cspm.toml";
pub const DEFAULT_SRC_FOLDER: &str = "src";

pub const PROJECT_INFO_FILE: &str = ".prj.json";

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
        let pinfo = read_project_info()?;
        self.modules_root = get_root(pinfo.global_modules, &ProjectRootMode::ModulesRoot)?;
        Ok(())
    }
}

pub fn create_info_file(prj_root: &path::Path, global: bool) -> Result<()> {
    let prj_info_file = prj_root.join(PROJECT_INFO_FILE);
    if !prj_info_file.exists() { fs::File::create(&prj_info_file)?; }
    let prj_info = ProjectInfo { global_modules: global };
    let prj_json = serde_json::to_string_pretty::<ProjectInfo>(&prj_info)?;
    fs::write(&prj_info_file, prj_json)?;
    Ok(())
}

pub fn read_project_info() -> Result<ProjectInfo> {
    let root = get_root(false, &ProjectRootMode::ProjectRoot)?;
    let pinfo = fs::read_to_string(root.join(PROJECT_INFO_FILE))?;
    let pinfo_json: ProjectInfo = serde_json::from_str(&pinfo)?;
    Ok(pinfo_json)
}
