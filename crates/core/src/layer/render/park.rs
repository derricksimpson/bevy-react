//! Keeps the members of clean cached layers out of the stock UI queue.
//!
//! Bevy's UI extraction is retained: a node's render data stays in the stock
//! stores until the node changes, and each stock `queue_*` system walks its
//! whole store every frame, adding one `TransparentUi` item per drawable. For
//! the members of a layer served from cache those items exist only to be
//! taken out again and dropped by `redistribute_ui_layers` — about 3% of a
//! frame's CPU on a screen of 500 static layers.
//!
//! Every stock store keeps `node → (camera render entity, items)`, and every
//! stock queue loop skips an entry whose camera resolves to no UI phase. So
//! [`park_cached_layer_members`] (before `RenderSystems::Queue`) writes
//! [`PARKED`] into that camera slot for the members of layers that do not
//! capture this frame, and puts the saved camera back the frame the layer
//! does. A node's data is never touched: bevy removes and re-creates entries
//! exactly as before, whatever the camera slot says.
//!
//! What keeps a parked entry from missing a capture (it would freeze a blank
//! layer into the cache):
//!
//! 1. Only [`route`] writes [`PARKED`], and only for a node that is, in this
//!    frame's [`ExtractedUiLayers`], a member of a layer whose
//!    `needs_capture` is false — the flag the capture pass itself reads.
//! 2. What an entry should be is a pure function of the membership map and
//!    the per-layer `needs_capture` flags. Whenever either differs from what
//!    the last pass saw ([`ExtractedUiLayers::membership_epoch`], the flags
//!    compared one by one), every entry of every covered store is routed
//!    again, before anything is queued.
//! 3. Between two such passes bevy can only make an entry *un*parked (a
//!    re-extracted entry gets its real camera); those are parked again from
//!    `ExtractedUiNodes::changed`. Missing one is harmless: the item is
//!    queued, stolen and dropped as before.
//! 4. A parked entry always has its camera saved: the row is written with
//!    the sentinel and dropped only by a full pass, after that pass restored
//!    the entry.
//!
//! Debug builds check (1) and (4) over every covered store after each pass.
//!
//! Not covered: gradients (bevy's store is private) and `UiMaterial` nodes
//! (one store per app material type). Their items are still queued, stolen
//! and dropped — routing is per item, so a layer may be parked in part.
//!
//! Re-verify on Bevy upgrades: each stock `queue_*` loop resolves its phase
//! from the entry's camera entity and `continue`s without one; the stores
//! keep `(camera, items)` per main entity in `pub` fields;
//! `ExtractedUiNodes::changed` lists every entry re-created this frame;
//! `gradient` is still private (cover it once it is not).

use bevy::ecs::entity::Entity;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Res, ResMut};
use bevy::platform::collections::HashMap;
use bevy::render::sync_world::{MainEntity, MainEntityHashMap};
use bevy::ui_render::ExtractedUiNodes;
use bevy::ui_render::box_shadow::ExtractedBoxShadows;
use bevy::ui_render::ui_texture_slice_pipeline::ExtractedUiTextureSlices;

use super::ExtractedUiLayers;

/// Camera slot of a parked entry. No entity has this id, and it is the value
/// the stock queue loops start from, so a parked entry resolves no phase.
const PARKED: Entity = Entity::PLACEHOLDER;

/// One stock retained store: node main entity → (camera render entity, items).
type Store<T> = MainEntityHashMap<(Entity, T)>;

/// Which entries are parked, and what the last full pass was derived from.
#[derive(Resource, Default)]
pub struct ParkedUiNodes {
    /// Parked node → the camera render entity stock extraction gave it.
    cameras: MainEntityHashMap<Entity>,
    /// [`ExtractedUiLayers::membership_epoch`] at the last full pass.
    epoch: u64,
    /// Per layer index at the last full pass: `!needs_capture`.
    clean: Vec<bool>,
}

/// Parks the stock render data of clean cached layers' members and restores
/// it for layers that capture this frame (see the module docs). Must run
/// before every stock `queue_*` system.
pub fn park_cached_layer_members(
    extracted: Res<ExtractedUiLayers>,
    mut parked: ResMut<ParkedUiNodes>,
    mut nodes: ResMut<ExtractedUiNodes>,
    mut shadows: ResMut<ExtractedBoxShadows>,
    mut slices: ResMut<ExtractedUiTextureSlices>,
) {
    park(
        &mut parked,
        extracted.membership_epoch,
        &extracted.membership,
        extracted.layers.iter().map(|layer| !layer.needs_capture),
        &mut nodes,
        &mut shadows.box_shadows,
        &mut slices.slices,
    );
}

fn park<S, T>(
    state: &mut ParkedUiNodes,
    epoch: u64,
    membership: &HashMap<MainEntity, usize>,
    clean: impl Iterator<Item = bool> + Clone,
    nodes: &mut ExtractedUiNodes,
    shadows: &mut Store<S>,
    slices: &mut Store<T>,
) {
    let ParkedUiNodes {
        cameras,
        epoch: seen_epoch,
        clean: seen_clean,
    } = state;
    // The debug check below reads this frame's flags itself, not what the
    // pass decided from them.
    #[cfg(debug_assertions)]
    let fresh: Vec<bool> = clean.clone().collect();
    let full = *seen_epoch != epoch || !seen_clean.iter().copied().eq(clean.clone());
    if full {
        *seen_epoch = epoch;
        seen_clean.clear();
        seen_clean.extend(clean);
    }
    // No clean layer (or no layer at all) and nothing parked.
    if cameras.is_empty() && !seen_clean.contains(&true) {
        return;
    }
    let want = |main: &MainEntity| {
        membership
            .get(main)
            .is_some_and(|&idx| seen_clean.get(idx) == Some(&true))
    };
    let ExtractedUiNodes { uinodes, changed } = nodes;
    if full {
        // Membership or a layer's clean/capturing state moved: route every
        // entry, then forget the nodes that are no longer parked.
        route_all(uinodes, &want, cameras);
        route_all(shadows, &want, cameras);
        route_all(slices, &want, cameras);
        cameras.retain(|main, _| want(main));
    } else {
        // Same routing as the last pass: only entries bevy re-created can be
        // off, and those come back unparked.
        for main in changed.iter().filter(|main| want(main)) {
            if let Some(entry) = uinodes.get_mut(main) {
                route(*main, entry, true, cameras);
            }
            if let Some(entry) = shadows.get_mut(main) {
                route(*main, entry, true, cameras);
            }
            if let Some(entry) = slices.get_mut(main) {
                route(*main, entry, true, cameras);
            }
        }
    }
    #[cfg(debug_assertions)]
    {
        let clean_member = |main: &MainEntity| {
            membership
                .get(main)
                .is_some_and(|&idx| fresh.get(idx) == Some(&true))
        };
        let stray = |main: &MainEntity, camera: Entity| {
            camera == PARKED && !(clean_member(main) && cameras.contains_key(main))
        };
        debug_assert!(
            !uinodes.iter().any(|(main, entry)| stray(main, entry.0))
                && !shadows.iter().any(|(main, entry)| stray(main, entry.0))
                && !slices.iter().any(|(main, entry)| stray(main, entry.0)),
            "a UI entry is parked outside a clean cached layer, or without its camera"
        );
    }
}

fn route_all<T>(
    store: &mut Store<T>,
    want: &impl Fn(&MainEntity) -> bool,
    cameras: &mut MainEntityHashMap<Entity>,
) {
    for (main, entry) in store.iter_mut() {
        route(*main, entry, want(main), cameras);
    }
}

/// Brings one entry's camera slot in line with `park`.
fn route<T>(
    main: MainEntity,
    entry: &mut (Entity, T),
    park: bool,
    cameras: &mut MainEntityHashMap<Entity>,
) {
    match (park, entry.0 == PARKED) {
        (true, false) => {
            cameras.insert(main, entry.0);
            entry.0 = PARKED;
        }
        (false, true) => match cameras.get(&main) {
            Some(&camera) => entry.0 = camera,
            None => debug_assert!(false, "parked UI entry without a saved camera"),
        },
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(i: u32) -> MainEntity {
        MainEntity::from(Entity::from_raw_u32(i).unwrap())
    }

    fn camera(i: u32) -> Entity {
        Entity::from_raw_u32(1000 + i).unwrap()
    }

    /// The covered stores plus what `park` is derived from. Entries carry no
    /// items: parking never looks at them.
    #[derive(Default)]
    struct World {
        state: ParkedUiNodes,
        nodes: ExtractedUiNodes,
        shadows: Store<()>,
        slices: Store<()>,
        membership: HashMap<MainEntity, usize>,
        epoch: u64,
        clean: Vec<bool>,
    }

    impl World {
        /// Nodes 1, 2 in layer 0 and node 3 in layer 1, node 4 plain UI, all
        /// extracted on camera 0.
        fn two_layers(clean: [bool; 2]) -> Self {
            let mut world = World {
                membership: [(node(1), 0), (node(2), 0), (node(3), 1)].into(),
                clean: clean.into(),
                ..Default::default()
            };
            for i in 1..=4 {
                world.extract(i, camera(0));
            }
            world
        }

        /// What stock extraction does to a changed node: a fresh entry with
        /// its real camera, listed in `changed`.
        fn extract(&mut self, i: u32, camera: Entity) {
            self.nodes
                .uinodes
                .insert(node(i), (camera, Default::default()));
            self.nodes.changed.insert(node(i));
        }

        /// One frame: `park`, then the next extraction's `changed.clear()`.
        fn run(&mut self) {
            park(
                &mut self.state,
                self.epoch,
                &self.membership,
                self.clean.iter().copied(),
                &mut self.nodes,
                &mut self.shadows,
                &mut self.slices,
            );
            self.nodes.changed.clear();
        }

        fn camera_of(&self, i: u32) -> Entity {
            self.nodes.uinodes[&node(i)].0
        }
    }

    #[test]
    fn parks_only_members_of_clean_layers() {
        let mut world = World::two_layers([true, false]);
        world.run();
        assert_eq!(world.camera_of(1), PARKED);
        assert_eq!(world.camera_of(2), PARKED);
        // A capturing layer's member and a plain node keep their camera.
        assert_eq!(world.camera_of(3), camera(0));
        assert_eq!(world.camera_of(4), camera(0));
    }

    /// The frame a layer captures, every member is back on the exact camera
    /// it was extracted with — without bevy re-extracting anything.
    #[test]
    fn a_capturing_layer_gets_its_members_back() {
        let mut world = World::two_layers([true, true]);
        world.extract(2, camera(7));
        world.run();
        assert_eq!(world.camera_of(1), PARKED);
        assert_eq!(world.camera_of(3), PARKED);

        world.clean = vec![false, true];
        world.run();
        assert_eq!(world.camera_of(1), camera(0));
        assert_eq!(world.camera_of(2), camera(7));
        assert_eq!(world.camera_of(3), PARKED);

        world.clean = vec![false, false];
        world.run();
        assert_eq!(world.camera_of(3), camera(0));
        assert!(world.state.cameras.is_empty());
    }

    /// A node that leaves its layer (membership moved, nothing re-extracted)
    /// is unparked; so is everything when the last layer goes.
    #[test]
    fn a_node_leaving_its_layer_is_unparked() {
        let mut world = World::two_layers([true, true]);
        world.run();

        world.membership.remove(&node(2));
        world.epoch += 1;
        world.run();
        assert_eq!(world.camera_of(1), PARKED);
        assert_eq!(world.camera_of(2), camera(0));

        // No layers at all: `extract_ui_layers` clears both.
        world.membership.clear();
        world.clean.clear();
        world.epoch += 1;
        world.run();
        for i in 1..=4 {
            assert_eq!(world.camera_of(i), camera(0));
        }
        assert!(world.state.cameras.is_empty());
    }

    /// Layer indices re-mapped under the same per-index flags (a re-ordered
    /// extraction): the epoch alone re-routes.
    #[test]
    fn a_membership_epoch_reroutes_under_equal_flags() {
        let mut world = World::two_layers([true, false]);
        world.run();
        assert_eq!(world.camera_of(1), PARKED);

        world.membership = [(node(1), 1), (node(2), 1), (node(3), 0)].into();
        world.epoch += 1;
        world.run();
        assert_eq!(world.camera_of(1), camera(0));
        assert_eq!(world.camera_of(2), camera(0));
        assert_eq!(world.camera_of(3), PARKED);
    }

    /// Bevy re-creates a changed member's entry with its real camera; on a
    /// frame with the same routing it is parked again, the new camera saved.
    #[test]
    fn a_reextracted_member_is_parked_again() {
        let mut world = World::two_layers([true, false]);
        world.run();

        world.extract(1, camera(5));
        world.run();
        assert_eq!(world.camera_of(1), PARKED);

        world.clean = vec![false, false];
        world.run();
        assert_eq!(world.camera_of(1), camera(5));
    }

    /// An entry bevy removed (hidden, despawned) leaves nothing behind.
    #[test]
    fn a_removed_entry_leaves_no_row() {
        let mut world = World::two_layers([true, false]);
        world.run();

        world.nodes.uinodes.remove(&node(1));
        world.nodes.changed.insert(node(1));
        world.run();
        world.clean = vec![false, false];
        world.run();
        assert!(world.state.cameras.is_empty());
        assert_eq!(world.camera_of(2), camera(0));
    }

    #[test]
    fn nothing_is_touched_while_every_layer_captures() {
        let mut world = World::two_layers([false, false]);
        world.run();
        for i in 1..=4 {
            assert_eq!(world.camera_of(i), camera(0));
        }
        assert!(world.state.cameras.is_empty());
    }

    /// Box shadows and texture slices live in their own stores under the
    /// same node; a node may have only one of them.
    #[test]
    fn shadow_and_slice_entries_follow_their_node() {
        let mut world = World::two_layers([true, false]);
        world.shadows.insert(node(1), (camera(0), ()));
        world.shadows.insert(node(3), (camera(0), ()));
        // Node 9: a member with a slice entry and no `uinodes` entry.
        world.membership.insert(node(9), 0);
        world.epoch += 1;
        world.slices.insert(node(9), (camera(2), ()));
        world.run();
        assert_eq!(world.shadows[&node(1)].0, PARKED);
        assert_eq!(world.shadows[&node(3)].0, camera(0));
        assert_eq!(world.slices[&node(9)].0, PARKED);

        world.clean = vec![false, false];
        world.run();
        assert_eq!(world.shadows[&node(1)].0, camera(0));
        assert_eq!(world.slices[&node(9)].0, camera(2));
    }

    /// Random frames of everything that can move — layers flipping between
    /// clean and capturing, membership edits, bevy re-creating or removing
    /// entries — against a model of the camera each entry was extracted
    /// with. After every frame an entry is parked exactly when its node is
    /// in a clean layer, its camera saved; any other entry holds its own.
    #[test]
    fn random_frames_keep_the_routing_invariants() {
        const NODES: u32 = 24;
        const LAYERS: usize = 4;
        // xorshift: deterministic, no dependency.
        let mut seed = 0x9E37_79B9_7F4A_7C15_u64;
        let mut rand = move |below: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % below as u64) as u32
        };
        let mut world = World {
            clean: vec![false; LAYERS],
            ..Default::default()
        };
        let mut model: HashMap<MainEntity, Entity> = HashMap::default();
        // Entries seen parked / queued: the walk must exercise both.
        let mut seen = [0usize; 2];

        for _ in 0..2000 {
            for _ in 0..rand(4) {
                let i = rand(NODES);
                match rand(6) {
                    // A layer starts or stops capturing.
                    0 | 1 => {
                        let layer = rand(LAYERS as u32) as usize;
                        world.clean[layer] = !world.clean[layer];
                    }
                    // A node joins a layer, moves to another, or leaves.
                    2 => {
                        match rand(LAYERS as u32 + 1) as usize {
                            LAYERS => world.membership.remove(&node(i)),
                            layer => world.membership.insert(node(i), layer),
                        };
                        world.epoch += 1;
                    }
                    // Bevy removes the node's data.
                    3 => {
                        world.nodes.uinodes.remove(&node(i));
                        world.nodes.changed.insert(node(i));
                        model.remove(&node(i));
                    }
                    // Bevy (re-)extracts the node, maybe on another camera.
                    _ => {
                        let camera = camera(rand(3));
                        world.extract(i, camera);
                        model.insert(node(i), camera);
                    }
                }
            }
            world.run();

            for (main, (slot, _)) in world.nodes.uinodes.iter() {
                let in_clean_layer = world
                    .membership
                    .get(main)
                    .is_some_and(|&layer| world.clean[layer]);
                assert_eq!(*slot == PARKED, in_clean_layer, "{main:?} misrouted");
                seen[in_clean_layer as usize] += 1;
                let camera = if in_clean_layer {
                    world.state.cameras[main]
                } else {
                    *slot
                };
                assert_eq!(camera, model[main], "{main:?} lost its camera");
            }
        }
        assert!(seen[0] > 1000 && seen[1] > 1000, "vacuous walk: {seen:?}");
    }
}
