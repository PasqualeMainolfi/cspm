use anyhow::Result;
use serde::{ Deserialize, Serialize };
use colored::*;
use std::{ collections::{ HashMap, HashSet }, fs, path::Path };
use crate::{
    colored_name,
    colored_name_version,
    colored_version,
    external_tools::check_csound_installed,
    registry::{ RegistryData, RegistryMode, LocalRegistry },
    common::{ ProjectRootMode, ManageToml, LogMessageType, log_message, get_root, MANIFEST_FILE },
};


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
                LogMessageType::Error(
                    "Main entry point is empty. Please specify the script entry point (.csd or .osc/.sco)".to_string()
                ),
                None,
                false
            );

            return Err(anyhow::anyhow!(mes_err));
        }

        if self.udo.is_some() {
            log_message(LogMessageType::Warning("Provided .udo as entry point. Nothing to run".to_string()), Some("RUN"), true);
            return Ok(("".to_string(), "".to_string()))
        }

        if self.csd.is_some() && self.orc.is_some() && self.sco.is_some() {
            let mes_err = log_message(LogMessageType::Error("Many entry point specified".to_string()), Some("RUN"), false);
            return Err(anyhow::anyhow!(mes_err))
        }

        if self.csd.is_none() && (self.orc.is_none() || self.sco.is_none()) {
            let mes_err = log_message(LogMessageType::Error("Missing .csd or .orc/.sco entry point".to_string()), Some("RUN"), false);
            return Err(anyhow::anyhow!(mes_err))
        }

        if self.csd.is_some() && (self.sco.is_some() || self.orc.is_some()) {
            log_message(
                LogMessageType::Warning(
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
                LogMessageType::Info(format!("Run {} and {} entry point", entry_point_orc, entry_point_sco)),
                Some("RUN"),
                true
            );

            return Ok((entry_point_orc, entry_point_sco))
        }

        if self.csd.is_some() {
            let entry_point_csd = self.csd.clone().unwrap_or(String::new());

            log_message(
                LogMessageType::Info(format!("Run {} entry point", entry_point_csd)),
                Some("RUN"),
                true
            );

            return Ok((entry_point_csd, String::new()))
        }

        let mes_err = log_message(
            LogMessageType::Error("Failed to run csound script".to_string()),
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
    pub fn check_manifest_deps(&self, modules_folder: &Path) -> Result<()> {
        let mut registry = LocalRegistry::new(modules_folder, RegistryMode::ModulesMode);
        registry.read_internal_registry()?;
        if let Some(rdata) = registry.registry {
            match rdata {
                RegistryData::ModulesRegistry(data) => {
                    for (d, v) in self.dependencies.iter() {
                        if let Some(rvers) = data.get(d) {
                            if v != rvers {

                                let mes_err = log_message(
                                    LogMessageType::Error(
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
                                LogMessageType::Error(
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
                LogMessageType::Error("Failed to read internal registry index".to_string()),
                Some("RESOLVE-DEPS"),
                false
            );

            return Err(anyhow::anyhow!(mes_err))
        }

        Ok(())
    }

    pub fn mandatory_fields_exists(&self) -> bool {
        if self.package.name.is_empty() ||self.package.version.is_empty() ||self.package.authors.is_empty() ||self.package.mode.is_empty() {
            log_message(
                LogMessageType::Error(
                    "Name, version and authors of the module or project must be specified in Cspm.toml file".to_string()
                ),
                None,
                true
            );
            return false
        }
        true
    }

    pub fn src_exists(&self, proot: &Path) -> bool {
        let spath = proot.join(&self.main.src);
        if !spath.exists() {
            log_message(
                LogMessageType::Warning(format!("Source folder {} not found", self.main.src.bold())),
                None,
                true
            );
            return false;
        }
        true
    }

    pub fn included_files_exists(&self, proot: &Path) -> bool {
        let spath = proot.join(&self.main.src);
        if !self.src_exists(proot) { return false }
        let mut flag = false;
        for extra_file in self.package.include.iter() {
            let pfile = proot.join(extra_file);
            if !pfile.exists() && pfile == spath {
                log_message(
                    LogMessageType::Error(
                        format!(
                            "Included {} file in Cspm.toml not found", (pfile.to_string_lossy().to_string()).bold())
                    ),
                    None,
                    true
                );
                flag = true
            }
        }

        if flag { return false }
        true
    }

    pub fn check_module_mode(&self) -> bool {
        match self.package.mode.as_str() {
            "cs-module" => {
                if self.main.udo.is_none() {
                    log_message(LogMessageType::Warning("Declared as cs-module, but entry point .udo not found".to_string()), Some("VALIDATE"), true);
                    return false;
                }
            },
            "cs-project" => {
                if self.main.csd.is_none() || (self.main.orc.is_none() && self.main.sco.is_none()) {
                    log_message(LogMessageType::Warning("Declared as cs-project, but entry point .csd or orc/sco not found".to_string()), Some("VALIDATE"), true);
                    return false
                }
            }
            _ => {
                if self.main.csd.is_none() || (self.main.orc.is_none() && self.main.sco.is_none()) {
                    log_message(LogMessageType::Warning("Document mode in Cspm.toml file must be 'cs-module' or 'cs-project'".to_string()), Some("VALIDATE"), true);
                    return false
                }
            }
        }

        true
    }

    pub fn check_csound_versions(&self) -> bool {
        let cs_version = check_csound_installed();
        match cs_version {
            Some(v) => {
                if v != self.package.cs_version {
                    log_message(
                        LogMessageType::Warning(
                            format!(
                                "Declared Csound version {} in Cspm.toml file and the one detected {} are different",
                                colored_version!(self.package.cs_version),
                                colored_version!(v)
                            )
                        ),
                        Some("BUILD"),
                        true
                    );
                    return false
                }
            },
            None => {
                log_message(
                    LogMessageType::Warning(
                        "Csound version not found".to_string()
                    ),
                    Some("BUILD"),
                    true
                );
                return false
            }
        }

        true
    }

    pub fn update_from_file(&mut self) -> Result<()> {
        let mpath = get_root(false, &ProjectRootMode::ProjectRoot, false)?.join(MANIFEST_FILE);
        let manifest = Manifest::open_toml(&mpath)?;

        self.package = manifest.package;
        self.main = manifest.main;
        self.dependencies = manifest.dependencies;
        self.plugins = manifest.plugins;

        Ok(())
    }

    pub fn add_dependency(&mut self, pname: &str, pversion: &str) {
        self.dependencies
            .entry(pname.to_string())
            .and_modify(|v| *v = pversion.to_string())
            .or_insert(pversion.to_string());
    }

}

impl ManageToml for Manifest {
    fn open_toml(mpath: &Path) -> Result<Self>
    where Self: Sized {
        let mstring = fs::read_to_string(mpath)?;
        let mtoml: Manifest = toml::from_str(&mstring)?;
        Ok(mtoml)
    }

    fn write_toml(mpath: &Path, mtoml: &Self) -> Result<()> {
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
