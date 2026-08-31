use rusqlite::types::Value;
use rusqlite::{params, Connection, OptionalExtension, Row as SqlRow};

use crate::models::{BillingCycle, SubscriptionCreate, SubscriptionStatus, SubscriptionUpdate};

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS subscription (
    id INTEGER PRIMARY KEY,
    name VARCHAR NOT NULL,
    price FLOAT NOT NULL,
    currency VARCHAR NOT NULL,
    billing_cycle VARCHAR(7) NOT NULL,
    billing_interval INTEGER NOT NULL,
    ending_date DATE NOT NULL,
    category VARCHAR NOT NULL,
    status VARCHAR(8) NOT NULL,
    auto_renew BOOLEAN NOT NULL
)
"#;

const SELECT_COLS: &str =
    "id, name, price, currency, billing_cycle, billing_interval, ending_date, category, status, auto_renew";

#[derive(Debug, Clone)]
pub struct SubRow {
    pub id: i64,
    pub name: String,
    pub price: f64,
    pub currency: String,
    pub billing_cycle: BillingCycle,
    pub billing_interval: i64,
    pub ending_date: String,
    pub category: String,
    pub status: SubscriptionStatus,
    pub auto_renew: bool,
}

fn map_row(row: &SqlRow) -> rusqlite::Result<SubRow> {
    let cycle_raw: String = row.get(4)?;
    let status_raw: String = row.get(8)?;
    Ok(SubRow {
        id: row.get(0)?,
        name: row.get(1)?,
        price: row.get(2)?,
        currency: row.get(3)?,
        billing_cycle: BillingCycle::from_db(&cycle_raw).ok_or_else(|| {
            rusqlite::Error::InvalidColumnType(
                4,
                "bad billing_cycle".into(),
                rusqlite::types::Type::Text,
            )
        })?,
        billing_interval: row.get(5)?,
        ending_date: row.get(6)?,
        category: row.get(7)?,
        status: SubscriptionStatus::from_db(&status_raw).ok_or_else(|| {
            rusqlite::Error::InvalidColumnType(8, "bad status".into(), rusqlite::types::Type::Text)
        })?,
        auto_renew: {
            let v: i64 = row.get(9)?;
            v != 0
        },
    })
}

/// 从 DATABASE_URL（SQLAlchemy 风格 sqlite://...）解析出 SQLite 文件路径。
pub fn db_path_from_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("sqlite://") {
        if rest.is_empty() {
            return ":memory:".to_string();
        }
        if let Some(p) = rest.strip_prefix('/') {
            return p.to_string();
        }
        return rest.to_string();
    }
    url.to_string()
}

pub fn open(url: &str) -> rusqlite::Result<Connection> {
    let conn = Connection::open(db_path_from_url(url))?;
    conn.execute_batch(SCHEMA)?;
    migrate(&conn)?;
    Ok(conn)
}

/// Apply idempotent migrations needed by databases created by older Python versions.
///
/// The first Python schema did not contain `billing_interval`; the current Python
/// application adds it with a default of one. Keep the same behavior so an existing
/// database can be mounted by the Rust service without a manual migration step.
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    let has_interval: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('subscription') WHERE name = 'billing_interval')",
        [],
        |row| row.get(0),
    )?;
    if !has_interval {
        tx.execute(
            "ALTER TABLE subscription ADD COLUMN billing_interval INTEGER DEFAULT 1 NOT NULL",
            [],
        )?;
    }
    tx.commit()
}

pub fn insert(conn: &Connection, c: &SubscriptionCreate) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO subscription
         (name, price, currency, billing_cycle, billing_interval, ending_date, category, status, auto_renew)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            c.name,
            c.price,
            c.currency,
            c.billing_cycle.db_value(),
            c.billing_interval,
            c.ending_date,
            c.category,
            c.status.db_value(),
            c.auto_renew as i64,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<SubRow>> {
    let sql = format!("SELECT {SELECT_COLS} FROM subscription WHERE id = ?1");
    conn.query_row(&sql, params![id], map_row).optional()
}

pub fn update(conn: &Connection, id: i64, u: &SubscriptionUpdate) -> rusqlite::Result<bool> {
    let mut sets: Vec<String> = Vec::new();
    let mut vals: Vec<Value> = Vec::new();

    if let Some(v) = &u.name {
        sets.push("name = ?".into());
        vals.push(Value::Text(v.clone()));
    }
    if let Some(v) = u.price {
        sets.push("price = ?".into());
        vals.push(Value::Real(v));
    }
    if let Some(v) = &u.currency {
        sets.push("currency = ?".into());
        vals.push(Value::Text(v.clone()));
    }
    if let Some(v) = u.billing_cycle {
        sets.push("billing_cycle = ?".into());
        vals.push(Value::Text(v.db_value().to_string()));
    }
    if let Some(v) = u.billing_interval {
        sets.push("billing_interval = ?".into());
        vals.push(Value::Integer(v));
    }
    if let Some(v) = &u.ending_date {
        sets.push("ending_date = ?".into());
        vals.push(Value::Text(v.clone()));
    }
    if let Some(v) = &u.category {
        sets.push("category = ?".into());
        vals.push(Value::Text(v.clone()));
    }
    if let Some(v) = u.status {
        sets.push("status = ?".into());
        vals.push(Value::Text(v.db_value().to_string()));
    }
    if let Some(v) = u.auto_renew {
        sets.push("auto_renew = ?".into());
        vals.push(Value::Integer(v as i64));
    }

    if sets.is_empty() {
        return Ok(get(conn, id)?.is_some());
    }

    let sql = format!("UPDATE subscription SET {} WHERE id = ?", sets.join(", "));
    vals.push(Value::Integer(id));
    let changed = conn.execute(&sql, rusqlite::params_from_iter(vals))?;
    Ok(changed > 0)
}

pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    let changed = conn.execute("DELETE FROM subscription WHERE id = ?1", params![id])?;
    Ok(changed > 0)
}

pub fn set_status(
    conn: &Connection,
    id: i64,
    status: SubscriptionStatus,
    clear_auto_renew: bool,
) -> rusqlite::Result<bool> {
    let changed = if clear_auto_renew {
        conn.execute(
            "UPDATE subscription SET status = ?1, auto_renew = 0 WHERE id = ?2",
            params![status.db_value(), id],
        )?
    } else {
        conn.execute(
            "UPDATE subscription SET status = ?1 WHERE id = ?2",
            params![status.db_value(), id],
        )?
    };
    Ok(changed > 0)
}

pub fn list(
    conn: &Connection,
    name: Option<&str>,
    status: Option<SubscriptionStatus>,
) -> rusqlite::Result<Vec<SubRow>> {
    let mut sql = format!("SELECT {SELECT_COLS} FROM subscription");
    let mut clauses: Vec<String> = Vec::new();
    let mut vals: Vec<Value> = Vec::new();

    if let Some(n) = name {
        clauses.push("name LIKE ? ESCAPE '\\'".into());
        vals.push(Value::Text(format!("%{n}%")));
    }
    if let Some(s) = status {
        clauses.push("status = ?".into());
        vals.push(Value::Text(s.db_value().to_string()));
    }
    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    sql.push_str(" ORDER BY ending_date ASC");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(vals), map_row)?;
    rows.collect()
}

pub fn upcoming(conn: &Connection, today: &str, end: &str) -> rusqlite::Result<Vec<SubRow>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM subscription WHERE status = 'ACTIVE' AND ending_date >= ?1 AND ending_date <= ?2 ORDER BY ending_date ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![today, end], map_row)?;
    rows.collect()
}

pub fn expired(conn: &Connection, today: &str) -> rusqlite::Result<Vec<SubRow>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM subscription WHERE status = 'ACTIVE' AND ending_date < ?1 ORDER BY ending_date DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![today], map_row)?;
    rows.collect()
}

pub fn active_unexpired(conn: &Connection, today: &str) -> rusqlite::Result<Vec<SubRow>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM subscription WHERE status = 'ACTIVE' AND ending_date >= ?1 ORDER BY ending_date ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![today], map_row)?;
    rows.collect()
}

pub fn to_out(r: &SubRow) -> crate::models::SubscriptionOut {
    crate::models::SubscriptionOut {
        name: r.name.clone(),
        price: r.price,
        currency: r.currency.clone(),
        billing_cycle: r.billing_cycle,
        billing_interval: r.billing_interval,
        ending_date: r.ending_date.clone(),
        category: r.category.clone(),
        status: r.status,
        auto_renew: r.auto_renew,
        id: r.id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE subscription (
                id INTEGER PRIMARY KEY,
                name VARCHAR NOT NULL,
                price FLOAT NOT NULL,
                currency VARCHAR NOT NULL,
                billing_cycle VARCHAR(7) NOT NULL,
                ending_date DATE NOT NULL,
                category VARCHAR NOT NULL,
                status VARCHAR(8) NOT NULL,
                auto_renew BOOLEAN NOT NULL
            );
            INSERT INTO subscription
                (id, name, price, currency, billing_cycle, ending_date, category, status, auto_renew)
            VALUES (7, 'Legacy', 9.9, 'CNY', 'MONTHLY', '2099-01-01', '其他', 'ACTIVE', 0);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn migrates_legacy_schema_and_is_idempotent() {
        let conn = legacy_connection();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();

        let row = get(&conn, 7).unwrap().unwrap();
        assert_eq!(row.id, 7);
        assert_eq!(row.name, "Legacy");
        assert_eq!(row.billing_interval, 1);

        let column_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('subscription') WHERE name = 'billing_interval'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(column_count, 1);
    }

    #[test]
    fn open_creates_current_schema() {
        let conn = open("sqlite://").unwrap();
        let column: String = conn
            .query_row(
                "SELECT name FROM pragma_table_info('subscription') WHERE name = 'billing_interval'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(column, "billing_interval");
    }
}
