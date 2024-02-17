use rustcracker::components::machine::MachineError;

pub enum VmManageError {
    NotFound,
    AlreadyExists,
    InternalError,
    DatabaseError,
    MachineError,
}

pub type VmManageResult<T> = Result<T, VmManageError>;

impl ToString for VmManageError {
    fn to_string(&self) -> String {
        match self {
            VmManageError::NotFound => "Volume not found".to_string(),
            VmManageError::AlreadyExists => "Volume already exists".to_string(),
            VmManageError::InternalError => "Internal error".to_string(),
            VmManageError::DatabaseError => "Database error".to_string(),
            VmManageError::MachineError => "Machine error".to_string(),
        }
    }
}

impl From<MachineError> for VmManageError {
    fn from(e: MachineError) -> Self {
        VmManageError::MachineError
    }
}

impl From<sqlx::Error> for VmManageError {
    fn from(value: sqlx::Error) -> Self {
        VmManageError::DatabaseError
    }
}