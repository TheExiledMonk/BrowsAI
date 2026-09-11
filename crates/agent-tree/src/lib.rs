use browsai_provenance::{Confidence, ProvenanceSource};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type AgentNodeId = String;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StructuralRole {
    Page,
    Region,
    Heading,
    Paragraph,
    Text,
    Button,
    Link,
    Textbox,
    Checkbox,
    Radio,
    Select,
    List,
    ListItem,
    Table,
    Row,
    Cell,
    Dialog,
    Menu,
    Tab,
    Image,
    Canvas,
    Video,
    Form,
    Frame,
    ShadowRoot,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SemanticRole {
    SearchField,
    SubmitAction,
    LoginAction,
    LogoutAction,
    Pagination,
    PrimaryNavigation,
    AccountMenu,
    ConfirmationDialog,
    FilePicker,
    PaymentForm,
    DatePicker,
    SearchResults,
    Challenge,
    ProfileInconsistency,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ApplicationType {
    Customer,
    Invoice,
    Order,
    Message,
    Conversation,
    Payment,
    Property,
    Lead,
    Document,
    Project,
    Task,
    Account,
    User,
    Product,
    Reservation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AgentValue {
    Text(String),
    Number(f64),
    Boolean(bool),
    Url(String),
    SecretReference(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct NodeState {
    pub visible: bool,
    pub enabled: bool,
    pub focused: bool,
    pub selected: bool,
    pub expanded: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Geometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Relationship {
    pub kind: String,
    pub target: AgentNodeId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActionDescriptor {
    pub name: String,
    pub consequence: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentNode {
    pub id: AgentNodeId,
    pub origin: Option<String>,
    /// Stable conceptual identity, independent of a transient engine/DOM ID.
    pub identity_key: Option<String>,
    pub structural_role: StructuralRole,
    pub semantic_role: Option<SemanticRole>,
    pub application_type: Option<ApplicationType>,
    pub name: Option<String>,
    pub value: Option<AgentValue>,
    pub description: Option<String>,
    pub state: NodeState,
    pub geometry: Option<Geometry>,
    pub relationships: Vec<Relationship>,
    pub actions: Vec<ActionDescriptor>,
    pub children: Vec<AgentNodeId>,
    pub provenance: Vec<ProvenanceSource>,
    pub confidence: Confidence,
    pub generation: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentRenderTree {
    pub root: AgentNodeId,
    pub nodes: Vec<AgentNode>,
    pub generation: u64,
    /// True when the engine stopped DOM projection at its safety limit.
    #[serde(default)]
    pub truncated: bool,
}

impl AgentRenderTree {
    pub fn new_page(name: impl Into<String>) -> Self {
        let id = format!("page:{}", Uuid::new_v4());
        let node = AgentNode {
            id: id.clone(),
            origin: None,
            identity_key: Some("page".into()),
            structural_role: StructuralRole::Page,
            semantic_role: None,
            application_type: None,
            name: Some(name.into()),
            value: None,
            description: None,
            state: NodeState {
                visible: true,
                enabled: true,
                ..Default::default()
            },
            geometry: None,
            relationships: vec![],
            actions: vec![],
            children: vec![],
            provenance: vec![],
            confidence: Confidence::DIRECT,
            generation: 0,
        };
        Self {
            root: id,
            nodes: vec![node],
            generation: 0,
            truncated: false,
        }
    }
    pub fn find(&self, id: &str) -> Option<&AgentNode> {
        self.nodes.iter().find(|node| node.id == id)
    }
    pub fn find_identity(&self, identity_key: &str) -> Vec<&AgentNode> {
        self.nodes
            .iter()
            .filter(|node| node.identity_key.as_deref() == Some(identity_key))
            .collect()
    }
}
