//! The texture graph: React owns the document and sends what affects the
//! result (`graph.set`); Bevy evaluates it on the CPU (`eval`), node by node
//! within a frame budget, and publishes a thumbnail per node, the 2D view's
//! image and the material maps (`textures`).

pub mod buf;
pub mod catalog;
mod eval;
mod nodes;
mod noise;
mod textures;

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy_react::{ReactAppExt, Request, react_event, react_message, react_request};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use catalog::{NodeDef, Stop};
pub use textures::{MaterialMaps, setup as setup_textures};

/// A parameter or inline input value, as React stores it.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Num(f32),
    Color([f32; 3]),
    Text(String),
    Ramp(Vec<Stop>),
}

#[derive(Deserialize, TS, Clone, Debug)]
pub struct GraphNode {
    pub id: u32,
    pub kind: String,
    /// Parameters and unlinked inputs' values by id; a missing one takes the
    /// catalog default.
    #[serde(default)]
    pub values: BTreeMap<String, Value>,
    /// Passes the first linked input through (Blender's mute).
    #[serde(default)]
    pub mute: bool,
}

#[derive(Deserialize, TS, Clone, Debug, PartialEq)]
pub struct GraphLink {
    pub from: u32,
    pub output: String,
    pub to: u32,
    pub input: String,
}

/// React → Bevy: the whole graph, whenever something that changes the
/// result changes (moving nodes doesn't).
#[react_message(name = "graph.set")]
#[derive(Clone, Debug, Default)]
pub struct SetGraph {
    /// Pixels per side of every node's image.
    pub resolution: u32,
    pub nodes: Vec<GraphNode>,
    pub links: Vec<GraphLink>,
}

/// React → Bevy: what the 2D view shows — one output of one node (the
/// Material Output's inputs count as its outputs), or nothing.
#[react_message(name = "graph.view")]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewOutput {
    pub node: Option<u32>,
    pub output: Option<String>,
}

/// React → Bevy (request): the node catalog, once at startup.
#[react_request(name = "catalog", response = Vec<NodeDef>)]
pub struct GetCatalog;

#[derive(Serialize, TS, Clone, Debug, PartialEq)]
pub struct Timing {
    pub id: u32,
    /// The node's last evaluation, in milliseconds.
    pub ms: f32,
}

/// Bevy → React: evaluation progress, sent when it changes.
#[react_event(name = "graph.status")]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    /// The nodes still to evaluate for the current graph.
    pub pending: Vec<u32>,
    /// The latest evaluation time of every node that has one.
    pub timings: Vec<Timing>,
}

pub struct GraphPlugin;

impl Plugin for GraphPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.init_resource::<eval::Evaluator>()
            .init_resource::<ViewOutputState>()
            .add_systems(Startup, setup_textures)
            .add_systems(
                Update,
                (
                    eval::evaluate,
                    textures::publish_thumbnails,
                    textures::publish_view,
                    textures::publish_material,
                    eval::report_status,
                )
                    .chain(),
            );
    }
}

pub fn register_bindings(app: &mut App) {
    app.add_react_request_handler(on_catalog)
        .add_react_handler(on_set_graph)
        .add_react_handler(on_view)
        .add_react_event::<Status>();
}

/// The latest `graph.view`.
#[derive(Resource, Default)]
pub struct ViewOutputState(pub ViewOutput);

fn on_catalog(req: On<Request<GetCatalog>>) {
    req.respond(catalog::CATALOG.clone());
}

fn on_set_graph(graph: On<SetGraph>, mut evaluator: ResMut<eval::Evaluator>) {
    evaluator.set(graph.event().clone());
}

fn on_view(view: On<ViewOutput>, mut state: ResMut<ViewOutputState>) {
    state.0 = view.event().clone();
}
