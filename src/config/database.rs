use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

pub fn open_db_connection(db_path: &PathBuf, in_mem: bool, create: bool) -> Result<Connection, String> {
	if in_mem {
		Connection::open_in_memory()
			.map_err(|x| format!("Could not get connection to in memory database: {}", x))
	} else {
		let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE;
		if create { flags |= OpenFlags::SQLITE_OPEN_CREATE; }
		Connection::open_with_flags(db_path, flags)
			.map_err(|x| format!("Could not open connection to database in file {}: {}", db_path.to_string_lossy(), x))
	}	
}

pub fn create_tables(c: &Connection) -> Result<(), String> {
	c.execute_batch("BEGIN;
	    CREATE TABLE IF NOT EXISTS indexing (file text, type text PRIMARY KEY, status int);
        CREATE TABLE IF NOT EXISTS mapping (filepath text PRIMARY KEY, fileid text, sample text, type text, status int);
        CREATE TABLE IF NOT EXISTS calling (filepath test PRIMARY KEY, poolid text, sample text, poolsize int, type text, status int);
        CREATE TABLE IF NOT EXISTS extract (filepath test PRIMARY KEY, sample text, status int);
		COMMIT;").map_err(|e| format!("Error in creating db tables: {}", e))
}