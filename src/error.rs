use rustcracker::components::machine::MachineError;
#[derive(Debug)]
pub enum VmManageError {
    NotFound,
    AlreadyExists,
    InternalError,
    DatabaseError(String),
    MachineError(String),
    StorageError(String),
    EnvironVarError(&'static str),
    NetworkError(String),
    IoError(String),
    SerdeError(String),
    EtcdError(String),
}
impl std::fmt::Display for VmManageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            VmManageError::NotFound => "Volume not found".to_string(),
            VmManageError::AlreadyExists => "Volume already exists".to_string(),
            VmManageError::InternalError => "Internal error".to_string(),
            VmManageError::DatabaseError(s) => format!("Database error: {s}"),
            VmManageError::MachineError(s) => format!("Machine start error: {s}"),
            VmManageError::StorageError(s) => format!("Storage provider error: {s}"),
            VmManageError::EnvironVarError(s) => format!("Environmental var error: {s}"),
            VmManageError::NetworkError(s) => format!("Network error: {s}"),
            VmManageError::IoError(s) => format!("Io error: {s}"),
            VmManageError::SerdeError(s) => format!("Serde error: {s}"),
            VmManageError::EtcdError(s) => format!("Etcd error: {s}"),
        };
        write!(f, "{}", s)
    }
}
impl std::error::Error for VmManageError {}

pub type VmManageResult<T> = Result<T, VmManageError>;

impl From<MachineError> for VmManageError {
    fn from(e: MachineError) -> Self {
        VmManageError::MachineError(e.to_string())
    }
}

impl From<sqlx::Error> for VmManageError {
    fn from(e: sqlx::Error) -> Self {
        VmManageError::DatabaseError(e.to_string())
    }
}

impl From<reqwest::Error> for VmManageError {
    fn from(e: reqwest::Error) -> Self {
        VmManageError::StorageError(e.to_string())
    }
}

impl From<std::io::Error> for VmManageError {
    fn from(e: std::io::Error) -> Self {
        VmManageError::IoError(e.to_string())
    }
}

impl From<serde_json::Error> for VmManageError {
    fn from(e: serde_json::Error) -> Self {
        VmManageError::SerdeError(e.to_string())
    }
}

impl From<etcd_client::Error> for VmManageError {
    fn from(e: etcd_client::Error) -> Self {
        VmManageError::EtcdError(e.to_string())
    }
}