//! Memory-safe XML parser for `WiX` source documents.
//!
//! Provides document object model extraction with line and column tracking
//! for precise diagnostic error reporting.

use crate::error::{Error, Result};
use std::collections::HashMap;

/// An XML node element representing a tag in a `WiX` source file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XmlNode {
    /// Element tag name (e.g. `Product`, `Directory`, `Component`, `File`).
    pub tag: String,
    /// Element attributes (e.g. `Id="MainComponent"`).
    pub attributes: HashMap<String, String>,
    /// Child XML elements nested within this element.
    pub children: Vec<Self>,
    /// Inner text content (trimmed of insignificant whitespace).
    pub text: String,
    /// 1-based source line number.
    pub line: usize,
    /// 1-based source column number.
    pub column: usize,
}

impl XmlNode {
    /// Retrieves the value of an attribute by name.
    ///
    /// # Arguments
    ///
    /// * `name` - Attribute name.
    ///
    /// # Returns
    ///
    /// Optional reference to attribute value string.
    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(String::as_str)
    }

    /// Finds the first child element with the specified tag name.
    ///
    /// # Arguments
    ///
    /// * `tag` - Tag name to match.
    ///
    /// # Returns
    ///
    /// Optional reference to child [`XmlNode`].
    #[must_use]
    pub fn find_child(&self, tag: &str) -> Option<&Self> {
        self.children.iter().find(|c| c.tag == tag)
    }

    /// Returns an iterator over all child elements with the specified tag name.
    ///
    /// # Arguments
    ///
    /// * `tag` - Tag name to match.
    ///
    /// # Returns
    ///
    /// Filtered iterator over child nodes.
    pub fn children_with_tag<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a Self> {
        self.children.iter().filter(move |c| c.tag == tag)
    }

    /// Serializes this [`XmlNode`] and its children to an XML string.
    ///
    /// # Returns
    ///
    /// Formatted XML string.
    #[must_use]
    pub fn to_xml_string(&self) -> String {
        let mut out = String::new();
        self.render_xml(&mut out, 0);
        out
    }

    /// Renders this node with indentation to a string buffer.
    fn render_xml(&self, out: &mut String, indent: usize) {
        let pad = " ".repeat(indent);
        out.push_str(&pad);
        out.push('<');
        out.push_str(&self.tag);

        let mut sorted_attrs: Vec<(&String, &String)> = self.attributes.iter().collect();
        sorted_attrs.sort_by_key(|(k, _)| *k);
        for (k, v) in sorted_attrs {
            out.push(' ');
            out.push_str(k);
            out.push_str("=\"");
            for ch in v.chars() {
                match ch {
                    '&' => out.push_str("&amp;"),
                    '<' => out.push_str("&lt;"),
                    '>' => out.push_str("&gt;"),
                    '"' => out.push_str("&quot;"),
                    '\'' => out.push_str("&apos;"),
                    c => out.push(c),
                }
            }
            out.push('"');
        }

        if self.children.is_empty() && self.text.is_empty() {
            out.push_str(" />\n");
            return;
        }

        out.push('>');

        if !self.text.is_empty() {
            for ch in self.text.chars() {
                match ch {
                    '&' => out.push_str("&amp;"),
                    '<' => out.push_str("&lt;"),
                    '>' => out.push_str("&gt;"),
                    c => out.push(c),
                }
            }
        }

        if !self.children.is_empty() {
            out.push('\n');
            for child in &self.children {
                child.render_xml(out, indent + 2);
            }
            out.push_str(&pad);
        }

        out.push_str("</");
        out.push_str(&self.tag);
        out.push_str(">\n");
    }
}

impl std::fmt::Display for XmlNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_xml_string())
    }
}

/// Fast, memory-safe parser for `WiX` XML documents.
#[derive(Debug, Default)]
pub struct XmlParser;

impl XmlParser {
    /// Creates a new [`XmlParser`].
    ///
    /// # Returns
    ///
    /// A new [`XmlParser`].
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Parses an XML string into a root [`XmlNode`].
    ///
    /// # Arguments
    ///
    /// * `input` - Raw XML string.
    ///
    /// # Returns
    ///
    /// The root [`XmlNode`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::XmlParse`] if the document is malformed or has unclosed tags.
    #[allow(clippy::too_many_lines)]
    pub fn parse(&self, input: &str) -> Result<XmlNode> {
        let mut chars = input.char_indices().peekable();
        let mut line = 1;
        let mut col = 1;
        let mut stack: Vec<XmlNode> = Vec::new();
        let mut root: Option<XmlNode> = None;

        while let Some(&(i, ch)) = chars.peek() {
            if ch == '\n' {
                line += 1;
                col = 1;
                let _ = chars.next();
                continue;
            }

            if ch == '<' {
                let start_col = col;
                let start_line = line;
                let _ = chars.next();
                col += 1;

                // Check for comment <!--
                if input[i..].starts_with("<!--") {
                    // Skip characters until -->
                    while let Some((_, c)) = chars.next() {
                        if c == '\n' {
                            line += 1;
                            col = 1;
                        } else {
                            col += 1;
                        }
                        if let Some(&(next_i, _)) = chars.peek() {
                            if input[next_i..].starts_with("-->") {
                                let _ = chars.next(); // -
                                let _ = chars.next(); // -
                                let _ = chars.next(); // >
                                col += 3;
                                break;
                            }
                        }
                    }
                    continue;
                }

                // Check for CDATA <![CDATA[
                if input[i..].starts_with("<![CDATA[") {
                    // Skip "<![CDATA[" (note '<' was already consumed at line 191)
                    for _ in 0..8 {
                        if let Some((_, c)) = chars.next() {
                            if c == '\n' {
                                line += 1;
                                col = 1;
                            } else {
                                col += 1;
                            }
                        }
                    }
                    let mut cdata_text = String::new();
                    while let Some(&(next_i, c)) = chars.peek() {
                        if input[next_i..].starts_with("]]>") {
                            let _ = chars.next(); // ]
                            let _ = chars.next(); // ]
                            let _ = chars.next(); // >
                            col += 3;
                            break;
                        }
                        cdata_text.push(c);
                        let _ = chars.next();
                        if c == '\n' {
                            line += 1;
                            col = 1;
                        } else {
                            col += 1;
                        }
                    }
                    if let Some(top) = stack.last_mut() {
                        top.text.push_str(&cdata_text);
                    }
                    continue;
                }

                // Check for XML declaration <?xml or processing instruction <?
                if input[i..].starts_with("<?") {
                    while let Some((_, c)) = chars.next() {
                        if c == '\n' {
                            line += 1;
                            col = 1;
                        } else {
                            col += 1;
                        }
                        if let Some(&(next_i, _)) = chars.peek() {
                            if input[next_i..].starts_with("?>") {
                                let _ = chars.next();
                                let _ = chars.next();
                                col += 2;
                                break;
                            }
                        }
                    }
                    continue;
                }

                // Check for closing tag </tag>
                if let Some(&(_, '/')) = chars.peek() {
                    let _ = chars.next(); // consume '/'
                    col += 1;
                    let mut close_tag = String::new();
                    let mut closed = false;
                    while let Some(&(_, c)) = chars.peek() {
                        let _ = chars.next();
                        col += 1;
                        if c == '>' {
                            closed = true;
                            break;
                        }
                        if !c.is_whitespace() {
                            close_tag.push(c);
                        }
                    }

                    if !closed {
                        return Err(Error::XmlParse {
                            line: start_line,
                            column: start_col,
                            message: format!("unclosed closing tag '</{close_tag}'"),
                        });
                    }

                    if let Some(top) = stack.pop() {
                        if top.tag != close_tag {
                            return Err(Error::XmlParse {
                                line: start_line,
                                column: start_col,
                                message: format!(
                                    "mismatched closing tag '</{close_tag}>', expected '</{}>'",
                                    top.tag
                                ),
                            });
                        }
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(top);
                        } else {
                            root = Some(top);
                        }
                    } else {
                        return Err(Error::XmlParse {
                            line: start_line,
                            column: start_col,
                            message: format!("unexpected closing tag '</{close_tag}>'"),
                        });
                    }
                    continue;
                }

                // Opening tag <tag attr="val"... > or <tag ... />
                let mut tag_content = String::new();
                let mut self_closing = false;

                while let Some(&(_, c)) = chars.peek() {
                    let _ = chars.next();
                    col += 1;
                    if c == '\n' {
                        line += 1;
                        col = 1;
                    }

                    if c == '>' {
                        if tag_content.ends_with('/') {
                            self_closing = true;
                            let _ = tag_content.pop();
                        }
                        break;
                    }
                    tag_content.push(c);
                }

                let (tag_name, attributes) = self.parse_tag_content(&tag_content)?;
                let node = XmlNode {
                    tag: tag_name,
                    attributes,
                    children: Vec::new(),
                    text: String::new(),
                    line: start_line,
                    column: start_col,
                };

                if self_closing {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(node);
                    } else if root.is_none() {
                        root = Some(node);
                    }
                } else {
                    stack.push(node);
                }
            } else {
                // Text content
                let mut text_content = String::new();
                while let Some(&(_, c)) = chars.peek() {
                    if c == '<' {
                        break;
                    }
                    let _ = chars.next();
                    if c == '\n' {
                        line += 1;
                        col = 1;
                    } else {
                        col += 1;
                    }
                    text_content.push(c);
                }
                let clean_text = text_content.trim();
                if !clean_text.is_empty() {
                    let unescaped = self.unescape_xml(clean_text);
                    if let Some(top) = stack.last_mut() {
                        if !top.text.is_empty() {
                            top.text.push(' ');
                        }
                        top.text.push_str(&unescaped);
                    }
                }
            }
        }

        if !stack.is_empty() {
            let unclosed = &stack[stack.len() - 1];
            return Err(Error::XmlParse {
                line: unclosed.line,
                column: unclosed.column,
                message: format!("unclosed element '<{}>'", unclosed.tag),
            });
        }

        root.ok_or_else(|| Error::XmlParse {
            line: 1,
            column: 1,
            message: "empty XML document".to_string(),
        })
    }

    /// Parses element tag name and key-value attributes from raw tag header text.
    fn parse_tag_content(&self, content: &str) -> Result<(String, HashMap<String, String>)> {
        let trimmed = content.trim_start();
        let mut parts = trimmed.split_whitespace();
        let tag_name = match parts.next() {
            Some(t) => t.to_string(),
            None => {
                return Err(Error::XmlParse {
                    line: 1,
                    column: 1,
                    message: "missing tag name".to_string(),
                });
            }
        };

        let mut attributes = HashMap::new();
        let mut cursor = tag_name.len();
        let bytes = trimmed.as_bytes();

        while cursor < bytes.len() {
            // Skip whitespace
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if cursor >= bytes.len() {
                break;
            }

            // Find attribute key up to '='
            let key_start = cursor;
            while cursor < bytes.len()
                && bytes[cursor] != b'='
                && !bytes[cursor].is_ascii_whitespace()
            {
                cursor += 1;
            }
            let key = std::str::from_utf8(&bytes[key_start..cursor])
                .unwrap_or("")
                .trim();
            if key.is_empty() {
                break;
            }

            // Skip whitespace up to '='
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if cursor < bytes.len() && bytes[cursor] == b'=' {
                cursor += 1;
            }

            // Skip whitespace up to quote
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }

            if cursor < bytes.len() && (bytes[cursor] == b'"' || bytes[cursor] == b'\'') {
                let quote = bytes[cursor];
                cursor += 1;
                let val_start = cursor;
                while cursor < bytes.len() && bytes[cursor] != quote {
                    cursor += 1;
                }
                let val = std::str::from_utf8(&bytes[val_start..cursor]).unwrap_or("");
                attributes.insert(key.to_string(), self.unescape_xml(val));
                if cursor < bytes.len() {
                    cursor += 1;
                }
            }
        }

        Ok((tag_name, attributes))
    }

    /// Replaces standard XML entities with literal characters.
    #[allow(clippy::unused_self)]
    fn unescape_xml(&self, val: &str) -> String {
        val.replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests standard XML parsing, hierarchy traversal, attribute extraction, and child counting.
    #[test]
    fn test_xml_parser_basic() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<!-- Sample WiX XML -->
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{12345678-1234-1234-1234-1234567890AB}" Name="MyApp" Version="1.0.0">
        <Package Description="Test Installer" />
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Component Id="MainComp">
                <File Id="MainExe" Source="app.exe" />
            </Component>
        </Directory>
    </Product>
</Wix>
"#;

        let parser = XmlParser::new();
        let res = parser.parse(xml);
        assert!(res.is_ok());
        let root = res.unwrap_or_default();

        assert_eq!(root.tag, "Wix");
        assert_eq!(
            root.attribute("xmlns"),
            Some("http://schemas.microsoft.com/wix/2006/wi")
        );

        let prod = root.find_child("Product");
        assert!(prod.is_some());
        assert_eq!(prod.and_then(|p| p.attribute("Name")), Some("MyApp"));
        assert_eq!(prod.and_then(|p| p.attribute("Version")), Some("1.0.0"));

        let package = prod.and_then(|p| p.find_child("Package"));
        assert_eq!(
            package.and_then(|pkg| pkg.attribute("Description")),
            Some("Test Installer")
        );

        let dir = prod.and_then(|p| p.find_child("Directory"));
        assert_eq!(dir.and_then(|d| d.attribute("Id")), Some("TARGETDIR"));

        let comp = dir.and_then(|d| d.find_child("Component"));
        assert_eq!(comp.and_then(|c| c.attribute("Id")), Some("MainComp"));

        let count = prod.map_or(0, |p| p.children_with_tag("Package").count());
        assert_eq!(count, 1);
    }

    /// Tests multiline comments, processing instructions, EOF inside comments/PIs, and tag newlines.
    #[test]
    fn test_xml_parser_comments_and_processing_instructions() {
        let parser = XmlParser;

        // Multiline comment with \n and processing instruction with \n
        let xml = r#"<?xml
  version="1.0"?>
<!-- Multiline
     comment -->
<Doc
  attr="val">
  <!-- Another comment -->
  Text content
</Doc>
"#;
        let res = parser.parse(xml);
        assert!(res.is_ok());
        let node = res.unwrap_or_default();
        assert_eq!(node.tag, "Doc");
        assert_eq!(node.attribute("attr"), Some("val"));
        assert_eq!(node.text, "Text content");

        // Comment reaching EOF without closing -->
        assert!(parser.parse("<Doc><!-- unterminated comment").is_err());

        // Processing instruction reaching EOF without closing ?>
        assert!(parser.parse("<Doc><?unterminated pi").is_err());
    }

    /// Tests attribute parsing: spacing around `=`, single quotes, unquoted attributes,
    /// self-closing tags at root and nested, closing tags with whitespace, and entity unescaping.
    #[test]
    fn test_xml_parser_attributes_spacing_and_entities() {
        let parser = XmlParser::new();

        // Self-closing root elements, including second self-closing root
        let res_root = parser.parse("<Root attr='single' /><Root2 />");
        assert!(res_root.is_ok());
        let root = res_root.unwrap_or_default();
        assert_eq!(root.tag, "Root");
        assert_eq!(root.attribute("attr"), Some("single"));

        // Whitespace around =, whitespace in closing tag, trailing whitespace in tag,
        // empty attribute key, unquoted attribute values, and text outside of root
        let xml_complex = r#"
Outside text before root
<Container a  =  "1" b= '2' c = "3" unquoted=val trailing="spaces"   >
    <Child ="no_key" />
    First text
    <!-- mid comment -->
    Second text
</ Container >
Outside text after root
"#;
        let res_complex = parser.parse(xml_complex);
        assert!(res_complex.is_ok());
        let container = res_complex.unwrap_or_default();
        assert_eq!(container.attribute("a"), Some("1"));
        assert_eq!(container.attribute("b"), Some("2"));
        assert_eq!(container.attribute("c"), Some("3"));
        assert_eq!(container.attribute("trailing"), Some("spaces"));
        assert_eq!(container.children.len(), 1);
        assert!(container.text.contains("First text Second text"));

        // All standard XML entity unescapes (&quot;, &apos;, &lt;, &gt;, &amp;)
        let xml_entities = r#"<Doc text="&apos;single&apos; &quot;double&quot; &amp; &lt;&gt;" />"#;
        let res_entities = parser.parse(xml_entities);
        assert!(res_entities.is_ok());
        let ent_node = res_entities.unwrap_or_default();
        assert_eq!(ent_node.attribute("text"), Some("'single' \"double\" & <>"));
    }

    /// Tests syntax error branches: empty document, missing tag name, unclosed tags,
    /// mismatched tags, unexpected closing tags, and unclosed quotes.
    #[test]
    fn test_xml_parser_errors() {
        let parser = XmlParser::new();

        // Empty document
        assert!(parser.parse("").is_err());
        assert!(parser.parse("   \n\t  ").is_err());

        // Missing tag name (<>)
        assert!(parser.parse("<>").is_err());
        assert!(parser.parse("< >").is_err());

        // Unclosed tags
        assert!(parser.parse("<Wix><Product></Wix>").is_err());
        assert!(parser.parse("<Wix>").is_err());

        // Unexpected closing tag
        assert!(parser.parse("</Wix>").is_err());
        assert!(parser.parse("<Wix></Other>").is_err());
        assert!(parser.parse("<Wix></Wix").is_err());

        // Unterminated attribute quotes
        assert!(parser.parse("<Doc attr=\"unterminated>").is_err());
        assert!(parser.parse("<Doc attr='unterminated>").is_err());

        // Trailing key without value or reaching EOF
        assert!(parser.parse("<Doc key ").is_err());
        assert!(parser.parse("<Doc key").is_err());
    }

    /// Tests CDATA section parsing including embedded quotes, operators, and multiline text.
    #[test]
    fn test_xml_parser_cdata() {
        let parser = XmlParser::new();
        let xml = r#"<Publish Event="EndDialog" Value="Return"><![CDATA[LICENSE_ACCEPTED="1" AND PROP_VAL > 0]]></Publish>"#;
        let res = parser.parse(xml);
        assert!(res.is_ok());
        let node = res.unwrap_or_default();
        assert_eq!(node.tag, "Publish");
        assert_eq!(node.text, "LICENSE_ACCEPTED=\"1\" AND PROP_VAL > 0");

        // Multiline CDATA
        let xml_multiline = "<Condition><![CDATA[\nLINE1\nLINE2\n]]></Condition>";
        let res_multi = parser.parse(xml_multiline);
        assert!(res_multi.is_ok());
        let node_multi = res_multi.unwrap_or_default();
        assert_eq!(node_multi.text, "\nLINE1\nLINE2\n");
    }

    /// Tests node query methods on non-existent elements and trait implementations.
    #[test]
    fn test_xml_node_queries_and_traits() {
        let node = XmlNode {
            tag: "Elem".to_string(),
            attributes: HashMap::new(),
            children: Vec::new(),
            text: "Hello".to_string(),
            line: 1,
            column: 1,
        };

        // None queries
        assert_eq!(node.attribute("missing"), None);
        assert_eq!(node.find_child("missing"), None);
        assert_eq!(node.children_with_tag("missing").count(), 0);

        // Traits: Clone, PartialEq, Debug
        let cloned_node = node.clone();
        assert_eq!(node, cloned_node);
        assert!(format!("{node:?}").contains("XmlNode"));

        let parser = XmlParser;
        assert!(format!("{parser:?}").contains("XmlParser"));
    }

    #[test]
    fn test_xml_serialization_and_display() {
        let mut child = XmlNode {
            tag: "Child".to_string(),
            attributes: std::iter::once((
                "escaped".to_string(),
                "a & b < c > d \" e ' f".to_string(),
            ))
            .collect(),
            children: Vec::new(),
            text: "Text with & < > characters".to_string(),
            line: 2,
            column: 3,
        };

        let parent = XmlNode {
            tag: "Parent".to_string(),
            attributes: HashMap::new(),
            children: vec![child.clone()],
            text: String::new(),
            line: 1,
            column: 1,
        };

        let xml_str = parent.to_xml_string();
        assert!(xml_str.contains("&amp;"));
        assert!(xml_str.contains("&lt;"));
        assert!(xml_str.contains("&gt;"));
        assert!(xml_str.contains("&quot;"));
        assert!(xml_str.contains("&apos;"));
        assert_eq!(format!("{parent}"), xml_str);

        // Self-closing empty child
        child.attributes.clear();
        child.text.clear();
        assert_eq!(child.to_xml_string(), "<Child />\n");
    }
}
