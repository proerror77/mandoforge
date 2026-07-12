// Ontology Source Adapters
//
// Normalises external data sources into OntologySourceBundle + OntologySeedPack so they
// can enter the existing onboarding pipeline without any changes to downstream logic.
//
// Supported inputs
//   File formats : CSV, JSON, Parquet (schema+sample), PDF (schema hints), Excel/XLSX
//   SaaS schemas : Salesforce, HubSpot, SAP S/4HANA, Oracle NetSuite
//   E-commerce   : Amazon Seller Central, Taobao/Tmall, TikTok Shop, Temu, Shopify, WooCommerce

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::AppError;
use crate::{
    OntologyActionTransactionProfile, OntologyOnboardingDataset, OntologyOnboardingField,
    OntologySeedActionMapping, OntologySeedMetricMapping, OntologySeedObjectMapping,
    OntologySeedPack, OntologySeedRelationMapping, OntologySourceBundle,
};

// ─────────────────────────────────────────────────────────────────────────────
// Top-level discriminated union
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OntologySourcePayload {
    Csv(CsvAdapterInput),
    Json(JsonAdapterInput),
    Parquet(ParquetAdapterInput),
    Pdf(PdfAdapterInput),
    Excel(ExcelAdapterInput),
    Salesforce(SalesforceAdapterInput),
    Hubspot(HubspotAdapterInput),
    SapS4Hana(SapS4HanaAdapterInput),
    OracleNetsuite(OracleNetsuiteAdapterInput),
    AmazonSellerCentral(AmazonSellerCentralAdapterInput),
    Taobao(TaobaoAdapterInput),
    TiktokShop(TiktokShopAdapterInput),
    Temu(TemuAdapterInput),
    Shopify(ShopifyAdapterInput),
    Woocommerce(WoocommerceAdapterInput),
}

// ─────────────────────────────────────────────────────────────────────────────
// Common helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Raw bytes supplied inline as base64, used when the payload arrives as a JSON body.
/// When the payload arrives via multipart the bytes are passed separately.
#[derive(Debug, Clone, Deserialize)]
pub struct RawFilePayload {
    pub filename: String,
    #[serde(default)]
    pub content_base64: Option<String>,
}

impl RawFilePayload {
    pub fn decode_bytes(&self) -> Result<Vec<u8>, AppError> {
        match &self.content_base64 {
            Some(b64) => base64::engine::general_purpose::STANDARD
                .decode(b64.trim())
                .map_err(|_| AppError::bad_request("invalid base64 in content_base64")),
            None => Err(AppError::bad_request(
                "file content required (content_base64 or multipart upload)",
            )),
        }
    }

    pub fn table_name(&self) -> String {
        let stem = std::path::Path::new(&self.filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("table");
        to_snake_case(stem)
    }
}

fn to_snake_case(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

/// Adapter output — carries everything needed by the onboarding pipeline.
#[derive(Debug, Clone, Serialize)]
pub struct OntologySourceAdapterOutput {
    pub bundle: OntologySourceBundle,
    pub seed: OntologySeedPack,
    pub adapter_type: String,
    pub source_label: String,
    pub schema_only: bool,
    pub warnings: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Field type inference (shared by all file adapters)
// ─────────────────────────────────────────────────────────────────────────────

fn infer_field_type(values: &[&str]) -> &'static str {
    let non_empty: Vec<&str> = values.iter().copied().filter(|v| !v.is_empty()).collect();
    if non_empty.is_empty() {
        return "string";
    }
    let all_int = non_empty.iter().all(|v| v.parse::<i64>().is_ok());
    if all_int {
        return "integer";
    }
    let all_float = non_empty.iter().all(|v| v.parse::<f64>().is_ok());
    if all_float {
        return "decimal";
    }
    let ts_patterns = ["-", "T", ":", "Z"];
    let looks_timestamp = non_empty
        .iter()
        .all(|v| v.len() >= 8 && ts_patterns.iter().any(|p| v.contains(p)));
    if looks_timestamp {
        return "timestamp";
    }
    "string"
}

fn build_dataset(
    table_name: &str,
    source_system: &str,
    source_object: &str,
    fields: Vec<OntologyOnboardingField>,
    rows: Vec<Value>,
) -> OntologyOnboardingDataset {
    OntologyOnboardingDataset {
        table_name: table_name.to_string(),
        source_system: source_system.to_string(),
        source_object: source_object.to_string(),
        fields,
        rows,
    }
}

/// Build a minimal single-object seed pack for generic file uploads.
fn generic_seed_pack(
    industry: &str,
    domain_scope: &str,
    source_mode: &str,
    tool_namespace: &str,
    datasets: &[OntologyOnboardingDataset],
) -> OntologySeedPack {
    let objects = datasets
        .iter()
        .map(|ds| OntologySeedObjectMapping {
            table_name: ds.table_name.clone(),
            object_name: pascal_case(&ds.table_name),
        })
        .collect();
    OntologySeedPack {
        industry: industry.to_string(),
        domain_scope: domain_scope.to_string(),
        source_mode: source_mode.to_string(),
        tool_namespace: tool_namespace.to_string(),
        objects,
        relations: vec![],
        metrics: vec![],
        actions: vec![],
    }
}

fn pascal_case(s: &str) -> String {
    s.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// CSV adapter
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct CsvAdapterInput {
    pub file: RawFilePayload,
    #[serde(default)]
    pub delimiter: Option<char>,
    #[serde(default)]
    pub sample_row_count: Option<usize>,
    #[serde(default)]
    pub table_name_override: Option<String>,
    #[serde(default)]
    pub source_system_override: Option<String>,
}

pub fn adapt_csv(
    input: CsvAdapterInput,
    bytes: &[u8],
) -> Result<OntologySourceAdapterOutput, AppError> {
    let sample_limit = input.sample_row_count.unwrap_or(8).min(50);
    let table_name = input
        .table_name_override
        .clone()
        .unwrap_or_else(|| input.file.table_name());
    let source_system = input
        .source_system_override
        .clone()
        .unwrap_or_else(|| "file_upload".to_string());

    let delimiter = input
        .delimiter
        .unwrap_or_else(|| detect_csv_delimiter(bytes)) as u8;
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(true)
        .from_reader(bytes);

    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| AppError::bad_request(format!("CSV header error: {e}")))?
        .iter()
        .map(to_snake_case)
        .collect();

    let mut all_rows: Vec<csv::StringRecord> = Vec::new();
    for result in reader.records() {
        let record = result.map_err(|e| AppError::bad_request(format!("CSV row error: {e}")))?;
        all_rows.push(record);
        if all_rows.len() >= 10_000 {
            break;
        }
    }

    let mut fields = Vec::new();
    for (col_idx, col_name) in headers.iter().enumerate() {
        let col_vals: Vec<&str> = all_rows.iter().filter_map(|row| row.get(col_idx)).collect();
        let field_type = infer_field_type(&col_vals);
        let sample_values: Vec<Value> = col_vals
            .iter()
            .take(sample_limit)
            .map(|v| Value::String(v.to_string()))
            .collect();
        fields.push(OntologyOnboardingField {
            name: col_name.clone(),
            field_type: field_type.to_string(),
            sample_values,
        });
    }

    let rows: Vec<Value> = all_rows
        .iter()
        .take(sample_limit)
        .map(|row| {
            let mut map = serde_json::Map::new();
            for (i, h) in headers.iter().enumerate() {
                let v = row.get(i).unwrap_or("");
                map.insert(h.clone(), Value::String(v.to_string()));
            }
            Value::Object(map)
        })
        .collect();

    let dataset = build_dataset(&table_name, &source_system, &table_name, fields, rows);
    let datasets = vec![dataset];
    let domain_scope = table_name.clone();
    let seed = generic_seed_pack("generic", &domain_scope, "file_csv", "data", &datasets);
    let bundle = OntologySourceBundle {
        industry: "generic".to_string(),
        source_mode: "file_csv".to_string(),
        tool_namespace: "data".to_string(),
        datasets,
    };
    Ok(OntologySourceAdapterOutput {
        bundle,
        seed,
        adapter_type: "csv".to_string(),
        source_label: format!("csv:{}", input.file.filename),
        schema_only: false,
        warnings: vec![],
    })
}

fn detect_csv_delimiter(bytes: &[u8]) -> char {
    let sample = &bytes[..bytes.len().min(4096)];
    let counts = b",\t|;"
        .iter()
        .map(|&d| (d as char, sample.iter().filter(|&&b| b == d).count()))
        .collect::<Vec<_>>();
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(d, _)| d)
        .unwrap_or(',')
}

// ─────────────────────────────────────────────────────────────────────────────
// JSON adapter
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JsonShape {
    ArrayOfObjects,
    ObjectOfArrays,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonAdapterInput {
    pub file: RawFilePayload,
    #[serde(default)]
    pub shape: Option<JsonShape>,
    #[serde(default)]
    pub sample_row_count: Option<usize>,
    #[serde(default)]
    pub table_name_override: Option<String>,
    #[serde(default)]
    pub source_system_override: Option<String>,
}

pub fn adapt_json(
    input: JsonAdapterInput,
    bytes: &[u8],
) -> Result<OntologySourceAdapterOutput, AppError> {
    let sample_limit = input.sample_row_count.unwrap_or(8).min(50);
    let table_name = input
        .table_name_override
        .clone()
        .unwrap_or_else(|| input.file.table_name());
    let source_system = input
        .source_system_override
        .clone()
        .unwrap_or_else(|| "file_upload".to_string());

    let root: Value = serde_json::from_slice(bytes)
        .map_err(|e| AppError::bad_request(format!("JSON parse error: {e}")))?;

    let rows: Vec<Value> = match (&input.shape, &root) {
        (Some(JsonShape::ObjectOfArrays), _) | (None, Value::Object(_)) => {
            if let Some(obj) = root.as_object() {
                let len = obj
                    .values()
                    .find_map(|v| v.as_array().map(|a| a.len()))
                    .unwrap_or(0);
                (0..len)
                    .map(|i| {
                        let mut map = serde_json::Map::new();
                        for (k, v) in obj {
                            if let Some(arr) = v.as_array() {
                                map.insert(k.clone(), arr.get(i).cloned().unwrap_or(Value::Null));
                            }
                        }
                        Value::Object(map)
                    })
                    .collect()
            } else {
                return Err(AppError::bad_request(
                    "JSON object-of-arrays must be a top-level object",
                ));
            }
        }
        _ => root
            .as_array()
            .ok_or_else(|| {
                AppError::bad_request("JSON must be an array of objects or object of arrays")
            })?
            .clone(),
    };

    let headers: Vec<String> = rows
        .iter()
        .filter_map(|r| r.as_object())
        .flat_map(|o| o.keys().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    let mut fields = Vec::new();
    for col in &headers {
        let col_vals: Vec<String> = rows
            .iter()
            .filter_map(|r| r.get(col))
            .filter_map(|v| match v {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                Value::Bool(b) => Some(b.to_string()),
                _ => None,
            })
            .collect();
        let col_str_refs: Vec<&str> = col_vals.iter().map(|s| s.as_str()).collect();
        let field_type = infer_field_type(&col_str_refs);
        let sample_values: Vec<Value> = rows
            .iter()
            .take(sample_limit)
            .filter_map(|r| r.get(col))
            .cloned()
            .collect();
        fields.push(OntologyOnboardingField {
            name: col.clone(),
            field_type: field_type.to_string(),
            sample_values,
        });
    }

    let sample_rows = rows.into_iter().take(sample_limit).collect();
    let dataset = build_dataset(
        &table_name,
        &source_system,
        &table_name,
        fields,
        sample_rows,
    );
    let datasets = vec![dataset];
    let domain_scope = table_name.clone();
    let seed = generic_seed_pack("generic", &domain_scope, "file_json", "data", &datasets);
    let bundle = OntologySourceBundle {
        industry: "generic".to_string(),
        source_mode: "file_json".to_string(),
        tool_namespace: "data".to_string(),
        datasets,
    };
    Ok(OntologySourceAdapterOutput {
        bundle,
        seed,
        adapter_type: "json".to_string(),
        source_label: format!("json:{}", input.file.filename),
        schema_only: false,
        warnings: vec![],
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Parquet adapter (schema + sample rows via parquet crate)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ParquetAdapterInput {
    pub file: RawFilePayload,
    #[serde(default)]
    pub sample_row_count: Option<usize>,
    #[serde(default)]
    pub table_name_override: Option<String>,
    #[serde(default)]
    pub source_system_override: Option<String>,
}

pub fn adapt_parquet(
    input: ParquetAdapterInput,
    bytes: &[u8],
) -> Result<OntologySourceAdapterOutput, AppError> {
    let sample_limit = input.sample_row_count.unwrap_or(8).min(50);
    let table_name = input
        .table_name_override
        .clone()
        .unwrap_or_else(|| input.file.table_name());
    let source_system = input
        .source_system_override
        .clone()
        .unwrap_or_else(|| "file_upload".to_string());
    let mut warnings = Vec::new();

    // Parse column names and types from the Parquet file footer metadata.
    // We use a simple byte scan for the Parquet magic bytes and schema encoding
    // via the parquet crate's low-level API.
    let (fields, rows, schema_only) =
        parse_parquet_schema_and_rows(bytes, sample_limit, &mut warnings)?;

    let dataset = build_dataset(&table_name, &source_system, &table_name, fields, rows);
    let datasets = vec![dataset];
    let domain_scope = table_name.clone();
    let seed = generic_seed_pack("generic", &domain_scope, "file_parquet", "data", &datasets);
    let bundle = OntologySourceBundle {
        industry: "generic".to_string(),
        source_mode: "file_parquet".to_string(),
        tool_namespace: "data".to_string(),
        datasets,
    };
    Ok(OntologySourceAdapterOutput {
        bundle,
        seed,
        adapter_type: "parquet".to_string(),
        source_label: format!("parquet:{}", input.file.filename),
        schema_only,
        warnings,
    })
}

fn parse_parquet_schema_and_rows(
    bytes: &[u8],
    sample_limit: usize,
    warnings: &mut Vec<String>,
) -> Result<(Vec<OntologyOnboardingField>, Vec<Value>, bool), AppError> {
    if bytes.len() < 8 || &bytes[..4] != b"PAR1" || &bytes[bytes.len() - 4..] != b"PAR1" {
        return Err(AppError::bad_request(
            "file does not appear to be a valid Parquet file",
        ));
    }

    use parquet::file::reader::{FileReader, SerializedFileReader};

    let owned = bytes::Bytes::copy_from_slice(bytes);
    let reader = SerializedFileReader::new(owned)
        .map_err(|e| AppError::bad_request(format!("Parquet read error: {e}")))?;

    let metadata = reader.metadata();
    let schema = metadata.file_metadata().schema_descr();

    let mut fields = Vec::new();
    for i in 0..schema.num_columns() {
        let col = schema.column(i);
        let name = to_snake_case(col.name());
        let field_type = parquet_physical_type_to_field_type(col.physical_type());
        fields.push(OntologyOnboardingField {
            name,
            field_type: field_type.to_string(),
            sample_values: vec![],
        });
    }

    let mut rows = Vec::new();
    let mut schema_only = false;

    if metadata.num_row_groups() == 0 {
        schema_only = true;
        warnings.push("Parquet file contains no row groups; schema only.".to_string());
    } else {
        match reader.get_row_iter(None) {
            Ok(iter) => {
                for result in iter.take(sample_limit) {
                    match result {
                        Ok(row) => {
                            let mut map = serde_json::Map::new();
                            for (col_name, field_val) in row.get_column_iter() {
                                map.insert(
                                    to_snake_case(col_name),
                                    parquet_field_to_json(field_val),
                                );
                            }
                            rows.push(Value::Object(map));
                        }
                        Err(e) => {
                            warnings.push(format!("Row decode error: {e}"));
                            break;
                        }
                    }
                }
                for field in &mut fields {
                    field.sample_values = rows
                        .iter()
                        .filter_map(|r| r.get(&field.name))
                        .cloned()
                        .collect();
                }
            }
            Err(e) => {
                warnings.push(format!("Could not iterate Parquet rows: {e}; schema only."));
                schema_only = true;
            }
        }
    }

    Ok((fields, rows, schema_only))
}

fn parquet_physical_type_to_field_type(pt: parquet::basic::Type) -> &'static str {
    use parquet::basic::Type as PhysType;
    match pt {
        PhysType::INT32 | PhysType::INT64 => "integer",
        PhysType::FLOAT | PhysType::DOUBLE => "decimal",
        PhysType::BOOLEAN => "boolean",
        _ => "string",
    }
}

fn parquet_field_to_json(v: &parquet::record::Field) -> Value {
    use parquet::record::Field;
    match v {
        Field::Null => Value::Null,
        Field::Bool(b) => json!(b),
        Field::Byte(b) => json!(b),
        Field::Short(s) => json!(s),
        Field::Int(i) => json!(i),
        Field::Long(l) => json!(l),
        Field::Float(f) => json!(f),
        Field::Double(d) => json!(d),
        Field::Str(s) => json!(s),
        Field::Bytes(b) => json!(hex::encode(b.data())),
        Field::Date(d) => json!(d),
        Field::TimestampMillis(t) => json!(t),
        Field::TimestampMicros(t) => json!(t),
        _ => Value::Null,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PDF adapter (schema hints only — extracts column headers from tables)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct PdfAdapterInput {
    pub file: RawFilePayload,
    #[serde(default)]
    pub table_name_override: Option<String>,
    #[serde(default)]
    pub source_system_override: Option<String>,
}

pub fn adapt_pdf(
    input: PdfAdapterInput,
    bytes: &[u8],
) -> Result<OntologySourceAdapterOutput, AppError> {
    let table_name = input
        .table_name_override
        .clone()
        .unwrap_or_else(|| input.file.table_name());
    let source_system = input
        .source_system_override
        .clone()
        .unwrap_or_else(|| "file_upload".to_string());

    let text = pdf_extract::extract_text_from_mem(bytes)
        .map_err(|e| AppError::bad_request(format!("PDF extract error: {e}")))?;

    let fields = extract_schema_hints_from_pdf_text(&text);

    let dataset = build_dataset(&table_name, &source_system, &table_name, fields, vec![]);
    let datasets = vec![dataset];
    let domain_scope = table_name.clone();
    let seed = generic_seed_pack("generic", &domain_scope, "file_pdf", "data", &datasets);
    let bundle = OntologySourceBundle {
        industry: "generic".to_string(),
        source_mode: "file_pdf".to_string(),
        tool_namespace: "data".to_string(),
        datasets,
    };
    Ok(OntologySourceAdapterOutput {
        bundle,
        seed,
        adapter_type: "pdf".to_string(),
        source_label: format!("pdf:{}", input.file.filename),
        schema_only: true,
        warnings: vec![
            "PDF adapter provides schema hints only; no row data is extracted.".to_string(),
        ],
    })
}

fn extract_schema_hints_from_pdf_text(text: &str) -> Vec<OntologyOnboardingField> {
    // Heuristic: look for lines that look like table headers — tokens that are
    // identifier-like (no spaces, mix of alphanumeric and underscores) on a short line.
    let mut seen = std::collections::BTreeSet::new();
    let mut fields = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.len() > 120 {
            continue;
        }
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.len() < 2 || tokens.len() > 12 {
            continue;
        }
        let identifier_like = tokens.iter().all(|t| {
            !t.is_empty()
                && t.chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                && t.chars().any(|c| c.is_alphabetic())
        });
        if identifier_like {
            for token in tokens {
                let key = to_snake_case(token);
                if key.len() >= 2 && seen.insert(key.clone()) {
                    fields.push(OntologyOnboardingField {
                        name: key,
                        field_type: "unknown".to_string(),
                        sample_values: vec![],
                    });
                }
            }
        }
    }
    fields
}

// ─────────────────────────────────────────────────────────────────────────────
// Excel adapter (XLSX / XLS via calamine)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ExcelAdapterInput {
    pub file: RawFilePayload,
    #[serde(default)]
    pub sheet: Option<String>,
    #[serde(default)]
    pub sample_row_count: Option<usize>,
    #[serde(default)]
    pub table_name_override: Option<String>,
    #[serde(default)]
    pub source_system_override: Option<String>,
}

pub fn adapt_excel(
    input: ExcelAdapterInput,
    bytes: &[u8],
) -> Result<OntologySourceAdapterOutput, AppError> {
    use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};

    let sample_limit = input.sample_row_count.unwrap_or(8).min(50);
    let table_name = input
        .table_name_override
        .clone()
        .unwrap_or_else(|| input.file.table_name());
    let source_system = input
        .source_system_override
        .clone()
        .unwrap_or_else(|| "file_upload".to_string());

    let cursor = std::io::Cursor::new(bytes);
    let mut workbook: Xlsx<_> = open_workbook_from_rs(cursor)
        .map_err(|e| AppError::bad_request(format!("Excel open error: {e}")))?;

    let sheet_name = input
        .sheet
        .clone()
        .unwrap_or_else(|| workbook.sheet_names().first().cloned().unwrap_or_default());

    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|e| AppError::bad_request(format!("Excel sheet error: {e}")))?;

    let mut row_iter = range.rows();
    let header_row = row_iter
        .next()
        .ok_or_else(|| AppError::bad_request("Excel sheet is empty"))?;

    let headers: Vec<String> = header_row
        .iter()
        .map(|cell| to_snake_case(&cell.to_string()))
        .collect();

    let data_rows: Vec<Vec<Data>> = row_iter.map(|r| r.to_vec()).collect();

    let mut fields = Vec::new();
    for (col_idx, col_name) in headers.iter().enumerate() {
        let col_vals: Vec<String> = data_rows
            .iter()
            .filter_map(|row| row.get(col_idx))
            .map(|cell| match cell {
                Data::Int(i) => i.to_string(),
                Data::Float(f) => f.to_string(),
                Data::String(s) => s.clone(),
                Data::Bool(b) => b.to_string(),
                _ => String::new(),
            })
            .collect();
        let col_str_refs: Vec<&str> = col_vals.iter().map(|s| s.as_str()).collect();
        let field_type =
            excel_col_type(&data_rows, col_idx).unwrap_or_else(|| infer_field_type(&col_str_refs));
        let sample_values: Vec<Value> = col_vals
            .iter()
            .take(sample_limit)
            .map(|v| Value::String(v.clone()))
            .collect();
        fields.push(OntologyOnboardingField {
            name: col_name.clone(),
            field_type: field_type.to_string(),
            sample_values,
        });
    }

    let rows: Vec<Value> = data_rows
        .iter()
        .take(sample_limit)
        .map(|row| {
            let mut map = serde_json::Map::new();
            for (i, h) in headers.iter().enumerate() {
                let v = row
                    .get(i)
                    .map(|c| match c {
                        Data::Int(x) => json!(x),
                        Data::Float(x) => json!(x),
                        Data::String(x) => json!(x),
                        Data::Bool(x) => json!(x),
                        Data::DateTime(_) => json!(c.to_string()),
                        _ => Value::Null,
                    })
                    .unwrap_or(Value::Null);
                map.insert(h.clone(), v);
            }
            Value::Object(map)
        })
        .collect();

    let dataset = build_dataset(&table_name, &source_system, &table_name, fields, rows);
    let datasets = vec![dataset];
    let domain_scope = table_name.clone();
    let seed = generic_seed_pack("generic", &domain_scope, "file_excel", "data", &datasets);
    let bundle = OntologySourceBundle {
        industry: "generic".to_string(),
        source_mode: "file_excel".to_string(),
        tool_namespace: "data".to_string(),
        datasets,
    };
    Ok(OntologySourceAdapterOutput {
        bundle,
        seed,
        adapter_type: "excel".to_string(),
        source_label: format!("excel:{}", input.file.filename),
        schema_only: false,
        warnings: vec![],
    })
}

fn excel_col_type(rows: &[Vec<calamine::Data>], col_idx: usize) -> Option<&'static str> {
    use calamine::Data;
    let first = rows.iter().find_map(|r| r.get(col_idx))?;
    Some(match first {
        Data::Int(_) => "integer",
        Data::Float(_) => "decimal",
        Data::Bool(_) => "boolean",
        Data::DateTime(_) => "timestamp",
        _ => "string",
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Schema-only template builder (shared by all SaaS/e-commerce platform adapters)
// ─────────────────────────────────────────────────────────────────────────────

type FieldSpec = (&'static str, &'static str); // (field_name, field_type)
type TableSpec = (&'static str, &'static [FieldSpec]); // (table_name, fields)

fn build_schema_only_dataset(
    table_name: &'static str,
    source_system: &str,
    fields_spec: &[FieldSpec],
) -> OntologyOnboardingDataset {
    let fields = fields_spec
        .iter()
        .map(|(name, ft)| OntologyOnboardingField {
            name: name.to_string(),
            field_type: ft.to_string(),
            sample_values: vec![],
        })
        .collect();
    build_dataset(table_name, source_system, table_name, fields, vec![])
}

fn build_platform_output(
    industry: &str,
    domain_scope: &str,
    source_mode: &str,
    tool_namespace: &str,
    adapter_type: &str,
    instance_label: &str,
    tables: &[TableSpec],
    relations: Vec<OntologySeedRelationMapping>,
    metrics: Vec<OntologySeedMetricMapping>,
    actions: Vec<OntologySeedActionMapping>,
) -> OntologySourceAdapterOutput {
    let datasets: Vec<OntologyOnboardingDataset> = tables
        .iter()
        .map(|(name, fields)| build_schema_only_dataset(name, instance_label, fields))
        .collect();

    let objects: Vec<OntologySeedObjectMapping> = tables
        .iter()
        .map(|(name, _)| OntologySeedObjectMapping {
            table_name: name.to_string(),
            object_name: pascal_case(name),
        })
        .collect();

    let seed = OntologySeedPack {
        industry: industry.to_string(),
        domain_scope: domain_scope.to_string(),
        source_mode: source_mode.to_string(),
        tool_namespace: tool_namespace.to_string(),
        objects,
        relations,
        metrics,
        actions,
    };
    let bundle = OntologySourceBundle {
        industry: industry.to_string(),
        source_mode: source_mode.to_string(),
        tool_namespace: tool_namespace.to_string(),
        datasets,
    };
    OntologySourceAdapterOutput {
        bundle,
        seed,
        adapter_type: adapter_type.to_string(),
        source_label: format!("{}:{}", adapter_type, instance_label),
        schema_only: true,
        warnings: vec![],
    }
}

fn overlay_export_csv(
    out: &mut OntologySourceAdapterOutput,
    export_file: Option<&RawFilePayload>,
    bytes: Option<&[u8]>,
    table_name: &str,
    source_system: &str,
) {
    let Some(file) = export_file else {
        return;
    };
    let Ok(raw) = bytes_or_inline(bytes, file) else {
        return;
    };
    let Ok(csv_out) = adapt_csv(
        CsvAdapterInput {
            file: file.clone(),
            delimiter: None,
            sample_row_count: Some(20),
            table_name_override: Some(table_name.to_string()),
            source_system_override: Some(source_system.to_string()),
        },
        &raw,
    ) else {
        return;
    };
    let Some(target) = out
        .bundle
        .datasets
        .iter_mut()
        .find(|dataset| dataset.table_name == table_name)
    else {
        return;
    };
    let Some(source) = csv_out.bundle.datasets.first() else {
        return;
    };
    target.rows = source.rows.clone();
    target.fields = source.fields.clone();
    out.schema_only = false;
}

fn rel(
    name: &str,
    from: &str,
    relation: &str,
    to: &str,
    src_table: &str,
    src_field: &str,
    ref_table: &str,
) -> OntologySeedRelationMapping {
    OntologySeedRelationMapping {
        name: name.to_string(),
        from_object: from.to_string(),
        relation: relation.to_string(),
        to_object: to.to_string(),
        source_table: src_table.to_string(),
        source_field: src_field.to_string(),
        reference_table: ref_table.to_string(),
    }
}

fn metric(name: &str, target: &str, expr: &str) -> OntologySeedMetricMapping {
    OntologySeedMetricMapping {
        name: name.to_string(),
        target_object: target.to_string(),
        expression: expr.to_string(),
        evidence: json!({}),
    }
}

fn action(
    name: &str,
    target: &str,
    approval_required: bool,
    inputs: Value,
    reads: Value,
) -> OntologySeedActionMapping {
    OntologySeedActionMapping {
        name: name.to_string(),
        target_object: target.to_string(),
        approval_required,
        inputs,
        reads,
        effects: json!({}),
        executor: json!({"type": "api_call"}),
        transaction_profile: OntologyActionTransactionProfile::ProposalOnly,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Salesforce adapter
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct SalesforceAdapterInput {
    #[serde(default)]
    pub objects: Option<Vec<SalesforceObject>>,
    #[serde(default)]
    pub instance_label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum SalesforceObject {
    Account,
    Contact,
    Lead,
    Opportunity,
    Case,
    Product2,
    Pricebook2Entry,
    Order,
}

static SF_ACCOUNT: &[FieldSpec] = &[
    ("id", "string"),
    ("name", "string"),
    ("type", "string"),
    ("industry", "string"),
    ("annual_revenue", "decimal"),
    ("billing_country", "string"),
    ("created_date", "timestamp"),
    ("owner_id", "string"),
];
static SF_CONTACT: &[FieldSpec] = &[
    ("id", "string"),
    ("account_id", "string"),
    ("first_name", "string"),
    ("last_name", "string"),
    ("email", "string"),
    ("phone", "string"),
    ("title", "string"),
    ("lead_source", "string"),
    ("created_date", "timestamp"),
];
static SF_LEAD: &[FieldSpec] = &[
    ("id", "string"),
    ("first_name", "string"),
    ("last_name", "string"),
    ("email", "string"),
    ("company", "string"),
    ("status", "string"),
    ("lead_source", "string"),
    ("annual_revenue", "decimal"),
    ("created_date", "timestamp"),
];
static SF_OPPORTUNITY: &[FieldSpec] = &[
    ("id", "string"),
    ("account_id", "string"),
    ("name", "string"),
    ("stage_name", "string"),
    ("amount", "decimal"),
    ("close_date", "date"),
    ("probability", "decimal"),
    ("type", "string"),
    ("created_date", "timestamp"),
];
static SF_CASE: &[FieldSpec] = &[
    ("id", "string"),
    ("account_id", "string"),
    ("contact_id", "string"),
    ("subject", "string"),
    ("status", "string"),
    ("priority", "string"),
    ("origin", "string"),
    ("created_date", "timestamp"),
    ("closed_date", "timestamp"),
];
static SF_PRODUCT2: &[FieldSpec] = &[
    ("id", "string"),
    ("name", "string"),
    ("product_code", "string"),
    ("description", "string"),
    ("is_active", "boolean"),
    ("family", "string"),
    ("created_date", "timestamp"),
];
static SF_PRICEBOOK2ENTRY: &[FieldSpec] = &[
    ("id", "string"),
    ("pricebook2_id", "string"),
    ("product2_id", "string"),
    ("unit_price", "decimal"),
    ("is_active", "boolean"),
    ("currency_iso_code", "string"),
];
static SF_ORDER: &[FieldSpec] = &[
    ("id", "string"),
    ("account_id", "string"),
    ("order_number", "string"),
    ("status", "string"),
    ("total_amount", "decimal"),
    ("effective_date", "date"),
    ("created_date", "timestamp"),
];

fn sf_table_for(obj: &SalesforceObject) -> (&'static str, &'static [FieldSpec]) {
    match obj {
        SalesforceObject::Account => ("account", SF_ACCOUNT),
        SalesforceObject::Contact => ("contact", SF_CONTACT),
        SalesforceObject::Lead => ("lead", SF_LEAD),
        SalesforceObject::Opportunity => ("opportunity", SF_OPPORTUNITY),
        SalesforceObject::Case => ("case", SF_CASE),
        SalesforceObject::Product2 => ("product2", SF_PRODUCT2),
        SalesforceObject::Pricebook2Entry => ("pricebook2entry", SF_PRICEBOOK2ENTRY),
        SalesforceObject::Order => ("order", SF_ORDER),
    }
}

pub fn adapt_salesforce(
    input: SalesforceAdapterInput,
) -> Result<OntologySourceAdapterOutput, AppError> {
    let all_objects = vec![
        SalesforceObject::Account,
        SalesforceObject::Contact,
        SalesforceObject::Lead,
        SalesforceObject::Opportunity,
        SalesforceObject::Case,
        SalesforceObject::Product2,
        SalesforceObject::Pricebook2Entry,
        SalesforceObject::Order,
    ];
    let selected = input.objects.as_deref().unwrap_or(&all_objects);
    let label = input.instance_label.as_deref().unwrap_or("salesforce");

    let tables: Vec<TableSpec> = selected.iter().map(sf_table_for).collect();
    let tables_static: Vec<(&'static str, &'static [FieldSpec])> = tables;

    let relations = vec![
        rel(
            "ContactBelongsToAccount",
            "Contact",
            "belongs_to",
            "Account",
            "contact",
            "account_id",
            "account",
        ),
        rel(
            "OpportunityBelongsToAccount",
            "Opportunity",
            "belongs_to",
            "Account",
            "opportunity",
            "account_id",
            "account",
        ),
        rel(
            "CaseBelongsToAccount",
            "Case",
            "belongs_to",
            "Account",
            "case",
            "account_id",
            "account",
        ),
        rel(
            "CaseBelongsToContact",
            "Case",
            "belongs_to",
            "Contact",
            "case",
            "contact_id",
            "contact",
        ),
        rel(
            "OrderBelongsToAccount",
            "Order",
            "belongs_to",
            "Account",
            "order",
            "account_id",
            "account",
        ),
        rel(
            "Pricebook2EntryHasProduct",
            "Pricebook2Entry",
            "references",
            "Product2",
            "pricebook2entry",
            "product2_id",
            "product2",
        ),
    ];
    let metrics = vec![
        metric(
            "total_pipeline_value",
            "Opportunity",
            "SUM(amount) WHERE stage_name NOT IN ('Closed Lost','Closed Won')",
        ),
        metric(
            "win_rate",
            "Opportunity",
            "COUNT(*) FILTER(stage_name='Closed Won') / COUNT(*) * 100",
        ),
        metric(
            "avg_deal_size",
            "Opportunity",
            "AVG(amount) WHERE stage_name='Closed Won'",
        ),
        metric(
            "open_case_count",
            "Case",
            "COUNT(*) WHERE status NOT IN ('Closed')",
        ),
        metric("annual_revenue_total", "Account", "SUM(annual_revenue)"),
    ];
    let actions = vec![
        action(
            "convert_lead",
            "Lead",
            true,
            json!({"status":"string","account_name":"string"}),
            json!({"reads":["Lead","Account"]}),
        ),
        action(
            "close_opportunity",
            "Opportunity",
            true,
            json!({"stage_name":"string","close_date":"date"}),
            json!({"reads":["Opportunity"]}),
        ),
        action(
            "escalate_case",
            "Case",
            false,
            json!({"priority":"string","owner_id":"string"}),
            json!({"reads":["Case"]}),
        ),
    ];

    Ok(build_platform_output(
        "crm",
        "salesforce",
        "salesforce_export",
        "crm",
        "salesforce",
        label,
        &tables_static,
        relations,
        metrics,
        actions,
    ))
}

// ─────────────────────────────────────────────────────────────────────────────
// HubSpot adapter
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct HubspotAdapterInput {
    #[serde(default)]
    pub objects: Option<Vec<HubspotObject>>,
    #[serde(default)]
    pub instance_label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HubspotObject {
    Contacts,
    Companies,
    Deals,
    Tickets,
    LineItems,
}

static HS_CONTACTS: &[FieldSpec] = &[
    ("vid", "string"),
    ("email", "string"),
    ("firstname", "string"),
    ("lastname", "string"),
    ("phone", "string"),
    ("company", "string"),
    ("lifecyclestage", "string"),
    ("hs_lead_status", "string"),
    ("createdate", "timestamp"),
    ("lastmodifieddate", "timestamp"),
];
static HS_COMPANIES: &[FieldSpec] = &[
    ("company_id", "string"),
    ("name", "string"),
    ("domain", "string"),
    ("industry", "string"),
    ("city", "string"),
    ("country", "string"),
    ("annualrevenue", "decimal"),
    ("numberofemployees", "integer"),
];
static HS_DEALS: &[FieldSpec] = &[
    ("deal_id", "string"),
    ("dealname", "string"),
    ("dealstage", "string"),
    ("amount", "decimal"),
    ("closedate", "date"),
    ("pipeline", "string"),
    ("associated_company_id", "string"),
    ("owner_id", "string"),
];
static HS_TICKETS: &[FieldSpec] = &[
    ("ticket_id", "string"),
    ("subject", "string"),
    ("content", "string"),
    ("status", "string"),
    ("priority", "string"),
    ("category", "string"),
    ("associated_contact_id", "string"),
    ("createdate", "timestamp"),
];
static HS_LINE_ITEMS: &[FieldSpec] = &[
    ("line_item_id", "string"),
    ("deal_id", "string"),
    ("product_id", "string"),
    ("name", "string"),
    ("quantity", "decimal"),
    ("price", "decimal"),
    ("amount", "decimal"),
    ("discount", "decimal"),
];

fn hs_table_for(obj: &HubspotObject) -> (&'static str, &'static [FieldSpec]) {
    match obj {
        HubspotObject::Contacts => ("contacts", HS_CONTACTS),
        HubspotObject::Companies => ("companies", HS_COMPANIES),
        HubspotObject::Deals => ("deals", HS_DEALS),
        HubspotObject::Tickets => ("tickets", HS_TICKETS),
        HubspotObject::LineItems => ("line_items", HS_LINE_ITEMS),
    }
}

pub fn adapt_hubspot(input: HubspotAdapterInput) -> Result<OntologySourceAdapterOutput, AppError> {
    let all = vec![
        HubspotObject::Contacts,
        HubspotObject::Companies,
        HubspotObject::Deals,
        HubspotObject::Tickets,
        HubspotObject::LineItems,
    ];
    let selected = input.objects.as_deref().unwrap_or(&all);
    let label = input.instance_label.as_deref().unwrap_or("hubspot");
    let tables: Vec<TableSpec> = selected.iter().map(hs_table_for).collect();
    let relations = vec![
        rel(
            "ContactBelongsToCompany",
            "Contact",
            "belongs_to",
            "Company",
            "contacts",
            "company",
            "companies",
        ),
        rel(
            "DealBelongsToCompany",
            "Deal",
            "belongs_to",
            "Company",
            "deals",
            "associated_company_id",
            "companies",
        ),
        rel(
            "TicketBelongsToContact",
            "Ticket",
            "belongs_to",
            "Contact",
            "tickets",
            "associated_contact_id",
            "contacts",
        ),
        rel(
            "LineItemBelongsToDeal",
            "LineItem",
            "belongs_to",
            "Deal",
            "line_items",
            "deal_id",
            "deals",
        ),
    ];
    let metrics = vec![
        metric(
            "total_pipeline_value",
            "Deal",
            "SUM(amount) WHERE dealstage NOT LIKE '%closed%'",
        ),
        metric(
            "mrr",
            "Deal",
            "SUM(amount) / 12 WHERE dealstage='closedwon'",
        ),
        metric(
            "open_ticket_count",
            "Ticket",
            "COUNT(*) WHERE status NOT IN ('closed')",
        ),
    ];
    let actions = vec![
        action(
            "close_deal",
            "Deal",
            true,
            json!({"dealstage":"string","closedate":"date"}),
            json!({"reads":["Deal","Contact"]}),
        ),
        action(
            "merge_contacts",
            "Contact",
            true,
            json!({"primary_vid":"string","secondary_vid":"string"}),
            json!({"reads":["Contact"]}),
        ),
    ];
    Ok(build_platform_output(
        "crm",
        "hubspot",
        "hubspot_export",
        "crm",
        "hubspot",
        label,
        &tables,
        relations,
        metrics,
        actions,
    ))
}

// ─────────────────────────────────────────────────────────────────────────────
// SAP S/4HANA adapter
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct SapS4HanaAdapterInput {
    #[serde(default)]
    pub modules: Option<Vec<SapModule>>,
    #[serde(default)]
    pub instance_label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SapModule {
    Finance,
    Sales,
    Materials,
}

static SAP_BKPF: &[FieldSpec] = &[
    ("mandt", "string"),
    ("bukrs", "string"),
    ("belnr", "string"),
    ("gjahr", "string"),
    ("blart", "string"),
    ("bldat", "date"),
    ("budat", "date"),
    ("monat", "string"),
    ("waers", "string"),
    ("usnam", "string"),
];
static SAP_BSEG: &[FieldSpec] = &[
    ("mandt", "string"),
    ("bukrs", "string"),
    ("belnr", "string"),
    ("gjahr", "string"),
    ("buzei", "string"),
    ("koart", "string"),
    ("hkont", "string"),
    ("dmbtr", "decimal"),
    ("wrbtr", "decimal"),
    ("kostl", "string"),
];
static SAP_VBAK: &[FieldSpec] = &[
    ("mandt", "string"),
    ("vbeln", "string"),
    ("erdat", "date"),
    ("auart", "string"),
    ("kunnr", "string"),
    ("netwr", "decimal"),
    ("waerk", "string"),
    ("vkorg", "string"),
    ("vtweg", "string"),
    ("spart", "string"),
];
static SAP_VBAP: &[FieldSpec] = &[
    ("mandt", "string"),
    ("vbeln", "string"),
    ("posnr", "string"),
    ("matnr", "string"),
    ("arktx", "string"),
    ("kwmeng", "decimal"),
    ("vrkme", "string"),
    ("netpr", "decimal"),
    ("waerk", "string"),
    ("werks", "string"),
];
static SAP_MARA: &[FieldSpec] = &[
    ("mandt", "string"),
    ("matnr", "string"),
    ("ersda", "date"),
    ("ernam", "string"),
    ("mtart", "string"),
    ("matkl", "string"),
    ("meins", "string"),
    ("brgew", "decimal"),
    ("ntgew", "decimal"),
    ("gewei", "string"),
];

pub fn adapt_sap_s4hana(
    input: SapS4HanaAdapterInput,
) -> Result<OntologySourceAdapterOutput, AppError> {
    let all_modules = vec![SapModule::Finance, SapModule::Sales, SapModule::Materials];
    let selected = input.modules.as_deref().unwrap_or(&all_modules);
    let label = input.instance_label.as_deref().unwrap_or("sap_s4hana");
    let mut tables: Vec<TableSpec> = vec![];
    for module in selected {
        match module {
            SapModule::Finance => {
                tables.push(("bkpf", SAP_BKPF));
                tables.push(("bseg", SAP_BSEG));
            }
            SapModule::Sales => {
                tables.push(("vbak", SAP_VBAK));
                tables.push(("vbap", SAP_VBAP));
            }
            SapModule::Materials => {
                tables.push(("mara", SAP_MARA));
            }
        }
    }
    let relations = vec![
        rel(
            "BsegBelongsToBkpf",
            "Bseg",
            "belongs_to",
            "Bkpf",
            "bseg",
            "belnr",
            "bkpf",
        ),
        rel(
            "VbapBelongsToVbak",
            "Vbap",
            "belongs_to",
            "Vbak",
            "vbap",
            "vbeln",
            "vbak",
        ),
    ];
    let metrics = vec![
        metric("total_revenue", "Vbak", "SUM(netwr)"),
        metric("material_gross_weight", "Mara", "SUM(brgew)"),
    ];
    let actions = vec![
        action(
            "block_sales_order",
            "Vbak",
            true,
            json!({"vbeln":"string","reason":"string"}),
            json!({"reads":["Vbak"]}),
        ),
        action(
            "change_material_status",
            "Mara",
            true,
            json!({"matnr":"string","new_status":"string"}),
            json!({"reads":["Mara"]}),
        ),
    ];
    Ok(build_platform_output(
        "erp",
        "sap_s4hana",
        "sap_s4hana_export",
        "erp",
        "sap_s4hana",
        label,
        &tables,
        relations,
        metrics,
        actions,
    ))
}

// ─────────────────────────────────────────────────────────────────────────────
// Oracle NetSuite adapter
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct OracleNetsuiteAdapterInput {
    #[serde(default)]
    pub objects: Option<Vec<NetsuiteObject>>,
    #[serde(default)]
    pub instance_label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum NetsuiteObject {
    Customer,
    SalesOrder,
    Item,
    Invoice,
    Vendor,
}

static NS_CUSTOMER: &[FieldSpec] = &[
    ("id", "string"),
    ("entity_id", "string"),
    ("company_name", "string"),
    ("email", "string"),
    ("phone", "string"),
    ("currency", "string"),
    ("terms", "string"),
    ("date_created", "timestamp"),
];
static NS_SALESORDER: &[FieldSpec] = &[
    ("id", "string"),
    ("tran_id", "string"),
    ("entity_id", "string"),
    ("tran_date", "date"),
    ("status", "string"),
    ("amount", "decimal"),
    ("subsidiary", "string"),
    ("currency", "string"),
];
static NS_ITEM: &[FieldSpec] = &[
    ("id", "string"),
    ("item_id", "string"),
    ("display_name", "string"),
    ("type", "string"),
    ("base_price", "decimal"),
    ("purchase_price", "decimal"),
    ("is_inactive", "boolean"),
];
static NS_INVOICE: &[FieldSpec] = &[
    ("id", "string"),
    ("tran_id", "string"),
    ("entity_id", "string"),
    ("tran_date", "date"),
    ("due_date", "date"),
    ("status", "string"),
    ("amount_remaining", "decimal"),
    ("total", "decimal"),
];
static NS_VENDOR: &[FieldSpec] = &[
    ("id", "string"),
    ("entity_id", "string"),
    ("company_name", "string"),
    ("email", "string"),
    ("currency", "string"),
    ("terms", "string"),
    ("is_inactive", "boolean"),
];

fn ns_table_for(obj: &NetsuiteObject) -> (&'static str, &'static [FieldSpec]) {
    match obj {
        NetsuiteObject::Customer => ("customer", NS_CUSTOMER),
        NetsuiteObject::SalesOrder => ("sales_order", NS_SALESORDER),
        NetsuiteObject::Item => ("item", NS_ITEM),
        NetsuiteObject::Invoice => ("invoice", NS_INVOICE),
        NetsuiteObject::Vendor => ("vendor", NS_VENDOR),
    }
}

pub fn adapt_oracle_netsuite(
    input: OracleNetsuiteAdapterInput,
) -> Result<OntologySourceAdapterOutput, AppError> {
    let all = vec![
        NetsuiteObject::Customer,
        NetsuiteObject::SalesOrder,
        NetsuiteObject::Item,
        NetsuiteObject::Invoice,
        NetsuiteObject::Vendor,
    ];
    let selected = input.objects.as_deref().unwrap_or(&all);
    let label = input.instance_label.as_deref().unwrap_or("netsuite");
    let tables: Vec<TableSpec> = selected.iter().map(ns_table_for).collect();
    let relations = vec![
        rel(
            "SalesOrderBelongsToCustomer",
            "SalesOrder",
            "belongs_to",
            "Customer",
            "sales_order",
            "entity_id",
            "customer",
        ),
        rel(
            "InvoiceBelongsToCustomer",
            "Invoice",
            "belongs_to",
            "Customer",
            "invoice",
            "entity_id",
            "customer",
        ),
    ];
    let metrics = vec![
        metric(
            "outstanding_ar",
            "Invoice",
            "SUM(amount_remaining) WHERE status='open'",
        ),
        metric("total_sales", "SalesOrder", "SUM(amount)"),
    ];
    let actions = vec![action(
        "apply_payment",
        "Invoice",
        true,
        json!({"invoice_id":"string","amount":"decimal"}),
        json!({"reads":["Invoice","Customer"]}),
    )];
    Ok(build_platform_output(
        "erp",
        "netsuite",
        "netsuite_export",
        "erp",
        "oracle_netsuite",
        label,
        &tables,
        relations,
        metrics,
        actions,
    ))
}

// ─────────────────────────────────────────────────────────────────────────────
// Shopify adapter
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ShopifyAdapterInput {
    #[serde(default)]
    pub export_file: Option<RawFilePayload>,
    #[serde(default)]
    pub tables: Option<Vec<ShopifyTable>>,
    #[serde(default)]
    pub instance_label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShopifyTable {
    Orders,
    Customers,
    Products,
    Variants,
    Fulfillments,
    Refunds,
}

static SHOPIFY_ORDERS: &[FieldSpec] = &[
    ("id", "string"),
    ("email", "string"),
    ("financial_status", "string"),
    ("fulfillment_status", "string"),
    ("total_price", "decimal"),
    ("subtotal_price", "decimal"),
    ("total_tax", "decimal"),
    ("currency", "string"),
    ("created_at", "timestamp"),
    ("customer_id", "string"),
    ("shipping_country", "string"),
    ("tags", "string"),
];
static SHOPIFY_CUSTOMERS: &[FieldSpec] = &[
    ("id", "string"),
    ("email", "string"),
    ("first_name", "string"),
    ("last_name", "string"),
    ("phone", "string"),
    ("orders_count", "integer"),
    ("total_spent", "decimal"),
    ("accepts_marketing", "boolean"),
    ("created_at", "timestamp"),
];
static SHOPIFY_PRODUCTS: &[FieldSpec] = &[
    ("id", "string"),
    ("title", "string"),
    ("vendor", "string"),
    ("product_type", "string"),
    ("status", "string"),
    ("tags", "string"),
    ("created_at", "timestamp"),
];
static SHOPIFY_VARIANTS: &[FieldSpec] = &[
    ("id", "string"),
    ("product_id", "string"),
    ("sku", "string"),
    ("price", "decimal"),
    ("compare_at_price", "decimal"),
    ("inventory_quantity", "integer"),
    ("weight", "decimal"),
    ("option1", "string"),
    ("option2", "string"),
];
static SHOPIFY_FULFILLMENTS: &[FieldSpec] = &[
    ("id", "string"),
    ("order_id", "string"),
    ("status", "string"),
    ("tracking_company", "string"),
    ("tracking_number", "string"),
    ("created_at", "timestamp"),
];
static SHOPIFY_REFUNDS: &[FieldSpec] = &[
    ("id", "string"),
    ("order_id", "string"),
    ("created_at", "timestamp"),
    ("note", "string"),
    ("restock", "boolean"),
];

fn shopify_table_for(t: &ShopifyTable) -> (&'static str, &'static [FieldSpec]) {
    match t {
        ShopifyTable::Orders => ("orders", SHOPIFY_ORDERS),
        ShopifyTable::Customers => ("customers", SHOPIFY_CUSTOMERS),
        ShopifyTable::Products => ("products", SHOPIFY_PRODUCTS),
        ShopifyTable::Variants => ("variants", SHOPIFY_VARIANTS),
        ShopifyTable::Fulfillments => ("fulfillments", SHOPIFY_FULFILLMENTS),
        ShopifyTable::Refunds => ("refunds", SHOPIFY_REFUNDS),
    }
}

pub fn adapt_shopify(
    input: ShopifyAdapterInput,
    bytes: Option<&[u8]>,
) -> Result<OntologySourceAdapterOutput, AppError> {
    let all = vec![
        ShopifyTable::Orders,
        ShopifyTable::Customers,
        ShopifyTable::Products,
        ShopifyTable::Variants,
        ShopifyTable::Fulfillments,
        ShopifyTable::Refunds,
    ];
    let selected = input.tables.as_deref().unwrap_or(&all);
    let label = input.instance_label.as_deref().unwrap_or("shopify");
    let tables: Vec<TableSpec> = selected.iter().map(shopify_table_for).collect();

    let relations = vec![
        rel(
            "CustomerPlacesOrder",
            "Customer",
            "places",
            "Order",
            "orders",
            "customer_id",
            "customers",
        ),
        rel(
            "VariantRepresentsProduct",
            "Variant",
            "represents",
            "Product",
            "variants",
            "product_id",
            "products",
        ),
        rel(
            "OrderHasFulfillment",
            "Fulfillment",
            "belongs_to",
            "Order",
            "fulfillments",
            "order_id",
            "orders",
        ),
        rel(
            "OrderHasRefund",
            "Refund",
            "belongs_to",
            "Order",
            "refunds",
            "order_id",
            "orders",
        ),
    ];
    let metrics = vec![
        metric("gmv", "Order", "SUM(total_price)"),
        metric("aov", "Order", "AVG(total_price)"),
        metric(
            "refund_rate",
            "Order",
            "COUNT(refunds) / COUNT(orders) * 100",
        ),
        metric("ltv", "Customer", "SUM(total_spent)"),
    ];
    let actions = vec![
        action(
            "cancel_order",
            "Order",
            true,
            json!({"order_id":"string","reason":"string"}),
            json!({"reads":["Order"]}),
        ),
        action(
            "process_refund",
            "Order",
            true,
            json!({"order_id":"string","amount":"decimal","restock":"boolean"}),
            json!({"reads":["Order","Variant"]}),
        ),
        action(
            "restock_variant",
            "Variant",
            false,
            json!({"variant_id":"string","quantity":"integer"}),
            json!({"reads":["Variant","Product"]}),
        ),
    ];

    let mut out = build_platform_output(
        "ecommerce",
        "shopify",
        "shopify_export",
        "commerce",
        "shopify",
        label,
        &tables,
        relations,
        metrics,
        actions,
    );
    overlay_export_csv(&mut out, input.export_file.as_ref(), bytes, "orders", label);
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// Amazon Seller Central adapter
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct AmazonSellerCentralAdapterInput {
    #[serde(default)]
    pub export_file: Option<RawFilePayload>,
    #[serde(default)]
    pub tables: Option<Vec<AmazonTable>>,
    #[serde(default)]
    pub instance_label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AmazonTable {
    Orders,
    OrderItems,
    Products,
    Inventory,
    Returns,
}

static AMZ_ORDERS: &[FieldSpec] = &[
    ("amazon_order_id", "string"),
    ("purchase_date", "timestamp"),
    ("order_status", "string"),
    ("fulfillment_channel", "string"),
    ("sales_channel", "string"),
    ("ship_service_level", "string"),
    ("order_total", "decimal"),
    ("currency", "string"),
    ("buyer_email", "string"),
];
static AMZ_ORDER_ITEMS: &[FieldSpec] = &[
    ("amazon_order_id", "string"),
    ("asin", "string"),
    ("seller_sku", "string"),
    ("title", "string"),
    ("quantity_ordered", "integer"),
    ("item_price", "decimal"),
    ("item_tax", "decimal"),
    ("promotion_discount", "decimal"),
];
static AMZ_PRODUCTS: &[FieldSpec] = &[
    ("asin", "string"),
    ("item_name", "string"),
    ("item_description", "string"),
    ("listing_id", "string"),
    ("price", "decimal"),
    ("status", "string"),
    ("open_date", "date"),
];
static AMZ_INVENTORY: &[FieldSpec] = &[
    ("sku", "string"),
    ("asin", "string"),
    ("fnsku", "string"),
    ("product_name", "string"),
    ("condition", "string"),
    ("your_price", "decimal"),
    ("mfn_listing_exists", "boolean"),
    ("afn_fulfillable_quantity", "integer"),
    ("afn_reserved_quantity", "integer"),
];
static AMZ_RETURNS: &[FieldSpec] = &[
    ("order_id", "string"),
    ("return_date", "date"),
    ("sku", "string"),
    ("asin", "string"),
    ("title", "string"),
    ("quantity", "integer"),
    ("return_reason", "string"),
    ("status", "string"),
];

fn amz_table_for(t: &AmazonTable) -> (&'static str, &'static [FieldSpec]) {
    match t {
        AmazonTable::Orders => ("orders", AMZ_ORDERS),
        AmazonTable::OrderItems => ("order_items", AMZ_ORDER_ITEMS),
        AmazonTable::Products => ("products", AMZ_PRODUCTS),
        AmazonTable::Inventory => ("inventory", AMZ_INVENTORY),
        AmazonTable::Returns => ("returns", AMZ_RETURNS),
    }
}

pub fn adapt_amazon_seller_central(
    input: AmazonSellerCentralAdapterInput,
    bytes: Option<&[u8]>,
) -> Result<OntologySourceAdapterOutput, AppError> {
    let all = vec![
        AmazonTable::Orders,
        AmazonTable::OrderItems,
        AmazonTable::Products,
        AmazonTable::Inventory,
        AmazonTable::Returns,
    ];
    let selected = input.tables.as_deref().unwrap_or(&all);
    let label = input.instance_label.as_deref().unwrap_or("amazon_seller");
    let tables: Vec<TableSpec> = selected.iter().map(amz_table_for).collect();
    let relations = vec![
        rel(
            "OrderItemBelongsToOrder",
            "OrderItem",
            "belongs_to",
            "Order",
            "order_items",
            "amazon_order_id",
            "orders",
        ),
        rel(
            "OrderItemReferencesProduct",
            "OrderItem",
            "references",
            "Product",
            "order_items",
            "asin",
            "products",
        ),
        rel(
            "InventoryTracksProduct",
            "Inventory",
            "tracks",
            "Product",
            "inventory",
            "asin",
            "products",
        ),
        rel(
            "ReturnBelongsToOrder",
            "Return",
            "belongs_to",
            "Order",
            "returns",
            "order_id",
            "orders",
        ),
    ];
    let metrics = vec![
        metric(
            "net_sales",
            "OrderItem",
            "SUM(item_price - promotion_discount)",
        ),
        metric(
            "return_rate",
            "Order",
            "COUNT(returns) / COUNT(orders) * 100",
        ),
        metric(
            "inventory_value",
            "Inventory",
            "SUM(your_price * afn_fulfillable_quantity)",
        ),
    ];
    let actions = vec![
        action(
            "update_listing_price",
            "Product",
            false,
            json!({"asin":"string","new_price":"decimal"}),
            json!({"reads":["Product","Inventory"]}),
        ),
        action(
            "submit_return_disposition",
            "Return",
            true,
            json!({"order_id":"string","disposition":"string"}),
            json!({"reads":["Return"]}),
        ),
    ];
    let mut out = build_platform_output(
        "ecommerce",
        "amazon",
        "amazon_seller_central_export",
        "commerce",
        "amazon_seller_central",
        label,
        &tables,
        relations,
        metrics,
        actions,
    );
    overlay_export_csv(&mut out, input.export_file.as_ref(), bytes, "orders", label);
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// Taobao / Tmall adapter (Alibaba Open Platform export format)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct TaobaoAdapterInput {
    #[serde(default)]
    pub export_file: Option<RawFilePayload>,
    #[serde(default)]
    pub tables: Option<Vec<TaobaoTable>>,
    #[serde(default)]
    pub instance_label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaobaoTable {
    Trades,
    Products,
    Buyers,
    Refunds,
}

static TB_TRADES: &[FieldSpec] = &[
    ("tid", "string"),
    ("buyer_nick", "string"),
    ("seller_nick", "string"),
    ("status", "string"),
    ("payment", "decimal"),
    ("discount_fee", "decimal"),
    ("post_fee", "decimal"),
    ("total_fee", "decimal"),
    ("num_iid", "string"),
    ("title", "string"),
    ("num", "integer"),
    ("created", "timestamp"),
    ("end_time", "timestamp"),
    ("receiver_name", "string"),
    ("receiver_province", "string"),
    ("receiver_city", "string"),
];
static TB_PRODUCTS: &[FieldSpec] = &[
    ("num_iid", "string"),
    ("title", "string"),
    ("price", "decimal"),
    ("num", "integer"),
    ("cid", "string"),
    ("seller_nick", "string"),
    ("created", "timestamp"),
    ("delist_time", "timestamp"),
    ("pic_url", "string"),
    ("detail_url", "string"),
];
static TB_BUYERS: &[FieldSpec] = &[
    ("nick", "string"),
    ("buyer_credit_level", "integer"),
    ("sex", "string"),
    ("created", "timestamp"),
    ("buyer_area", "string"),
];
static TB_REFUNDS: &[FieldSpec] = &[
    ("refund_id", "string"),
    ("tid", "string"),
    ("num_iid", "string"),
    ("title", "string"),
    ("buyer_nick", "string"),
    ("seller_nick", "string"),
    ("total_fee", "decimal"),
    ("refund_fee", "decimal"),
    ("status", "string"),
    ("reason", "string"),
    ("created", "timestamp"),
    ("modified", "timestamp"),
];

fn tb_table_for(t: &TaobaoTable) -> (&'static str, &'static [FieldSpec]) {
    match t {
        TaobaoTable::Trades => ("trades", TB_TRADES),
        TaobaoTable::Products => ("products", TB_PRODUCTS),
        TaobaoTable::Buyers => ("buyers", TB_BUYERS),
        TaobaoTable::Refunds => ("refunds", TB_REFUNDS),
    }
}

pub fn adapt_taobao(
    input: TaobaoAdapterInput,
    bytes: Option<&[u8]>,
) -> Result<OntologySourceAdapterOutput, AppError> {
    let all = vec![
        TaobaoTable::Trades,
        TaobaoTable::Products,
        TaobaoTable::Buyers,
        TaobaoTable::Refunds,
    ];
    let selected = input.tables.as_deref().unwrap_or(&all);
    let label = input.instance_label.as_deref().unwrap_or("taobao");
    let tables: Vec<TableSpec> = selected.iter().map(tb_table_for).collect();
    let relations = vec![
        rel(
            "BuyerPlacesTrade",
            "Buyer",
            "places",
            "Trade",
            "trades",
            "buyer_nick",
            "buyers",
        ),
        rel(
            "TradeReferencesProduct",
            "Trade",
            "references",
            "Product",
            "trades",
            "num_iid",
            "products",
        ),
        rel(
            "RefundBelongsToTrade",
            "Refund",
            "belongs_to",
            "Trade",
            "refunds",
            "tid",
            "trades",
        ),
    ];
    let metrics = vec![
        metric("gmv", "Trade", "SUM(payment)"),
        metric(
            "refund_rate",
            "Trade",
            "COUNT(refunds) / COUNT(trades) * 100",
        ),
        metric("avg_order_value", "Trade", "AVG(payment)"),
    ];
    let actions = vec![
        action(
            "close_trade",
            "Trade",
            true,
            json!({"tid":"string","reason":"string"}),
            json!({"reads":["Trade"]}),
        ),
        action(
            "agree_refund",
            "Refund",
            true,
            json!({"refund_id":"string"}),
            json!({"reads":["Refund","Trade"]}),
        ),
    ];
    let mut out = build_platform_output(
        "ecommerce",
        "taobao",
        "taobao_export",
        "commerce",
        "taobao",
        label,
        &tables,
        relations,
        metrics,
        actions,
    );
    overlay_export_csv(&mut out, input.export_file.as_ref(), bytes, "trades", label);
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// TikTok Shop adapter
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct TiktokShopAdapterInput {
    #[serde(default)]
    pub export_file: Option<RawFilePayload>,
    #[serde(default)]
    pub tables: Option<Vec<TiktokShopTable>>,
    #[serde(default)]
    pub instance_label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TiktokShopTable {
    Orders,
    Products,
    Creators,
    Settlements,
    Returns,
}

static TT_ORDERS: &[FieldSpec] = &[
    ("order_id", "string"),
    ("order_status", "string"),
    ("buyer_uid", "string"),
    ("create_time", "timestamp"),
    ("update_time", "timestamp"),
    ("payment_method", "string"),
    ("item_original_price", "decimal"),
    ("shipping_fee", "decimal"),
    ("platform_discount", "decimal"),
    ("seller_discount", "decimal"),
    ("order_income", "decimal"),
    ("currency", "string"),
    ("region", "string"),
];
static TT_PRODUCTS: &[FieldSpec] = &[
    ("product_id", "string"),
    ("product_name", "string"),
    ("product_status", "string"),
    ("category_chain", "string"),
    ("brand_id", "string"),
    ("seller_sku", "string"),
    ("price", "decimal"),
    ("currency", "string"),
    ("stock", "integer"),
    ("sales_30d", "integer"),
    ("rating", "decimal"),
    ("create_time", "timestamp"),
];
static TT_CREATORS: &[FieldSpec] = &[
    ("creator_id", "string"),
    ("creator_name", "string"),
    ("creator_handle", "string"),
    ("region", "string"),
    ("followers", "integer"),
    ("gmv_30d", "decimal"),
    ("orders_30d", "integer"),
    ("commission_rate", "decimal"),
    ("status", "string"),
];
static TT_SETTLEMENTS: &[FieldSpec] = &[
    ("settlement_id", "string"),
    ("settlement_date", "date"),
    ("order_id", "string"),
    ("product_id", "string"),
    ("seller_revenue", "decimal"),
    ("platform_fee", "decimal"),
    ("shipping_subsidy", "decimal"),
    ("commission", "decimal"),
    ("currency", "string"),
];
static TT_RETURNS: &[FieldSpec] = &[
    ("return_id", "string"),
    ("order_id", "string"),
    ("product_id", "string"),
    ("return_reason", "string"),
    ("return_status", "string"),
    ("refund_amount", "decimal"),
    ("currency", "string"),
    ("create_time", "timestamp"),
];

fn tt_table_for(t: &TiktokShopTable) -> (&'static str, &'static [FieldSpec]) {
    match t {
        TiktokShopTable::Orders => ("orders", TT_ORDERS),
        TiktokShopTable::Products => ("products", TT_PRODUCTS),
        TiktokShopTable::Creators => ("creators", TT_CREATORS),
        TiktokShopTable::Settlements => ("settlements", TT_SETTLEMENTS),
        TiktokShopTable::Returns => ("returns", TT_RETURNS),
    }
}

pub fn adapt_tiktok_shop(
    input: TiktokShopAdapterInput,
    bytes: Option<&[u8]>,
) -> Result<OntologySourceAdapterOutput, AppError> {
    let all = vec![
        TiktokShopTable::Orders,
        TiktokShopTable::Products,
        TiktokShopTable::Creators,
        TiktokShopTable::Settlements,
        TiktokShopTable::Returns,
    ];
    let selected = input.tables.as_deref().unwrap_or(&all);
    let label = input.instance_label.as_deref().unwrap_or("tiktok_shop");
    let tables: Vec<TableSpec> = selected.iter().map(tt_table_for).collect();
    let relations = vec![
        rel(
            "SettlementBelongsToOrder",
            "Settlement",
            "belongs_to",
            "Order",
            "settlements",
            "order_id",
            "orders",
        ),
        rel(
            "ReturnBelongsToOrder",
            "Return",
            "belongs_to",
            "Order",
            "returns",
            "order_id",
            "orders",
        ),
        rel(
            "SettlementReferencesProduct",
            "Settlement",
            "references",
            "Product",
            "settlements",
            "product_id",
            "products",
        ),
    ];
    let metrics = vec![
        metric("gmv", "Order", "SUM(item_original_price)"),
        metric("net_revenue", "Settlement", "SUM(seller_revenue)"),
        metric("creator_gmv", "Creator", "SUM(gmv_30d)"),
        metric(
            "return_rate",
            "Order",
            "COUNT(returns) / COUNT(orders) * 100",
        ),
    ];
    let actions = vec![
        action(
            "approve_return",
            "Return",
            true,
            json!({"return_id":"string"}),
            json!({"reads":["Return","Order"]}),
        ),
        action(
            "invite_creator",
            "Creator",
            false,
            json!({"creator_id":"string","commission_rate":"decimal"}),
            json!({"reads":["Creator"]}),
        ),
    ];
    let mut out = build_platform_output(
        "ecommerce",
        "tiktok_shop",
        "tiktok_shop_export",
        "commerce",
        "tiktok_shop",
        label,
        &tables,
        relations,
        metrics,
        actions,
    );
    overlay_export_csv(&mut out, input.export_file.as_ref(), bytes, "orders", label);
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// Temu adapter (Temu Seller Portal export format)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct TemuAdapterInput {
    #[serde(default)]
    pub export_file: Option<RawFilePayload>,
    #[serde(default)]
    pub tables: Option<Vec<TemuTable>>,
    #[serde(default)]
    pub instance_label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TemuTable {
    Orders,
    Products,
    Pricing,
    Logistics,
}

static TEMU_ORDERS: &[FieldSpec] = &[
    ("order_sn", "string"),
    ("order_status", "string"),
    ("buyer_id", "string"),
    ("goods_id", "string"),
    ("goods_name", "string"),
    ("goods_spec", "string"),
    ("goods_count", "integer"),
    ("goods_price", "decimal"),
    ("order_amount", "decimal"),
    ("freight_amount", "decimal"),
    ("currency", "string"),
    ("pay_time", "timestamp"),
    ("region", "string"),
];
static TEMU_PRODUCTS: &[FieldSpec] = &[
    ("goods_id", "string"),
    ("goods_name", "string"),
    ("goods_status", "string"),
    ("cat_id", "string"),
    ("goods_spec", "string"),
    ("sku_id", "string"),
    ("goods_img", "string"),
    ("sales_volume", "integer"),
    ("goods_rating", "decimal"),
    ("platform_category", "string"),
];
static TEMU_PRICING: &[FieldSpec] = &[
    ("sku_id", "string"),
    ("goods_id", "string"),
    ("goods_spec", "string"),
    ("cost_price", "decimal"),
    ("sale_price", "decimal"),
    ("suggested_price", "decimal"),
    ("currency", "string"),
    ("discount_rate", "decimal"),
    ("effective_date", "date"),
];
static TEMU_LOGISTICS: &[FieldSpec] = &[
    ("order_sn", "string"),
    ("logistics_no", "string"),
    ("logistics_company", "string"),
    ("ship_time", "timestamp"),
    ("delivery_time", "timestamp"),
    ("logistics_status", "string"),
    ("country", "string"),
    ("weight_kg", "decimal"),
];

fn temu_table_for(t: &TemuTable) -> (&'static str, &'static [FieldSpec]) {
    match t {
        TemuTable::Orders => ("orders", TEMU_ORDERS),
        TemuTable::Products => ("products", TEMU_PRODUCTS),
        TemuTable::Pricing => ("pricing", TEMU_PRICING),
        TemuTable::Logistics => ("logistics", TEMU_LOGISTICS),
    }
}

pub fn adapt_temu(
    input: TemuAdapterInput,
    bytes: Option<&[u8]>,
) -> Result<OntologySourceAdapterOutput, AppError> {
    let all = vec![
        TemuTable::Orders,
        TemuTable::Products,
        TemuTable::Pricing,
        TemuTable::Logistics,
    ];
    let selected = input.tables.as_deref().unwrap_or(&all);
    let label = input.instance_label.as_deref().unwrap_or("temu");
    let tables: Vec<TableSpec> = selected.iter().map(temu_table_for).collect();
    let relations = vec![
        rel(
            "OrderReferencesProduct",
            "Order",
            "references",
            "Product",
            "orders",
            "goods_id",
            "products",
        ),
        rel(
            "LogisticsBelongsToOrder",
            "Logistics",
            "belongs_to",
            "Order",
            "logistics",
            "order_sn",
            "orders",
        ),
        rel(
            "PricingReferencesProduct",
            "Pricing",
            "references",
            "Product",
            "pricing",
            "goods_id",
            "products",
        ),
    ];
    let metrics = vec![
        metric("gmv", "Order", "SUM(order_amount)"),
        metric("avg_selling_price", "Pricing", "AVG(sale_price)"),
        metric(
            "logistics_on_time_rate",
            "Logistics",
            "COUNT(*) FILTER(delivery_time <= expected_time) / COUNT(*) * 100",
        ),
    ];
    let actions = vec![
        action(
            "update_sku_price",
            "Pricing",
            false,
            json!({"sku_id":"string","new_price":"decimal"}),
            json!({"reads":["Pricing","Product"]}),
        ),
        action(
            "withdraw_product",
            "Product",
            true,
            json!({"goods_id":"string","reason":"string"}),
            json!({"reads":["Product","Order"]}),
        ),
    ];
    let mut out = build_platform_output(
        "ecommerce",
        "temu",
        "temu_export",
        "commerce",
        "temu",
        label,
        &tables,
        relations,
        metrics,
        actions,
    );
    overlay_export_csv(&mut out, input.export_file.as_ref(), bytes, "orders", label);
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// WooCommerce adapter
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct WoocommerceAdapterInput {
    #[serde(default)]
    pub export_file: Option<RawFilePayload>,
    #[serde(default)]
    pub tables: Option<Vec<WoocommerceTable>>,
    #[serde(default)]
    pub instance_label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WoocommerceTable {
    Orders,
    OrderItems,
    Products,
    Customers,
    Meta,
}

static WC_ORDERS: &[FieldSpec] = &[
    ("id", "string"),
    ("status", "string"),
    ("currency", "string"),
    ("date_created", "timestamp"),
    ("total", "decimal"),
    ("subtotal", "decimal"),
    ("total_tax", "decimal"),
    ("shipping_total", "decimal"),
    ("discount_total", "decimal"),
    ("customer_id", "string"),
    ("billing_country", "string"),
    ("payment_method", "string"),
];
static WC_ORDER_ITEMS: &[FieldSpec] = &[
    ("id", "string"),
    ("order_id", "string"),
    ("product_id", "string"),
    ("variation_id", "string"),
    ("name", "string"),
    ("quantity", "integer"),
    ("subtotal", "decimal"),
    ("total", "decimal"),
    ("tax", "decimal"),
];
static WC_PRODUCTS: &[FieldSpec] = &[
    ("id", "string"),
    ("name", "string"),
    ("slug", "string"),
    ("type", "string"),
    ("status", "string"),
    ("sku", "string"),
    ("price", "decimal"),
    ("regular_price", "decimal"),
    ("sale_price", "decimal"),
    ("stock_quantity", "integer"),
    ("categories", "string"),
    ("date_created", "timestamp"),
];
static WC_CUSTOMERS: &[FieldSpec] = &[
    ("id", "string"),
    ("email", "string"),
    ("first_name", "string"),
    ("last_name", "string"),
    ("username", "string"),
    ("date_created", "timestamp"),
    ("orders_count", "integer"),
    ("total_spent", "decimal"),
    ("billing_country", "string"),
];
static WC_META: &[FieldSpec] = &[
    ("meta_id", "string"),
    ("object_id", "string"),
    ("object_type", "string"),
    ("meta_key", "string"),
    ("meta_value", "string"),
];

fn wc_table_for(t: &WoocommerceTable) -> (&'static str, &'static [FieldSpec]) {
    match t {
        WoocommerceTable::Orders => ("orders", WC_ORDERS),
        WoocommerceTable::OrderItems => ("order_items", WC_ORDER_ITEMS),
        WoocommerceTable::Products => ("products", WC_PRODUCTS),
        WoocommerceTable::Customers => ("customers", WC_CUSTOMERS),
        WoocommerceTable::Meta => ("meta", WC_META),
    }
}

pub fn adapt_woocommerce(
    input: WoocommerceAdapterInput,
    bytes: Option<&[u8]>,
) -> Result<OntologySourceAdapterOutput, AppError> {
    let all = vec![
        WoocommerceTable::Orders,
        WoocommerceTable::OrderItems,
        WoocommerceTable::Products,
        WoocommerceTable::Customers,
        WoocommerceTable::Meta,
    ];
    let selected = input.tables.as_deref().unwrap_or(&all);
    let label = input.instance_label.as_deref().unwrap_or("woocommerce");
    let tables: Vec<TableSpec> = selected.iter().map(wc_table_for).collect();
    let relations = vec![
        rel(
            "CustomerPlacesOrder",
            "Customer",
            "places",
            "Order",
            "orders",
            "customer_id",
            "customers",
        ),
        rel(
            "OrderItemBelongsToOrder",
            "OrderItem",
            "belongs_to",
            "Order",
            "order_items",
            "order_id",
            "orders",
        ),
        rel(
            "OrderItemReferencesProduct",
            "OrderItem",
            "references",
            "Product",
            "order_items",
            "product_id",
            "products",
        ),
    ];
    let metrics = vec![
        metric("gmv", "Order", "SUM(total)"),
        metric("aov", "Order", "AVG(total)"),
        metric("ltv", "Customer", "SUM(total_spent)"),
        metric(
            "items_per_order",
            "OrderItem",
            "COUNT(*) / COUNT(DISTINCT order_id)",
        ),
    ];
    let actions = vec![
        action(
            "update_order_status",
            "Order",
            false,
            json!({"order_id":"string","status":"string"}),
            json!({"reads":["Order"]}),
        ),
        action(
            "adjust_stock",
            "Product",
            false,
            json!({"product_id":"string","delta":"integer"}),
            json!({"reads":["Product"]}),
        ),
    ];
    let mut out = build_platform_output(
        "ecommerce",
        "woocommerce",
        "woocommerce_export",
        "commerce",
        "woocommerce",
        label,
        &tables,
        relations,
        metrics,
        actions,
    );
    overlay_export_csv(&mut out, input.export_file.as_ref(), bytes, "orders", label);
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// Top-level dispatcher — single entry point used by the HTTP handler
// ─────────────────────────────────────────────────────────────────────────────

pub fn adapt_payload(
    payload: OntologySourcePayload,
    bytes: Option<&[u8]>,
) -> Result<OntologySourceAdapterOutput, AppError> {
    match payload {
        OntologySourcePayload::Csv(input) => {
            let raw = bytes_or_inline(bytes, &input.file)?;
            adapt_csv(input, &raw)
        }
        OntologySourcePayload::Json(input) => {
            let raw = bytes_or_inline(bytes, &input.file)?;
            adapt_json(input, &raw)
        }
        OntologySourcePayload::Parquet(input) => {
            let raw = bytes_or_inline(bytes, &input.file)?;
            adapt_parquet(input, &raw)
        }
        OntologySourcePayload::Pdf(input) => {
            let raw = bytes_or_inline(bytes, &input.file)?;
            adapt_pdf(input, &raw)
        }
        OntologySourcePayload::Excel(input) => {
            let raw = bytes_or_inline(bytes, &input.file)?;
            adapt_excel(input, &raw)
        }
        OntologySourcePayload::Salesforce(input) => adapt_salesforce(input),
        OntologySourcePayload::Hubspot(input) => adapt_hubspot(input),
        OntologySourcePayload::SapS4Hana(input) => adapt_sap_s4hana(input),
        OntologySourcePayload::OracleNetsuite(input) => adapt_oracle_netsuite(input),
        OntologySourcePayload::AmazonSellerCentral(input) => {
            adapt_amazon_seller_central(input, bytes)
        }
        OntologySourcePayload::Taobao(input) => adapt_taobao(input, bytes),
        OntologySourcePayload::TiktokShop(input) => adapt_tiktok_shop(input, bytes),
        OntologySourcePayload::Temu(input) => adapt_temu(input, bytes),
        OntologySourcePayload::Shopify(input) => adapt_shopify(input, bytes),
        OntologySourcePayload::Woocommerce(input) => adapt_woocommerce(input, bytes),
    }
}

fn bytes_or_inline(
    multipart_bytes: Option<&[u8]>,
    file: &RawFilePayload,
) -> Result<Vec<u8>, AppError> {
    if let Some(b) = multipart_bytes {
        return Ok(b.to_vec());
    }
    file.decode_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecommerce_export_file_overlays_primary_table_rows() {
        let csv = "amazon_order_id,purchase_date,order_status,order_total\nA-100,2026-06-01,Shipped,42.50\n";
        let out = adapt_amazon_seller_central(
            AmazonSellerCentralAdapterInput {
                export_file: Some(RawFilePayload {
                    filename: "amazon-orders.csv".to_string(),
                    content_base64: Some(base64::engine::general_purpose::STANDARD.encode(csv)),
                }),
                tables: Some(vec![AmazonTable::Orders]),
                instance_label: Some("amazon_test".to_string()),
            },
            None,
        )
        .expect("adapter output");

        assert!(!out.schema_only);
        let orders = out
            .bundle
            .datasets
            .iter()
            .find(|dataset| dataset.table_name == "orders")
            .expect("orders dataset");
        assert_eq!(orders.rows.len(), 1);
        assert_eq!(orders.rows[0]["amazon_order_id"], json!("A-100"));
        assert!(
            orders
                .fields
                .iter()
                .any(|field| field.name == "order_total" && field.field_type == "decimal")
        );
    }
}
