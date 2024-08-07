use sieve_cache::SieveCache;

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use crate::Result;

// Maximum per database page cache size in kibi-bytes.
const MAX_PAGE_CACHE_SIZE: i64 = 1000;

// Maximum number of resident connections to keep in the cache.
const MAX_MEMORY_RESIDENT_DBS: usize = 10;

// Maximum concurrent connections.
const MAX_CONCURRENT_CONNS: usize = 100;

/// The resource manager is responsible for managing connections to databases,
/// transactions, and more.
pub struct ResourceManager {
    /// A cache of memory resident databases.
    ///
    /// We keep a tuple of database and connection in the cache because we
    /// need at least one connection to SQLite to keep the database in memory.
    memory_resident_dbs: RefCell<SieveCache<String, (Rc<libsql::Database>, Rc<libsql::Connection>)>>,

    /// Open connections to databases.
    ///
    /// This is map from batons to connections. We use batons to identify a
    /// session. SQL statements executed with the same baton are guaranteed
    /// to be executed with the same SQLite connection, ensuring transaction
    /// and isolation guarantees.
    conns: RefCell<SieveCache<String, Rc<libsql::Connection>>>,
}

impl ResourceManager {
    pub fn new() -> Self {
        let memory_resident_dbs = SieveCache::new(MAX_MEMORY_RESIDENT_DBS).unwrap();
        let conns = SieveCache::new(MAX_CONCURRENT_CONNS).unwrap();
        ResourceManager {
            memory_resident_dbs: RefCell::new(memory_resident_dbs),
            conns: RefCell::new(conns),
        }
    }

    pub async fn get_conn(&self, db_name: &str, baton: &str) -> Result<Rc<libsql::Connection>> {
        let mut conns = self.conns.borrow_mut();
        if let Some(conn) = conns.get(baton) {
            return Ok(conn.clone());
        }
        let mut memory_resident_dbs = self.memory_resident_dbs.borrow_mut();
        if let Some((db, _)) = memory_resident_dbs.get(db_name) {
            let conn = Rc::new(db.connect()?);
            conns.insert(baton.to_string(), conn.clone());
            return Ok(conn);
        }
        let (db, placeholder_conn) = self.open_conn(db_name).await?;
        memory_resident_dbs.insert(db_name.to_string(), (db.clone(), placeholder_conn));
        let conn = Rc::new(db.connect()?);
        conns.insert(baton.to_string(), conn.clone());
        Ok(conn)
    }

    async fn open_conn(
        &self,
        db_name: &str,
    ) -> Result<(Rc<libsql::Database>, Rc<libsql::Connection>)> {
        let db_dir = Path::new("data").join(db_name);
        std::fs::create_dir_all(db_dir.as_path()).unwrap();
        let db_path = db_dir.join(format!("{}.db", db_name));
        let db = libsql::Builder::new_local(db_path).build().await.unwrap();
        let conn = db.connect().unwrap();
        conn.query("PRAGMA journal_mode = WAL", libsql::params![])
            .await
            .unwrap();
        let cache_size_pragma = format!("PRAGMA cache_size = -{}", MAX_PAGE_CACHE_SIZE);
        conn.query(&cache_size_pragma, libsql::params![])
            .await
            .unwrap();
        // Enable exclusive file locking.
        conn.query("PRAGMA locking_mode = EXCLUSIVE", libsql::params![])
            .await
            .unwrap();
        Ok((Rc::new(db), Rc::new(conn)))
    }

    pub fn drop_conn(&self, _db_name: &str, baton: &str) -> Result<()> {
        let mut conns = self.conns.borrow_mut();
        conns.remove(baton);
        Ok(())
    }
}
