use rusqlite::Connection;

pub fn init_db(path: &str) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS feeds (
            id INTEGER PRIMARY KEY,
            url TEXT NOT NULL UNIQUE,
            name TEXT,
            concurrency_limit INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS schedules (
            id INTEGER PRIMARY KEY,
            cron_expression TEXT NOT NULL,
            active BOOLEAN NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS email_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            smtp_host TEXT NOT NULL,
            smtp_port INTEGER NOT NULL,
            smtp_password TEXT NOT NULL,
            email_address TEXT NOT NULL,
            to_email TEXT NOT NULL,
            enable_auto_send BOOLEAN NOT NULL DEFAULT 0
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS read_it_later (
            id INTEGER PRIMARY KEY,
            url TEXT NOT NULL UNIQUE,
            read BOOLEAN NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        )",
        [],
    )?;
    
    Ok(conn)
}