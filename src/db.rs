use chrono::Utc;
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

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

    Ok(conn)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Feed {
    pub id: Option<i64>,
    pub url: String,
    pub name: Option<String>,
    #[serde(default)]
    pub concurrency_limit: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Schedule {
    pub id: Option<i64>,
    pub cron_expression: String,
    pub active: bool,
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
