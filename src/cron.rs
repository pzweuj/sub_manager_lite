use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::{params, Connection};

use crate::models::BillingCycle;

const MAX_CATCHUP_ITERATIONS: usize = 10000;

pub fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

pub fn add_months(d: NaiveDate, months: i32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 + months;
    let year = total.div_euclid(12);
    let month0 = total.rem_euclid(12) as u32;
    let month = month0 + 1;
    let first_of_next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .expect("valid date");
    let last_day = (first_of_next - Duration::days(1)).day();
    let day = d.day().min(last_day);
    NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
}

pub fn add_years(d: NaiveDate, years: i32) -> NaiveDate {
    add_months(d, years * 12)
}

pub fn add_weeks(d: NaiveDate, weeks: i64) -> NaiveDate {
    d + Duration::weeks(weeks)
}

pub fn compute_next_due(ending: NaiveDate, cycle: BillingCycle, interval: i64) -> NaiveDate {
    let interval = if interval < 1 { 1 } else { interval };
    match cycle {
        BillingCycle::Monthly => add_months(ending, interval as i32),
        BillingCycle::Yearly => add_years(ending, interval as i32),
        BillingCycle::Weekly => add_weeks(ending, interval),
    }
}

/// 处理到期订阅：
/// - 自动取消：Active && !auto_renew && ending_date < today → Canceled
/// - 自动续期：Active && auto_renew && ending_date <= today，从到期日累加周期直到 > today
///   返回 (自动取消数量, 自动续期数量)。
pub fn process_due_subscriptions(conn: &mut Connection) -> rusqlite::Result<(usize, usize)> {
    let today = today();
    let today_str = today.format("%Y-%m-%d").to_string();
    let tx = conn.transaction()?;

    let mut cancelled = 0;
    let mut renewed = 0;

    // 自动取消
    let to_cancel: Vec<i64> = {
        let mut stmt = tx.prepare(
            "SELECT id FROM subscription WHERE status = 'ACTIVE' AND auto_renew = 0 AND ending_date < ?1",
        )?;
        let ids = stmt.query_map(params![today_str], |r| r.get::<_, i64>(0))?;
        ids.collect::<Result<Vec<_>, _>>()?
    };

    for id in &to_cancel {
        tx.execute(
            "UPDATE subscription SET status = 'CANCELED' WHERE id = ?1",
            params![id],
        )?;
        cancelled += 1;
    }

    // 自动续期
    let to_renew: Vec<(i64, String, String, i64)> = {
        let mut stmt = tx.prepare(
            "SELECT id, ending_date, billing_cycle, billing_interval FROM subscription
             WHERE status = 'ACTIVE' AND auto_renew = 1 AND ending_date <= ?1",
        )?;
        let rows = stmt.query_map(params![today_str], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    for (id, ending_raw, cycle_raw, interval) in to_renew {
        // A malformed persisted row is a processing failure. Return the error
        // while the transaction is still open so all earlier cancellations or
        // renewals are rolled back together.
        let cycle = BillingCycle::from_db(&cycle_raw).ok_or_else(|| {
            rusqlite::Error::InvalidColumnType(
                2,
                "invalid billing_cycle".to_string(),
                rusqlite::types::Type::Text,
            )
        })?;
        let mut new_date = NaiveDate::parse_from_str(&ending_raw, "%Y-%m-%d").map_err(|_| {
            rusqlite::Error::InvalidColumnType(
                1,
                "invalid ending_date".to_string(),
                rusqlite::types::Type::Text,
            )
        })?;

        let mut steps = 0;
        while new_date <= today && steps < MAX_CATCHUP_ITERATIONS {
            new_date = compute_next_due(new_date, cycle, interval);
            steps += 1;
        }
        if new_date <= today {
            // 达到迭代上限仍未追赶至未来，跳过以防异常数据死循环
            continue;
        }

        let new_str = new_date.format("%Y-%m-%d").to_string();
        tx.execute(
            "UPDATE subscription SET ending_date = ?1 WHERE id = ?2",
            params![new_str, id],
        )?;
        renewed += 1;
    }

    tx.commit()?;
    Ok((cancelled, renewed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn insert_subscription(
        conn: &Connection,
        id: i64,
        ending_date: NaiveDate,
        auto_renew: bool,
        cycle: &str,
    ) {
        conn.execute(
            "INSERT INTO subscription
                (id, name, price, currency, billing_cycle, billing_interval, ending_date, category, status, auto_renew)
             VALUES (?1, ?2, 1, 'CNY', ?3, 1, ?4, '其他', 'ACTIVE', ?5)",
            params![id, format!("test-{id}"), cycle, ending_date.to_string(), auto_renew as i64],
        )
        .unwrap();
    }

    #[test]
    fn processes_expiry_and_renewal_in_one_pass() {
        let mut conn = db::open("sqlite://").unwrap();
        let today = today();
        insert_subscription(&conn, 1, today - Duration::days(1), false, "MONTHLY");
        insert_subscription(&conn, 2, today - Duration::days(14), true, "WEEKLY");

        let result = process_due_subscriptions(&mut conn).unwrap();
        assert_eq!(result, (1, 1));

        let status: String = conn
            .query_row("SELECT status FROM subscription WHERE id = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "CANCELED");
        let renewed_date: String = conn
            .query_row(
                "SELECT ending_date FROM subscription WHERE id = 2",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(NaiveDate::parse_from_str(&renewed_date, "%Y-%m-%d").unwrap() > today);
    }

    #[test]
    fn rolls_back_all_expiry_changes_when_renewal_fails() {
        let mut conn = db::open("sqlite://").unwrap();
        let today = today();
        insert_subscription(&conn, 1, today - Duration::days(1), false, "MONTHLY");
        insert_subscription(&conn, 2, today - Duration::days(14), true, "WEEKLY");
        conn.execute_batch(
            "CREATE TRIGGER fail_renew BEFORE UPDATE OF ending_date ON subscription
             BEGIN SELECT RAISE(ABORT, 'renewal blocked'); END;",
        )
        .unwrap();

        assert!(process_due_subscriptions(&mut conn).is_err());
        let status: String = conn
            .query_row("SELECT status FROM subscription WHERE id = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "ACTIVE");
        let old_date: String = conn
            .query_row(
                "SELECT ending_date FROM subscription WHERE id = 2",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(old_date, (today - Duration::days(14)).to_string());
    }
}
