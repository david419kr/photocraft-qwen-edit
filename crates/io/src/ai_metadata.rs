//! The document's AI prompt history property in XMP. Its payload is opaque to the file layer.
use roxmltree::{Document, Node, ParsingOptions};

pub const NAMESPACE: &str = "urn:photocraft:ai-prompts:1";
const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
pub const MAX_XMP_BYTES: usize = 16 << 20;
const EMPTY: &str = "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"></rdf:RDF></x:xmpmeta>";

pub fn has_history(xmp: Option<&str>) -> bool {
    xmp.is_some_and(|text| text.contains(NAMESPACE))
}

fn parse(text: &str) -> Result<Document<'_>, String> {
    if text.len() > MAX_XMP_BYTES {
        return Err("XMP packet exceeds the prompt history size limit".into());
    }
    Document::parse_with_options(text, ParsingOptions { nodes_limit: 100_000, ..Default::default() }).map_err(|e| format!("Invalid XMP: {e}"))
}

fn is_history(node: Node<'_, '_>) -> bool {
    node.has_tag_name((NAMESPACE, "History"))
}

pub fn payload(xmp: Option<&str>) -> Result<Option<String>, String> {
    let Some(text) = xmp.filter(|_| has_history(xmp)) else { return Ok(None) };
    let xml = parse(text)?;
    let mut nodes = xml.descendants().filter(|node| is_history(*node));
    let Some(node) = nodes.next() else { return Ok(None) };
    if nodes.next().is_some() || node.attribute("encoding") != Some("zlib-hex") || node.children().any(|child| child.is_element()) {
        return Err("Invalid AI prompt history property".into());
    }
    Ok(Some(node.text().unwrap_or_default().trim().to_owned()))
}

fn insert_child(text: &mut String, range: std::ops::Range<usize>, child: &str) -> Result<(), String> {
    let span = text.get(range.clone()).ok_or("Invalid XMP element range")?;
    if span.ends_with("/>") {
        let name =
            span.strip_prefix('<').and_then(|s| s.split(|c: char| c.is_whitespace() || matches!(c, '/' | '>')).next()).ok_or("Invalid XMP element name")?;
        let replacement = format!(">{child}</{name}>");
        let start = range.end.checked_sub(2).ok_or("Invalid XMP element")?;
        if text.get(start..range.end).is_none() {
            return Err("Invalid XMP boundary".into());
        }
        text.replace_range(start..range.end, &replacement);
    } else {
        let close = span.rfind("</").ok_or("XMP element has no closing tag")?;
        let at = range.start.checked_add(close).ok_or("XMP offset overflow")?;
        if !text.is_char_boundary(at) {
            return Err("Invalid XMP text boundary".into());
        }
        text.insert_str(at, child);
    }
    Ok(())
}

pub fn with_payload(xmp: Option<&str>, payload: &str) -> Result<String, String> {
    if payload.len() > MAX_XMP_BYTES || !payload.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Invalid AI prompt history payload".into());
    }
    let source = xmp.filter(|s| !s.trim().is_empty()).unwrap_or(EMPTY);
    if source.len() > MAX_XMP_BYTES {
        return Err("XMP packet exceeds the prompt history size limit".into());
    }
    let mut text = source.to_owned();
    let xml = parse(&text)?;
    let mut ranges = Vec::new();
    for node in xml.descendants().filter(|node| is_history(*node)) {
        let parent = node.parent().filter(|parent| {
            parent.has_tag_name((RDF, "Description"))
                && parent.children().filter(Node::is_element).count() == 1
                && parent.attributes().all(|attr| attr.namespace() == Some(RDF) && attr.name() == "about")
                && parent.children().filter(Node::is_text).all(|n| n.text().unwrap_or_default().trim().is_empty())
        });
        ranges.push(parent.unwrap_or(node).range());
    }
    ranges.sort_by_key(|range| range.start);
    ranges.dedup();
    for range in ranges.into_iter().rev() {
        if text.get(range.clone()).is_none() {
            return Err("Invalid XMP property boundary".into());
        }
        text.replace_range(range, "");
    }
    let xml = parse(&text)?;
    let desc = format!(
        "<rdf:Description xmlns:rdf=\"{RDF}\" xmlns:pcraft=\"{NAMESPACE}\" rdf:about=\"\"><pcraft:History encoding=\"zlib-hex\">{payload}</pcraft:History></rdf:Description>"
    );
    let rdf = xml.descendants().find(|node| node.has_tag_name((RDF, "RDF")));
    let range = rdf.unwrap_or(xml.root_element()).range();
    let body = if rdf.is_some() { desc } else { format!("<rdf:RDF xmlns:rdf=\"{RDF}\">{desc}</rdf:RDF>") };
    insert_child(&mut text, range, &body)?;
    if text.len() > MAX_XMP_BYTES {
        return Err("AI prompt history exceeds the XMP size limit".into());
    }
    Ok(text)
}

pub fn only_history(xmp: Option<&str>) -> Result<Option<String>, String> {
    payload(xmp)?.map(|value| with_payload(None, &value)).transpose()
}
