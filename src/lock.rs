use anyhow::Result;
use std::{ fs, path::Path, collections::HashSet };
use serde::{ Deserialize, Serialize };
use crate::common::ManageToml;


#[derive(Default, Serialize, Deserialize)]
pub struct LockFile {
    pub version: u32,

    #[serde(default, rename = "package", skip_serializing_if = "Vec::is_empty")]
    pub package: Vec<LockChild>,

    #[serde(default, rename = "plugins", skip_serializing_if = "HashSet::is_empty")]
    pub plugins: HashSet<String>
}

impl ManageToml for LockFile {
    fn open_toml(mpath: &Path) -> Result<Self>
    where Self: Sized {
        let mstring = fs::read_to_string(mpath)?;
        let mtoml: LockFile = toml::from_str(&mstring)?;
        Ok(mtoml)
    }

    fn write_toml(mpath: &Path, mtoml: &Self) -> Result<()> {
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
