// Module "route" — traceroute via mtr (= route.py)
//
// Concepts Rust introduits :
//   - serde_json::Value pour lire un JSON sans définir de struct (lecture souple)
//   - la propagation d'erreurs avec Result<_, String> et ok_or / map_err

use std::process::Command;

/// Un saut du traceroute.
#[derive(Debug, Clone)]
pub struct Hop {
    pub idx: i64,
    pub host: String,
    pub loss: f64,
    pub sent: i64,
    pub last: f64,
    pub avg: f64,
    pub best: f64,
    pub worst: f64,
}

/// Lance `mtr -n -c <count> --json <address>` et analyse le résultat.
pub fn run_route(address: &str, count: u32) -> Result<Vec<Hop>, String> {
    let output = Command::new("mtr")
        .args(["-n", "-c", &count.to_string(), "--json", address])
        .output()
        .map_err(|e| format!("Impossible de lancer mtr : {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() && stdout.trim().is_empty() {
        let err = String::from_utf8_lossy(&output.stderr);
        let err = err.trim();
        return Err(if err.is_empty() {
            "échec de mtr".to_string()
        } else {
            err.to_string()
        });
    }

    // On parse en Value générique (un arbre JSON) plutôt qu'en struct figée :
    // plus souple, comme le dict.get(...) avec valeurs par défaut côté Python.
    let data: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|_| "Réponse mtr illisible.".to_string())?;

    let hubs = data
        .get("report")
        .and_then(|r| r.get("hubs"))
        .and_then(|h| h.as_array())
        .ok_or_else(|| "Réponse mtr illisible.".to_string())?;

    // Petites fermetures pour extraire un champ avec valeur par défaut.
    let mut hops = Vec::new();
    for h in hubs {
        let f = |key: &str| h.get(key).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let i = |key: &str| h.get(key).and_then(|v| v.as_i64()).unwrap_or(0);
        hops.push(Hop {
            idx: i("count"),
            host: h
                .get("host")
                .and_then(|v| v.as_str())
                .unwrap_or("???")
                .to_string(),
            loss: f("Loss%"),
            sent: i("Snt"),
            last: f("Last"),
            avg: f("Avg"),
            best: f("Best"),
            worst: f("Wrst"),
        });
    }
    Ok(hops)
}
