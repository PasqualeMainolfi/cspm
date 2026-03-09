use directories::ProjectDirs;
use anyhow::Result;
use std::{fs, path};

// CSPM INTERNAL STRUCTURE:
// env -> global or local (project)
// cache -> always global (folder that contains a unique json file for cache registry and module folders named pkg_name@pkg_version)
// module folder -> can be global or local (flag -g)

pub const CSPM_MANIFEST: &str = include_str!("../Cargo.toml");
pub const CSD_MAIN_TEMPLATE: &str = include_str!("../templates/main_template.csd");
pub const UDO_MAIN_TEMPLATE: &str = include_str!("../templates/main_template.udo");

pub const LOCK_VERSION: u32 = 1;

pub const CS_MODULES_CACHE_FOLDER: &str = ".cs_modules_cache";
pub const CS_CACHE_INDEX: &str = ".cs_modules_cache_index.json";

pub const CS_MODULE_META: &str = "meta.json";
pub const CS_MODULES_FOLDER: &str = "cs_modules";
pub const CS_MODULES_INDEX: &str = ".cs_modules_index.json";

pub const REGISTRY: &str = ""; // registry
pub const REGISTRY_INDEX: &str = ""; // package indexes

pub const LOCK_FILE: &str = "Cspm.lock";
pub const MANIFEST_FILE: &str = "Cspm.toml";
pub const DEFAULT_SRC_FOLDER: &str = "src";



pub fn get_root(global: bool, mode: &str) -> Result<path::PathBuf> {
    match global {
        true => {
            let pdir = ProjectDirs::from("org", "csound", "cspm").expect("[ERROR] Cannot determine home directory");
            let config_dir = pdir.config_dir();
            if !config_dir.exists() {
                println!("[INFO] Create global cache folder {}", pdir.data_dir().to_string_lossy());
                fs::create_dir_all(config_dir)?;
            }
            println!("[INFO] Global cache folder {}", pdir.data_dir().to_string_lossy());
            Ok(pdir.data_dir().to_path_buf())
        },
        false => {
            let pdir = std::env::current_dir()?;
            println!("[INFO] Local env {} (root for {})", pdir.to_string_lossy(), mode);
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
    pub fn new(global: bool) -> Result<Self> {
        let project_root = get_root(false, "project-folder")?;
        let modules_root = get_root(global, "modules-folder")?;
        let cache_root = get_root(true, "cache-folder")?;
        Ok(Self {
            project_root,
            modules_root,
            cache_root
        })
    }
}
