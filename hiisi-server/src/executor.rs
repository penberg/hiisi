//! Query executor.

use crate::database::{StepResult, Stmt, Type};
use crate::manager::ResourceManager;
use crate::proto;
use crate::{HiisiError, Result};
use std::rc::Rc;

pub struct Request {
    pub database: String,
    pub req: proto::PipelineReqBody,
}

impl Request {
    fn baton(&self) -> String {
        self.req.baton.to_owned().unwrap_or(generate_baton())
    }
}

fn generate_baton() -> String {
    // NOTE: This is different from the baton generation in libSQL server.
    uuid::Uuid::new_v4().to_string()
}

pub fn execute_client_req(
    manager: Rc<ResourceManager>,
    req: Request,
) -> Result<proto::PipelineRespBody> {
    let db_name = &req.database;
    let baton = &req.baton();
    let req = &req.req;
    let mut responses = Vec::new();
    responses
        .try_reserve(req.requests.len())
        .map_err(|_| HiisiError::OutOfMemory)?;
    for req in &req.requests {
        let resp = match req {
            proto::StreamRequest::None => todo!(),
            proto::StreamRequest::Close(_) => exec_close(manager.clone(), db_name, baton)?,
            proto::StreamRequest::Execute(req) => {
                exec_execute(manager.clone(), &req, db_name, baton)?
            }
            proto::StreamRequest::Batch(_) => todo!(),
            proto::StreamRequest::Sequence(_) => todo!(),
            proto::StreamRequest::Describe(_) => todo!(),
            proto::StreamRequest::StoreSql(_) => todo!(),
            proto::StreamRequest::CloseSql(_) => todo!(),
            proto::StreamRequest::GetAutocommit(_) => todo!(),
        };
        responses.push(resp);
    }
    return Ok(proto::PipelineRespBody {
        baton: Some(baton.to_owned()),
        base_url: None,
        results: responses,
    });
}

fn exec_close(
    manager: Rc<ResourceManager>,
    db_name: &str,
    baton: &str,
) -> Result<proto::StreamResult> {
    log::trace!("Closing connection: {} (baton = {})", db_name, baton);
    manager.drop_conn(db_name, baton)?;
    Ok(proto::StreamResult::Ok {
        response: proto::StreamResponse::Close(proto::CloseStreamResp {}),
    })
}

fn exec_execute(
    manager: Rc<ResourceManager>,
    req: &proto::ExecuteStreamReq,
    db_name: &str,
    baton: &str,
) -> Result<proto::StreamResult> {
    log::trace!(
        "Executing SQL statement: {:?} on {} (baton = {}",
        req.stmt,
        db_name,
        baton
    );
    let conn = manager.get_conn(db_name, baton)?;
    let sql = req.stmt.sql.as_ref().ok_or(HiisiError::InternalError(
        "No SQL statement found".to_string(),
    ))?;
    let stmt = conn.prepare(sql)?;
    let result = make_execute_result(stmt)?;
    Ok(result)
}

fn make_execute_result(stmt: Stmt) -> Result<proto::StreamResult> {
    let column_count = stmt.column_count();
    let mut cols = Vec::with_capacity(column_count as usize);
    for i in 0..column_count {
        let name = stmt
            .column_name(i)
            .ok_or(HiisiError::InternalError(format!(
                "No column name found for column {}",
                i
            )))?;
        let decltype = stmt.column_decltype(i);
        let col = proto::Col {
            name: Some(name.into()),
            decltype: decltype.map(Into::into),
        };
        cols.push(col);
    }
    let mut rows = Vec::new();
    loop {
        match stmt.step()? {
            StepResult::Row => {
                let row = to_row(&stmt, column_count)?;
                rows.push(row);
            }
            StepResult::Done => break,
        }
    }
    let resp = proto::ExecuteStreamResp {
        result: proto::StmtResult {
            cols,
            rows,
            affected_row_count: 0,
            last_insert_rowid: None,
            replication_index: None,
            rows_read: 0,
            rows_written: 0,
            query_duration_ms: 0.0,
        },
    };
    Ok(proto::StreamResult::Ok {
        response: proto::StreamResponse::Execute(resp),
    })
}

fn to_row(stmt: &Stmt, column_count: i32) -> Result<proto::Row> {
    let mut values = Vec::new();
    for i in 0..column_count {
        let value = match stmt.column_type(i) {
            Type::Null => proto::Value::Null,
            Type::Integer => {
                let i = stmt.column_int(i);
                proto::Value::Integer { value: i }
            }
            Type::Float => {
                let f = stmt.column_float(i);
                proto::Value::Float { value: f }
            }
            Type::Text => {
                let s = stmt.column_text(i).to_string();
                proto::Value::Text { value: s.into() }
            }
            Type::Blob => {
                let b = stmt.column_blob(i);
                proto::Value::Blob {
                    value: b.to_owned().into(),
                }
            }
        };
        values.push(value);
    }
    Ok(proto::Row { values })
}
