//! Aide sur la tablette : le manuel de `docs/` (le même que la fenêtre d'aide du PC), mis en HTML
//! pour l'onglet Aide de la page. Markdown simple : titres, paragraphes, listes, tableaux, blocs
//! de code, **gras** et `code` (les liens sont rendus comme du texte).

const AIDE_FR: &str = include_str!("../docs/aide.md");
const HELP_EN: &str = include_str!("../docs/help.md");

/// Fragment HTML de l'aide (français, ou anglais si `en`), version en tête.
pub fn html(en: bool) -> String {
    let src = if en { HELP_EN } else { AIDE_FR };
    let mut out = format!("<div class=\"help-ver\">OldStar v{}</div>\n", crate::VERSION);
    out.push_str(&to_html(src));
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Texte d'une ligne : **gras**, `code`, [lien](cible) rendu en texte.
fn inline(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("**") {
            if let Some(end) = r.find("**") {
                out.push_str(&format!("<b>{}</b>", inline(&r[..end])));
                rest = &r[end + 2..];
                continue;
            }
        }
        if let Some(r) = rest.strip_prefix('`') {
            if let Some(end) = r.find('`') {
                out.push_str(&format!("<code>{}</code>", esc(&r[..end])));
                rest = &r[end + 1..];
                continue;
            }
        }
        if let Some(r) = rest.strip_prefix('[') {
            if let Some(close) = r.find("](") {
                if let Some(end) = r[close..].find(')') {
                    out.push_str(&inline(&r[..close]));
                    rest = &r[close + end + 1..];
                    continue;
                }
            }
        }
        let c = rest.chars().next().unwrap_or(' ');
        out.push_str(&esc(&c.to_string()));
        rest = &rest[c.len_utf8()..];
    }
    out
}

fn cells(line: &str) -> Vec<&str> {
    line.trim().trim_matches('|').split('|').map(str::trim).collect()
}

fn to_html(src: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim();
        if t.is_empty() {
            i += 1;
        } else if t.starts_with("```") {
            i += 1;
            out.push_str("<pre>");
            while i < lines.len() && !lines[i].trim().starts_with("```") {
                out.push_str(&esc(lines[i]));
                out.push('\n');
                i += 1;
            }
            out.push_str("</pre>\n");
            i += 1;
        } else if t.starts_with('#') {
            let level = t.chars().take_while(|&c| c == '#').count().min(4);
            out.push_str(&format!("<h{level}>{}</h{level}>\n", inline(t[level..].trim())));
            i += 1;
        } else if t.starts_with('|') {
            out.push_str("<table>\n");
            let mut first = true;
            while i < lines.len() && lines[i].trim().starts_with('|') {
                let row = cells(lines[i]);
                i += 1;
                if row.iter().all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':')) {
                    continue;
                }
                let tag = if first { "th" } else { "td" };
                first = false;
                out.push_str("<tr>");
                for c in row {
                    out.push_str(&format!("<{tag}>{}</{tag}>", inline(c)));
                }
                out.push_str("</tr>\n");
            }
            out.push_str("</table>\n");
        } else if let Some((tag, _)) = item(t) {
            out.push_str(&format!("<{tag}>\n"));
            while i < lines.len() {
                let Some((kind, text)) = item(lines[i].trim()) else { break };
                if kind != tag {
                    break;
                }
                let mut body = text.to_string();
                i += 1;
                // Suite de l'élément : lignes en retrait.
                while i < lines.len() && lines[i].starts_with("  ") && !lines[i].trim().is_empty() && item(lines[i].trim()).is_none() {
                    body.push(' ');
                    body.push_str(lines[i].trim());
                    i += 1;
                }
                out.push_str(&format!("<li>{}</li>\n", inline(&body)));
            }
            out.push_str(&format!("</{tag}>\n"));
        } else {
            let mut body = t.to_string();
            i += 1;
            while i < lines.len() {
                let n = lines[i].trim();
                if n.is_empty() || n.starts_with('#') || n.starts_with('|') || n.starts_with("```") || item(n).is_some() {
                    break;
                }
                body.push(' ');
                body.push_str(n);
                i += 1;
            }
            out.push_str(&format!("<p>{}</p>\n", inline(&body)));
        }
    }
    out
}

/// Élément de liste : (`ul` ou `ol`, texte).
fn item(t: &str) -> Option<(&'static str, &str)> {
    if let Some(r) = t.strip_prefix("- ") {
        return Some(("ul", r));
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 {
        if let Some(r) = t[digits..].strip_prefix(". ") {
            return Some(("ol", r));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_simple() {
        let h = to_html("# Titre\n\nUn **gras** et `a<b`.\nsuite\n\n- un\n  deux\n- trois\n\n1. a\n2. b\n\n| A | B |\n|---|---|\n| x | **y** |\n");
        assert!(h.contains("<h1>Titre</h1>"), "{h}");
        assert!(h.contains("<p>Un <b>gras</b> et <code>a&lt;b</code>. suite</p>"), "{h}");
        assert!(h.contains("<ul>\n<li>un deux</li>\n<li>trois</li>\n</ul>"), "{h}");
        assert!(h.contains("<ol>\n<li>a</li>\n<li>b</li>\n</ol>"), "{h}");
        assert!(h.contains("<tr><th>A</th><th>B</th></tr>\n<tr><td>x</td><td><b>y</b></td></tr>"), "{h}");
    }

    #[test]
    fn manuels_complets() {
        for en in [false, true] {
            let h = html(en);
            assert!(h.contains(crate::VERSION));
            assert!(!h.contains("**"), "gras non fermé");
            assert_eq!(h.matches("<table>").count(), h.matches("</table>").count());
        }
    }
}
