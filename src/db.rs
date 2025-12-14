use chrono::Utc;
use rusqlite::{params, Connection, Result};

use crate::models::{EmailConfig, Feed, Schedule};

pub fn init_db(path: &str) -> Result<Connection> {
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

    {
        let mut stmt = conn.prepare("PRAGMA table_info(email_config)")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let mut has_column = false;
        for row in rows {
            if row? == "enable_auto_send" {
                has_column = true;
                break;
            }
        }

        if !has_column {
            conn.execute(
                "ALTER TABLE email_config ADD COLUMN enable_auto_send BOOLEAN NOT NULL DEFAULT 0",
                [],
            )?;
        }
    }

    Ok(conn)
}


pub fn add_feed(
    conn: &Connection,
    url: &str,
    name: Option<&str>,
    concurrency_limit: usize,
) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO feeds (url, name, concurrency_limit, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![url, name, concurrency_limit, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

pub fn get_feeds(conn: &Connection) -> Result<Vec<Feed>> {
    let mut stmt = conn.prepare("SELECT id, url, name, concurrency_limit FROM feeds")?;
    let feed_iter = stmt.query_map([], |row| {
        Ok(Feed {
            id: Some(row.get(0)?),
            url: row.get(1)?,
            name: row.get(2)?,
            concurrency_limit: row.get(3)?,
        })
    })?;

    let mut feeds = Vec::new();
    for feed in feed_iter {
        feeds.push(feed?);
    }
    Ok(feeds)
}

pub fn delete_feed(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM feeds WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn add_schedule(conn: &Connection, cron_expression: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO schedules (cron_expression, active, created_at) VALUES (?1, ?2, ?3)",
        params![cron_expression, true, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

pub fn get_schedules(conn: &Connection) -> Result<Vec<Schedule>> {
    let mut stmt = conn.prepare("SELECT id, cron_expression, active FROM schedules")?;
    let schedule_iter = stmt.query_map([], |row| {
        Ok(Schedule {
            id: Some(row.get(0)?),
            cron_expression: row.get(1)?,
            active: row.get(2)?,
        })
    })?;

    let mut schedules = Vec::new();
    for schedule in schedule_iter {
        schedules.push(schedule?);
    }
    Ok(schedules)
}

pub fn delete_schedule(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM schedules WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn get_email_config(conn: &Connection) -> Result<Option<EmailConfig>> {
    let mut stmt = conn.prepare(
        "SELECT smtp_host, smtp_port, smtp_password, email_address, to_email, enable_auto_send FROM email_config WHERE id = 1",
    )?;
    let mut config_iter = stmt.query_map([], |row| {
        Ok(EmailConfig {
            smtp_host: row.get(0)?,
            smtp_port: row.get(1)?,
            smtp_password: row.get(2)?,
            email_address: row.get(3)?,
            to_email: row.get(4)?,
            enable_auto_send: row.get(5).unwrap_or(false),
        })
    })?;

    if let Some(config) = config_iter.next() {
        Ok(Some(config?))
    } else {
        Ok(None)
    }
}

pub fn save_email_config(conn: &Connection, config: &EmailConfig) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO email_config (id, smtp_host, smtp_port, smtp_password, email_address, to_email, enable_auto_send)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            config.smtp_host,
            config.smtp_port,
            config.smtp_password,
            config.email_address,
            config.to_email,
            config.enable_auto_send
        ],
    )?;
    Ok(())
}
