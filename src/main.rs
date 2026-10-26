use ecommerce_account_erasure::{erase_account, Checkout, InfraiClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = InfraiClient::from_env()?;
    let checkout = Checkout {
        order_id: std::env::var("ORDER_ID").unwrap_or_else(|_| "order_demo".into()),
        user_id: std::env::var("USER_ID").unwrap_or_else(|_| "user_demo".into()),
        receipt_email: "receipt@example.test".into(),
    };
    let decision = erase_account(&client, &checkout).await?;
    if let Ok(temp_key_id) = std::env::var("TEMP_KEY_ID") {
        client.revoke_key(&temp_key_id).await?;
        println!("temporary account key revoked: {temp_key_id}");
    }
    println!("account deletion decision: {decision:?}");
    Ok(())
}
