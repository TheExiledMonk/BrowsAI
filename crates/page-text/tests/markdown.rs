use browsai_agent_tree::{
    AgentNode, AgentRenderTree, AgentValue, Geometry, NodeState, SemanticRole, StructuralRole,
};
use browsai_page_text::{MarkdownEmitter, MarkdownOptions, MarkdownTag, PageTextFormat};
use browsai_provenance::Confidence;

fn node(
    id: &str,
    role: StructuralRole,
    name: Option<&str>,
    value: Option<AgentValue>,
    state: NodeState,
    children: Vec<&str>,
) -> AgentNode {
    let state = NodeState {
        visible: true,
        enabled: true,
        ..state
    };
    AgentNode {
        id: id.into(),
        origin: None,
        identity_key: None,
        structural_role: role,
        semantic_role: None,
        application_type: None,
        name: name.map(|s| s.into()),
        value,
        description: None,
        state,
        geometry: None,
        relationships: vec![],
        actions: vec![],
        children: children.into_iter().map(String::from).collect(),
        provenance: vec![],
        confidence: Confidence::DIRECT,
        generation: 0,
    }
}

fn append(tree: &mut AgentRenderTree, node: AgentNode) {
    tree.nodes.push(node);
}

#[test]
fn page_title_becomes_level_one_heading() {
    let tree = AgentRenderTree::new_page("Example Domain");
    let opts = MarkdownOptions::default();
    let view = MarkdownEmitter::new(&tree).emit(&opts);
    assert!(
        view.content
            .contains("[heading level=\"1\"]Example Domain[/heading]"),
        "got: {}",
        view.content
    );
    assert!(view
        .content
        .starts_with("<page-content trust=\"untrusted\">"));
    assert!(view.content.ends_with("</page-content>"));
}

#[test]
fn link_with_href_emits_standard_markdown() {
    let mut tree = AgentRenderTree::new_page("Example");
    let link = node(
        "link-1",
        StructuralRole::Link,
        Some("More information"),
        Some(AgentValue::Url("https://example.test/about".into())),
        NodeState::default(),
        vec![],
    );
    append(&mut tree, link);
    tree.nodes[0].children.push("link-1".into());
    let view = MarkdownEmitter::new(&tree).emit(&MarkdownOptions::default());
    let expected = "[More information](https://example.test/about)";
    assert!(view.content.contains(expected), "got: {}", view.content);
    assert!(
        !view.content.contains("[link"),
        "no BB-code fallback when href present: {}",
        view.content
    );
    let span = view
        .node_index
        .iter()
        .find(|span| span.id == "link-1")
        .expect("link span indexed");
    assert_eq!(span.tag, MarkdownTag::Link);
    assert_eq!(span.href.as_deref(), Some("https://example.test/about"));
}

#[test]
fn link_without_href_falls_back_to_bb_code() {
    let mut tree = AgentRenderTree::new_page("Example");
    let link = node(
        "link-1",
        StructuralRole::Link,
        Some("Anchor"),
        None,
        NodeState::default(),
        vec![],
    );
    append(&mut tree, link);
    tree.nodes[0].children.push("link-1".into());
    let view = MarkdownEmitter::new(&tree).emit(&MarkdownOptions::default());
    assert!(
        view.content.contains("[link id=\"link-1\""),
        "got: {}",
        view.content
    );
    assert!(view.content.contains("Anchor"));
}

#[test]
fn image_with_src_emits_standard_markdown() {
    let mut tree = AgentRenderTree::new_page("Page");
    let img = node(
        "img-1",
        StructuralRole::Image,
        Some("A logo"),
        Some(AgentValue::Url("https://example.test/logo.png".into())),
        NodeState::default(),
        vec![],
    );
    append(&mut tree, img);
    tree.nodes[0].children.push("img-1".into());
    let view = MarkdownEmitter::new(&tree).emit(&MarkdownOptions::default());
    let expected = "![A logo](https://example.test/logo.png)";
    assert!(view.content.contains(expected), "got: {}", view.content);
}

#[test]
fn textbox_with_empty_value_is_self_closing() {
    let mut tree = AgentRenderTree::new_page("Search");
    let textbox = node(
        "tb-1",
        StructuralRole::Textbox,
        Some("Search box"),
        None,
        NodeState::default(),
        vec![],
    );
    append(&mut tree, textbox);
    tree.nodes[0].children.push("tb-1".into());
    let view = MarkdownEmitter::new(&tree).emit(&MarkdownOptions::default());
    assert!(
        view.content
            .contains("[textbox id=\"tb-1\" name=\"Search box\"][/textbox]"),
        "got: {}",
        view.content
    );
}

#[test]
fn disabled_button_emits_disabled_attribute() {
    let mut tree = AgentRenderTree::new_page("Form");
    let mut button = node(
        "btn-1",
        StructuralRole::Button,
        Some("Submit"),
        None,
        NodeState::default(),
        vec![],
    );
    button.state.enabled = false;
    append(&mut tree, button);
    tree.nodes[0].children.push("btn-1".into());
    let view = MarkdownEmitter::new(&tree).emit(&MarkdownOptions::default());
    assert!(
        view.content
            .contains("[button id=\"btn-1\" name=\"Submit\" disabled=\"true\"]Submit[/button]"),
        "got: {}",
        view.content
    );
}

#[test]
fn invisible_nodes_are_skipped_by_default() {
    let mut tree = AgentRenderTree::new_page("Example");
    let mut hidden = node(
        "p-hidden",
        StructuralRole::Paragraph,
        Some("hidden text"),
        None,
        NodeState::default(),
        vec![],
    );
    hidden.state.visible = false;
    append(&mut tree, hidden);
    tree.nodes[0].children.push("p-hidden".into());
    let view = MarkdownEmitter::new(&tree).emit(&MarkdownOptions::default());
    assert!(!view.content.contains("hidden text"));
    assert_eq!(view.skipped_nodes, 1);
}

#[test]
fn invisible_nodes_appear_when_opted_in() {
    let mut tree = AgentRenderTree::new_page("Example");
    let mut hidden = node(
        "p-hidden",
        StructuralRole::Paragraph,
        Some("hidden text"),
        None,
        NodeState::default(),
        vec![],
    );
    hidden.state.visible = false;
    append(&mut tree, hidden);
    tree.nodes[0].children.push("p-hidden".into());
    let opts = MarkdownOptions {
        include_invisible: true,
        ..Default::default()
    };
    let view = MarkdownEmitter::new(&tree).emit(&opts);
    assert!(view.content.contains("hidden text"));
}

#[test]
fn select_with_options_emits_nested_option_tags() {
    let mut tree = AgentRenderTree::new_page("Region");
    let mut opt1 = node(
        "opt-1",
        StructuralRole::Text,
        None,
        Some(AgentValue::Text("US".into())),
        NodeState {
            selected: true,
            ..Default::default()
        },
        vec![],
    );
    opt1.structural_role = StructuralRole::Text;
    let opt2 = node(
        "opt-2",
        StructuralRole::Text,
        None,
        Some(AgentValue::Text("CA".into())),
        NodeState::default(),
        vec![],
    );
    append(&mut tree, opt1);
    append(&mut tree, opt2);
    let mut select = node(
        "sel-1",
        StructuralRole::Select,
        Some("region"),
        None,
        NodeState::default(),
        vec!["opt-1", "opt-2"],
    );
    select.state.enabled = true;
    append(&mut tree, select);
    tree.nodes[0].children.push("sel-1".into());
    let view = MarkdownEmitter::new(&tree).emit(&MarkdownOptions::default());
    assert!(
        view.content
            .contains("[select id=\"sel-1\" name=\"region\"]"),
        "got: {}",
        view.content
    );
    assert!(
        view.content
            .contains("[option id=\"opt-1\" value=\"US\" selected=\"true\"]US[/option]"),
        "got: {}",
        view.content
    );
    assert!(
        view.content
            .contains("[option id=\"opt-2\" value=\"CA\"]CA[/option]"),
        "got: {}",
        view.content
    );
}

#[test]
fn list_items_are_indented_and_collapsed() {
    let mut tree = AgentRenderTree::new_page("List");
    let link = node(
        "link-1",
        StructuralRole::Link,
        Some("First"),
        Some(AgentValue::Url("/first".into())),
        NodeState::default(),
        vec![],
    );
    let link2 = node(
        "link-2",
        StructuralRole::Link,
        Some("Second"),
        Some(AgentValue::Url("/second".into())),
        NodeState::default(),
        vec![],
    );
    let li1 = node(
        "li-1",
        StructuralRole::ListItem,
        None,
        None,
        NodeState::default(),
        vec!["link-1"],
    );
    let li2 = node(
        "li-2",
        StructuralRole::ListItem,
        None,
        None,
        NodeState::default(),
        vec!["link-2"],
    );
    let list = node(
        "list-1",
        StructuralRole::List,
        None,
        None,
        NodeState::default(),
        vec!["li-1", "li-2"],
    );
    for n in [link, link2, li1, li2, list] {
        append(&mut tree, n);
    }
    tree.nodes[0].children.push("list-1".into());
    let view = MarkdownEmitter::new(&tree).emit(&MarkdownOptions::default());
    assert!(
        view.content.contains("- [First](/first)"),
        "got: {}",
        view.content
    );
    assert!(
        view.content.contains("- [Second](/second)"),
        "got: {}",
        view.content
    );
}

#[test]
fn table_emits_markdown_table_with_separator() {
    let mut tree = AgentRenderTree::new_page("Table");
    let mut cells_a = vec![];
    let mut cells_b = vec![];
    let cell_a = node(
        "cell-a",
        StructuralRole::Cell,
        Some("Name"),
        None,
        NodeState::default(),
        vec![],
    );
    let cell_b = node(
        "cell-b",
        StructuralRole::Cell,
        Some("Price"),
        None,
        NodeState::default(),
        vec![],
    );
    cells_a.push(cell_a);
    cells_a.push(cell_b.clone());
    cells_b.push(node(
        "cell-c",
        StructuralRole::Cell,
        Some("Widget"),
        None,
        NodeState::default(),
        vec![],
    ));
    cells_b.push(node(
        "cell-d",
        StructuralRole::Cell,
        Some("$5"),
        None,
        NodeState::default(),
        vec![],
    ));
    append(&mut tree, cell_b);
    let row_h = node(
        "row-h",
        StructuralRole::Row,
        None,
        None,
        NodeState::default(),
        vec!["cell-a", "cell-b"],
    );
    let row_1 = node(
        "row-1",
        StructuralRole::Row,
        None,
        None,
        NodeState::default(),
        vec!["cell-c", "cell-d"],
    );
    for n in cells_a.into_iter().chain(cells_b) {
        if !tree.nodes.iter().any(|existing| existing.id == n.id) {
            append(&mut tree, n);
        }
    }
    append(&mut tree, row_h);
    append(&mut tree, row_1);
    let table = node(
        "tbl-1",
        StructuralRole::Table,
        None,
        None,
        NodeState::default(),
        vec!["row-h", "row-1"],
    );
    append(&mut tree, table);
    tree.nodes[0].children.push("tbl-1".into());
    let view = MarkdownEmitter::new(&tree).emit(&MarkdownOptions::default());
    assert!(
        view.content.contains("| Name | Price |"),
        "got: {}",
        view.content
    );
    assert!(
        view.content.contains("| --- | --- |"),
        "got: {}",
        view.content
    );
    assert!(
        view.content.contains("| Widget | $5 |"),
        "got: {}",
        view.content
    );
}

#[test]
fn raw_option_strips_envelope() {
    let tree = AgentRenderTree::new_page("X");
    let opts = MarkdownOptions {
        raw: true,
        ..Default::default()
    };
    let view = MarkdownEmitter::new(&tree).emit(&opts);
    assert!(!view.content.contains("<page-content"));
    assert!(!view.content.contains("</page-content>"));
}

#[test]
fn heading_level_uses_description_when_present() {
    let mut tree = AgentRenderTree::new_page("Page");
    let mut h = node(
        "h-1",
        StructuralRole::Heading,
        Some("Sub"),
        None,
        NodeState::default(),
        vec![],
    );
    h.description = Some("H3".into());
    append(&mut tree, h);
    tree.nodes[0].children.push("h-1".into());
    let view = MarkdownEmitter::new(&tree).emit(&MarkdownOptions::default());
    assert!(
        view.content.contains("[heading level=\"3\"]Sub[/heading]"),
        "got: {}",
        view.content
    );
}

#[test]
fn challenge_node_surfaces_provider_attribute() {
    let mut tree = AgentRenderTree::new_page("Page");
    let mut challenge = node(
        "ch-1",
        StructuralRole::Region,
        Some("Verify you are human"),
        None,
        NodeState::default(),
        vec![],
    );
    challenge.semantic_role = Some(SemanticRole::Challenge);
    append(&mut tree, challenge);
    tree.nodes[0].children.push("ch-1".into());
    let view = MarkdownEmitter::new(&tree).emit(&MarkdownOptions::default());
    assert!(
        view.content.contains("provider=\"unknown\""),
        "got: {}",
        view.content
    );
}

#[test]
fn spans_track_byte_offsets_into_envelope() {
    let mut tree = AgentRenderTree::new_page("Title");
    let link = node(
        "link-1",
        StructuralRole::Link,
        Some("A"),
        Some(AgentValue::Url("/a".into())),
        NodeState::default(),
        vec![],
    );
    append(&mut tree, link);
    tree.nodes[0].children.push("link-1".into());
    let view = MarkdownEmitter::new(&tree).emit(&MarkdownOptions::default());
    let span = view
        .node_index
        .iter()
        .find(|s| s.id == "link-1")
        .expect("span");
    let slice = &view.content[span.start..span.end];
    assert!(slice.starts_with("[A](/a)"), "slice was: {slice}");
    assert!(slice.ends_with("](/a)"), "slice was: {slice}");
}

#[test]
fn long_text_is_truncated_with_ellipsis() {
    let mut tree = AgentRenderTree::new_page("P");
    let long = "a".repeat(2000);
    let p = node(
        "p-1",
        StructuralRole::Paragraph,
        None,
        Some(AgentValue::Text(long)),
        NodeState::default(),
        vec![],
    );
    append(&mut tree, p);
    tree.nodes[0].children.push("p-1".into());
    let opts = MarkdownOptions {
        max_text_bytes: 50,
        ..Default::default()
    };
    let view = MarkdownEmitter::new(&tree).emit(&opts);
    assert!(view.content.contains("..."));
}

#[test]
fn geometry_drives_heading_level_fallback() {
    let mut tree = AgentRenderTree::new_page("P");
    let mut h = node(
        "h-1",
        StructuralRole::Heading,
        Some("Far down"),
        None,
        NodeState::default(),
        vec![],
    );
    h.geometry = Some(Geometry {
        x: 0.0,
        y: 256.0,
        width: 200.0,
        height: 24.0,
    });
    append(&mut tree, h);
    tree.nodes[0].children.push("h-1".into());
    let view = MarkdownEmitter::new(&tree).emit(&MarkdownOptions::default());
    assert!(
        view.content.contains("[heading level=\"5\"]"),
        "got: {}",
        view.content
    );
}

#[test]
fn plain_format_strips_bb_code_tags() {
    let mut tree = AgentRenderTree::new_page("Example");
    let link = node(
        "link-1",
        StructuralRole::Link,
        Some("More info"),
        Some(AgentValue::Url("/about".into())),
        NodeState::default(),
        vec![],
    );
    append(&mut tree, link);
    tree.nodes[0].children.push("link-1".into());
    let opts = MarkdownOptions {
        format: PageTextFormat::Plain,
        ..Default::default()
    };
    let view = MarkdownEmitter::new(&tree).emit(&opts);
    assert!(!view.content.contains("[link"), "got: {}", view.content);
    assert!(view.content.contains("More info"), "got: {}", view.content);
    assert!(
        view.content.contains("<page-content"),
        "envelope still present"
    );
}

#[test]
fn plain_format_renders_textbox_as_placeholder() {
    let mut tree = AgentRenderTree::new_page("Form");
    let tb = node(
        "tb-1",
        StructuralRole::Textbox,
        Some("email"),
        None,
        NodeState::default(),
        vec![],
    );
    append(&mut tree, tb);
    tree.nodes[0].children.push("tb-1".into());
    let opts = MarkdownOptions {
        format: PageTextFormat::Plain,
        ..Default::default()
    };
    let view = MarkdownEmitter::new(&tree).emit(&opts);
    assert!(
        view.content.contains("(input: email)"),
        "got: {}",
        view.content
    );
    assert!(!view.content.contains("[textbox"));
}

#[test]
fn plain_format_renders_checkbox_with_mark() {
    let mut tree = AgentRenderTree::new_page("Form");
    let mut cb = node(
        "cb-1",
        StructuralRole::Checkbox,
        Some("Subscribe"),
        Some(AgentValue::Boolean(true)),
        NodeState::default(),
        vec![],
    );
    cb.state.selected = true;
    append(&mut tree, cb);
    tree.nodes[0].children.push("cb-1".into());
    let opts = MarkdownOptions {
        format: PageTextFormat::Plain,
        ..Default::default()
    };
    let view = MarkdownEmitter::new(&tree).emit(&opts);
    assert!(
        view.content.contains("[x] Subscribe"),
        "got: {}",
        view.content
    );
}

#[test]
fn plain_format_renders_select_with_selected_value() {
    let mut tree = AgentRenderTree::new_page("Form");
    let opt1 = node(
        "opt-1",
        StructuralRole::Text,
        None,
        Some(AgentValue::Text("US".into())),
        NodeState {
            selected: true,
            ..Default::default()
        },
        vec![],
    );
    let opt2 = node(
        "opt-2",
        StructuralRole::Text,
        None,
        Some(AgentValue::Text("CA".into())),
        NodeState::default(),
        vec![],
    );
    append(&mut tree, opt1);
    append(&mut tree, opt2);
    let mut select = node(
        "sel-1",
        StructuralRole::Select,
        Some("region"),
        None,
        NodeState::default(),
        vec!["opt-1", "opt-2"],
    );
    select.state.enabled = true;
    append(&mut tree, select);
    tree.nodes[0].children.push("sel-1".into());
    let opts = MarkdownOptions {
        format: PageTextFormat::Plain,
        ..Default::default()
    };
    let view = MarkdownEmitter::new(&tree).emit(&opts);
    assert!(
        view.content.contains("(region: US)"),
        "got: {}",
        view.content
    );
    assert!(!view.content.contains("[select"));
    assert!(!view.content.contains("[option"));
}

#[test]
fn plain_format_surfaces_challenge_marker() {
    let mut tree = AgentRenderTree::new_page("Page");
    let mut challenge = node(
        "ch-1",
        StructuralRole::Region,
        Some("Verify you are human"),
        None,
        NodeState::default(),
        vec![],
    );
    challenge.semantic_role = Some(SemanticRole::Challenge);
    challenge.description = Some("hcaptcha".into());
    append(&mut tree, challenge);
    tree.nodes[0].children.push("ch-1".into());
    let opts = MarkdownOptions {
        format: PageTextFormat::Plain,
        ..Default::default()
    };
    let view = MarkdownEmitter::new(&tree).emit(&opts);
    assert!(
        view.content.contains("(challenge: hcaptcha)"),
        "got: {}",
        view.content
    );
    assert!(!view.content.contains("[region"));
}

#[test]
fn plain_format_renders_table_without_separator() {
    let mut tree = AgentRenderTree::new_page("Page");
    let cell_a = node(
        "cell-a",
        StructuralRole::Cell,
        Some("Name"),
        None,
        NodeState::default(),
        vec![],
    );
    let cell_b = node(
        "cell-b",
        StructuralRole::Cell,
        Some("Price"),
        None,
        NodeState::default(),
        vec![],
    );
    let cell_c = node(
        "cell-c",
        StructuralRole::Cell,
        Some("Widget"),
        None,
        NodeState::default(),
        vec![],
    );
    let cell_d = node(
        "cell-d",
        StructuralRole::Cell,
        Some("$5"),
        None,
        NodeState::default(),
        vec![],
    );
    let row_h = node(
        "row-h",
        StructuralRole::Row,
        None,
        None,
        NodeState::default(),
        vec!["cell-a", "cell-b"],
    );
    let row_1 = node(
        "row-1",
        StructuralRole::Row,
        None,
        None,
        NodeState::default(),
        vec!["cell-c", "cell-d"],
    );
    append(&mut tree, cell_a);
    append(&mut tree, cell_b);
    append(&mut tree, cell_c);
    append(&mut tree, cell_d);
    append(&mut tree, row_h);
    append(&mut tree, row_1);
    let table = node(
        "tbl-1",
        StructuralRole::Table,
        None,
        None,
        NodeState::default(),
        vec!["row-h", "row-1"],
    );
    append(&mut tree, table);
    tree.nodes[0].children.push("tbl-1".into());
    let opts = MarkdownOptions {
        format: PageTextFormat::Plain,
        ..Default::default()
    };
    let view = MarkdownEmitter::new(&tree).emit(&opts);
    assert!(
        view.content.contains("Name | Price"),
        "got: {}",
        view.content
    );
    assert!(
        view.content.contains("Widget | $5"),
        "got: {}",
        view.content
    );
    assert!(
        !view.content.contains("---"),
        "no markdown separator in plain mode"
    );
}

#[test]
fn plain_format_node_index_is_empty() {
    let mut tree = AgentRenderTree::new_page("Page");
    let link = node(
        "link-1",
        StructuralRole::Link,
        Some("X"),
        Some(AgentValue::Url("/".into())),
        NodeState::default(),
        vec![],
    );
    append(&mut tree, link);
    tree.nodes[0].children.push("link-1".into());
    let opts = MarkdownOptions {
        format: PageTextFormat::Plain,
        ..Default::default()
    };
    let view = MarkdownEmitter::new(&tree).emit(&opts);
    assert!(view.node_index.is_empty());
}
