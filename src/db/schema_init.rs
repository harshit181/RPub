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
            schedule_type TEXT NOT NULL DEFAULT 'rss',
            created_at TEXT NOT NULL
        )",
        [],
    )?;

    //For init migration ,will move it to external script
    let count: i32 = conn
        .query_row(
            "SELECT count(*) FROM pragma_table_info('schedules') WHERE name='schedule_type'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if count == 0 {
        conn.execute(
            "ALTER TABLE schedules ADD COLUMN schedule_type TEXT NOT NULL DEFAULT 'rss'",
            [],
        )?;
    }

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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS general_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            fetch_since_hours INTEGER NOT NULL DEFAULT 24,
            image_timeout_seconds INTEGER NOT NULL DEFAULT 45
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS feed_processor (
            feed_id INTEGER PRIMARY KEY,
            processor INTEGER NOT NULL DEFAULT 1 CHECK (processor IN (1, 2, 3)),
            custom_config TEXT,
            FOREIGN KEY (feed_id) REFERENCES feeds(id) ON DELETE CASCADE
        )",
        [],
    )?;
    Ok(conn)
}
