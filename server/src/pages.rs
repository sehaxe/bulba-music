use crate::index;
use anyhow::Result;
use rusqlite::Connection;

/// Server-rendered pages for sharing: the phone opens the same URL over
/// Tailscale and sees what the desktop sees, without an app store in between.
/// Everything is escaped here — these pages render user-controlled tags.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn shell(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"ru\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>{t}</title>\
<style>\
:root{{--paper:#141210;--ink:#f2ece4;--mute:#9a9086;--line:#2a2622;--signal:#f26b3a}}\
*{{box-sizing:border-box}}\
body{{margin:0;background:var(--paper);color:var(--ink);\
font-family:ui-monospace,SFMono-Regular,Menlo,monospace;line-height:1.55}}\
main{{max-width:760px;margin:0 auto;padding:28px 20px 64px}}\
a{{color:var(--signal);text-decoration:none}}\
h1{{font-size:26px;margin:14px 0 4px;letter-spacing:-0.02em}}\
h2{{font-size:14px;color:var(--mute);font-weight:500;margin:28px 0 10px;\
text-transform:lowercase;letter-spacing:.04em}}\
.meta{{color:var(--mute);font-size:13px}}\
.cover{{width:160px;height:160px;border-radius:14px;object-fit:cover;\
background:#221e1a;display:block}}\
.row{{display:flex;gap:18px;align-items:flex-end;flex-wrap:wrap}}\
li{{list-style:none;padding:7px 0;border-bottom:1px solid var(--line);\
display:flex;justify-content:space-between;gap:12px;font-size:14px}}\
ul{{padding:0;margin:0}}\
.n{{color:var(--mute);min-width:26px;display:inline-block}}\
.tag{{font-size:11px;color:var(--mute)}}\
footer{{margin-top:36px;color:var(--mute);font-size:12px}}\
</style></head><body><main>{body}</main></body></html>",
        t = esc(title),
        body = body
    )
}

fn cover_tag(album_id: Option<i64>, has_cover: i64) -> String {
    match album_id {
        Some(id) if has_cover == 1 => format!(
            "<img class=\"cover\" src=\"/api/albums/{}/cover\" alt=\"\">",
            id
        ),
        _ => "<div class=\"cover\"></div>".to_string(),
    }
}

pub fn artist_page(conn: &Connection, id: i64, server: &str) -> Result<Option<String>> {
    let Some(page) = index::artist(conn, id)? else {
        return Ok(None);
    };
    let a = &page.artist;
    let years = match (page.first_year, page.last_year) {
        (Some(f), Some(l)) if f != l => format!("{f}–{l}"),
        (Some(f), _) => format!("{f}"),
        _ => "—".to_string(),
    };
    let mut body = format!(
        "<div class=\"row\">{}<div><span class=\"tag\">артист</span><h1>{}</h1>\
<div class=\"meta\">{} альбомов · {} треков · {} · {} включений</div></div></div>",
        cover_tag(a.cover_album, a.has_cover),
        esc(&a.name),
        a.albums,
        a.tracks,
        years,
        a.plays
    );

    if !page.top_tracks.is_empty() {
        body.push_str("<h2>часто слушаешь</h2><ul>");
        for t in &page.top_tracks {
            body.push_str(&format!(
                "<li><span>{}</span><span class=\"meta\">{}×</span></li>",
                esc(&t.title),
                t.plays
            ));
        }
        body.push_str("</ul>");
    }

    body.push_str("<h2>альбомы</h2><ul>");
    for al in &page.albums {
        body.push_str(&format!(
            "<li><span><a href=\"/al/{}\">{}</a></span><span class=\"meta\">{}</span></li>",
            al.id,
            esc(&al.title),
            al.year.map(|y| y.to_string()).unwrap_or_else(|| "—".into())
        ));
    }
    body.push_str("</ul>");
    body.push_str(&format!(
        "<footer>bulba music · <a href=\"http://{server}\">{}</a></footer>",
        esc(server)
    ));
    Ok(Some(shell(&a.name, &body)))
}

pub fn album_page(conn: &Connection, id: i64, server: &str) -> Result<Option<String>> {
    let Some(page) = index::album(conn, id)? else {
        return Ok(None);
    };
    let al = &page.album;
    let mins = page.total_ms / 60_000;
    let mut body = format!(
        "<div class=\"row\">{}<div><span class=\"tag\">альбом</span>\
<h1>{}</h1><div class=\"meta\"><a href=\"/a/{}\">{}</a> · {} · {} треков · {} мин</div>\
</div></div>",
        cover_tag(Some(al.id), al.has_cover),
        esc(&al.title),
        al.artist_id,
        esc(&al.artist),
        al.year.map(|y| y.to_string()).unwrap_or_else(|| "—".into()),
        page.tracks.len(),
        mins
    );
    body.push_str("<h2>треки</h2><ul>");
    for t in &page.tracks {
        let no = t
            .track_no
            .map(|n| format!("{n:02}"))
            .unwrap_or_else(|| "—".into());
        let dur = t
            .duration_ms
            .map(|ms| format!("{}:{:02}", ms / 60_000, (ms / 1000) % 60))
            .unwrap_or_else(|| "—".into());
        body.push_str(&format!(
            "<li><span><span class=\"n\">{}</span>{}</span><span class=\"meta\">{}</span></li>",
            no,
            esc(&t.title),
            dur
        ));
    }
    body.push_str("</ul>");
    body.push_str(&format!(
        "<footer>bulba music · <a href=\"http://{server}\">{}</a></footer>",
        esc(server)
    ));
    Ok(Some(shell(&format!("{} — {}", al.title, al.artist), &body)))
}
