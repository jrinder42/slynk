use rusqlite::{params, Connection, Result};
use std::path::Path;
use std::time::SystemTime;

/// Database manager for the file index.
pub struct Db {
    conn: Connection,
}

impl Db {
    /// Opens the database and ensures the schema is up to date.
    pub fn new(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        
        // Initialize the index table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS file_index (
                path TEXT PRIMARY KEY,
                mtime INTEGER,
                size INTEGER,
                synced_at INTEGER
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    /// Checks local disk against the index and returns files that need syncing.
    pub fn get_changed_files(&self, root_path: &Path) -> Result<Vec<String>> {
        let mut changed = Vec::new();
        let mut stack = vec![root_path.to_path_buf()];

        while let Some(current_path) = stack.pop() {
            if current_path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&current_path) {
                    for entry in entries.flatten() {
                        stack.push(entry.path());
                    }
                }
            } else {
                let path_str = current_path.to_string_lossy().to_string();
                if let Ok(metadata) = std::fs::metadata(&current_path) {
                    let mtime = metadata.modified().unwrap_or(SystemTime::now())
                        .duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
                    let size = metadata.len() as i64;

                    // Query the DB for this path
                    let mut stmt = self.conn.prepare("SELECT mtime, size FROM file_index WHERE path = ?")?;
                    let mut rows = stmt.query(params![path_str])?;

                    let needs_sync = if let Some(row) = rows.next()? {
                        let saved_mtime: i64 = row.get(0)?;
                        let saved_size: i64 = row.get(1)?;
                        
                        if saved_mtime != mtime {
                            println!("CHANGE DETECTED (mtime): {}", path_str);
                            true
                        } else if saved_size != size {
                            println!("CHANGE DETECTED (size): {}", path_str);
                            true
                        } else {
                            false
                        }
                    } else {
                        println!("NEW FILE DETECTED: {}", path_str);
                        true
                    };

                    if needs_sync {
                        changed.push(path_str);
                    }
                }
            }
        }

        Ok(changed)
    }

    /// Batch updates the index for an entire directory using transactions to release RAM periodically.
    pub fn update_directory_index_batch(&mut self, root_path: &Path, batch_size: u32) -> Result<()> {
        let mut stack = vec![root_path.to_path_buf()];
        let mut counter = 0;

        // Start initial transaction
        let mut tx = self.conn.transaction()?;

        while let Some(current) = stack.pop() {
            if current.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&current) {
                    for entry in entries.flatten() {
                        stack.push(entry.path());
                    }
                }
            } else {
                if let Ok(metadata) = std::fs::metadata(&current) {
                    let mtime = metadata.modified().unwrap_or(SystemTime::now())
                        .duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
                    let size = metadata.len() as i64;
                    let now = chrono::Utc::now().timestamp();

                    tx.execute(
                        "INSERT OR REPLACE INTO file_index (path, mtime, size, synced_at) VALUES (?, ?, ?, ?)",
                        params![current.to_string_lossy().to_string(), mtime, size, now],
                    )?;

                    counter += 1;

                    // Every batch_size files, commit and start a new transaction to flush to disk and free RAM
                    if counter >= batch_size {
                        tx.commit()?;
                        tx = self.conn.transaction()?;
                        counter = 0;
                        println!("Committed batch of {} files to index.", batch_size);
                    }
                }
            }
        }

        // Final commit for any remaining items
        tx.commit()?;
        Ok(())
    }

    /// Clears the entire index.
    pub fn clear_index(&self) -> Result<()> {
        self.conn.execute("DELETE FROM file_index", [])?;
        Ok(())
    }

    /// Removes a specific path or all paths starting with this prefix (for directories) from the index.
    pub fn remove_path(&self, path: &str) -> Result<()> {
        // Remove the exact path
        self.conn.execute("DELETE FROM file_index WHERE path = ?", params![path])?;
        // Also remove any children if it's a directory
        let prefix = format!("{}%", path);
        self.conn.execute("DELETE FROM file_index WHERE path LIKE ?", params![prefix])?;
        Ok(())
    }
}
