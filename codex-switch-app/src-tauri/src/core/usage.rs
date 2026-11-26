use chrono::{Local, TimeZone};
use serde_json::{json, Value};

pub(crate) fn summarize(response: &Value) -> Value {
    let primary = &response["rate_limit"]["primary_window"];
    let secondary = &response["rate_limit"]["secondary_window"];
    let windows = [primary, secondary];
    let five_hour = windows
        .iter()
        .copied()
        .find(|v| v["limit_window_seconds"].as_i64() == Some(18000))
        .unwrap_or(primary);
    let weekly = windows
        .iter()
        .copied()
        .find(|v| v["limit_window_seconds"].as_i64() == Some(604800))
        .or_else(|| (!secondary.is_null() && secondary != five_hour).then_some(secondary));
    let left = |v: &Value| {
        v["used_percent"]
            .as_f64()
            .filter(|v| v.is_finite())
            .map(|used| (100.0 - used).clamp(0.0, 100.0))
    };
    let reset = |v: &Value| {
        v["reset_at"]
            .as_i64()
            .and_then(|v| Local.timestamp_opt(v, 0).single())
            .map(|v| v.format("%m-%d %H:%M").to_string())
    };
    let resets: Vec<String> = [reset(five_hour), weekly.and_then(reset)]
        .into_iter()
        .flatten()
        .collect();
    json!({
        "plan": response["plan_type"].as_str().unwrap_or("-"),
        "primaryLeft": left(five_hour),
        "primaryResetAt": five_hour["reset_at"].as_i64(),
        "secondaryLeft": weekly.and_then(left),
        "reset": if resets.is_empty() { "-".to_string() } else { resets.join(" | ") }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn usage_windows_are_selected_by_duration_even_when_swapped() {
        let summary = summarize(&json!({"plan_type":"plus", "rate_limit":{
            "primary_window":{"limit_window_seconds":604800,"used_percent":20},
            "secondary_window":{"limit_window_seconds":18000,"used_percent":30,"reset_at":1800000000}
        }}));
        assert_eq!(summary["primaryLeft"], 70.0);
        assert_eq!(summary["secondaryLeft"], 80.0);
        assert_eq!(summary["primaryResetAt"], 1800000000);
        assert_eq!(summarize(&json!({}))["primaryLeft"], Value::Null);
    }
}
