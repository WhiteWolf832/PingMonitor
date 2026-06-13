// Module "storage" — persistance SQLite de l'historique (= storage.py)
//
// Concepts Rust introduits :
//   - rusqlite : un binding SQLite typé, avec gestion d'erreurs par Result
//   - le type Result<T, rusqlite::Error> propagé par `?`
//   - les chemins XDG construits avec std::path::PathBuf
//
// Différence d'architecture avec Python : ici la base vit sur le thread UI
// (qui reçoit DÉJÀ chaque échantillon via le channel), donc pas besoin de la
// partager entre threads ni de verrou — le compilateur nous épargne ce souci.

use std::path::PathBuf;

use rusqlite::{Connection, params};

pub struct Storage {
    conn: Connection,
}

impl Storage {
    /// Ouvre (ou crée) la base au chemin standard XDG et prépare le schéma.
    pub fn open() -> rusqlite::Result<Self> {
        let path = data_path();
        if let Some(dir) = path.parent() {
            // create_dir_all = mkdir -p ; .ok() ignore l'erreur si le dossier existe.
            std::fs::create_dir_all(dir).ok();
        }
        let conn = Connection::open(&path)?;
        // WAL = écritures concurrentes plus fluides (comme côté Python).
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let storage = Self { conn };
        storage.init_schema()?;
        Ok(storage)
    }

    /// Base en mémoire (repli si le disque est indisponible).
    pub fn open_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let storage = Self { conn };
        storage.init_schema()?;
        Ok(storage)
    }

    fn init_schema(&self) -> rusqlite::Result<()> {
        // execute_batch lance plusieurs instructions d'un coup.
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS samples (
                address TEXT NOT NULL,
                ts      REAL NOT NULL,
                latency REAL,            -- NULL = perte
                lost    INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_samples_addr_ts ON samples(address, ts);",
        )
    }

    /// Insère un échantillon. `Option<f64>` se mappe directement sur NULL/REAL.
    pub fn add(&self, address: &str, ts: f64, latency: Option<f64>) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO samples(address, ts, latency, lost) VALUES (?1, ?2, ?3, ?4)",
            params![address, ts, latency, latency.is_none() as i32],
        )?;
        Ok(())
    }

    /// Renvoie les points (ts, latence) d'un hôte dans [since, until], triés.
    /// C'est ce que lira le graphique navigable (étape 5b).
    pub fn range(
        &self,
        address: &str,
        since: f64,
        until: f64,
    ) -> rusqlite::Result<Vec<(f64, Option<f64>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT ts, latency FROM samples
             WHERE address = ?1 AND ts >= ?2 AND ts <= ?3
             ORDER BY ts ASC",
        )?;
        // query_map transforme chaque ligne SQL en un tuple Rust typé.
        let iter = stmt.query_map(params![address, since, until], |row| {
            Ok((row.get::<_, f64>(0)?, row.get::<_, Option<f64>>(1)?))
        })?;
        // On rassemble en Vec ; collect() s'arrête à la première erreur éventuelle.
        iter.collect()
    }

    /// Supprime les échantillons trop vieux (rétention ~30 jours).
    pub fn prune(&self, older_than: f64) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM samples WHERE ts < ?1", params![older_than])?;
        Ok(())
    }
}

/// Chemin de la base : $XDG_DATA_HOME/ping-monitor-rs/history.db
/// (ou ~/.local/share/... par défaut). Nom distinct de la version Python pour
/// ne pas mélanger les deux bases.
fn data_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            home.join(".local/share")
        });
    base.join("ping-monitor-rs").join("history.db")
}
