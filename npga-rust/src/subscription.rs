use crate::{
    links,
    model::{Node, Settings, Subscription},
};
use anyhow::{Context, Result, bail};
use std::{io::Read, time::Duration};

pub fn refresh(settings: &Settings, subscription: &Subscription) -> Result<Vec<Node>> {
    let url = url::Url::parse(&subscription.link).context("订阅地址无效")?;
    if !matches!(url.scheme(), "https" | "http") {
        bail!("订阅只允许 HTTP/HTTPS");
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(
            settings.request_timeout.clamp(1000, 60000),
        ))
        .build()?;
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, &subscription.user_agent)
        .send()
        .map_err(reqwest::Error::without_url)
        .context("下载订阅失败")?
        .error_for_status()
        .map_err(reqwest::Error::without_url)
        .context("订阅 HTTP 状态异常")?;
    let mut bytes = vec![];
    response.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 4 * 1024 * 1024 {
        bail!("订阅超过 4 MiB 限制");
    }
    parse_nodes(
        subscription,
        &String::from_utf8(bytes).context("订阅编码不是 UTF-8")?,
    )
}

/// Parses a subscription body into the nodes it owns.
///
/// The nodes are returned separately from the rest of the settings so the
/// caller can merge them into the *live* configuration. Replacing a whole
/// settings snapshot would silently discard edits the user made while the
/// download was in flight.
pub fn parse_nodes(subscription: &Subscription, text: &str) -> Result<Vec<Node>> {
    let mut parsed = links::parse(text)?;
    // A partially broken subscription must not delete working nodes either.
    if parsed.rejected > 0 {
        bail!("订阅含 {} 个无效条目，已保留原节点", parsed.rejected);
    }
    for node in &mut parsed.nodes {
        node.group = subscription.remark.clone();
        node.set("SubscriptionId", subscription.id.clone());
    }
    Ok(parsed.nodes)
}

pub fn replace(settings: &Settings, subscription: &Subscription, text: &str) -> Result<Settings> {
    let nodes = parse_nodes(subscription, text)?;
    let selected = settings
        .nodes
        .get(settings.selected_node)
        .map(|n| n.identity());
    let mut next = settings.clone();
    next.nodes
        .retain(|n| n.text("SubscriptionId") != subscription.id);
    next.nodes.extend(nodes);
    next.selected_node = selected
        .and_then(|identity| next.nodes.iter().position(|n| n.identity() == identity))
        .unwrap_or(0);
    Ok(next)
}

/// Merges freshly downloaded nodes into a live node list.
///
/// Only the nodes owned by `subscription` are replaced; nodes from other
/// subscriptions and manually added nodes are preserved.
pub fn merge_nodes(current: &[Node], subscription: &Subscription, text: &str) -> Result<Vec<Node>> {
    let nodes = parse_nodes(subscription, text)?;
    let mut merged: Vec<Node> = current
        .iter()
        .filter(|n| n.text("SubscriptionId") != subscription.id)
        .cloned()
        .collect();
    merged.extend(nodes);
    Ok(merged)
}
