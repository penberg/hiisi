use local_sync::semaphore::SemaphorePermit;

pub struct Transaction {
    pub permit: Box<SemaphorePermit>,
}

pub enum TxStmt {
    Begin,
    Commit,
    Rollback,
}

pub fn parse_tx_stmt(sql: &str) -> Option<TxStmt> {
    if stmt_is_begin_tx(sql) {
        Some(TxStmt::Begin)
    } else if stmt_is_commit_tx(sql) {
        Some(TxStmt::Commit)
    } else if stmt_is_rollback_tx(sql) {
        Some(TxStmt::Rollback)
    } else {
        None
    }
}

fn stmt_is_begin_tx(sql: &str) -> bool {
    sql.trim().to_lowercase().starts_with("begin")
}

fn stmt_is_commit_tx(sql: &str) -> bool {
    sql.trim().to_lowercase().starts_with("commit")
}

fn stmt_is_rollback_tx(sql: &str) -> bool {
    sql.trim().to_lowercase().starts_with("rollback")
}
