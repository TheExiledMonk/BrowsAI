//! Markdown view over an [`AgentRenderTree`] for LLM consumption.
//!
//! The emitter walks the tree in document order and produces a single
//! markdown string. Actionable widgets are wrapped in BB-code-style tags
//! (e.g. `[link id="n3"]text[/link]`) so the LLM can both read the page
//! as prose and resolve the same [`AgentNodeId`] back through the
//! existing `/follow-link` and `/action` endpoints.
//!
//! The markdown output is **not** intended for re-rendering as HTML.
//! The tags are an agent-only convention that sits on top of standard
//! markdown so the LLM can pick them out with a single regex.
//!
//! [`Plain`] mode strips every BB-code tag and emits only the prose
//! underneath. Use it when the LLM is purely reading the page (Q&A,
//! summarisation) and does not need to issue actions against the
//! resulting output.

use browsai_agent_tree::{
    AgentNode, AgentNodeId, AgentRenderTree, AgentValue, SemanticRole, StructuralRole,
};
use serde::{Deserialize, Serialize};

pub const PAGE_TEXT_FORMAT_VERSION: u16 = 1;

/// Output style. Both modes share the same node walk and the same
/// `<page-content trust="untrusted">` envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageTextFormat {
    /// Standard markdown with BB-code-style actionable tags. Default.
    Markdown,
    /// Plain prose only — actionable nodes render as their accessible
    /// text (or `(input: name)` / `(button: name)` placeholders for
    /// self-closing widgets) so the LLM gets a cheaper prompt when it
    /// only needs to read.
    Plain,
}

impl Default for PageTextFormat {
    fn default() -> Self {
        Self::Markdown
    }
}

/// Output knob set. All fields default to the safe, prose-friendly choice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkdownOptions {
    pub format: PageTextFormat,
    /// Emit nodes whose `state.visible` is `false`. Off by default.
    pub include_invisible: bool,
    /// Emit nodes whose `state.enabled` is `false`. On by default with
    /// `disabled=true` surfaced as an attribute.
    pub include_disabled: bool,
    /// Truncate any single piece of node text to this many bytes.
    /// Defaults to 500 to keep prompts bounded.
    pub max_text_bytes: usize,
    /// Heading depth past which a heading is demoted to a paragraph.
    /// Defaults to 6 (the markdown spec ceiling).
    pub max_heading_level: u8,
    /// When `true`, omit the surrounding `<page-content trust="untrusted">`
    /// envelope. Default off — host prompts should always see the
    /// envelope so the LLM can be told which content is data, not
    /// instruction.
    pub raw: bool,
}

impl Default for MarkdownOptions {
    fn default() -> Self {
        Self {
            format: PageTextFormat::Markdown,
            include_invisible: false,
            include_disabled: true,
            max_text_bytes: 500,
            max_heading_level: 6,
            raw: false,
        }
    }
}

/// Tag emitted by the markdown view. Distinct from
/// [`browsai_agent_tree::StructuralRole`] so that semantic refinements
/// (e.g. `Challenge`) can be expressed even when the underlying role is
/// `Region`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarkdownTag {
    Link,
    Button,
    Textbox,
    Checkbox,
    Radio,
    Select,
    Option,
    Image,
    Heading,
    Region,
    Dialog,
    Challenge,
    Form,
    List,
    ListItem,
    Table,
    Row,
    Cell,
}

impl MarkdownTag {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Link => "link",
            Self::Button => "button",
            Self::Textbox => "textbox",
            Self::Checkbox => "checkbox",
            Self::Radio => "radio",
            Self::Select => "select",
            Self::Option => "option",
            Self::Image => "image",
            Self::Heading => "heading",
            Self::Region => "region",
            Self::Dialog => "dialog",
            Self::Challenge => "challenge",
            Self::Form => "form",
            Self::List => "list",
            Self::ListItem => "listitem",
            Self::Table => "table",
            Self::Row => "row",
            Self::Cell => "cell",
        }
    }
}

/// A single actionable span in the emitted markdown. The byte offsets
/// point into [`MarkdownView::content`] so a strict host can parse
/// nodes programmatically without scanning the string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeSpan {
    pub id: AgentNodeId,
    pub tag: MarkdownTag,
    pub start: usize,
    pub end: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Result of [`MarkdownEmitter::emit`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkdownView {
    pub format_version: u16,
    /// `<page-content trust="untrusted">…</page-content>` envelope
    /// unless [`MarkdownOptions::raw`] is set.
    pub content: String,
    /// All actionable spans emitted into `content`. May be empty.
    pub node_index: Vec<NodeSpan>,
    /// Number of source nodes that were skipped (invisible, empty, etc.).
    pub skipped_nodes: usize,
}

pub struct MarkdownEmitter<'a> {
    tree: &'a AgentRenderTree,
    out: String,
    index: Vec<NodeSpan>,
    skipped: usize,
    list_depth: u32,
}

impl<'a> MarkdownEmitter<'a> {
    pub fn new(tree: &'a AgentRenderTree) -> Self {
        Self {
            tree,
            out: String::new(),
            index: Vec::new(),
            skipped: 0,
            list_depth: 0,
        }
    }

    fn is_plain(opts: &MarkdownOptions) -> bool {
        opts.format == PageTextFormat::Plain
    }

    pub fn emit(mut self, opts: &MarkdownOptions) -> MarkdownView {
        let body_start = self.out.len();
        if let Some(root) = self.tree.find(&self.tree.root).cloned() {
            self.emit_page(&root, opts);
        }
        let body = self.out[body_start..].to_string();
        let envelope_prefix = if opts.raw {
            ""
        } else {
            "<page-content trust=\"untrusted\">\n"
        };
        let wrapped = if envelope_prefix.is_empty() {
            body
        } else {
            let mut s = String::with_capacity(envelope_prefix.len() + body.len() + 32);
            s.push_str(envelope_prefix);
            s.push_str(&body);
            if !body.ends_with('\n') {
                s.push('\n');
            }
            s.push_str("</page-content>");
            s
        };
        self.out.clear();
        self.out.push_str(&wrapped);
        let prefix_len = envelope_prefix.len();
        for span in &mut self.index {
            span.start += prefix_len;
            span.end += prefix_len;
        }
        MarkdownView {
            format_version: PAGE_TEXT_FORMAT_VERSION,
            content: self.out,
            node_index: self.index,
            skipped_nodes: self.skipped,
        }
    }

    fn emit_page(&mut self, node: &AgentNode, opts: &MarkdownOptions) {
        if let Some(title) = node.name.as_deref() {
            if Self::is_plain(opts) {
                self.out.push_str(title.trim());
                self.out.push_str("\n\n");
            } else {
                self.out.push_str(&format!(
                    "[heading level=\"1\"]{}[/heading]\n",
                    escape_attr_inline(title.trim())
                ));
            }
        }
        for child_id in &node.children {
            self.emit_child(child_id, opts);
        }
    }

    fn emit_child(&mut self, id: &str, opts: &MarkdownOptions) {
        let Some(node) = self.tree.find(id) else {
            self.skipped += 1;
            return;
        };
        let node = node.clone();
        if !opts.include_invisible && !node.state.visible {
            self.skipped += 1;
            return;
        }
        if !opts.include_disabled && !node.state.enabled {
            self.skipped += 1;
            return;
        }
        self.emit_node(&node, opts);
    }

    fn emit_node(&mut self, node: &AgentNode, opts: &MarkdownOptions) {
        match node.structural_role {
            StructuralRole::Page => self.emit_page(node, opts),
            StructuralRole::Heading => self.emit_heading_node(node, opts),
            StructuralRole::Paragraph | StructuralRole::Text => self.emit_text_block(node, opts),
            StructuralRole::Link => self.emit_actionable(node, MarkdownTag::Link, opts),
            StructuralRole::Button => self.emit_actionable(node, MarkdownTag::Button, opts),
            StructuralRole::Textbox => self.emit_actionable(node, MarkdownTag::Textbox, opts),
            StructuralRole::Checkbox => self.emit_actionable(node, MarkdownTag::Checkbox, opts),
            StructuralRole::Radio => self.emit_actionable(node, MarkdownTag::Radio, opts),
            StructuralRole::Select => self.emit_select(node, opts),
            StructuralRole::Image => self.emit_actionable(node, MarkdownTag::Image, opts),
            StructuralRole::List => self.emit_list(node, opts),
            StructuralRole::ListItem => self.emit_list_item(node, opts),
            StructuralRole::Table => self.emit_table(node, opts),
            StructuralRole::Row | StructuralRole::Cell => self.emit_raw_children(node, opts),
            StructuralRole::Region => self.emit_region(node, opts),
            StructuralRole::Dialog => self.emit_dialog(node, opts),
            StructuralRole::Form => self.emit_form(node, opts),
            StructuralRole::Menu
            | StructuralRole::Tab
            | StructuralRole::Frame
            | StructuralRole::ShadowRoot => self.emit_region(node, opts),
            StructuralRole::Canvas => {
                let label = node.name.clone().unwrap_or_else(|| "(canvas)".into());
                self.out.push_str(&truncate(&label, opts.max_text_bytes));
                self.out.push('\n');
            }
            StructuralRole::Video => {
                let label = node.name.clone().unwrap_or_else(|| "(video)".into());
                let body = truncate(&label, opts.max_text_bytes);
                if Self::is_plain(opts) {
                    self.out.push_str(&format!("(video: {})\n", body));
                } else {
                    let start = self.out.len();
                    self.out.push_str("[video");
                    push_attr(&mut self.out, "id", &node.id);
                    self.out.push(']');
                    self.out.push_str(&body);
                    self.out.push_str("[/video]\n");
                    let end = self.out.len();
                    self.index.push(NodeSpan {
                        id: node.id.clone(),
                        tag: MarkdownTag::Region,
                        start,
                        end,
                        href: None,
                        name: node.name.clone(),
                    });
                }
            }
            StructuralRole::Unknown => self.emit_raw_children(node, opts),
        }
    }

    fn emit_heading_node(&mut self, node: &AgentNode, opts: &MarkdownOptions) {
        let text = node_text(node, opts);
        if text.is_empty() {
            self.skipped += 1;
            return;
        }
        if Self::is_plain(opts) {
            self.out.push_str(&text);
            self.out.push_str("\n\n");
        } else {
            let level = heading_level(node, opts.max_heading_level);
            self.out.push_str(&format!(
                "[heading level=\"{}\"]{}[/heading]\n",
                level,
                escape_attr_inline(&text)
            ));
        }
    }

    fn emit_text_block(&mut self, node: &AgentNode, opts: &MarkdownOptions) {
        let text = node_text(node, opts);
        if text.is_empty() {
            self.skipped += 1;
            return;
        }
        self.out.push_str(&text);
        self.out.push('\n');
    }

    fn emit_actionable(&mut self, node: &AgentNode, tag: MarkdownTag, opts: &MarkdownOptions) {
        if Self::is_plain(opts) {
            self.emit_actionable_plain(node, tag, opts);
            return;
        }
        let start = self.out.len();
        self.out.push('[');
        self.out.push_str(tag.as_str());
        push_attr(&mut self.out, "id", &node.id);
        emit_extra_attrs(&mut self.out, node, tag);
        self.out.push(']');
        let body_text = actionable_body_text(node, tag, opts);
        let empty_body = body_text.is_empty() || tag == MarkdownTag::Textbox;
        if !empty_body {
            self.out.push_str(&body_text);
            self.out.push_str("[/");
            self.out.push_str(tag.as_str());
            self.out.push(']');
        } else {
            self.out.push_str("[/");
            self.out.push_str(tag.as_str());
            self.out.push(']');
        }
        let end = self.out.len();
        self.out.push('\n');
        self.index.push(NodeSpan {
            id: node.id.clone(),
            tag,
            start,
            end,
            href: link_href(node),
            name: node.name.clone(),
        });
    }

    fn emit_actionable_plain(
        &mut self,
        node: &AgentNode,
        tag: MarkdownTag,
        opts: &MarkdownOptions,
    ) {
        let body_text = actionable_body_text(node, tag, opts);
        match tag {
            MarkdownTag::Link => {
                if !body_text.is_empty() {
                    self.out.push_str(&body_text);
                    self.out.push('\n');
                }
            }
            MarkdownTag::Button => {
                self.out.push_str(&body_text);
                if !node.state.enabled {
                    self.out.push_str(" (disabled)");
                }
                self.out.push('\n');
            }
            MarkdownTag::Textbox => {
                let name = node.name.as_deref().unwrap_or("input");
                if body_text.is_empty() {
                    self.out.push_str(&format!("(input: {})\n", name));
                } else {
                    self.out
                        .push_str(&format!("(input: {}) {}\n", name, body_text));
                }
            }
            MarkdownTag::Checkbox | MarkdownTag::Radio => {
                let mark = if node.state.selected { "[x]" } else { "[ ]" };
                self.out.push_str(&format!("{} {}\n", mark, body_text));
            }
            MarkdownTag::Image => {
                if !body_text.is_empty() {
                    self.out.push_str(&body_text);
                    self.out.push('\n');
                }
            }
            _ => {
                self.out.push_str(&body_text);
                self.out.push('\n');
            }
        }
        let _ = opts;
    }

    fn emit_select(&mut self, node: &AgentNode, opts: &MarkdownOptions) {
        if Self::is_plain(opts) {
            self.emit_select_plain(node, opts);
            return;
        }
        let start = self.out.len();
        self.out.push_str("[select");
        push_attr(&mut self.out, "id", &node.id);
        if let Some(name) = node.name.as_deref() {
            push_attr(&mut self.out, "name", name);
        }
        if !node.state.enabled {
            push_attr(&mut self.out, "disabled", "true");
        }
        self.out.push_str("]\n");
        for child_id in &node.children {
            let Some(child) = self.tree.find(child_id) else {
                continue;
            };
            let child = child.clone();
            if !opts.include_invisible && !child.state.visible {
                continue;
            }
            let option_text = node_text(&child, opts);
            let value = match &child.value {
                Some(AgentValue::Text(s)) => Some(s.clone()),
                Some(AgentValue::Url(s)) => Some(s.clone()),
                _ => None,
            };
            let option_start = self.out.len();
            self.out.push_str("  [option");
            push_attr(&mut self.out, "id", &child.id);
            if let Some(v) = value.as_deref() {
                push_attr(&mut self.out, "value", v);
            }
            if child.state.selected {
                push_attr(&mut self.out, "selected", "true");
            }
            self.out.push(']');
            let body = if !option_text.is_empty() {
                option_text
            } else {
                value.clone().unwrap_or_default()
            };
            self.out.push_str(&body);
            self.out.push_str("[/option]");
            let option_end = self.out.len();
            self.out.push('\n');
            self.index.push(NodeSpan {
                id: child.id.clone(),
                tag: MarkdownTag::Option,
                start: option_start,
                end: option_end,
                href: None,
                name: child.name.clone(),
            });
        }
        self.out.push_str("[/select]");
        let end = self.out.len();
        self.out.push('\n');
        self.index.push(NodeSpan {
            id: node.id.clone(),
            tag: MarkdownTag::Select,
            start,
            end,
            href: None,
            name: node.name.clone(),
        });
    }

    fn emit_select_plain(&mut self, node: &AgentNode, opts: &MarkdownOptions) {
        let label = node.name.clone().unwrap_or_else(|| "(select)".into());
        let mut selected_text: Option<String> = None;
        let mut option_count = 0usize;
        for child_id in &node.children {
            let Some(child) = self.tree.find(child_id) else {
                continue;
            };
            if !opts.include_invisible && !child.state.visible {
                continue;
            }
            option_count += 1;
            let text = node_text(child, opts);
            let value = match &child.value {
                Some(AgentValue::Text(s)) => Some(s.clone()),
                Some(AgentValue::Url(s)) => Some(s.clone()),
                _ => None,
            };
            if child.state.selected {
                let body = if !text.is_empty() {
                    text
                } else {
                    value.unwrap_or_default()
                };
                selected_text = Some(body);
            }
        }
        match selected_text {
            Some(sel) => {
                self.out.push_str(&format!("({}: {})\n", label, sel));
            }
            None => {
                if option_count == 0 {
                    self.out.push_str(&format!("({}: (empty))\n", label));
                } else {
                    self.out
                        .push_str(&format!("({}: {} options)\n", label, option_count));
                }
            }
        }
    }

    fn emit_list(&mut self, node: &AgentNode, opts: &MarkdownOptions) {
        self.list_depth += 1;
        for child_id in &node.children {
            self.emit_child(child_id, opts);
        }
        self.list_depth -= 1;
    }

    fn emit_list_item(&mut self, node: &AgentNode, opts: &MarkdownOptions) {
        for _ in 0..self.list_depth {
            self.out.push_str("  ");
        }
        self.out.push_str("- ");
        let leading_len = self.out.len();
        for child_id in &node.children {
            self.emit_child(child_id, opts);
        }
        // Collapse newlines so the item stays on one logical line.
        let body = self.out[leading_len..]
            .replace('\n', " ")
            .trim()
            .to_string();
        self.out.truncate(leading_len);
        self.out.push_str(&body);
        self.out.push('\n');
    }

    fn emit_table(&mut self, node: &AgentNode, opts: &MarkdownOptions) {
        if Self::is_plain(opts) {
            self.emit_table_plain(node, opts);
            return;
        }
        let rows: Vec<AgentNode> = node
            .children
            .iter()
            .filter_map(|id| self.tree.find(id).cloned())
            .filter(|n| n.structural_role == StructuralRole::Row)
            .collect();
        let (header, body_rows) = match rows.split_first() {
            Some((first, rest)) => (first.clone(), rest.to_vec()),
            None => {
                self.out.push_str(&format!(
                    "[table id=\"{}\"]empty[/table]\n",
                    escape_attr_inline(&node.id)
                ));
                return;
            }
        };
        let column_count = header.children.len().max(1);
        let header_cells = self.render_row_cells(&header, opts);
        self.out.push_str("| ");
        for (i, cell) in header_cells.iter().enumerate() {
            if i > 0 {
                self.out.push_str(" | ");
            }
            self.out.push_str(cell);
        }
        self.out.push_str(" |\n|");
        for _ in 0..column_count {
            self.out.push_str(" --- |");
        }
        self.out.push('\n');
        for row in &body_rows {
            let cells = self.render_row_cells(row, opts);
            self.out.push_str("| ");
            for (i, cell) in cells.iter().enumerate() {
                if i > 0 {
                    self.out.push_str(" | ");
                }
                self.out.push_str(cell);
            }
            self.out.push_str(" |\n");
        }
        self.out.push('\n');
    }

    fn emit_table_plain(&mut self, node: &AgentNode, opts: &MarkdownOptions) {
        let rows: Vec<AgentNode> = node
            .children
            .iter()
            .filter_map(|id| self.tree.find(id).cloned())
            .filter(|n| n.structural_role == StructuralRole::Row)
            .collect();
        for row in &rows {
            let cells = self.render_row_cells(row, opts);
            self.out.push_str(&cells.join(" | "));
            self.out.push('\n');
        }
        if !rows.is_empty() {
            self.out.push('\n');
        }
    }

    fn render_row_cells(&mut self, row: &AgentNode, opts: &MarkdownOptions) -> Vec<String> {
        let mut cells = Vec::new();
        for child_id in &row.children {
            let Some(cell) = self.tree.find(child_id).cloned() else {
                cells.push(String::new());
                continue;
            };
            if cell.structural_role == StructuralRole::Cell {
                let before = self.out.len();
                for grandchild in &cell.children {
                    self.emit_child(grandchild, opts);
                }
                let raw = self.out[before..].to_string();
                self.out.truncate(before);
                let cleaned = raw.replace('\n', " ").trim().to_string();
                let text = if cleaned.is_empty() {
                    node_text(&cell, opts)
                } else {
                    cleaned
                };
                cells.push(escape_pipes(&text));
            } else {
                cells.push(escape_pipes(&node_text(&cell, opts)));
            }
        }
        cells
    }

    fn emit_region(&mut self, node: &AgentNode, opts: &MarkdownOptions) {
        if Self::is_plain(opts) {
            if node.semantic_role == Some(SemanticRole::Challenge) {
                let provider = node.description.as_deref().unwrap_or("unknown");
                self.out.push_str(&format!("(challenge: {})\n", provider));
            }
            for child_id in &node.children {
                self.emit_child(child_id, opts);
            }
            return;
        }
        let start = self.out.len();
        self.out.push_str("[region");
        push_attr(&mut self.out, "id", &node.id);
        emit_extra_attrs(&mut self.out, node, MarkdownTag::Region);
        self.out.push_str("]\n");
        for child_id in &node.children {
            self.emit_child(child_id, opts);
        }
        self.out.push_str("[/region]");
        let end = self.out.len();
        self.out.push('\n');
        self.index.push(NodeSpan {
            id: node.id.clone(),
            tag: MarkdownTag::Region,
            start,
            end,
            href: None,
            name: node.name.clone(),
        });
    }

    fn emit_dialog(&mut self, node: &AgentNode, opts: &MarkdownOptions) {
        if Self::is_plain(opts) {
            if node.semantic_role == Some(SemanticRole::Challenge) {
                let provider = node.description.as_deref().unwrap_or("unknown");
                self.out.push_str(&format!("(challenge: {})\n", provider));
            }
            if let Some(name) = node.name.as_deref() {
                self.out.push_str(&format!("(dialog: {})\n", name));
            }
            for child_id in &node.children {
                self.emit_child(child_id, opts);
            }
            return;
        }
        let start = self.out.len();
        self.out.push_str("[dialog");
        push_attr(&mut self.out, "id", &node.id);
        emit_extra_attrs(&mut self.out, node, MarkdownTag::Dialog);
        self.out.push_str("]\n");
        for child_id in &node.children {
            self.emit_child(child_id, opts);
        }
        self.out.push_str("[/dialog]");
        let end = self.out.len();
        self.out.push('\n');
        self.index.push(NodeSpan {
            id: node.id.clone(),
            tag: MarkdownTag::Dialog,
            start,
            end,
            href: None,
            name: node.name.clone(),
        });
    }

    fn emit_form(&mut self, node: &AgentNode, opts: &MarkdownOptions) {
        self.emit_raw_children(node, opts);
    }

    fn emit_raw_children(&mut self, node: &AgentNode, opts: &MarkdownOptions) {
        for child_id in &node.children {
            self.emit_child(child_id, opts);
        }
    }
}

fn emit_extra_attrs(out: &mut String, node: &AgentNode, tag: MarkdownTag) {
    if let Some(name) = node.name.as_deref() {
        if !matches!(
            tag,
            MarkdownTag::Region | MarkdownTag::Dialog | MarkdownTag::Select
        ) {
            push_attr(out, "name", name);
        }
    }
    match tag {
        MarkdownTag::Link => {
            if let Some(url) = link_href(node) {
                push_attr(out, "href", &url);
            }
        }
        MarkdownTag::Button => {
            if !node.state.enabled {
                push_attr(out, "disabled", "true");
            }
        }
        MarkdownTag::Textbox => {
            if let Some(placeholder) = placeholder(node) {
                push_attr(out, "placeholder", &placeholder);
            }
            if !node.state.enabled {
                push_attr(out, "disabled", "true");
            }
        }
        MarkdownTag::Checkbox | MarkdownTag::Radio => {
            if !node.state.enabled {
                push_attr(out, "disabled", "true");
            }
            if node.state.selected {
                push_attr(out, "checked", "true");
            }
        }
        MarkdownTag::Image => {
            if let Some(src) = image_src(node) {
                push_attr(out, "src", &src);
            }
        }
        _ => {}
    }
    // Surface the semantic challenge marker regardless of structural role,
    // so a challenge rendered as a region/div still carries provider info.
    if node.semantic_role == Some(SemanticRole::Challenge) {
        let provider = node.description.as_deref().unwrap_or("unknown");
        push_attr(out, "provider", provider);
    }
}

fn link_href(node: &AgentNode) -> Option<String> {
    if let Some(AgentValue::Url(u)) = &node.value {
        return Some(u.clone());
    }
    if let Some(description) = node.description.as_deref() {
        if description.starts_with("http://") || description.starts_with("https://") {
            return Some(description.to_string());
        }
    }
    None
}

fn placeholder(node: &AgentNode) -> Option<String> {
    node.description
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn image_src(node: &AgentNode) -> Option<String> {
    if let Some(AgentValue::Url(u)) = &node.value {
        return Some(u.clone());
    }
    None
}

fn node_text(node: &AgentNode, opts: &MarkdownOptions) -> String {
    let raw = match &node.value {
        Some(AgentValue::Text(s)) => s.clone(),
        Some(AgentValue::Url(s)) => s.clone(),
        Some(AgentValue::SecretReference(_)) => "(secret)".to_string(),
        Some(AgentValue::Number(n)) => n.to_string(),
        Some(AgentValue::Boolean(b)) => b.to_string(),
        None => node.name.clone().unwrap_or_default(),
    };
    truncate(&raw, opts.max_text_bytes)
}

/// Body text for actionable widgets. Links/images use the accessible
/// name (not the URL value, which is surfaced as `href=`/`src=` instead).
/// Inputs/textareas use the current value from `node.value` so the LLM
/// sees what's typed in. Checkboxes/radios prefer the label so the LLM
/// reads what the toggle applies to.
fn actionable_body_text(node: &AgentNode, tag: MarkdownTag, opts: &MarkdownOptions) -> String {
    let raw = match tag {
        MarkdownTag::Link | MarkdownTag::Image => node.name.clone().unwrap_or_default(),
        MarkdownTag::Textbox => match &node.value {
            Some(AgentValue::Text(s)) => s.clone(),
            Some(AgentValue::Number(n)) => n.to_string(),
            _ => node.name.clone().unwrap_or_default(),
        },
        MarkdownTag::Checkbox | MarkdownTag::Radio => match &node.value {
            Some(AgentValue::Text(s)) if !s.is_empty() => s.clone(),
            _ => node.name.clone().unwrap_or_default(),
        },
        _ => node_text(node, opts),
    };
    truncate(&raw, opts.max_text_bytes)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut cut = max;
    while !s.is_char_boundary(cut) && cut > 0 {
        cut -= 1;
    }
    let mut out = String::with_capacity(cut + 3);
    out.push_str(&s[..cut]);
    out.push_str("...");
    out
}

fn push_attr(out: &mut String, key: &str, value: &str) {
    out.push(' ');
    out.push_str(key);
    out.push_str("=\"");
    push_attr_escaped(out, value);
    out.push('"');
}

fn escape_attr_inline(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    push_attr_escaped(&mut out, s);
    out
}

fn push_attr_escaped(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("&quot;"),
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

fn escape_pipes(s: &str) -> String {
    s.replace('|', "\\|")
}

fn heading_level(node: &AgentNode, max: u8) -> u8 {
    if let Some(desc) = node.description.as_deref() {
        if let Some(stripped) = desc.strip_prefix('H').or_else(|| desc.strip_prefix('h')) {
            if let Ok(n) = stripped.parse::<u8>() {
                return n.max(1).min(max.max(1));
            }
        }
    }
    if let Some(geom) = &node.geometry {
        let approx = ((geom.y / 64.0).floor() as i32 + 1).max(1) as u8;
        return approx.min(max.max(1));
    }
    2
}
