use crate::AppError;

pub(crate) fn normalize_ontology_review_decision(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "approve" | "approved" => Ok("approve".to_string()),
        "reject" | "rejected" => Ok("reject".to_string()),
        "request_changes" | "changes_requested" => Ok("request_changes".to_string()),
        _ => Err(AppError::bad_request(
            "ontology proposal review decision must be approve, reject, or request_changes",
        )),
    }
}
