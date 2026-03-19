use colored::*;
use directories::ProjectDirs;
use anyhow::Result;
use std::{ fs, path::{ Path, PathBuf } };
use serde::{ Deserialize, Serialize };


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

pub const REMOTE_MREGISTRY: &str = "https://api.github.com/repos/PasqualeMainolfi/cs-modules/contents/modules";
pub const REMOTE_PREGISTRY: &str = "https://api.github.com/repos/PasqualeMainolfi/cs-modules/contents/projects";
pub const REMOTE_MREGISTRY_INDEX: &str = "https://raw.githubusercontent.com/PasqualeMainolfi/cs-modules/main/csm-registry.json";
pub const REMOTE_PREGISTRY_INDEX: &str = "https://raw.githubusercontent.com/PasqualeMainolfi/cs-modules/main/csp-registry.json";

pub const LOCK_FILE: &str = "Cspm.lock";
pub const MANIFEST_FILE: &str = "Cspm.toml";
pub const DEFAULT_SRC_FOLDER: &str = "src";

pub const PROJECT_INFO_FILE: &str = ".config.toml";


pub enum LogMessageType {
    Info(String),
    Warning(String),
    Error(String)
}

pub fn log_message(mtype: LogMessageType, context: Option<&str>, display: bool) -> String {
    let cont = match context {
        Some(c) => format!("::{}", c),
        None => "".to_string()
    };

    let m = match mtype {
        LogMessageType::Info(m) => {
            let mtype = format!("[INFO{}]", cont);
            format!("{} {}", mtype.white().bold(), m)
        }
        LogMessageType::Warning(m) => {
            let mtype = format!("[WARNING{}]", cont);
            format!("{} {}", mtype.yellow().bold(), m)
        }
        LogMessageType::Error(m) => {
            let mtype = format!("[ERROR{}]", cont);
            format!("{} {}", mtype.red().bold(), m)
        }
    };

    if display { println!("{}", m); }
    return m;
}

pub trait ManageToml {
    fn open_toml(mpath: &Path) -> Result<Self>
    where Self: Sized;
    fn write_toml(mpath: &Path, mtoml: &Self) -> Result<()>;
}

#[derive(Serialize, Deserialize)]
pub struct GitHubItem {
    pub name: String,
    pub r#type: String,
    pub download_url: Option<String>
}

#[derive(Serialize, Deserialize)]
pub struct ProjectInfo {
    pub version: u32,
    pub global_modules: bool
}

impl ManageToml for ProjectInfo {
    fn open_toml(pinfo_path: &Path) -> Result<Self>
    where Self: Sized {
        let pinfo = fs::read_to_string(pinfo_path)?;
        let ptoml: ProjectInfo = toml::from_str(&pinfo)?;
        Ok(ptoml)
    }

    fn write_toml(pinfo_path: &Path, ptoml: &Self) -> Result<()> {
        let ptoml= toml::to_string_pretty::<ProjectInfo>(&ptoml)?;
        std::fs::write(pinfo_path, ptoml)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum ProjectRootMode {
    CacheRoot,
    ModulesRoot,
    ProjectRoot
}

pub fn get_root(global: bool, mode: &ProjectRootMode, display: bool) -> Result<PathBuf> {
    match global {
        true => {
            let mes_err = log_message(LogMessageType::Error("Cannot determine home directory".to_string()), None, false);
            let pdir = ProjectDirs::from("org", "csound", "cspm").expect(mes_err.as_str());
            let config_dir = pdir.config_dir();
            if !config_dir.exists() {
                log_message(
                    LogMessageType::Info(
                        format!("Create global cache root {}", pdir.data_dir().to_string_lossy())
                    ), None, true
                );

                fs::create_dir_all(config_dir)?;
            }

            if display {
                log_message(
                    LogMessageType::Info(
                        format!("Global cache root {}", pdir.data_dir().to_string_lossy())
                    ), None, true
                );
            }

            Ok(pdir.data_dir().to_path_buf())
        },
        false => {
            let pdir = std::env::current_dir()?;

            if display {
                log_message(
                    LogMessageType::Info(
                        format!("Local env {} (root for {:?})", pdir.to_string_lossy(), mode)
                    ), None, true
                );
            }

            Ok(pdir.to_path_buf())
        }
    }
}

pub struct ProjectRoots {
    pub project_root: PathBuf,
    pub modules_root: PathBuf,
    pub cache_root: PathBuf,
    display: bool
}

impl ProjectRoots {
    pub fn new(display: bool) -> Result<Self> {
        let project_root = get_root(false, &ProjectRootMode::ProjectRoot, display)?;
        let cache_root = get_root(true, &ProjectRootMode::CacheRoot, display)?;
        Ok(Self {
            project_root,
            modules_root: PathBuf::new(),
            cache_root,
            display
        })
    }

    pub fn set_modules_root(&mut self, global: Option<bool>) -> Result<()> {
        let internal_global = if let Some(g) = global { g } else {
            let pinfo = ProjectInfo::open_toml(&self.project_root.join(PROJECT_INFO_FILE))?;
            pinfo.global_modules
        };

        self.modules_root = get_root(internal_global, &ProjectRootMode::ModulesRoot, self.display)?;
        Ok(())
    }
}

pub struct ProjectPaths {
    pub manifest_file: PathBuf,
    pub lock_file: PathBuf,
    pub cache_folder: PathBuf,
    pub cache_registry: PathBuf,
    pub modules_folder: PathBuf,
    pub modules_registry: PathBuf,
    pub project_info_file: PathBuf,
    pub gitignore_file: PathBuf
}

impl ProjectPaths {
    pub fn new(proots: &ProjectRoots) -> Self {
        let cache_folder = proots.cache_root.join(CS_MODULES_CACHE_FOLDER);
        let cache_registry = cache_folder.join(CS_CACHE_INDEX);
        let modules_folder = proots.modules_root.join(CS_MODULES_FOLDER);
        let modules_registry = modules_folder.join(CS_MODULES_INDEX);

        Self {
            manifest_file: proots.project_root.join(MANIFEST_FILE),
            lock_file: proots.project_root.join(LOCK_FILE),
            cache_folder: cache_folder,
            cache_registry,
            modules_folder: modules_folder,
            modules_registry,
            project_info_file: proots.project_root.join(PROJECT_INFO_FILE),
            gitignore_file: proots.project_root.join(".gitignore")
        }
    }
}

pub fn create_info_file(prj_root: &Path, global: bool) -> Result<()> {
    let prj_info_file = prj_root.join(PROJECT_INFO_FILE);
    if !prj_info_file.exists() { fs::File::create(&prj_info_file)?; }
    let prj_info = ProjectInfo { version: CONFIG_VERSION, global_modules: global };
    let prj_toml = toml::to_string_pretty::<ProjectInfo>(&prj_info)?;
    fs::write(&prj_info_file, prj_toml)?;
    Ok(())
}

pub fn create_gitignore_file(prj_root: &Path) -> Result<()> {
    let gitignore_path = prj_root.join(".gitignore");
    if !gitignore_path.exists() { fs::File::create(&gitignore_path)?; }
    fs::write(&gitignore_path, GITIGNORE_TEMPLATE)?;
    Ok(())
}
