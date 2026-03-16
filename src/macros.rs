#[macro_export]
macro_rules! colored_name {
    ($name:expr) => {
        ($name.green())
    };
}

#[macro_export]
macro_rules! colored_version {
    ($ver:expr) => {
        ($ver.blue())
    };
}

#[macro_export]
macro_rules! colored_name_version {
    ($name:expr, $ver:expr) => {
        (format!("{}@{}", colored_name!($name), colored_version!($ver)))
    };
}

#[macro_export]
macro_rules! cmd_exists {
    ($cmd:expr) => {
        process::Command::new($cmd)
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    };
}

#[macro_export]
macro_rules! build_dir {
    ($pth:expr) => {
        (if !$pth.is_dir() { fs::create_dir_all($pth)?; })
    };
}
