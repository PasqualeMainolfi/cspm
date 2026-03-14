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
