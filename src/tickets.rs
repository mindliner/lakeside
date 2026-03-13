use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Metadata attached to a ticket (e.g., holder name, email, seat type).
pub type TicketMetadata = BTreeMap<String, String>;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TokenBundleRecord {
    pub amount: u64,
    pub token: String,
    pub format: String,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum TicketStatus {
    Unclaimed,
    Claimed,
    Reissued,
}

impl Default for TicketStatus {
    fn default() -> Self {
        TicketStatus::Unclaimed
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TicketRecord {
    pub ticket_code: String,
    pub ticket_hash: String,
    #[serde(default)]
    pub display_code: String,
    #[serde(default)]
    pub status: TicketStatus,
    #[serde(default)]
    pub claimed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub token_bundles: Vec<TokenBundleRecord>,
    #[serde(default)]
    pub metadata: TicketMetadata,
}

impl TicketRecord {
    pub fn new(ticket_code: String, display_code: String) -> Self {
        let ticket_hash = derive_ticket_hash(&ticket_code);
        TicketRecord {
            display_code,
            ticket_code,
            ticket_hash,
            status: TicketStatus::Unclaimed,
            claimed_at: None,
            token_bundles: Vec::new(),
            metadata: TicketMetadata::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TicketStore {
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub tickets: Vec<TicketRecord>,
}

impl Default for TicketStore {
    fn default() -> Self {
        TicketStore {
            updated_at: Utc::now(),
            tickets: Vec::new(),
        }
    }
}

impl TicketStore {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(TicketStore::default());
        }

        let data = fs::read_to_string(path)
            .with_context(|| format!("reading tickets file {}", path.display()))?;
        let mut store: TicketStore = serde_json::from_str(&data)
            .with_context(|| format!("parsing tickets file {}", path.display()))?;

        // Backfill display_code if older files don't have it yet.
        for ticket in &mut store.tickets {
            if ticket.display_code.is_empty() {
                ticket.display_code = ticket.ticket_code.clone();
            }
        }

        Ok(store)
    }

    pub fn save(&mut self, path: &Path) -> Result<()> {
        ensure_parent_dir(path)?;
        self.updated_at = Utc::now();
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)
            .with_context(|| format!("writing tickets file {}", path.display()))?;
        Ok(())
    }
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
    }
    Ok(())
}

pub fn normalize_ticket_code(input: &str, uppercase: bool, strip_hyphen: bool) -> String {
    let mut cleaned: String = input
        .trim()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if strip_hyphen {
        cleaned = cleaned.replace('-', "");
    }
    if uppercase {
        cleaned = cleaned.to_ascii_uppercase();
    }
    cleaned
}

pub fn derive_ticket_hash(ticket_code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(ticket_code.as_bytes());
    hex::encode(hasher.finalize())
}

pub struct ImportOptions {
    pub csv_path: PathBuf,
    pub store_path: PathBuf,
    pub code_column: String,
    pub delimiter: Option<char>,
    pub uppercase: bool,
    pub strip_hyphen: bool,
    pub metadata_columns: Vec<String>,
}

pub struct ImportReport {
    pub inserted: usize,
    pub updated: usize,
    pub skipped: usize,
    pub total_after: usize,
}

pub fn init_store(path: &Path, force: bool) -> Result<()> {
    if path.exists() && !force {
        bail!(
            "Tickets file {} already exists (use --force to overwrite)",
            path.display()
        );
    }

    let mut store = TicketStore::default();
    store.save(path)
}

pub fn import_from_csv(opts: ImportOptions) -> Result<ImportReport> {
    let mut store = TicketStore::load(&opts.store_path)?;
    let delimiter = opts.delimiter.unwrap_or(',') as u8;

    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .has_headers(true)
        .delimiter(delimiter)
        .from_path(&opts.csv_path)
        .with_context(|| format!("opening CSV {}", opts.csv_path.display()))?;

    let headers = reader
        .headers()
        .with_context(|| format!("reading headers from {}", opts.csv_path.display()))?
        .clone();

    let code_idx = headers
        .iter()
        .position(|h| h.trim().eq_ignore_ascii_case(&opts.code_column))
        .ok_or_else(|| {
            anyhow!(
                "CSV {} is missing the '{}' column",
                opts.csv_path.display(),
                opts.code_column
            )
        })?;

    let mut metadata_indices: Vec<(String, usize)> = Vec::new();
    for column in &opts.metadata_columns {
        if let Some(idx) = headers
            .iter()
            .position(|h| h.trim().eq_ignore_ascii_case(column))
        {
            metadata_indices.push((column.to_string(), idx));
        } else {
            eprintln!(
                "Warning: metadata column '{}' not found in {}",
                column,
                opts.csv_path.display()
            );
        }
    }

    let mut inserted = 0usize;
    let mut updated = 0usize;
    let mut skipped = 0usize;

    for result in reader.records() {
        let record = result.with_context(|| {
            format!(
                "reading row in {} (delimiter '{}')",
                opts.csv_path.display(),
                delimiter as char
            )
        })?;

        let raw_code = record.get(code_idx).unwrap_or("").trim();
        if raw_code.is_empty() {
            skipped += 1;
            continue;
        }

        let normalized = normalize_ticket_code(raw_code, opts.uppercase, opts.strip_hyphen);
        if normalized.is_empty() {
            skipped += 1;
            continue;
        }
        let ticket_hash = derive_ticket_hash(&normalized);

        let mut metadata: TicketMetadata = TicketMetadata::new();
        for (column, idx) in &metadata_indices {
            if let Some(value) = record.get(*idx) {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    metadata.insert(column.clone(), trimmed.to_string());
                }
            }
        }

        match store
            .tickets
            .iter_mut()
            .find(|ticket| ticket.ticket_hash == ticket_hash)
        {
            Some(existing) => {
                if existing.display_code.is_empty() {
                    existing.display_code = raw_code.to_string();
                }
                for (key, value) in metadata {
                    existing.metadata.insert(key, value);
                }
                updated += 1;
            }
            None => {
                let mut record = TicketRecord::new(normalized.clone(), raw_code.to_string());
                if !metadata.is_empty() {
                    record.metadata = metadata;
                }
                store.tickets.push(record);
                inserted += 1;
            }
        }
    }

    store
        .tickets
        .sort_by(|a, b| a.ticket_code.cmp(&b.ticket_code));

    let total_after = store.tickets.len();
    store.save(&opts.store_path)?;

    Ok(ImportReport {
        inserted,
        updated,
        skipped,
        total_after,
    })
}

pub struct ListSummary {
    pub path: PathBuf,
    pub updated_at: DateTime<Utc>,
    pub total: usize,
    pub unclaimed: usize,
    pub claimed: usize,
    pub reissued: usize,
}

pub fn list_summary(path: &Path) -> Result<ListSummary> {
    if !path.exists() {
        bail!("Tickets file {} does not exist", path.display());
    }

    let store = TicketStore::load(path)?;
    let mut claimed = 0usize;
    let mut reissued = 0usize;

    for ticket in &store.tickets {
        match ticket.status {
            TicketStatus::Unclaimed => {}
            TicketStatus::Claimed => claimed += 1,
            TicketStatus::Reissued => reissued += 1,
        }
    }

    let unclaimed = store.tickets.len().saturating_sub(claimed + reissued);

    Ok(ListSummary {
        path: path.to_path_buf(),
        updated_at: store.updated_at,
        total: store.tickets.len(),
        unclaimed,
        claimed,
        reissued,
    })
}
