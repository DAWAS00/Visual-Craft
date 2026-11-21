# Implementing diagrams in FigCraft

Design proposal reviewed 2026-11-18. The [diagram feature review](diagram-feature-review.md) covers the broader diagram catalog and Visual Paradigm workflows.

## First release

Build an editable flowchart workflow inside FigCraft Boards. A user can start with a shape, choose and connect the next shape, paste a flowchart as text, or rearrange an existing selection. The result is a set of ordinary Board layers with stable IDs. Each diagram owns its contents. Reusable UML model elements and coordinated views are later design decisions.

**Working assumption.** Start with the visual editor. Use FigCraft’s document, canvas, text, history, and collaboration infrastructure as the foundation. Add graph layout and input handling in Rust so the same feature works in the desktop app and browser. [FigCraft architecture](architecture.md) · [Board model](../crates/doc/src/board.rs)

### End-to-end example

Start with a blank Board. Place **Review request**. Drag its **+** handle and choose a **Decision** named **Approved?**. Add a **Yes** connector to **Ship** and a **No** connector to **Revise**. Connect **Revise** back to **Review request**. Select the four shapes, arrange them left to right, and move the decision to check that its connectors and labels follow. Save, reopen, and export the diagram with both labels.

The same graph should be possible with one text import. The entire import should undo and redo as one action.

## Where the code goes

| Responsibility | Code to use | Proposed change |
| --- | --- | --- |
| Board command registration | [Board specs](../crates/engine/src/boards/board.rs) and [engine registry](../crates/engine/src/cmd.rs) | Register import and arrangement commands. Extend `board.addConnected` with optional parameters. |
| Shapes, connectors, and labels | [Board item creation](../crates/engine/src/boards/board/items.rs), [connector creation](../crates/engine/src/boards/board/connectors.rs), and [labels](../crates/engine/src/boards/board/edit.rs) | Compose the existing constructors and label setter. Expose a constructor within the Board module where necessary. |
| Graph parsing and planning | New code under `crates/engine/src/boards/board/` | Keep an in-memory graph with node keys, shape types, labels, edges, and orientation. Produce positions and routes before editing the document. |
| Recompute, history, and persistence | [Command execution](../crates/engine/src/session/exec.rs), [derived data](../crates/engine/src/derived.rs), and [Board connector recompute](../crates/engine/src/boards/board/derived.rs) | Apply changes through one undoable command. Use existing Board metadata and connector geometry. |
| Commands in the UI | [Board toolbar](../crates/ui-egui/src/toolbar.rs), [Board picker](../crates/ui-egui/src/widgets/board_picker.rs), [connected-shape overlay](../crates/ui-egui/src/canvas/board_connect.rs), and [UI command dispatch](../crates/ui-egui/src/app/commands.rs) | Add the insert dialog, shape chooser, and arrange action. Send mutations through `FigApp::run`. |
| Control, CLI, and MCP | [Shared command architecture](architecture.md#commands-one-path-for-every-surface) | Document the same engine commands and verify them with programmatic parameters. |

The importer can use the [template builder](../crates/engine/src/boards/board_templates.rs) as a precedent for creating multiple native objects in one edit. Include every connector label ID in the result and selection. Each label is a sibling layer and may otherwise be missing from a selection export or copy. [Label storage](../crates/engine/src/boards/board/edit.rs) · [Export selection](../crates/engine/src/cmd/file.rs)

## Command contracts

The following names and parameters are proposed. Register each mutation with history enabled. Validate its parameters and require an editable Board through the engine, including when called from an automation client.

| Command | Input | Result and behavior |
| --- | --- | --- |
| `board.addConnected` | Existing parameters plus optional `shape` and `label` | Use the chosen type when the source is a shape. `label` sets connector text. Return existing `id`, `text`, and `connector` fields plus `label` when created. Omitting the new fields retains the current same-shape or sticky behavior and existing shortcuts. |
| `board.insertFlowchart` | Required `source`, optional `x` and `y`, both or neither (one alone, or a non-finite number, is an error) | Validate source, lay it out, and insert on the active Board page. With coordinates, anchor the overall layout’s top-left there. Otherwise center it on the view. Return a source-key to layer-ID map, connector IDs, label IDs, and any routing warnings. Select the created diagram. |
| `board.arrangeDiagram` | Optional `ids`, optional `direction` of `LR` or `TB` | Arrange the given shapes or the current selection, defaulting to left to right. Return moved shape IDs, rerouted connector IDs, and routing warnings. Keep the selection. |

For arrangement, v1 takes at least two editable, unrotated Board shapes that are direct children of the active page. Include connectors automatically when both endpoints are selected shapes. Ignore connector and label IDs in the selection itself. Refuse other object types, locked shapes, unsupported shape transforms, and locked internal connectors. Shapes outside the selection keep their positions. Connectors crossing the selection boundary remain attached and use normal endpoint recompute. If the selected shapes have zero internal edges, report that arrangement needs a connection. [Current endpoint commands](../crates/engine/src/boards/board/connectors.rs) · [Route recompute](../crates/engine/src/boards/board/derived.rs)

## Implementation steps

### 1. Extend connected-shape creation

1. Add optional `shape` and `label` parameters to `board.addConnected`. Parse and validate them before ending active text editing or creating objects. Keep the current defaults for calls that omit them. A sticky can still create a sticky, and a shape choice applies to shape sources.
2. Keep the handle click as quick creation. For a drag from a shape onto blank canvas, show a compact chooser at the drop point with **Same shape**, **Process**, **Decision**, and **Terminator**. Map these to the current shape, rectangle, diamond, and rounded rectangle. Confirming creates the shape and arrow and starts editing the node name. Escape cancels the chooser. A drop on an existing target uses the existing join behavior. Preserve current sticky drag behavior.
3. Store the source ID, handle side, and drop position in temporary UI state until confirmation. If the source, page, or file disappears or editing access changes, clear the pending choice. At commit, resolve the source ID again.
4. Commit through the existing connected-shape command. Use the existing requested text-edit action for UI callers and the same result IDs for automation.

[Connected-shape handler](../crates/engine/src/boards/board/items.rs) · [Current handle tests](../crates/ui-egui/src/board_connect_tests.rs)

### 2. Parse a small flowchart format

Support the flowchart forms used in the example below. Accept an explicit `flowchart LR`, `flowchart TB`, or `flowchart TD` header, with `TD` normalized to `TB`. Accept named nodes with quoted labels, plain arrows, and arrows labeled with `|…|`. Allow standalone node declarations and inline declarations on an edge. Resolve bare references after parsing so an edge can refer to a node declared later. Define an ASCII identifier grammar and use Unicode for labels. Treat labels as literal text. Document quoting and escaping in the command help.

```text
flowchart LR
    review["Review request"] --> decision{"Approved?"}
    decision -->|Yes| ship("Ship")
    decision -->|No| revise["Revise"]
    revise --> review
```

Validate the full graph up front. Report malformed syntax with a line and column. Report unresolved node keys, conflicting definitions, identical edges (same ends and same label; two edges between the same nodes with different labels are valid, each is its own connector), self-loops, and unsupported syntax. Multi-node cycles are valid. Proposed initial limits are 64 KiB of input, 100 nodes, 200 edges, and 120 Unicode characters per label. Check these limits against FigCraft’s actual live-session payload budget before publishing them. Size shapes against the existing text measurement and wrapping behavior, using larger bounds for decisions when their insets require it. [Board shape text insets](../crates/engine/src/boards/board/items.rs)

### 3. Share a deterministic layout planner

1. Build a graph from parsed nodes or the selected Board shapes. Use stable source order for import and stable layer IDs for arrangement. Use the real node sizes.
2. Find weakly connected components. Walk each component in stable order and mark directed back edges for routing. Rank the graph with those edges excluded, then restore them as real connections. Place ranks with measured spacing and center the branches on the cross-axis. Pack independent components with a gap.
3. Place flow edges from facing sides. Reserve space between ranks and separate the channels used for retry edges. Check candidate elbow routes against the other shapes inside the diagram. Existing routing checks only the two endpoint boxes. Use a bounded candidate search with stable ordering and report any edge for which only a fallback route is available.
4. Encode planned elbow routes in the existing `ConnectorPath::Runs` representation. Convert interior coordinates to offsets from the endpoint midpoint, as expected by `route_along`. Keep endpoints attached to their layer IDs. Import creates elbow connectors. Arrange can replace runs on internal elbow connectors. Straight and curved connectors retain their routing modes and use existing endpoint recompute. Test the reconstructed route after saving and after moving an endpoint.
5. Anchor an arranged diagram at the prior top-left of its selected shape bounds. Preserve current styles, node sizes, text, and selection during arrangement. Apply the planned result in one edit. With the same graph and geometry, rerunning arrange should leave positions and routes identical.

Node and label layout should be planned within documented geometry limits. Outside the arranged graph, normal Board editing continues to recalculate attachments when users move shapes. Running **Arrange diagram** again recalculates the graph layout. [Board routing primitives](../crates/doc/src/board_route.rs) · [Connector path model](../crates/doc/src/board.rs)

### 4. Wire the UI to the engine

1. Put **Insert diagram…** near **Templates** on Boards. Add a parameterless Actions entry that raises the UI request `dialog.insertDiagram` (dropped by the control channel like every `dialog.*`). The parameterized import command remains callable directly. The Actions palette filters out commands that require parameters. [Actions palette](../crates/ui-egui/src/menus/palette.rs)
2. Give the dialog a multiline source field, an example, and **Insert** and **Cancel** controls. Keep the source in the dialog if the engine reports an error. Show the validation location using the existing command error surface. Guard the active document and page before submitting. The dialog owns the draft and opens only in the interactive UI.
3. Add **Arrange diagram** to the Board selection workflow and Actions. Offer **Left to right** and **Top to bottom**. Show the operation only where the selection supports it. Report any route fallback in the command result so the UI and automation callers can surface it.
4. Add translatable labels and keyboard focus for both dialogs and menus. Put focus on the text field when the import dialog opens, return it to the canvas after insertion, and let Escape cancel before any document edit. Use FigCraft’s theme tokens and existing menu and dialog components.

The engine command is the only writer of imported or rearranged geometry. The UI can retry after a parse error without creating a partially imported diagram. Use the normal document save and live-session paths. As part of implementation, check command journaling for retained source text and avoid keeping a large duplicate when the imported result is already in the document. [Engine execution and journal](../crates/engine/src/session/exec.rs) · [Project rules](../AGENTS.md)

## Acceptance checks

| Area | Required result |
| --- | --- |
| Existing behavior | A click still creates a connected shape of the same type. Dropping on an existing shape still joins it. Existing sticky shortcuts still work. |
| Connected choice | Choosing Decision creates a diamond and an attached arrow in one undo step. Cancel leaves the document unchanged. |
| Input | The example imports four shapes and four arrows. The Yes and No labels are editable. Bad syntax, forbidden self-loops, and over-limit input create zero layers and no undo entry. |
| Layout | Both directions work. The retry edge in the example follows a visible return channel. Disconnected components stay apart. A second arrange with the same inputs produces the same positions and routes. |
| Selection scope | Arrange moves only eligible selected shapes. Connectors with an outside peer follow their endpoint as before. Locked objects and unsupported selections produce a clear error. |
| Save, copy, and export | Reopening preserves the layout. Copying the selected diagram to another Board remaps the internal endpoints and labels. SVG/PDF exports include both decision labels. |
| Live sessions | A remote client receives the diagram and route metadata. Moving an endpoint looks consistent on both clients. Undo takes back the initiating user’s insertion as one edit. |
| Platform and automation | Verify the Board workflow and keyboard behavior on desktop and web. Run the same engine command through CLI or MCP. Check one selected flowchart screenshot against the acceptance example. |

Put parser and graph fixtures near the new engine code. Add engine tests for atomic insertion, arrange, persisted routes, undo, and live sessions. Add headless egui tests for the chooser, cancellation, dialog errors, and successful insertion. Existing template tests are a starting point for undo and sync checks. Before merging implementation, run format, Clippy, workspace tests, layer and asset checks, and the WebAssembly build through `cargo xtask ci`. [Template tests](../crates/engine/src/board_tables_tests.rs) · [Board sync tests](../crates/collab/src/board_sync_tests.rs) · [Quality gates](../AGENTS.md#quality-gates-before-every-commit)

## How the feature grows

After flowcharts ship, use the same primitives for hierarchy diagrams, concept maps, and swimlanes. Tree layout and ordered lane containers can be added to the geometry planner when those workflows need them.

Before implementing full UML, ERD, or SysML editing, decide whether named elements are shared across views. If required, introduce stable model element and relationship IDs and store each diagram appearance independently. Add notation-specific rules and layout one family at a time. Class and ERD diagrams test structured compartments, endpoint markers, and row anchors. Sequence and timing diagrams need ordered events or time coordinates. [Coverage and proposed design by diagram type](diagram-feature-review.md#uml-diagrams)

## Code structure (proposed revision)

This section replaces the "Graph parsing and planning" row of the table above, which puts the
parser and planner under `crates/engine/src/boards/board/`. Those would be the largest pieces of the
feature and would sit at layer 5 beside the Session, so they could only be tested through a document.
The precedent in the repository points the other way: `mind_layout` is a pure function in a low layer
(`crates/doc/src/board_mindmap.rs:39`, boxes in, positions out) and `mindmap.rs` in the engine is a
thin adapter (384 lines). Diagrams follow it.

### Layers

| Layer | Code | Owns | Knows nothing about |
| --- | --- | --- | --- |
| L1 `doc` | `BoardStyle { Plain, Diagram }` field on the document, skipped when default | What a saved file says | Commands, UI |
| L2 new crate `figcraft-diagram` | `crates/diagram`: parse, graph, rank, route, plan | Pure geometry and text: source in, `Graph` out; `Graph` and node sizes in, `Plan` out | `Document`, `Session`, fonts, egui |
| L5 `engine` | `boards/board/diagram/{convert,insert,arrange}.rs` and `file.newDiagram` beside `new_file` in `cmd/file.rs` | Command specs, parameter validation, measuring node sizes, turning a `Plan` into layers and connectors in one undoable edit | How a layout is computed |
| L6 `ui-egui` | `src/diagram/{mod,start_dialog,library,chooser,insert_dialog,arrange_menu}.rs` | Widgets and their temporary state; every change goes through `app.run` | Parsing, layout, document internals |

Registration: add `("diagram", 2)` to `TABLE` in `xtask/src/layers.rs`. `members = ["crates/*"]`
already includes the crate. The crate's `Cargo.toml` has `[lints] workspace = true` and depends on
`figcraft-geom` only (`proptest` as a dev-dependency); the root `[workspace.dependencies]` gets a
`figcraft-diagram` entry, the engine's manifest depends on it, and `Cargo.lock` is updated. With
`figcraft-geom` as its only production dependency it builds for wasm and `cargo xtask layers`
passes.

### `figcraft-diagram` modules

| Module | Public surface | Notes |
| --- | --- | --- |
| `limits.rs` | `MAX_SOURCE_BYTES`, `MAX_NODES`, `MAX_EDGES`, `MAX_LABEL_CHARS`, `MAX_ROUTE_STEPS` | One place for every bound; checked before any allocation that depends on input |
| `graph.rs` | `Graph`, `NodeIx` (typed index), `Node { key, kind, label }`, `Edge { from, to, label }`, `Direction`, `NodeKind { Process, Decision, Terminator }` | Nodes in source order; `NodeIx` is the only way to name a node; `Graph::new` validates (self loops, duplicates, limits) so an invalid graph cannot exist |
| `parse.rs` | `parse_flowchart(&str) -> Result<Graph, ParseError>` | `ParseError { line, column, kind }` with a hand-written `Display` and `Error`, since the crate's only production dependency is `figcraft-geom` (`proptest` is a dev-dependency); no recursion; labels are literal text |
| `rank.rs` | crate-private | Weak components, iterative depth-first search for back edges, longest-path ranking, bounded barycenter sweeps (fixed count) |
| `route.rs` | crate-private | Channel allocation for back edges; candidate elbow search against obstacle boxes within `MAX_ROUTE_STEPS`; reports `fallback` instead of failing |
| `plan.rs` | `plan(&Problem, &Options) -> Result<Plan, PlanError>`; `Problem { sizes: Vec<Size>, edges: Vec<(NodeIx, NodeIx)>, direction }` (built by `Graph::to_problem(&[Size])` for an import, or by the engine for an arrangement); `Plan { positions: Vec<Point>, routes: Vec<Route>, warnings: Vec<Warning> }` | Fallible: it checks that every edge names a node, that every size is positive and bounded, that options and the anchor are finite and in range, and that the result still fits the coordinate range after packing and translation (finite inputs can overflow). Pure and deterministic: ordered maps and stable sorts only, never a hash order. Sizes come in, so there is no font dependency |

`Route` holds absolute points. Converting them to `ConnectorPath::Runs` offsets from the endpoint
midpoint needs doc types, so that conversion stays in the engine's `convert.rs`.

### Engine adapter rules

- `board.addConnected` gets a small `ConnectedParams { shape: Option<BoardShape>, label: Option<String> }`
  parsed and validated before text editing ends or anything is created (`items.rs:197`). Omitted
  fields keep today's behaviour.
- `insert.rs` runs: check limits, parse, measure each label with `engine/src/measure.rs`, plan, then
  apply. The apply step is the only one that touches the document, and it is one command, so the
  existing guard and rollback make it atomic.
- `arrange.rs` builds a `Problem` (not a `Graph`) from the selection through `convert.rs`, plans,
  and replaces positions, runs and sides on internal elbow connectors only.
- Neither command opens a dialog. `file.newDiagram {name?, type?, template?}` always creates (type
  defaults to `flowchart`; `type` is `flowchart` or `mindMap`). The start dialog is a separate UI
  request, `dialog.newDiagram`, raised by the menu item and the home card; the control channel drops
  it, as it drops every `dialog.*`. The dialog submits `file.newDiagram` with explicit parameters,
  and Blank is "no template". The new document is built and normalized on a scratch session the
  way `board_templates::preview` builds a template on a board of its own, then adopted as the new
  tab, so the normal command path (which normalizes only the tab active at entry) is not relied on;
  slice 2 confirms the adoption point.
- `convert.rs` is the only file that imports both `doc` and `figcraft-diagram`. The crate exists for
  dependency isolation (wasm, layers, fuzzing), not to shorten files.

### Adapter contracts

- **Connector ends are explicit.** `route_along` picks each end's side from the other end's centre
  unless the connector names one (`crates/doc/src/board_route.rs:54`). The adapter writes the
  planned `side` on both ends of every connector it creates (import) and of every internal elbow
  connector it arranges, so a planned return channel cannot flip when a shape moves. Other
  connectors keep their sides.
- **After an endpoint moves there is no clearance guarantee.** `route_along` rebuilds saved runs
  from the new attachment midpoint without looking at other shapes, so moving a shape can pull a
  return channel through another one even with the sides fixed. What is guaranteed is attachment
  and right angles; running Arrange again plans the channel afresh. The acceptance moves test that
  and nothing stronger.
- **Runs are measured from the attachment points.** `ConnectorPath::Runs` offsets are relative to
  the midpoint of `attach_points` (which include the 4 pt end gap and any manual anchor), not of the
  shape boxes. `convert.rs` converts the planned points with that midpoint, drops degenerate
  segments, refuses a route over `MAX_RUNS` (64) by falling back to the router, and a test rebuilds
  the route with `route_along` and compares it to the plan, in `LR` and `TB`, after a save. After an
  endpoint move the route necessarily changes (the last corner follows the destination), so that
  test checks attachment, right angles and that the return still runs outside the shapes, not
  identity. The planner receives the end gap in its options and computes the same gap-adjusted
  endpoints as the adapter; the adapter asserts they equal `attach_points`. Obstacle clearance is
  checked on the reconstructed route, and any fallback to the router is a warning in the result.
- **Sizes before creation.** `TextMeasure` reads an existing `Document` and `NodeId`
  (`engine/src/measure.rs`), but `figcraft_text::layout` and `layout_cached` take a bare `TextData`,
  so preflight builds one with the Board text defaults shared with `make_shape` and measures it
  before any node exists. Decisions get larger bounds for their insets.
- **Journal.** `command_carries_import_bytes` (`session/exec.rs:7`) already keeps large payloads out
  of the journal, including those inside `command.batch`. `board.insertFlowchart` joins it for its
  `source`, so a 64 KiB text is not kept for the whole session. That predicate only gates the
  journal, and the executor assigns `last_command` after a successful history-enabled handler
  returns (`session/exec.rs:116`), so a clear inside the handler would be overwritten. The one
  executor change this feature makes is that bookkeeping line: a small `NOT_REPEATABLE` list
  (`board.insertFlowchart`) makes a success set `last_command` to `None` instead of the command, so
  an earlier `edit.delete` cannot be repeated against the newly selected diagram. It holds inside a
  batch too. Tests cover both, and that Repeat then says there is nothing to repeat.
- **Chooser sizes.** The new shape from the chooser keeps the source's width and height for Same,
  Process and Terminator; Decision uses at least 160 × 120 so its text fits. The `at` drop point
  (#1919) composes with `shape` and `label`: the centre is the drop point, the connector leaves by
  the handle's side.
- **Transaction (handler level; the executor is not changed).** `add_connected` calls
  `textedit::end` before creating layers. Ending a text interaction commits it: it can add an undo
  entry and send a live edit, and the session's rollback does not undo that. So the handler's order
  is fixed, and everything that can be refused is refused first: (1) validate every parameter;
  (2) check access (`board::require` only checks the file kind, so the handler also checks
  `active_read_only()` and a version preview, which covers a tab that is read-only during a pending
  `collab.publish`), resolve the source, run the structural checks creation would hit (for example,
  a parent inside an instance), compute the geometry of the shape, the connector **and the label**,
  prove the connector constructible (a straight connector whose attachment points coincide is
  rejected by its constructor) and check that the document has enough free ids for the shape, its
  children, the connector and the label's two inserts (`edit.rs:34`, `doc/src/lib.rs:152`), all
  without touching the document; (3) end text editing; (4) create. `textedit::end` has two paths
  that can change more than the edited text: a widget's editable field runs `widget.setText`, whose
  plugin callback can change the Board and has already synced, and an emptied created text box is
  removed. So step 2 refuses, before ending anything, when the open edit is a widget field
  (`widgets::field_of`), with "finish editing the widget field first". The same refusal covers the
  other snapshot-restoring path: an emptied created text box whose layer is absent from the
  interaction's starting snapshot (Text on a path) makes `end` call `cancel_interaction`, which
  restores the whole document and its id allocator, so a Board shape made while that draft was
  open would vanish and its id could be reused. The handler detects both (`field_of`, and
  `interaction.doc.node(edited text)` is `None`) and refuses with "finish the text first". A
  regression test keeps the source and the open draft in that case. The only remaining thing `end`
  does is remove an emptied created text box that is in the snapshot, which only frees ids and
  cannot remove the source (a Board object's own text stays), so the preflight of step 2 still
  holds in step 4.
  After step 3 only internal failures remain. Their outcome is the existing one, not a stronger promise: a standalone failure
  can restore the pre-handler snapshot, which holds an emptied created text box that the commit
  removed, and a failed group restores its original history (issues 1 and 2 below). The
  fault-injection tests (a failure after the shape, after the connector, after the label and during
  normalization, with changed text being edited and a populated redo stack) assert exactly those
  existing outcomes, so a later executor fix changes them knowingly.
- **Existing engine issues found while researching this (not part of this feature).** Tracing
  `Session::execute`, `grouped` and `textedit::end` showed latent problems that `board.addConnected`
  already has today, and that the new commands must not widen. They are recorded here so they can be
  filed and fixed on their own, with their own review, and the diagram work does not wait for them:
  1. After `textedit::end` inside a handler, a later failure restores the pre-handler snapshot, which
     still holds an emptied text box that the commit removed (`session/exec.rs:97`,
     `canvas/textedit.rs:128`).
  2. A failed group (`session/undo.rs:264`) restores document and history but not the open
     interaction or `text_edit`; a successful group folds a mid-group text commit into its own undo
     step, and in a live session the group's diff is against the replica, which has the pre-typing
     text, so batch undo keeps the draft locally and reverts it live.
  3. `FigApp::run` drains `ui_requests` queued by a command that then failed (`edit: true` queues
     `board.editText` before normalization).
  4. `edit.repeat` runs the saved handler with the saved parameters (`cmd/edit.rs:537`), and
     `textedit::end` can run `widget.setText`, whose plugin callback can replace `last_command`.
  5. Live Undo or Redo inside a group still travels to peers (`sharing/live/sync.rs:74`).
  A design sketch exists and is the starting point for that separate work: a table of commands
  that end a text edit beside `ENDS_PREVIEW`; a `prepare(&Session, &Value)` step before any commit
  (parameters, source resolution, constructibility, structural and access checks), returning the
  effective command id and prepared parameters together; commit before the snapshot for a single
  command; `group_checkpoint` saving and restoring `interaction`, `text_edit` and the length of
  `ui_requests`; and the commit ending at each child when it is reached, not at batch entry.
  Open points the review raised against it: separate rollback and undo baselines, and keeping the
  checkpoint and sync hold through the outer validation and normalization.
  What this feature does instead: the handler validates everything first (including access), then
  ends the text edit as today; the new commands are refused while a text edit is open; a successful
  insert clears `last_command`. The first three issues above are therefore unchanged by the feature,
  not fixed by it.
- **Insert and arrange during a text edit.** Both are refused while a text interaction is open
  (the engine suppresses ordinary command history then). The command `dialog.insertDiagram` only
  queues its UI request; it never touches the document, so the control channel (which runs it and
  then drops the request) cannot commit a draft. When the interactive UI receives the request it
  first checks edit access, then ends the open text edit through the existing commit path, then
  opens the dialog: the draft is committed when the dialog opens (the user's typing is finished),
  not when Insert is pressed, so a failed or cancelled import changes nothing beyond that commit.
  The Arrange button does the same before it runs `board.arrangeDiagram`. The engine's refusal
  therefore never fires from the UI, and the acceptance checks for Escape and for errors are stated
  against the document as it was when the dialog opened.
- **Planner input.** The planner does not take a `Graph`. It takes a layout problem: node sizes,
  directed edges as `NodeIx` pairs (parallel and repeated edges allowed; they rank once and are
  routed apart), a direction, and the end gap. `Graph` (the parser's output, with node kinds and
  labels) converts into it, and so does an arrangement of existing Board shapes, whose 23 variants
  all enter as plain boxes with their measured size. `Graph::new` rejects identical edges only at
  import; arrange keeps every existing connector and its id.
- **Arrange scope.** Connector and label ids in the selection are ignored; every other object that
  is not an unlocked, unrotated Board shape is refused. Elbow connectors between selected shapes
  get new runs and explicit sides, and their manual anchors are cleared (listed in the result).
  Straight and curved connectors keep their mode and anchors and follow their endpoints through the
  normal recompute; a test covers a manually anchored curve. Connectors that cross the selection
  boundary are listed separately, since normalization updates them too. A second arrange with the
  same inputs adds no history entry. One engine function decides eligibility and the UI calls it,
  so the button and the command agree.
- **Labels.** v1 keeps the existing label placement (the midpoint by route length). Collision
  handling for labels is out of scope.

### UI rules

- One `DiagramUi` struct in `app.ui` holds all temporary state: start dialog, library search and
  collapse, `Chooser { Closed, Open { source, side, at } }`, insert-dialog draft text.
- A widget function takes `(&mut FigApp, &mut Ui, &Tokens)` and returns; it never edits the document
  itself. The chooser re-resolves its source id on confirm and clears itself if the source is gone or
  editing access changed.
- Pending chooser and dialog state is bound to the document and page identity as well as the source
  id, and is cleared when any of them changes.
- Strings go through `tr`; colours come from `Tokens`.

### Never-crash and determinism

- The new crate declares the same `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]` as `figcraft-layout`.
- Every index from data uses `.get()` or a `NodeIx` made by `Graph::new`; every float is checked
  finite at the engine boundary before it reaches the planner; every loop has a bound from `limits.rs`.
- Same graph and sizes give the same `Plan` (tested by planning twice, and by planning a shuffled
  copy of the same edge list).

### Tests by layer

| Layer | Tests |
| --- | --- |
| `figcraft-diagram` | Parser fixtures with line and column of each error; labelled parallel edges parse; a planner `PlanError` for mismatched or non-finite sizes; `proptest` that arbitrary text never panics and respects every limit; plan determinism; a back edge gets its own channel; components stay apart; no route crosses a node box unless `fallback` is set |
| `engine` | `file.newDiagram`: a failed template leaves the current tab active; creation takes `type` `flowchart` and the Flow chart template in slice 2, and `mindMap` is refused until its slice. The starter content is part of the new tab's initial state, so undo does not empty it (a new tab has no undo baseline before its first edit). Journal: `board.insertFlowchart` called directly and inside `command.batch`, checking the large `source` is not kept twice in the journal and `last_command`. The junk-parameter sweep (which today runs on a Design file and is stopped by Board guards) gets a Board with an eligible selection. Atomic insert (a failing source creates no layers and no undo entry); one undo step; arrange on a selection; routes survive save and reopen and an endpoint move; the junk-parameter sweep in `engine/src/tests.rs` extended to the three commands; live-session convergence beside `board_sync_tests.rs` |
| `ui-egui` | Headless tests for the chooser (confirm, Escape, source removed), the insert dialog (error keeps the text), the start dialog, and arrange enablement, in the style of `board_connect_tests.rs` |

### Saved files, live sessions and `BoardStyle`

- `BoardStyle` defaults to `Plain` when absent and is omitted when `Plain`. Tests: a legacy file, a
  Diagram round trip, and what an older reader keeps after resaving (it opens a plain board and
  drops the field; that is the accepted mixed-version behaviour).
- Live sessions: **the style is local in v1, and the wire protocol does not change.** Document
  properties travel in `SetDoc` as the closed enum `DocProp` (`collab/src/change.rs:193`) over
  postcard, and the server decodes `ClientMsg` before dispatch (`collab-net/src/hub/conn.rs:156`),
  so an old server rejects a new variant, and `Hello::client_version` only says what the joining
  client can read, not what the server or the other peers can. A gated variant would need server
  capability discovery before upload, per-recipient filtering of snapshots, reconnect deltas,
  accepted changes and rejection fixes, and preserved sequence numbers when filtering empties a
  batch (`collab/src/client.rs:497`). That is a separate slice, not part of this feature. In v1 a
  peer sees a plain board with every layer intact. The creator keeps the Diagram style when a live
  document replaces the local copy (`engine/src/sharing/live/events.rs:80`): the engine re-applies
  the local style to the replacement of the same document uid and falls back to `Plain` for
  another. Tests: two clients converge on Board content while only the creator has the Diagram UI,
  and the creator still has it after a replacement.
- The `.fig` and Penpot importers that set `FileKind::Board` leave the style `Plain`.
- Recents: the preview reader is the pinned external crate `craft_figcraft_format` and its closed
  `PreviewKind` (`format/src/thumbnail.rs:148`), so this repository cannot expose the style alone.
  In v1 a saved diagram shows the Board mark in Recents and on the home screen's file list; the
  Diagram mark there needs a change in the shared library first (per AGENTS.md, shared format code
  goes through craft-libs). The tab and the new-file card show the Diagram mark.
- Live delivery is measured, not assumed: the encoded change batch of a maximum-size insert,
  arrange, undo and redo (shapes with children, sibling labels, route metadata) is checked against
  the core defaults of 64 MiB per client message and 500,000 changes per batch, the shipped public
  server's 17 MiB, and the server's node limit, and two clients cover acceptance, rejection and
  selective undo after a peer edit. The limits of 100 nodes and 200 edges stay provisional until
  that check.
- Because live sync runs after a local command succeeds, a local success does not mean the server
  accepted it. The tests say which they cover, and slice 4 decides what the user sees when a
  locally successful insert is rejected (the existing live-session error path, not a silent loss).

### Slice order, revised

1. `ConnectedParams`, the chooser and the updated headless drag test (no new crate). The existing
   blank-drop test at `ui-egui/src/board_connect_tests.rs:155` expects immediate creation, so it now
   asserts no edit before confirmation; quick click, sticky drag and joining an existing target keep
   their tests.
2. `BoardStyle` (save and local re-apply after a live replacement; no wire change), `file.newDiagram`, `dialog.newDiagram`, start dialog,
   library.
3. `figcraft-diagram` with `graph`, `parse` and a bounded rank layout, then `board.insertFlowchart`
   and its dialog, using the existing elbow router. Import is useful here, without waiting for
   obstacle routing.
4. `route` and the full `plan` (obstacle avoidance, return channels), then
   `board.arrangeDiagram`, with their acceptance checks.
5. Mind-map type in the start dialog, built on the existing `board.mindMap`. The toolbar entry and
   drag to re-parent stay in gap 24.
6. Typed diagrams (owner decision, 2026-11-19): format version 2 in craft-libs, the typed core,
   then UML Class, ERD and UML Sequence. Their model, commands and tasks are in
   [diagram-type-specifications](diagram-type-specifications.md#tasks) (slices 8–14 there). They
   do not change steps 1–5, which stay plain Board layers.
