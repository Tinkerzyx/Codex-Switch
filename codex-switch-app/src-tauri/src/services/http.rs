use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::time::Duration;

pub(crate) fn client() -> Result<Client, String> {
    let mut builder = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(format!("Codex-Switch/{}", env!("CARGO_PKG_VERSION")));
    let has_env_proxy = [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ]
    .iter()
    .any(|key| std::env::var_os(key).is_some_and(|value| !value.is_empty()));
    if !has_env_proxy {
        if let Some(proxy) =
            crate::platform::system_proxy().and_then(|v| crate::parse_system_cli_proxy(&v))
        {
            builder = builder.proxy(reqwest::Proxy::all(proxy).map_err(|e| e.to_string())?);
        }
    }
    builder
        .build()
        .map_err(|e| format!("Unable to initialize HTTP: {e}"))
}

pub(crate) fn usage(access_token: &str, account_id: Option<&str>) -> Result<Value, String> {
    let mut request = client()?
        .get("https://chatgpt.com/backend-api/wham/usage")
        .bearer_auth(access_token)
        .header("Accept", "application/json")
        .header("Referer", "https://chatgpt.com/codex/settings/usage")
        .header("originator", "codex-desktop");
    if let Some(account_id) = account_id {
        request = request.header("ChatGPT-Account-Id", account_id);
    }
    let response = request.send().map_err(|_| {
        "Usage request failed. Check your connection and proxy settings.".to_string()
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Usage request returned HTTP {}.", status.as_u16()));
    }
    let response: Value = response
        .json()
        .map_err(|_| "Usage returned invalid JSON.".to_string())?;
    Ok(crate::core::usage::summarize(&response))
}

pub(crate) fn payment(
    fetch_cashier: bool,
    product: &crate::StorePaymentProduct,
) -> Result<Value, String> {
    let client = client()?;
    let response = client.post(crate::CATFK_ORDER_URL)
        .header("Origin", "https://catfk.com").header("Referer", product.item_url)
        .json(&json!({"goods_key":product.goods_key,"quantity":1,"coupon_code":"","channel_id":1,
            "contact":"codex.switch@example.com","query_password":"","select_cards_ids":[],"extend":{}}))
        .send().map_err(|_| "Unable to create payment order.".to_string())?
        .error_for_status().map_err(|e| format!("Payment request failed: {}", e.status().map(|s| s.as_u16()).unwrap_or(0)))?;
    let order: Value = response
        .json()
        .map_err(|_| "Payment returned invalid JSON.".to_string())?;
    let payurl = order["data"]["payurl"]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "Payment order did not contain a payment URL.".to_string())?;
    if order["code"].as_i64() != Some(1) {
        return Err("Payment provider rejected the order.".into());
    }
    let cashier = if fetch_cashier {
        client
            .get(payurl)
            .header("Referer", product.item_url)
            .send()
            .map_err(|_| "Unable to load payment page.".to_string())?
            .error_for_status()
            .map_err(|_| "Payment page returned an error.".to_string())?
            .text()
            .map_err(|_| "Unable to read payment page.".to_string())?
    } else {
        String::new()
    };
    let text = |value: &Value| {
        value
            .as_str()
            .map(str::to_string)
            .or_else(|| value.is_number().then(|| value.to_string()))
    };
    Ok(
        json!({"ok":true,"message":"Payment order created.","tradeNo":text(&order["data"]["trade_no"]).unwrap_or_default(),
        "totalAmount":text(&order["data"]["total_amount"]),"payurl":payurl,"cashierHtml":cashier}),
    )
}
