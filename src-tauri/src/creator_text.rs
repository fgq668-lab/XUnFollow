//! Plain-text publishing layout. Never writes back old drafts or counts a copy as publication.
use regex::Regex;
use std::sync::OnceLock;

fn rules() -> &'static Vec<(Regex, &'static str)> {
    static RULES: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    RULES.get_or_init(|| {
        vec![
            (Regex::new(r"(?m)^[ \t]{0,3}#{1,6}[ \t]+").unwrap(), ""),
            (Regex::new(r"(?m)^[ \t]*>[ \t]?").unwrap(), ""),
            (Regex::new(r"(?m)^[ \t]*[-*+][ \t]+").unwrap(), ""),
            (Regex::new(r"\*\*([^*\n]*\p{L}[^*\n]*)\*\*").unwrap(), "$1"),
            (Regex::new(r"__([^_\n]+)__").unwrap(), "$1"),
            (Regex::new(r"`+([^`\n]+)`+").unwrap(), "$1"),
            (
                Regex::new(r"\[([^\]\n]+)\]\((https?://[^\s)]+)\)").unwrap(),
                "$1 $2",
            ),
        ]
    })
}
pub fn body(input: &str) -> String {
    let mut text = input.replace("\r\n", "\n").replace('\r', "\n");
    // Remove only fence markers, not their contents.
    text = text
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.strip_prefix("```").is_some_and(|language| {
                language
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+'))
            })
        })
        .collect::<Vec<_>>()
        .join("\n");
    for (rule, replacement) in rules() {
        text = rule.replace_all(&text, *replacement).into_owned();
    }
    let mut lines = Vec::new();
    for line in text.lines().map(str::trim) {
        if !line.is_empty() || lines.last().is_some_and(|s: &String| !s.is_empty()) {
            lines.push(line.to_owned());
        }
    }
    let text = lines.join("\n").trim().to_owned();
    if text.contains('\n') || text.chars().count() <= 90 {
        return text;
    }
    // Older single-block drafts: break on sentence boundaries, never invent headings.
    let mut out = String::new();
    let mut width = 0;
    let mut breaks = 0;
    let chars = text.chars().collect::<Vec<_>>();
    for (i, ch) in chars.iter().enumerate() {
        out.push(*ch);
        width += 1;
        if matches!(ch, '。' | '！' | '？' | '!' | '?')
            && width >= 35
            && chars.len() - i > 18
            && breaks < 3
        {
            out.push_str("\n\n");
            width = 0;
            breaks += 1;
        }
    }
    out
}
pub fn post(publish_body: &str, urls: impl IntoIterator<Item = String>) -> String {
    let mut text = publish_body.to_owned();
    let mut seen = std::collections::BTreeSet::new();
    let urls = urls
        .into_iter()
        .filter(|url| !text.contains(url) && seen.insert(url.clone()))
        .collect::<Vec<_>>();
    if !urls.is_empty() {
        text.push_str("\n\n来源：\n");
        text.push_str(&urls.join("\n"));
    }
    text
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn markdown_layout_becomes_copyable_plain_text_without_losing_links() {
        assert_eq!(body("## **小工具**\r\n\r\n> 少点按钮。\r\n\r\n\r\n- 保留 `Ctrl+C`。\n[原帖](https://x.com/test/status/1)"), "小工具\n\n少点按钮。\n\n保留 Ctrl+C。\n原帖 https://x.com/test/status/1");
        assert_eq!(
            body("#创业\n2**3**4\n```js\nconst a = 1;\n```"),
            "#创业\n2**3**4\nconst a = 1;"
        );
    }
    #[test]
    fn short_posts_keep_their_line_breaks_and_long_ones_get_sentence_spacing() {
        assert_eq!(
            body("代码能跑。\n\n先别问为什么。"),
            "代码能跑。\n\n先别问为什么。"
        );
        let original = "讲一个具体的小细节，比写满一整屏的空话更容易让人看懂，也不用急着把所有背景都交代清楚。段落之间留一点空间，复制到输入框里还是这个样子，读起来不用挤在一起。说完自己的那个小想法就停，不用再替别人总结一遍，也不用在最后喊口号。";
        let formatted = body(original);
        assert!(formatted.contains("\n\n"));
        assert_eq!(formatted.replace('\n', ""), original);
        assert_eq!(body(&formatted), formatted);
    }
    #[test]
    fn sources_are_separate_unique_and_do_not_duplicate_body_links() {
        let url = "https://x.com/test/status/1".to_owned();
        assert_eq!(
            post("内容", [url.clone(), url.clone()]),
            format!("内容\n\n来源：\n{url}")
        );
        assert_eq!(post(&url, [url.clone()]), url);
    }
}
