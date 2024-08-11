use std::path::{Path, PathBuf};

use crate::error::HiisiError;
use crate::Result;

pub struct Database {
    path: PathBuf,
}

impl Database {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn connect(&self) -> Result<Connection> {
        Connection::open(&self.path)
    }
}
pub struct Connection {
    conn: *mut libsql_ffi::sqlite3,
}

impl Drop for Connection {
    fn drop(&mut self) {
        unsafe { libsql_ffi::sqlite3_close(self.conn) };
    }
}

impl Connection {
    pub fn open(path: &Path) -> Result<Self> {
        let mut conn = std::ptr::null_mut();
        let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        let flags = libsql_ffi::SQLITE_OPEN_READWRITE
            | libsql_ffi::SQLITE_OPEN_CREATE
            | libsql_ffi::SQLITE_OPEN_NOMUTEX;
        let vfs = std::ptr::null();
        let rc =
            unsafe { libsql_ffi::sqlite3_open_v2(path.as_ptr(), &mut conn, flags.into(), vfs) };
        if rc != libsql_ffi::SQLITE_OK {
            return Err(HiisiError::SqliteError(rc));
        }
        Ok(Self { conn })
    }

    pub fn prepare(&self, sql: &str) -> Result<Stmt> {
        let mut stmt = std::ptr::null_mut();
        let sql = std::ffi::CString::new(sql).unwrap();
        let rc = unsafe {
            libsql_ffi::sqlite3_prepare_v2(
                self.conn,
                sql.as_ptr(),
                -1,
                &mut stmt,
                std::ptr::null_mut(),
            )
        };
        if rc != libsql_ffi::SQLITE_OK {
            return Err(HiisiError::SqliteError(rc));
        }
        Ok(Stmt { stmt })
    }

    pub fn pragma(&self, name: &str, value: impl Into<String>) -> Result<()> {
        let name = std::ffi::CString::new(name).unwrap();
        let rc = unsafe {
            libsql_ffi::sqlite3_exec(
                self.conn,
                format!("PRAGMA {}={}", name.to_str().unwrap(), value.into()).as_ptr() as *const i8,
                None,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if rc != libsql_ffi::SQLITE_OK {
            return Err(HiisiError::SqliteError(rc));
        }
        Ok(())
    }
}

pub enum Type {
    Integer,
    Float,
    Text,
    Blob,
    Null,
}

pub enum StepResult {
    Row,
    Done,
}

pub struct Stmt {
    stmt: *mut libsql_ffi::sqlite3_stmt,
}

impl Drop for Stmt {
    fn drop(&mut self) {
        unsafe { libsql_ffi::sqlite3_finalize(self.stmt) };
    }
}

impl Stmt {
    pub fn step(&self) -> Result<StepResult> {
        let rc = unsafe { libsql_ffi::sqlite3_step(self.stmt) };
        match rc {
            libsql_ffi::SQLITE_ROW => Ok(StepResult::Row),
            libsql_ffi::SQLITE_DONE => Ok(StepResult::Done),
            _ => Err(HiisiError::SqliteError(rc)),
        }
    }

    pub fn column_count(&self) -> i32 {
        unsafe { libsql_ffi::sqlite3_column_count(self.stmt) }
    }

    pub fn column_name(&self, index: i32) -> Option<&str> {
        let name = unsafe { libsql_ffi::sqlite3_column_name(self.stmt, index) };
        if name.is_null() {
            return None;
        }
        let name = unsafe { std::ffi::CStr::from_ptr(name) };
        Some(name.to_str().unwrap())
    }

    pub fn column_decltype(&self, index: i32) -> Option<&str> {
        let decltype = unsafe { libsql_ffi::sqlite3_column_decltype(self.stmt, index) };
        if decltype.is_null() {
            return None;
        }
        let decltype = unsafe { std::ffi::CStr::from_ptr(decltype) };
        Some(decltype.to_str().unwrap())
    }

    pub fn column_type(&self, index: i32) -> Type {
        let ty = unsafe { libsql_ffi::sqlite3_column_type(self.stmt, index) };
        match ty {
            libsql_ffi::SQLITE_INTEGER => Type::Integer,
            libsql_ffi::SQLITE_FLOAT => Type::Float,
            libsql_ffi::SQLITE_TEXT => Type::Text,
            libsql_ffi::SQLITE_BLOB => Type::Blob,
            libsql_ffi::SQLITE_NULL => Type::Null,
            _ => unreachable!(),
        }
    }

    pub fn column_int(&self, index: i32) -> i64 {
        unsafe { libsql_ffi::sqlite3_column_int64(self.stmt, index) }
    }

    pub fn column_float(&self, index: i32) -> f64 {
        unsafe { libsql_ffi::sqlite3_column_double(self.stmt, index) }
    }

    pub fn column_text(&self, index: i32) -> &str {
        let text = unsafe { libsql_ffi::sqlite3_column_text(self.stmt, index) };
        let text = unsafe { std::ffi::CStr::from_ptr(text as *const i8) };
        text.to_str().unwrap()
    }

    pub fn column_blob(&self, index: i32) -> &[u8] {
        let blob = unsafe { libsql_ffi::sqlite3_column_blob(self.stmt, index) };
        let len = unsafe { libsql_ffi::sqlite3_column_bytes(self.stmt, index) };
        unsafe { std::slice::from_raw_parts(blob as *const u8, len as usize) }
    }
}
