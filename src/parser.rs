use anyhow::Result;
use serde::{ Deserialize, Serialize };
use sha2::{ Sha256, Digest };
use walkdir::WalkDir;
use colored::*;
use crate::confres::REMOTE_MREGISTRY_INDEX;
use crate::{ colored_name, colored_version, colored_name_version };
use crate::utils::{ MessageType, log_message, fetch_remote_registry_index };
use std::{ collections::{ HashMap, HashSet }, fs, path };


pub trait ManageToml {
    fn open_toml(mpath: &path::Path) -> Result<Self>
    where Self: Sized;
    fn write_toml(mpath: &path::Path, mtoml: &Self) -> Result<()>;
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
    fn open_toml(pinfo_path: &path::Path) -> Result<Self>
    where Self: Sized {
        let pinfo = fs::read_to_string(pinfo_path)?;
        let ptoml: ProjectInfo = toml::from_str(&pinfo)?;
        Ok(ptoml)
    }

    fn write_toml(pinfo_path: &path::Path, ptoml: &Self) -> Result<()> {
        let ptoml= toml::to_string_pretty::<ProjectInfo>(&ptoml)?;
        std::fs::write(pinfo_path, ptoml)?;
        Ok(())
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct LockFile {
    pub version: u32,

    #[serde(default, rename = "package", skip_serializing_if = "Vec::is_empty")]
    pub package: Vec<LockChild>,

    #[serde(default, rename = "plugins", skip_serializing_if = "HashSet::is_empty")]
    pub plugins: HashSet<String>
}

impl ManageToml for LockFile {
    fn open_toml(mpath: &path::Path) -> Result<Self>
    where Self: Sized {
        let mstring = fs::read_to_string(mpath)?;
        let mtoml: LockFile = toml::from_str(&mstring)?;
        Ok(mtoml)
    }

    fn write_toml(mpath: &path::Path, mtoml: &Self) -> Result<()> {
        let mtoml= toml::to_string_pretty::<LockFile>(&mtoml)?;
        std::fs::write(mpath, mtoml)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct LockChild {
    pub name: String,
    pub version: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub checksum: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>
}

#[derive(Serialize, Deserialize, Default)]
pub struct MainEntry {
    #[serde(default)]
    pub src: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csd: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orc: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sco: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udo: Option<String>
}

impl MainEntry {
    pub fn is_empty(&self) -> bool {
        self.src.is_empty() &&
        self.csd.is_none()  &&
        self.orc.is_none()  &&
        self.sco.is_none()  &&
        self.udo.is_none()
    }

    pub fn get_entry_point(&self) -> Result<(String, String)> {
        if self.is_empty() {
            let mes_err = log_message(
                MessageType::Error(
                    "Main entry point is empty. Please specify the script entry point (.csd or .osc/.sco)".to_string()
                ),
                None,
                false
            );

            return Err(anyhow::anyhow!(mes_err));
        }

        if self.udo.is_some() {
            log_message(MessageType::Warning("Provided .udo as entry point. Nothing to run".to_string()), Some("RUN"), true);
            return Ok(("".to_string(), "".to_string()))
        }

        if self.csd.is_some() && self.orc.is_some() && self.sco.is_some() {
            let mes_err = log_message(MessageType::Error("Many entry point specified".to_string()), Some("RUN"), false);
            return Err(anyhow::anyhow!(mes_err))
        }

        if self.csd.is_none() && (self.orc.is_none() || self.sco.is_none()) {
            let mes_err = log_message(MessageType::Error("Missing .csd or .orc/.sco entry point".to_string()), Some("RUN"), false);
            return Err(anyhow::anyhow!(mes_err))
        }

        if self.csd.is_some() && (self.sco.is_some() || self.orc.is_some()) {
            log_message(
                MessageType::Warning(
                    "Run .csd entry point. Specified .sco or .osc script will be ignored".to_string()
                ),
                Some("RUN"),
                true
            );

            return Ok((self.csd.clone().unwrap_or(String::new()), String::new()))
        }

        if self.orc.is_some() && self.sco.is_some() {
            let entry_point_orc = self.orc.clone().unwrap_or(String::new());
            let entry_point_sco = self.sco.clone().unwrap_or(String::new());

            log_message(
                MessageType::Info(format!("Run {} and {} entry point", entry_point_orc, entry_point_sco)),
                Some("RUN"),
                true
            );

            return Ok((entry_point_orc, entry_point_sco))
        }

        if self.csd.is_some() {
            let entry_point_csd = self.csd.clone().unwrap_or(String::new());

            log_message(
                MessageType::Info(format!("Run {} entry point", entry_point_csd)),
                Some("RUN"),
                true
            );

            return Ok((entry_point_csd, String::new()))
        }

        let mes_err = log_message(
            MessageType::Error("Failed to run csound script".to_string()),
            Some("RUN"),
            false
        );

        return Err(anyhow::anyhow!(mes_err))
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct Manifest {
    #[serde(rename = "package")]
    pub package: MainPackage,

    #[serde(default, skip_serializing_if = "MainEntry::is_empty")]
    pub main: MainEntry,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub dependencies: HashMap<String, String>,

    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub plugins: HashSet<String>
}

impl Manifest {
    pub fn check_manifest_deps(modules_folder: &path::Path, manifest: &Manifest) -> Result<()> {
        let mut registry = Registry::new(modules_folder, RegistryMode::ModulesMode);
        registry.read_internal_registry()?;
        if let Some(rdata) = registry.registry {
            match rdata {
                RegistryData::ModulesRegistry(data) => {
                    for (d, v) in manifest.dependencies.iter() {
                        if let Some(rvers) = data.get(d) {
                            if v != rvers {

                                let mes_err = log_message(
                                    MessageType::Error(
                                        format!(
                                            "The module {} declared in the Cspm.toml has a different version {} than the one installed {}",
                                            colored_name!(d),
                                            colored_version!(v),
                                            colored_version!(rvers)
                                        )
                                    ),
                                    Some("RESOLVE-DEPS"),
                                    false
                                );

                                return Err(anyhow::anyhow!(mes_err))
                            }
                        } else {

                            let mes_err = log_message(
                                MessageType::Error(
                                    format!(
                                        "The module {} declared in the Cspm.toml is not installed",
                                        colored_name_version!(d, v)
                                    )
                                ),
                                Some("RESOLVE-DEPS"),
                                false
                            );

                            return Err(anyhow::anyhow!(mes_err))
                        }
                    }
                }
                RegistryData::CacheRegistry(_) => { }
            }
        } else {

            let mes_err = log_message(
                MessageType::Error("Failed to read internal registry index".to_string()),
                Some("RESOLVE-DEPS"),
                false
            );

            return Err(anyhow::anyhow!(mes_err))
        }

        Ok(())
    }
}

impl ManageToml for Manifest {
    fn open_toml(mpath: &path::Path) -> Result<Self>
    where Self: Sized {
        let mstring = fs::read_to_string(mpath)?;
        let mtoml: Manifest = toml::from_str(&mstring)?;
        Ok(mtoml)
    }

    fn write_toml(mpath: &path::Path, mtoml: &Self) -> Result<()> {
        let mtoml = toml::to_string_pretty::<Manifest>(&mtoml)?;
        std::fs::write(mpath, mtoml)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct MainPackage {
    pub name: String,
    pub version: String,
    pub mode: String,
    pub description: String,
    pub repository: String,
    pub authors: Vec<String>,
    pub license: String,
    pub cs_version: String,
    pub include: Vec<String>
}

#[derive(Serialize, Deserialize)]
pub struct CacheMeta {
    pub source: String,
    pub checksum: String
}

pub enum RegistryMode {
    CacheMode,
    ModulesMode
}

pub enum RegistryData {
    CacheRegistry(HashMap<String, HashSet<String>>),
    ModulesRegistry(HashMap<String, String>)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RemoteRegistryIndex {
    #[serde(default)]
    pub versions: Vec<String>,

    #[serde(default)]
    pub authors: Vec<String>,

    #[serde(default)]
    pub description: String
}

pub struct Registry {
    pub registry: Option<RegistryData>,
    rpath: path::PathBuf,
    rmode: RegistryMode,
}

impl Registry {
    pub fn new(path_to_registry: &path::Path, rmode: RegistryMode) -> Self {
        Self {
            registry: None,
            rpath: path_to_registry.to_path_buf(),
            rmode,
        }
    }

    pub fn read_internal_registry(&mut self) -> Result<()> {
        let condition = self.rpath.exists() && self.rpath.is_file();
        match self.rmode {
            RegistryMode::CacheMode => {
                let mindex: HashMap<String, HashSet<String>> = if condition {
                    let mstring = fs::read_to_string(&self.rpath)?;
                    serde_json::from_str(&mstring)?
                } else {
                    HashMap::new()
                };
                self.registry = Some(RegistryData::CacheRegistry(mindex));
                return Ok(())
            }
            RegistryMode::ModulesMode => {
                let mindex: HashMap<String, String> = if condition {
                    let mstring = fs::read_to_string(&self.rpath)?;
                    serde_json::from_str(&mstring)?
                } else {
                    HashMap::new()
                };
                self.registry = Some(RegistryData::ModulesRegistry(mindex));
                return Ok(())
            }
        }
    }

    pub fn write_internal_registry(&self) -> Result<()> {
        if let Some(ref mindex) = self.registry {
            let mindex_string = match mindex {
                RegistryData::CacheRegistry(map) => {
                    serde_json::to_string_pretty::<HashMap<String, HashSet<String>>>(&map)?
                },
                RegistryData::ModulesRegistry(map) => {
                    serde_json::to_string_pretty::<HashMap<String, String>>(&map)?
                }
            };
            fs::write(&self.rpath, mindex_string)?;
            return Ok(())
        }

        let mes_err = log_message(MessageType::Error("Failed to write registry index".to_string()), None, false);
        Err(anyhow::anyhow!(mes_err))
    }

    pub fn remove_entry_from_registry(&mut self, entry_name: String) {
        if let Some(ref mut registry) = self.registry {
            let (current_name, current_version) = ModuleTools::parse_module_name(&entry_name);
            match registry {
                RegistryData::CacheRegistry(map) => {
                    let mut to_delete = false;
                    if let Some(ref mut vers) = map.get_mut(&current_name) {
                        if vers.contains(&current_version) { vers.remove(&current_version); }
                        to_delete = vers.is_empty();
                    }
                    if to_delete { map.remove(&current_name); }
                },
                RegistryData::ModulesRegistry(map) => {
                    let mut to_delete = false;
                    if let Some(vers) = map.get_mut(&current_name) {
                        if vers == &current_version || current_version.is_empty() { to_delete = true; }
                    }
                    if to_delete { map.remove(&current_name); }
                }
            }
        }
    }

    pub fn add_entry_to_registry(&mut self, entry_name: &str, entry_version: &str) {
        if let Some(ref mut registry) = self.registry {
            match registry {
                RegistryData::CacheRegistry(map) => {
                    map
                        .entry(entry_name.to_string())
                        .and_modify(|v| { v.insert(entry_version.to_string()); })
                        .or_insert_with(|| {
                            let mut hset = HashSet::new();
                            hset.insert(entry_version.to_string());
                            hset
                        });
                },
                RegistryData::ModulesRegistry(map) => {
                    map
                        .entry(entry_name.to_string())
                        .and_modify(|v| *v = entry_version.to_string())
                        .or_insert_with(|| entry_version.to_string());
                }
            }
        }
    }

    pub fn query_registry(&self, pkg_name: &str) -> Option<String> {
        if let Some(ref registry) = self.registry {
            match registry {
                RegistryData::ModulesRegistry(data) => {
                    if let Some(version) = data.get(pkg_name) { return Some(version.clone()) }
                },
                RegistryData::CacheRegistry(_) => { }
            }
        }
        None
    }

    pub fn from_registry_to_list(&self) -> HashSet<String> {
        if let Some(ref registry) = self.registry {
            match registry {
                RegistryData::ModulesRegistry(data) => {
                    return data.iter().map(|(d, v)| format!("{}@{}", d, v)).collect::<HashSet<String>>()
                },
                RegistryData::CacheRegistry(_) => { }
            }
        }
        HashSet::new()
    }
}

pub struct ModuleTools { }

impl ModuleTools {
    pub fn compute_checksum(path_to_module: &path::Path) -> Result<String> {
        let mut hasher = Sha256::new();

        // deterministic
        let mut contents: Vec<_> = WalkDir::new(path_to_module)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_file())
            .collect();

        contents.sort_by(|a, b| a.path().cmp(b.path()));

        for entry in contents {
            if let Ok(rel_path) = entry.path().strip_prefix(path_to_module) {
                hasher.update(rel_path.to_string_lossy().as_bytes());
            }
            let inside = fs::read(entry.path())?;
            hasher.update(inside);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    pub fn resolve_module_version(pname: &str, version: Option<String>) -> Result<String> {

        log_message(
            MessageType::Info("Check for last available version".to_string()),
            Some("RESOLVE-DEPS"),
            true
        );

        let indexes: HashMap<String, RemoteRegistryIndex> = fetch_remote_registry_index(REMOTE_MREGISTRY_INDEX)?;

        if let Some(index) = indexes.get(pname) {
            let versions = &index.versions;
            if let Some(passed_version) = version {
                if versions.contains(&passed_version) {
                    return Ok(passed_version.to_string())
                } else {
                    let mes_err = log_message(
                        MessageType::Error(
                            format!("Version {} for module {} does not exists", colored_name!(pname), colored_version!(passed_version))
                        ),
                        Some("RESOLVE-DEPS"),
                        false
                    );

                    return Err(anyhow::anyhow!(mes_err))
                }
            } else {
                if let Some(latest) = versions.last() {
                    return Ok(latest.clone())
                }
            }
        }

        let mes_err = log_message(
            MessageType::Error(
                format!("Module {} not found in remote registry", colored_name!(pname))
            ),
            Some("RESOLVE-DEPS"),
            false
        );

        return Err(anyhow::anyhow!(mes_err))
    }

    pub fn parse_module_name(package_name: &str) -> (String, String) {
        let mut package_iter = package_name.split('@');
        let name = package_iter.next().unwrap_or("");
        let version = package_iter.next().unwrap_or("");
        (name.to_string(), version.to_string())
    }

}


pub enum VersionStatus {
    Same,
    Young,
    Old
}

#[derive(Debug)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32
}

impl Version {
    pub fn parse(version: &str) -> Result<Version> {
        let splitted_version = version
            .split('.')
            .map(|c| c.parse::<u32>())
            .collect::<Result<Vec<_>, _>>();

        match splitted_version {
            Ok(vers) => {
                Ok(Version { major: vers[0], minor: vers[1], patch: vers[2] })
            },
            Err(_) => {
                let mes_err = log_message(
                    MessageType::Error(
                        "Invalid version. Version must be in numeric format [major.minor.patch]. Semantic version is not allowed".to_string()
                    ),
                    None,
                    false
                );

                return Err(anyhow::anyhow!(mes_err));
            }
        }
    }

    pub fn compare(&self, other: &Version) -> VersionStatus {
        if self.major > other.major { return VersionStatus::Young }
        else if  self.major < other.major { return VersionStatus::Old }
        else {
            if self.minor > other.minor { return VersionStatus::Young }
            else if self.minor < other.minor { return VersionStatus::Old }
            else {
                if self.patch > other.patch { return VersionStatus::Young }
                else if self.patch < other.patch { return VersionStatus::Old }
                else {
                    return VersionStatus::Same
                }
            }
        }
    }
}
