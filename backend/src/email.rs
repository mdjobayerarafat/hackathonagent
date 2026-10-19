use crate::config::Config;
use crate::models::Hackathon;
use anyhow::{bail, Context, Result};
use chrono::{NaiveDate, Utc};
use chrono_tz::Asia::Shanghai;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;

const RESEND_ENDPOINT: &str = "https://api.resend.com/emails";
const MAX_DIGEST_ITEMS: i64 = 20;

#[derive(Debug, Clone, Serialize)]
pub struct DigestResult {
    pub count: usize,
    pub resend_id: Option<String>,
    pub hackathon_ids: Vec<Uuid>,
}

pub async fn send_digest(pool: &sqlx::PgPool, config: &Config) -> Result<Option<DigestResult>> {
    if !config.email_configured() {
        bail!("email is not configured (set RESEND_API_KEY and EMAIL_TO)");
    }

    let today = Utc::now().with_timezone(&Shanghai).date_naive();
    let items = sqlx::query_as::<_, Hackathon>(
        "SELECT * FROM hackathons
         WHERE status = 'new'
           AND (registration_deadline IS NULL OR registration_deadline >= $1)
         ORDER BY registration_deadline ASC NULLS LAST, created_at DESC
         LIMIT $2",
    )
    .bind(today)
    .bind(MAX_DIGEST_ITEMS)
    .fetch_all(pool)
    .await
    .context("loading new hackathons for digest")?;

    if items.is_empty() {
        log_email(pool, &[], None, "skipped", None, config, Some("No new hackathons"))
            .await?;
        return Ok(None);
    }

    let count = items.len();
    let subject = format!(
        "Hackathon Hunter · {count} new hackathon{} / {count} 个新黑客松",
        if count == 1 { "" } else { "s" }
    );
    let html = render_digest_html(&items, &config.dashboard_url);
    let text = render_digest_text(&items, &config.dashboard_url);
    let recipients = parse_recipients(&config.email_to);

    let resend_id = match send_resend(config, &recipients, &subject, &html, &text).await {
        Ok(id) => id,
        Err(error) => {
            let message = error.to_string();
            log_email(
                pool,
                &items.iter().map(|item| item.id).collect::<Vec<_>>(),
                None,
                "failed",
                Some(&message),
                config,
                Some(&subject),
            )
            .await?;
            return Err(error);
        }
    };

    let ids = items.iter().map(|item| item.id).collect::<Vec<_>>();
    log_email(
        pool,
        &ids,
        resend_id.as_deref(),
        "sent",
        None,
        config,
        Some(&subject),
    )
    .await?;

    sqlx::query("UPDATE hackathons SET status = 'notified' WHERE id = ANY($1) AND status = 'new'")
        .bind(&ids)
        .execute(pool)
        .await
        .context("marking hackathons notified")?;

    tracing::info!(count, resend_id = ?resend_id, "digest email delivered");

    Ok(Some(DigestResult {
        count,
        resend_id,
        hackathon_ids: ids,
    }))
}

fn parse_recipients(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

async fn send_resend(
    config: &Config,
    recipients: &[String],
    subject: &str,
    html: &str,
    text: &str,
) -> Result<Option<String>> {
    if config.resend_api_key.trim().is_empty() {
        bail!("RESEND_API_KEY is not configured");
    }
    if recipients.is_empty() {
        bail!("EMAIL_TO is not configured");
    }

    let payload = json!({
        "from": config.email_from,
        "to": recipients,
        "subject": subject,
        "html": html,
        "text": text,
    });

    let http = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("building Resend HTTP client")?;

    let response = http
        .post(RESEND_ENDPOINT)
        .bearer_auth(&config.resend_api_key)
        .json(&payload)
        .send()
        .await
        .context("sending email through Resend")?;

    let status = response.status();
    let body = response
        .text()
        .await
        .context("reading Resend response")?;

    if !status.is_success() {
        bail!("Resend returned {status}: {}", truncate(&body, 600));
    }

    #[derive(Deserialize)]
    struct ResendResponse {
        id: Option<String>,
    }
    Ok(serde_json::from_str::<ResendResponse>(&body)
        .ok()
        .and_then(|response| response.id))
}

#[derive(Debug, Clone, Serialize)]
pub struct TestEmailResult {
    pub resend_id: Option<String>,
    pub recipients: Vec<String>,
}

/// Sends a small diagnostic email so the Resend configuration can be verified
/// from the settings page without waiting for the next agent run.
pub async fn send_test_email(
    pool: &sqlx::PgPool,
    config: &Config,
    to_override: Option<&str>,
) -> Result<TestEmailResult> {
    if config.resend_api_key.trim().is_empty() {
        bail!("RESEND_API_KEY is not configured");
    }

    let recipients = to_override
        .map(parse_recipients)
        .filter(|list| !list.is_empty())
        .unwrap_or_else(|| parse_recipients(&config.email_to));
    if recipients.is_empty() {
        bail!("EMAIL_TO is not configured");
    }

    let subject = "Hackathon Hunter · Resend test / 邮件测试";
    let html = render_test_html(config, &recipients);
    let text = render_test_text(config, &recipients);

    match send_resend(config, &recipients, subject, &html, &text).await {
        Ok(resend_id) => {
            log_email(
                pool,
                &[],
                resend_id.as_deref(),
                "sent",
                None,
                config,
                Some(subject),
            )
            .await?;
            tracing::info!(?resend_id, recipients = ?recipients, "test email delivered");
            Ok(TestEmailResult {
                resend_id,
                recipients,
            })
        }
        Err(error) => {
            log_email(
                pool,
                &[],
                None,
                "failed",
                Some(&error.to_string()),
                config,
                Some(subject),
            )
            .await?;
            Err(error)
        }
    }
}

fn render_test_html(config: &Config, recipients: &[String]) -> String {
    let to = escape_html(&recipients.join(", "));
    let dashboard = escape_html(&config.dashboard_url);
    let schedule = escape_html(&format!(
        "{} · {}",
        config.cron_schedule, config.cron_timezone
    ));
    let sender = escape_html(&config.email_from);

    format!(
        r##"<!DOCTYPE html>
<html lang="en" xmlns="http://www.w3.org/1999/xhtml">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Resend test</title></head>
<body style="margin:0;padding:0;background-color:#eef2f7;">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" style="background-color:#eef2f7;"><tr><td align="center" style="padding:28px 12px;">
<table role="presentation" width="600" cellpadding="0" cellspacing="0" border="0" style="width:100%;max-width:600px;background-color:#ffffff;border-radius:18px;overflow:hidden;border:1px solid #e2e8f0;">
<tr><td bgcolor="#111827" style="background-color:#111827;background-image:linear-gradient(135deg,#111827,#312e81 60%,#4f46e5);padding:32px;">
<div style="font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:11px;letter-spacing:2.5px;text-transform:uppercase;color:#c7d2fe;font-weight:700;">Hackathon Hunter</div>
<div style="font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:26px;line-height:33px;color:#ffffff;font-weight:800;margin-top:10px;">Your email pipeline is live.</div>
<div style="font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:14px;line-height:22px;color:#c7d2fe;margin-top:8px;">邮件发送配置测试成功 · Resend test email</div>
</td></tr>
<tr><td style="padding:30px 32px;">
<div style="font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:15px;line-height:24px;color:#334155;">This is a test from your Hackathon Hunter settings page. If you can read this, Resend is configured correctly and the daily digest will be delivered to <strong>{to}</strong>.</div>
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" style="margin-top:22px;background-color:#f8fafc;border:1px solid #e2e8f0;border-radius:12px;">
<tr><td style="padding:16px 18px;font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:13px;line-height:22px;color:#475569;"><strong style="color:#0f172a;">Sender:</strong> {sender}<br><strong style="color:#0f172a;">Schedule:</strong> {schedule}<br><strong style="color:#0f172a;">Digest recipients:</strong> {to}</td></tr>
</table>
<table role="presentation" cellpadding="0" cellspacing="0" border="0" style="margin-top:24px;"><tr><td bgcolor="#4f46e5" style="border-radius:9px;"><a href="{dashboard}" target="_blank" rel="noopener" style="display:inline-block;padding:12px 24px;font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:14px;line-height:20px;font-weight:700;color:#ffffff;text-decoration:none;">Open dashboard →</a></td></tr></table>
</td></tr>
<tr><td bgcolor="#f8fafc" style="background-color:#f8fafc;border-top:1px solid #e2e8f0;padding:20px 32px;font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:12px;line-height:19px;color:#94a3b8;text-align:center;">Sent by Hackathon Hunter · Asia/Shanghai</td></tr>
</table>
</td></tr></table>
</body></html>"##
    )
}

fn render_test_text(config: &Config, recipients: &[String]) -> String {
    format!(
        "HACKATHON HUNTER\nYour email pipeline is live.\n\nResend is configured correctly.\nRecipients: {}\nSender: {}\nSchedule: {} ({})\nDashboard: {}\n",
        recipients.join(", "),
        config.email_from,
        config.cron_schedule,
        config.cron_timezone,
        config.dashboard_url
    )
}

#[allow(clippy::too_many_arguments)]
async fn log_email(
    pool: &sqlx::PgPool,
    ids: &[Uuid],
    resend_id: Option<&str>,
    status: &str,
    error: Option<&str>,
    config: &Config,
    subject: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO email_log (hackathon_ids, resend_id, status, error, recipient, subject)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(ids)
    .bind(resend_id)
    .bind(status)
    .bind(error)
    .bind(&config.email_to)
    .bind(subject)
    .execute(pool)
    .await
    .context("writing email log")?;
    Ok(())
}

fn truncate(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn format_cny(amount: f64) -> String {
    let rounded = amount.round() as i64;
    let digits = rounded.abs().to_string();
    let mut grouped = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let grouped: String = grouped.chars().rev().collect();
    format!("¥{grouped}")
}

fn format_dates(item: &Hackathon) -> String {
    let format = |date: NaiveDate| date.format("%b %-d, %Y").to_string();
    match (item.start_date, item.end_date) {
        (Some(start), Some(end)) if start == end => format(start),
        (Some(start), Some(end)) => format!("{} – {}", format(start), format(end)),
        (Some(start), None) => format(start),
        (None, Some(end)) => format!("Until {}", format(end)),
        (None, None) => "Dates TBA".to_owned(),
    }
}

fn format_location(item: &Hackathon) -> String {
    if item.is_online == Some(true) {
        return "Online · 线上".to_owned();
    }
    match &item.location {
        Some(location) if !location.trim().is_empty() => location.clone(),
        _ => "Location TBA".to_owned(),
    }
}

fn format_prize(item: &Hackathon) -> Option<String> {
    item.prize_summary
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| item.prize_amount_cny.map(format_cny))
        .or_else(|| item.prize_amount_cny.map(|amount| format!("{} CNY", format_cny(amount))))
}

#[derive(Debug)]
struct DeadlineBadge {
    label: String,
    background: &'static str,
}

fn deadline_badge(deadline: Option<NaiveDate>) -> DeadlineBadge {
    let today = Utc::now().with_timezone(&Shanghai).date_naive();
    let Some(deadline) = deadline else {
        return DeadlineBadge {
            label: "Rolling deadline".to_owned(),
            background: "#475569",
        };
    };
    let days = (deadline - today).num_days();
    let label = match days {
        d if d <= 0 => "Closes today".to_owned(),
        1 => "1 day left".to_owned(),
        d if d <= 7 => format!("{d} days left"),
        d => format!("{d} days left · {}", deadline.format("%b %-d")),
    };
    let background = if days <= 3 {
        "#dc2626"
    } else if days <= 7 {
        "#d97706"
    } else {
        "#047857"
    };
    DeadlineBadge { label, background }
}

pub fn render_digest_html(items: &[Hackathon], dashboard_url: &str) -> String {
    let count = items.len();
    let summary = format!(
        "{count} new hackathon{} found",
        if count == 1 { "" } else { "s" }
    );
    let preheader = format!(
        "{summary} in China · 发现 {count} 个新黑客松，报名截止日期与奖金一览"
    );

    let mut html = String::with_capacity(32_000);
    html.push_str("<!DOCTYPE html>\n<html lang=\"en\" xmlns=\"http://www.w3.org/1999/xhtml\">\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("<meta name=\"x-apple-disable-message-reformatting\">\n");
    html.push_str("<meta name=\"color-scheme\" content=\"light\">\n");
    html.push_str("<title>Hackathon Hunter Digest</title>\n");
    html.push_str(
        "<style>\n\
         @media only screen and (max-width: 620px) {\n\
           .container { width: 100% !important; }\n\
           .px { padding-left: 20px !important; padding-right: 20px !important; }\n\
           .stack { display: block !important; width: 100% !important; }\n\
           .btn a { display: block !important; width: 100% !important; box-sizing: border-box; }\n\
           .headline { font-size: 24px !important; line-height: 31px !important; }\n\
         }\n\
         a { color: #4f46e5; }\n\
         </style>\n",
    );
    html.push_str("</head>\n");
    html.push_str("<body style=\"margin:0;padding:0;background-color:#eef2f7;-webkit-text-size-adjust:100%;-ms-text-size-adjust:100%;\">\n");
    html.push_str(&format!(
        "<div style=\"display:none;font-size:1px;line-height:1px;max-height:0;max-width:0;opacity:0;overflow:hidden;mso-hide:all;\">{}</div>\n",
        escape_html(&preheader)
    ));

    html.push_str("<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"background-color:#eef2f7;\">\n<tr><td align=\"center\" style=\"padding:28px 12px;\">\n");
    html.push_str("<table role=\"presentation\" class=\"container\" width=\"600\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"width:600px;max-width:600px;background-color:#ffffff;border-radius:18px;overflow:hidden;border:1px solid #e2e8f0;\">\n");

    // Header
    html.push_str("<tr><td bgcolor=\"#111827\" style=\"background-color:#111827;background-image:linear-gradient(135deg,#111827 0%,#312e81 55%,#4f46e5 100%);padding:34px 32px 30px;\">\n");
    html.push_str("<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\">\n<tr><td>\n");
    html.push_str("<div style=\"font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:11px;letter-spacing:2.5px;text-transform:uppercase;color:#c7d2fe;font-weight:700;\">Hackathon Hunter</div>\n");
    html.push_str("<div style=\"font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:27px;line-height:34px;color:#ffffff;font-weight:800;margin-top:10px;\">Your weekly edge in China&rsquo;s hackathon scene.</div>\n");
    html.push_str("<div style=\"font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:14px;line-height:22px;color:#c7d2fe;margin-top:10px;\">中国黑客松 · AI 精选 · 每天 08:00 Asia/Shanghai</div>\n");
    html.push_str("</td></tr></table>\n</td></tr>\n");

    // Summary
    html.push_str("<tr><td class=\"px\" style=\"padding:28px 32px 10px;\">\n");
    html.push_str(&format!(
        "<div style=\"font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:13px;font-weight:700;letter-spacing:1.6px;text-transform:uppercase;color:#4f46e5;\">Fresh finds</div>\n<div class=\"headline\" style=\"font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:30px;line-height:38px;color:#0f172a;font-weight:800;margin-top:6px;\">{}</div>\n<div style=\"font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:15px;line-height:24px;color:#64748b;margin-top:8px;\">发现 {count} 个新黑客松 · Deadlines are in China Standard Time</div>\n",
        escape_html(&summary)
    ));
    html.push_str("</td></tr>\n");

    for item in items {
        let title = if item.title.is_empty() {
            "Untitled hackathon"
        } else {
            &item.title
        };
        let title_zh = item
            .title_zh
            .as_deref()
            .filter(|value| !value.trim().is_empty() && *value != title);
        let register_url = item
            .registration_url
            .as_deref()
            .unwrap_or(&item.source_url);
        let badge = deadline_badge(item.registration_deadline);
        let prize = format_prize(item);
        let dates = format_dates(item);
        let location = format_location(item);
        let description = truncate(item.description.trim(), 260);
        let organizer = item
            .organizer
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("Organizer not listed");

        html.push_str("<tr><td class=\"px\" style=\"padding:14px 32px 0;\">\n");
        html.push_str("<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"border:1px solid #e2e8f0;border-radius:14px;background-color:#ffffff;\">\n");
        html.push_str("<tr><td style=\"padding:22px 22px 20px;\">\n");

        html.push_str(&format!(
            "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\"><tr>\
             <td class=\"stack\" valign=\"top\" style=\"padding-bottom:6px;\">\
             <span style=\"display:inline-block;background-color:{};color:#ffffff;font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:11px;line-height:16px;font-weight:700;letter-spacing:.6px;text-transform:uppercase;padding:5px 10px;border-radius:999px;\">{}</span>\
             </td></tr></table>\n",
            badge.background,
            escape_html(&badge.label)
        ));

        html.push_str(&format!(
            "<div style=\"font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:21px;line-height:28px;font-weight:800;color:#0f172a;margin-top:14px;\">{}</div>\n",
            escape_html(title)
        ));
        if let Some(title_zh) = title_zh {
            html.push_str(&format!(
                "<div style=\"font-family:'PingFang SC','Microsoft YaHei','Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:16px;line-height:24px;color:#475569;margin-top:4px;\">{}</div>\n",
                escape_html(title_zh)
            ));
        }
        html.push_str(&format!(
            "<div style=\"font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:13px;line-height:20px;color:#64748b;margin-top:6px;\">{}</div>\n",
            escape_html(organizer)
        ));
        html.push_str(&format!(
            "<div style=\"font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:14px;line-height:23px;color:#334155;margin-top:12px;\">{}</div>\n",
            escape_html(&description)
        ));

        html.push_str("<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"margin-top:16px;\">\n");
        html.push_str(&format!(
            "<tr><td style=\"padding:5px 0;font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:13px;line-height:20px;color:#64748b;width:92px;\">Dates</td><td style=\"padding:5px 0;font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:13px;line-height:20px;color:#1e293b;font-weight:600;\">{}</td></tr>\n",
            escape_html(&dates)
        ));
        html.push_str(&format!(
            "<tr><td style=\"padding:5px 0;font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:13px;line-height:20px;color:#64748b;\">Location</td><td style=\"padding:5px 0;font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:13px;line-height:20px;color:#1e293b;font-weight:600;\">{}</td></tr>\n",
            escape_html(&location)
        ));
        if let Some(prize) = &prize {
            html.push_str(&format!(
                "<tr><td style=\"padding:5px 0;font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:13px;line-height:20px;color:#64748b;\">Prize</td><td style=\"padding:5px 0;\"><span style=\"display:inline-block;background-color:#fef3c7;color:#92400e;font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:13px;line-height:20px;font-weight:700;padding:4px 9px;border-radius:6px;\">{}</span></td></tr>\n",
                escape_html(prize)
            ));
        }
        if let Some(deadline) = item.registration_deadline {
            html.push_str(&format!(
                "<tr><td style=\"padding:5px 0;font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:13px;line-height:20px;color:#64748b;\">Apply by</td><td style=\"padding:5px 0;font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:13px;line-height:20px;color:#1e293b;font-weight:600;\">{}</td></tr>\n",
                escape_html(&deadline.format("%B %-d, %Y").to_string())
            ));
        }
        html.push_str("</table>\n");

        html.push_str("<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"margin-top:20px;\"><tr>\n");
        html.push_str(&format!(
            "<td class=\"btn\" style=\"padding-right:10px;\"><table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\"><tr><td bgcolor=\"#4f46e5\" style=\"border-radius:9px;background-color:#4f46e5;\"><a href=\"{}\" target=\"_blank\" rel=\"noopener\" style=\"display:inline-block;padding:12px 22px;font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:14px;line-height:20px;font-weight:700;color:#ffffff;text-decoration:none;border-radius:9px;\">Register now &rarr;</a></td></tr></table></td>\n",
            escape_html(register_url)
        ));
        html.push_str(&format!(
            "<td style=\"font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:12px;line-height:18px;\"><a href=\"{}\" target=\"_blank\" rel=\"noopener\" style=\"color:#64748b;text-decoration:underline;\">Source &amp; details</a></td>\n",
            escape_html(&item.source_url)
        ));
        html.push_str("</tr></table>\n");

        html.push_str("</td></tr></table>\n</td></tr>\n");
    }

    // Dashboard CTA
    html.push_str("<tr><td class=\"px\" style=\"padding:30px 32px 36px;\">\n");
    html.push_str(&format!(
        "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"background-color:#f8fafc;border:1px solid #e2e8f0;border-radius:14px;\"><tr><td align=\"center\" style=\"padding:26px 20px;\">\
         <div style=\"font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:17px;line-height:24px;color:#0f172a;font-weight:700;\">Skip the noise. See everything in one place.</div>\
         <div style=\"font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:13px;line-height:20px;color:#64748b;margin-top:6px;\">Filter by deadline, track, prize and location. 在新面板中筛选所有黑客松。</div>\
         <table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"margin-top:18px;\"><tr><td bgcolor=\"#0f172a\" style=\"border-radius:9px;\"><a href=\"{}\" target=\"_blank\" rel=\"noopener\" style=\"display:inline-block;padding:12px 24px;font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:14px;line-height:20px;font-weight:700;color:#ffffff;text-decoration:none;\">View all in dashboard &rarr;</a></td></tr></table>\
         </td></tr></table>\n",
        escape_html(dashboard_url)
    ));
    html.push_str("</td></tr>\n");

    html.push_str("<tr><td bgcolor=\"#f8fafc\" style=\"background-color:#f8fafc;border-top:1px solid #e2e8f0;padding:22px 32px;\">\n");
    html.push_str("<div style=\"font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;font-size:12px;line-height:19px;color:#94a3b8;text-align:center;\">You receive this because you run Hackathon Hunter.<br>Found by SearXNG, extracted by 9router, delivered by Resend.<br><span style=\"color:#cbd5e1;\">Hackathon Hunter · Asia/Shanghai · Built for builders.</span></div>\n");
    html.push_str("</td></tr>\n");

    html.push_str("</table>\n</td></tr></table>\n</body>\n</html>");
    html
}

pub fn render_digest_text(items: &[Hackathon], dashboard_url: &str) -> String {
    let count = items.len();
    let mut text = String::new();
    text.push_str("HACKATHON HUNTER\n");
    text.push_str("China hackathons, curated by AI.\n\n");
    text.push_str(&format!(
        "{count} new hackathon{} found\n",
        if count == 1 { "" } else { "s" }
    ));
    text.push_str("========================================\n\n");

    for (index, item) in items.iter().enumerate() {
        let title = if item.title.is_empty() {
            "Untitled hackathon"
        } else {
            &item.title
        };
        text.push_str(&format!("{}. {}\n", index + 1, title));
        if let Some(title_zh) = item.title_zh.as_deref().filter(|value| !value.is_empty()) {
            text.push_str(&format!("   {title_zh}\n"));
        }
        if let Some(organizer) = item.organizer.as_deref().filter(|value| !value.is_empty()) {
            text.push_str(&format!("   Organizer: {organizer}\n"));
        }
        text.push_str(&format!("   Dates: {}\n", format_dates(item)));
        text.push_str(&format!("   Location: {}\n", format_location(item)));
        if let Some(prize) = format_prize(item) {
            text.push_str(&format!("   Prize: {prize}\n"));
        }
        if let Some(deadline) = item.registration_deadline {
            text.push_str(&format!(
                "   Apply by: {} ({})\n",
                deadline.format("%Y-%m-%d"),
                deadline_badge(Some(deadline)).label
            ));
        }
        text.push_str(&format!(
            "   Description: {}\n",
            truncate(item.description.trim(), 260)
        ));
        if let Some(url) = item.registration_url.as_deref() {
            text.push_str(&format!("   Register: {url}\n"));
        }
        text.push_str(&format!("   Source: {}\n\n", item.source_url));
    }

    text.push_str(&format!("View all in dashboard: {dashboard_url}\n\n"));
    text.push_str("You receive this because you run Hackathon Hunter.\n");
    text.push_str("Found by SearXNG, extracted by 9router, delivered by Resend.\n");
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: u128, deadline: Option<NaiveDate>) -> Hackathon {
        let now = Utc::now();
        Hackathon {
            id: Uuid::from_u128(id),
            title: "AI <Hack> 2026".to_owned(),
            title_zh: Some("AI 黑客松 2026".to_owned()),
            organizer: Some("Example Labs".to_owned()),
            description: "Build something useful & bold in 48 hours.".to_owned(),
            location: Some("Shanghai".to_owned()),
            is_online: Some(false),
            start_date: Some(NaiveDate::from_ymd_opt(2026, 6, 12).unwrap()),
            end_date: Some(NaiveDate::from_ymd_opt(2026, 6, 14).unwrap()),
            registration_deadline: deadline,
            registration_url: Some("https://example.com/register?x=1&y=2".to_owned()),
            registration_url_normalized: Some("https://example.com/register".to_owned()),
            source_url: "https://example.com/event".to_owned(),
            source_url_normalized: "https://example.com/event".to_owned(),
            prize_summary: Some("50,000 CNY prize pool".to_owned()),
            prize_amount_cny: Some(50000.0),
            tracks_or_themes: vec!["AI".to_owned()],
            eligibility: None,
            tags: vec!["ai".to_owned()],
            confidence: 0.9,
            status: "new".to_owned(),
            dedupe_key: format!("reg:https://example.com/{id}"),
            content_hash: "hash".to_owned(),
            first_seen_at: now,
            last_seen_at: now,
            content_changed_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn html_is_responsive_table_based_and_escaped() {
        let items = vec![sample(1, None)];
        let html = render_digest_html(&items, "https://dashboard.example");
        assert!(html.contains("role=\"presentation\""));
        assert!(html.contains("width=\"600\""));
        assert!(html.contains("Register now"));
        assert!(html.contains("View all in dashboard"));
        assert!(html.contains("AI &lt;Hack&gt; 2026"));
        assert!(!html.contains("AI <Hack> 2026"));
        assert!(html.contains("https://example.com/register?x=1&amp;y=2"));
        assert!(html.contains("@media only screen"));
        assert!(html.contains("AI 黑客松 2026"));
        assert!(html.contains("1 new hackathon found"));
    }

    #[test]
    fn text_fallback_contains_actionable_details() {
        let items = vec![sample(1, Some(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()))];
        let text = render_digest_text(&items, "https://dashboard.example");
        assert!(text.contains("AI <Hack> 2026"));
        assert!(text.contains("Register: https://example.com/register?x=1&y=2"));
        assert!(text.contains("View all in dashboard: https://dashboard.example"));
        assert!(text.contains("AI 黑客松 2026"));
    }

    #[test]
    fn deadline_badges_reflect_urgency() {
        let today = Utc::now().with_timezone(&Shanghai).date_naive();
        assert_eq!(deadline_badge(Some(today)).background, "#dc2626");
        assert_eq!(deadline_badge(None).background, "#475569");
        assert_eq!(
            deadline_badge(Some(today + chrono::Duration::days(30))).background,
            "#047857"
        );
    }

    #[test]
    fn formats_prize_with_thousands_separator() {
        assert_eq!(format_cny(1234567.0), "¥1,234,567");
        assert_eq!(format_cny(50000.0), "¥50,000");
    }
}
