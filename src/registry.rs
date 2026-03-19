use colored::*;
use anyhow::Result;
use sha2::{ Sha256, Digest };
use walkdir::WalkDir;
use serde::{ Serialize, Deserialize };
use std::{fs, path::{ Path, PathBuf }, collections::{ HashMap, HashSet }};
use crate::{ LogMessageType, log_message, colored_name, colored_name_version, colored_version, pkg_full_name };
use crate::common::{ GitHubItem, REMOTE_MREGISTRY, REMOTE_MREGISTRY_INDEX } ;


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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoteRegistryIndex {
    #[serde(default)]
    pub versions: Vec<String>,

    #[serde(default)]
    pub authors: Vec<String>,

    #[serde(default)]
    pub description: String
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoteRegistry {
    pub index_url: String,
    pub resources_url: String,
}

impl RemoteRegistry {
    pub fn new(index_url: &str, resources_url: &str) -> Self {
        Self { index_url: index_url.to_string(), resources_url: resources_url.to_string() }
    }

    pub fn fetch_and_get(&self, key: &str) -> Result<RemoteRegistryIndex> {
        let client = reqwest::blocking::Client::new();
        let response = client
           .get(&self.index_url)
           .header("User-Agent", "cspm")
           .send()?
           .error_for_status()?;

        let rjson: HashMap<String, RemoteRegistryIndex> = response.json()?;
        match rjson.get(key) {
            Some(pkg) => {
                return Ok(pkg.clone())
            },
            None => {
                let mes_err = log_message(
                    LogMessageType::Error(format!("{} does not exists in remote registry", colored_name!(key))),
                    None,
                    true
                );
                return Err(anyhow::anyhow!(mes_err))
            }
        }
    }

    fn download_from_helper(url: &str, dest: &Path) -> Result<()> {
        let client = reqwest::blocking::Client::new();
        let response: Vec<GitHubItem> = client
           .get(url)
           .header("User-Agent", "cspm")
           .send()?
           .error_for_status()?
           .json()?;

        fs::create_dir_all(dest)?;

        for item in response {
            let item_destination = dest.join(item.name.clone());
            match item.r#type.as_str() {
                "file" => {
                    if let Some(down_url) = item.download_url {
                        let bytes_response = client
                            .get(down_url)
                            .header("User-Agent", "cspm")
                            .send()?
                            .error_for_status()?
                            .bytes()?;

                        fs::write(item_destination, &bytes_response)?;
                    }
                },
                "dir" => {
                    let sub_url = format!("{}/{}", url, item.name);
                    RemoteRegistry::download_from_helper(&sub_url, &item_destination)?;
                }
                _ => continue
            }
        }

        Ok(())
    }

    pub fn download_from_main_source(&self, dest: PathBuf) -> Result<()> {
        RemoteRegistry::download_from_helper(&self.resources_url, &dest)
    }

    pub fn download_package(&self, name: &str, version: &str, cache_path: &Path, dest: &Path) -> Result<()> {
        let remote_module_url = format!("{}/{}/{}", self.resources_url, name, version);

        log_message(
            LogMessageType::Info(format!("Download module {} from {}", colored_name!(name), self.resources_url)),
            Some("DOWNLOAD"),
            true
        );

        let destination = cache_path.join(dest);
        if let Err(e) = RemoteRegistry::download_from_helper(&remote_module_url, &destination) {
            let mes_err = log_message(
                LogMessageType::Error(format!("Failed to download module:\n{}", e)),
                Some("DOWNLOAD"),
                false
            );

            return Err(anyhow::anyhow!(mes_err))
        }

        Ok(())
    }
}


pub struct LocalRegistry {
    pub registry: Option<RegistryData>,
    rpath: PathBuf,
    rmode: RegistryMode,
}

impl LocalRegistry {
    pub fn new(path_to_registry: &Path, rmode: RegistryMode) -> Self {
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

        let mes_err = log_message(LogMessageType::Error("Failed to write registry index".to_string()), None, false);
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
                    return data.iter().map(|(d, v)| pkg_full_name!(d, v)).collect::<HashSet<String>>()
                },
                RegistryData::CacheRegistry(_) => { }
            }
        }
        HashSet::new()
    }

    pub fn check_version_conflicts(&self, pname: &str, pversion: &str) -> Result<()> {
        if let Some(ref reg) = self.registry {
            match reg {
                RegistryData::ModulesRegistry(map) => {
                    if let Some(v) = map.get(pname) {
                        if v != pversion {

                            let err_mes = log_message(
                                LogMessageType::Error(
                                    format!(
                                        "Dependency conflict: {} requested but {} is already installed",
                                        colored_name_version!(pname, pversion),
                                        colored_name_version!(pname, v)
                                    )
                                ),
                                None,
                                false
                            );

                            return Err(anyhow::anyhow!(err_mes))
                        }
                    }
                },
                RegistryData::CacheRegistry(_) => { }
            }
        } else {
            let err_mes = log_message(
                LogMessageType::Error("Failed to read registry index".to_string()), None, false
            );

            return Err(anyhow::anyhow!(err_mes))
        }

        Ok(())
    }
}

pub struct ModuleTools { }

impl ModuleTools {
    pub fn compute_checksum(path_to_module: &Path) -> Result<String> {
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
            LogMessageType::Info("Check for last available version".to_string()),
            Some("RESOLVE-DEPS"),
            true
        );

        let remote_registry = RemoteRegistry::new(REMOTE_MREGISTRY_INDEX, REMOTE_MREGISTRY);
        // let indexes: HashMap<String, RemoteRegistryIndex> = fetch_remote_registry_index(REMOTE_MREGISTRY_INDEX)?;

        if let Ok(index) = remote_registry.fetch_and_get(pname) {
            let versions = &index.versions;
            if let Some(passed_version) = version {
                if versions.contains(&passed_version) {
                    return Ok(passed_version.to_string())
                } else {
                    let mes_err = log_message(
                        LogMessageType::Error(
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
            LogMessageType::Error(
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
                    LogMessageType::Error(
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
