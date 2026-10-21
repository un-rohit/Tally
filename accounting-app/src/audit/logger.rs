use crate::error::Result;
use rusqlite::{params, Connection};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub struct AuditLogger;

impl AuditLogger {
    pub const GENESIS_HASH: &'static str = "0000000000000000000000000000000000000000000000000000000000000000";

    /// Logs an immutable, cryptographically chained audit record.
    pub fn log(
        conn: &Connection,
        company_id: &str,
        entity_type: &str,
        entity_id: &str,
        action: &str,
        user_id: Option<&str>,
        data_before: Option<&Value>,
        data_after: Option<&Value>,
    ) -> Result<String> {
        // Fetch last hash in chain for this company
        let prev_hash: String = conn
            .query_row(
                "SELECT record_hash FROM audit_trail 
                 WHERE company_id = ?1 
                 ORDER BY log_id DESC LIMIT 1",
                params![company_id],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| Self::GENESIS_HASH.to_string());

        let before_json = data_before.map(|v| v.to_string());
        let after_json = data_after.map(|v| v.to_string());

        // Calculate SHA-256 hash of this record chained with prev_hash
        let mut hasher = Sha256::new();
        hasher.update(company_id.as_bytes());
        hasher.update(entity_type.as_bytes());
        hasher.update(entity_id.as_bytes());
        hasher.update(action.as_bytes());
        if let Some(uid) = user_id {
            hasher.update(uid.as_bytes());
        }
        if let Some(b) = &before_json {
            hasher.update(b.as_bytes());
        }
        if let Some(a) = &after_json {
            hasher.update(a.as_bytes());
        }
        hasher.update(prev_hash.as_bytes());

        let record_hash = format!("{:x}", hasher.finalize());

        conn.execute(
            "INSERT INTO audit_trail (company_id, entity_type, entity_id, action, user_id, data_before, data_after, prev_hash, record_hash) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                company_id,
                entity_type,
                entity_id,
                action,
                user_id,
                before_json,
                after_json,
                prev_hash,
                record_hash
            ],
        )?;

        Ok(record_hash)
    }

    /// Verifies the cryptographic integrity of the entire audit chain.
    pub fn verify_chain(conn: &Connection, company_id: &str) -> Result<bool> {
        let mut stmt = conn.prepare(
            "SELECT log_id, company_id, entity_type, entity_id, action, user_id, 
                    data_before, data_after, prev_hash, record_hash 
             FROM audit_trail 
             WHERE company_id = ?1 
             ORDER BY log_id ASC",
        )?;

        let rows = stmt.query_map(params![company_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, String>(9)?,
            ))
        })?;

        let mut expected_prev = Self::GENESIS_HASH.to_string();

        for row in rows {
            let (_, cid, etype, eid, act, uid, b_json, a_json, p_hash, stored_hash) = row?;

            if let Some(actual_p) = p_hash {
                if actual_p != expected_prev {
                    return Ok(false); // Chain broken
                }
            }

            let mut hasher = Sha256::new();
            hasher.update(cid.as_bytes());
            hasher.update(etype.as_bytes());
            hasher.update(eid.as_bytes());
            hasher.update(act.as_bytes());
            if let Some(u) = &uid {
                hasher.update(u.as_bytes());
            }
            if let Some(b) = &b_json {
                hasher.update(b.as_bytes());
            }
            if let Some(a) = &a_json {
                hasher.update(a.as_bytes());
            }
            hasher.update(expected_prev.as_bytes());

            let computed = format!("{:x}", hasher.finalize());
            if computed != stored_hash {
                return Ok(false); // Record modified
            }

            expected_prev = stored_hash;
        }

        Ok(true)
    }
}
