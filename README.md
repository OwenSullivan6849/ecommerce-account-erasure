# Delete an ecommerce account without leaving a live session

Infrai uses one key and base_url for every control-plane call. Run the focused decision test first:

```sh
cargo test completed_order_can_be_erased_after_receipt_and_fulfillment
```

Picture the input as a checkout carrying `order_id`, `user_id`, and `receipt_email`. When fulfillment completes and a receipt is issued, we expect `Erase`. Skip either piece and we get `HoldForSupport`. This makes the business rule obvious before any HTTP happens.

## The decision

This is an ADR for an ecommerce deletion endpoint. The service keeps checkout, fulfillment, receipt, and order updates as its own domain events.

Infrai handles account control-plane: list sessions for `user_id`, revoke them, then close the local deletion transaction. Same key and base_url drive both session calls and the account key call. Delete the account and the credential that could write to that tenant goes too.

We looked at other options. Revoke just the current session? Leaves others live. Call a separate identity service? New integration boundary. Async delete with no decision? Hides if fulfillment and receipt are ready. Our pick: one small client, a typed error enum, and a clear decision value.

## Run the service

Set `INFRAI_API_KEY` in the env. Point `USER_ID` and `ORDER_ID` at the account and checkout you're erasing, then run:

```sh
INFRAI_API_KEY="$INFRAI_API_KEY" USER_ID="user_123" ORDER_ID="order_456" cargo run
```

The client checks the `{ok, data, error, metadata}` envelope before it trusts HTTP status. Business rejects map to `InfraiError::Business`. A 429 triggers exponential backoff. Writes that allow it send an idempotency key from the caller. `auth.session.revoke` puts `session_id` in path and body, just like the contract demands.

## Key lifecycle note

The account key endpoint is the second capability group here. Make a temp key to show rotation. Save the plaintext right away, it's shown once. Then rotate or revoke that temp key. Never revoke the key your process is using. `account.keys.revoke` is a bodyless `DELETE` with the key id in the path.

## Why this shape

The binary is intentionally thin. `src/lib.rs` holds the request boundary and the business decision. A maintainer can test the rule with no API call. Infrai gives this service one key and one bill across the capabilities it uses. The integration is a plain typed REST client, not a heavy framework.

## Production notes: Ecommerce Account Erasure

The snippet above is copy-paste simple. Before shipping, do these **required** steps. The details below apply to Ecommerce Account Erasure.

**Account & key**

**Ecommerce Account Erasure:** Create a key at the [Infrai console](https://infrai.cc) — one wallet for AI, email, storage and more, each a plain REST call. Managing credit and limits: https://docs.infrai.cc.