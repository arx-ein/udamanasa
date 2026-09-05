//! メッセージログのCRUD
use udamanami_shared::{DeleteMessage, Message, UpdateMessage};
use worker::*;

pub async fn insert_messages(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let body = req.text().await?;
    let Ok(messages) = serde_json::from_str::<Vec<Message>>(&body) else {
        return Response::error("Failed to parse request body", 400);
    };

    if messages.is_empty() {
        return Response::ok("No messages to insert");
    }

    let d1 = ctx.env.d1("DB")?;

    let stmts: Vec<D1PreparedStatement> = messages
        .into_iter()
        .map(|message| {
            d1.prepare(
                "INSERT INTO message (message_id, channel_id, user_id, timestamp, content)
                 VALUES (?, ?, ?, ?, ?)
ON CONFLICT (message_id) DO UPDATE SET content = excluded.content;
",
            )
            .bind(&[
                message.message_id.into(),
                message.channel_id.into(),
                message.user_id.into(),
                message.timestamp.into(),
                message.content.into(),
            ])
        })
        .collect::<Result<Vec<_>>>()?;

    let _ = d1.batch(stmts).await?;
    Response::ok("Messages inserted successfully")
}

pub async fn update_message(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let body = req.text().await?;
    let Ok(message) = serde_json::from_str::<UpdateMessage>(&body) else {
        return Response::error("Failed to parse request body", 400);
    };

    let d1 = ctx.env.d1("DB")?;

    let _ = d1
        .prepare("UPDATE message SET content = ? WHERE message_id = ?")
        .bind(&[message.content.into(), message.message_id.into()])?
        .run()
        .await?;

    Response::ok("Message updated successfully")
}

pub async fn delete_message(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let body = req.text().await?;
    let Ok(message) = serde_json::from_str::<DeleteMessage>(&body) else {
        return Response::error("Failed to parse request body", 400);
    };

    let d1 = ctx.env.d1("DB")?;

    let _ = d1
        .prepare("DELETE FROM message WHERE message_id = ?1")
        .bind(&[message.message_id.into()])?
        .run()
        .await?;

    Response::ok("Message deleted successfully")
}

/// 条件(件数上限・並び順・期間)を指定してチャンネルのメッセージを取得する
///
/// クエリパラメータ: channel_id (必須), limit (必須), order (Asc/Desc), from, to,
/// after, after_message_id。after の組は排他的な複合カーソル。
pub async fn get_messages(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let url = req.url()?;

    let Some(channel_id) = crate::query_param(&url, "channel_id") else {
        return Response::error("Missing query parameter: channel_id", 400);
    };
    let Some(limit) = crate::query_param(&url, "limit").and_then(|v| v.parse::<usize>().ok()) else {
        return Response::error("Missing or invalid query parameter: limit", 400);
    };
    let order = match crate::query_param(&url, "order").as_deref() {
        Some("Desc") => "DESC",
        _ => "ASC",
    };
    let summary_pending_only =
        crate::query_param(&url, "summary_pending_only").as_deref() == Some("true");

    let d1 = ctx.env.d1("DB")?;

    // ASC/DESC はプレースホルダにバインドできないため SQL に直接埋め込む(match済みの固定文字列)
    let result = d1
        .prepare(format!(
            "SELECT message.*, user.username FROM message
             LEFT JOIN user ON message.user_id = user.user_id
             WHERE channel_id = ?1
             AND (?6 = 1 OR ?2 IS NULL OR timestamp >= ?2)
             AND (?6 = 1 OR ?3 IS NULL OR timestamp <= ?3)
             AND (
                 ?6 = 1
                 OR ?4 IS NULL
                 OR timestamp > ?4
                 OR (timestamp = ?4 AND ?5 IS NOT NULL AND message_id > ?5)
             )
             AND (?6 = 0 OR summary_pending = 1)
             ORDER BY timestamp {order}, message_id {order} LIMIT ?7"
        ))
        .bind(&[
            channel_id.into(),
            crate::opt_to_js(crate::query_param(&url, "from")),
            crate::opt_to_js(crate::query_param(&url, "to")),
            crate::opt_to_js(crate::query_param(&url, "after")),
            crate::opt_to_js(crate::query_param(&url, "after_message_id")),
            summary_pending_only.into(),
            limit.into(),
        ])?
        .run()
        .await?;

    Response::from_json(&result.results::<Message>()?)
}
