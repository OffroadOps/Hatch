use crate::{
    links,
    model::{Node, Settings, Subscription},
};
use anyhow::{Context, Result, bail};
use std::{io::Read, time::Duration};

pub fn refresh(settings: &Settings, subscription: &Subscription) -> Result<ParsedNodes> {
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

/// Nodes read from a subscription plus the number of entries that were
/// skipped, so the caller can warn about partial updates.
pub struct ParsedNodes {
    pub nodes: Vec<Node>,
    pub skipped: usize,
}

/// Parses a subscription body into the nodes it owns.
///
/// The nodes are returned separately from the rest of the settings so the
/// caller can merge them into the *live* configuration. Replacing a whole
/// settings snapshot would silently discard edits the user made while the
/// download was in flight.
///
/// A malformed entry is skipped instead of failing the whole refresh: one bad
/// line must not keep otherwise valid nodes from updating. `links::parse`
/// still errors when *nothing* could be read, which is what keeps a broken or
/// truncated response from replacing the existing nodes.
pub fn parse_nodes(subscription: &Subscription, text: &str) -> Result<ParsedNodes> {
    let mut parsed = links::parse(text)?;
    for node in &mut parsed.nodes {
        node.group = subscription.remark.clone();
        node.set("SubscriptionId", subscription.id.clone());
    }
    Ok(ParsedNodes {
        nodes: parsed.nodes,
        skipped: parsed.rejected,
    })
}

pub fn replace(settings: &Settings, subscription: &Subscription, text: &str) -> Result<Settings> {
    let nodes = parse_nodes(subscription, text)?.nodes;
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
    let nodes = parse_nodes(subscription, text)?.nodes;
    let mut merged: Vec<Node> = current
        .iter()
        .filter(|n| n.text("SubscriptionId") != subscription.id)
        .cloned()
        .collect();
    merged.extend(nodes);
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subscription() -> Subscription {
        Subscription {
            remark: "测试订阅".into(),
            ..Subscription::default()
        }
    }

    #[test]
    fn partial_subscription_keeps_valid_nodes_and_reports_skips() {
        let subscription = subscription();
        let text =
            "trojan://p%2Ba%3Ass@example.com:443#ok\nnot-a-link\ntrojan://missing-host?x=1\n";
        let parsed = parse_nodes(&subscription, text).unwrap();
        assert_eq!(parsed.nodes.len(), 1);
        assert_eq!(parsed.skipped, 2);
        assert_eq!(parsed.nodes[0].remark, "ok");
        assert_eq!(parsed.nodes[0].text("SubscriptionId"), subscription.id);
    }

    #[test]
    fn unreadable_subscription_is_still_rejected() {
        let subscription = subscription();
        // Nothing valid means a broken or truncated download: the caller must
        // keep the existing nodes instead of replacing them with an empty list.
        assert!(parse_nodes(&subscription, "not-a-link").is_err());
        assert!(parse_nodes(&subscription, "").is_err());
        assert!(parse_nodes(&subscription, "<html>upstream error</html>").is_err());
    }
}
