# UML Sequence diagram: how we will build and test it

Status 2026-11-20: not started. Revised the same day after two code reviews of the Class and Use Case work and five Codex rounds on this plan. Round 5's four findings are fixed here; that was the last round, so those fixes were not reviewed again. Part A step 1 can start now.

This is the next UML diagram after Class and Use Case. The spec is [§ 3 of the diagram type specifications](diagram-type-specifications.md#3-uml-sequence-diagram-typed-page-kind-sequence), slice 13.

## Order and dependencies

- **Decision D0 puts ERD (slice 12) before Sequence.** This plan follows D0: Part A first (it helps every typed kind, ERD included), then ERD unless the owner reorders, then Part B. If the owner moves Sequence ahead of ERD, record that in D0's row with the date.
- **Saving, loading and importing typed pages need slices 8 and 9.** Slice 8 is format version 2 in craft-libs, and its PR needs owner approval. Slice 9 is the file-wide ID allocator (D5).
  - Until both land, typed pages are a **preview**. They work in the app, but save, load and `file.import` of typed content stay refused, as they are today.
  - The preview has its own done list (below). The full acceptance closes only after 8 and 9.
- **Native-only.** Typed diagrams are native-only in this release (spec summary). One gate decides availability for every typed kind (A8). `file.newDiagram`, `diagram.*` dispatch, load and the chooser all use it, and its browser behaviour is tested natively.

The work has two parts:

- **Part A, groundwork.** Fix what the reviews found in the Class and Use Case code, make adding a typed kind safe, and build the shared export path ERD also needs. No new diagram.
- **Part B, the Sequence diagram.** Built on that groundwork, in small files from the first commit.

## Mistakes in the earlier diagrams, and the guard against each

Each row is something that went wrong or was found in Class or Use Case. The guard makes the mistake fail at compile time or in a test.

| # | Mistake found | Where | Guard |
|---|---|---|---|
| 1 | Each typed kind is listed by hand in about a dozen places: `TYPED_KEYS`, the serde `Key` alias, `TYPED_LAYER_KEYS`, `TYPED_PREFIXES`, `DIAGRAM_TYPES*`, the `*_problem` chain, page duplicate and delete, repair, command registration, and the chooser's types and templates. Forgetting one fails open. | format `typed_probe.rs`, engine `use_case/gate.rs`, `cmd.rs`, `cmd/file.rs`, `cmd/page.rs`, doc `lib.rs`, ui `dialogs/new_diagram.rs` | A1: the kinds are declared once (doc). Each crate dispatches on them with exhaustive matches (no `_ =>` arm), so a new kind is a compile error in every crate until handled. The old arrays are deleted. |
| 2 | Class copied Use Case's bounded readers, and both diagrams reach into each other (`multiplicity_ok`, `find_cycle`, `measure::text_size`, the gate, the glyphs). Use Case keeps its own parameter readers beside `diagram_params`. | doc, engine, ui | A2: shared modules. A test fails if a file in one diagram module tree names a sibling diagram module. |
| 3 | Files over 1,000 lines mixing several jobs. | doc `class_diagram.rs` 1,021, `use_case.rs` 1,097; engine `class_diagram/mod.rs` 1,041, `use_case/mod.rs` 1,008 | A3 splits them. A test fails if a governed file is over 400 lines. |
| 4 | A card that grows into its neighbour is refused, and the error names cards by number. | engine `class_diagram` `add_member`, `measure::remeasure` | A4: refusals name elements by name and say what to do. Sequence lays everything out itself, so nothing can overlap. |
| 5 | `check` ignores operations inherited through generalization. | doc `class_diagram.rs` `Warning::MissingOperation` | A4 fixes it. Rules over relationships use the shared fixed-point closure, tested with chains of at least two links. |
| 6 | `addMember` for a literal accepts documented fields that don't apply; `updateMember` refuses them. | engine `class_diagram` `new_member` | A4: each command and variant declares its allowed fields, nested objects included. The test has a valid fixture for every variant (a literal `addMember` among them) with one inapplicable field injected. |
| 7 | `remove` reports the IDs asked for, not what the cascade removed, in both Class and Use Case. | engine `class_diagram`, `use_case` | A4 fixes both from the before and after counts. The numeric `removed` stays (top-level elements plus links; owned rows such as members don't count), and per-kind counts are added. A link reached through two requested IDs counts once. |
| 8 | Diagram panels call `app.session.execute` directly, skipping what `app.run` does. | `class_panel`, `use_case_panel` | A5: `run` and `run_quiet` share one body. Clippy `disallowed-methods` lists `Session::execute` and `Session::execute_as`; the lint is allowed workspace-wide and denied inside each diagram panel module. |
| 9 | A doc comment describes layers the renderer doesn't draw (`LINK_LAYERS`). | doc `class_diagram.rs` | A4 fixes it. Each renderer has a worst-case layer count test against `layer_estimate`. |
| 10 | The bounded `pages` decoder keeps the last of two equal page keys. | doc `class_diagram.rs`, `use_case.rs` | A2: the shared decoder refuses a repeated key. |
| 11 | Resize state holds only a model-local ID, and a release outside the window is dropped. | ui `diagram_resize.rs` | A5: key the state by file, page and element; keep the last effective preview; test pointer loss, cancel and a page switch mid-drag. |
| 12 | Use Case placement subtracts a fixed half size (23, 51), but an actor is measured (61.9 wide), so it isn't centred on the press. The test is right. | ui `use_case_panel/canvas.rs` `place` | A6 places using the size the engine measures. The test's assertion stays. |
| 13 | A blank typed page whose name starts with `-` counts as a divider, and `page.select` then won't open it. | doc `lib.rs` `is_divider` | A4: typed pages are never dividers. A test creates and reselects a typed page named `-x`. |
| 14 | In-app testing found bugs the headless tests missed: Escape couldn't close a refused draft; a new element from the handle wasn't selected; the chooser was too wide. | ui `class_panel/edit.rs` | Part B: every gesture has a headless test, then a running-app pass with screenshots before the slice counts as done. |
| 15 | Fixes after a Codex review were not reviewed again. | S1 and C2 | Each part's last fixes go back to Codex until it approves or five rounds run out. The plan records which. |

## Part A, groundwork

### What can be done now, and what waits for slices 8 and 9

Part A is done now, except these items, which are written below so their design is fixed but are built with slices 8 and 9:

| Waits for | Item |
|---|---|
| Slice 8 (format v2) | The writer choosing version 2 whenever a typed record exists anywhere in the archive (current document, stored versions, branch base, head and history); reading `typed` in a v2 file; the v2 load contract (every snapshot checked before publishing and before repair, the aggregate layer limit, regeneration in the shared load path, and open, recovery, branch switch and `version.restore`, `.preview` and `.duplicate` through `regenerate`); the D11 live checks for `file.import` and `version.restore`; the A8 load test on the same v2 bytes on `Native` and `Web` |
| Slice 9 (D5) | The whole file-wide ID allocator, and remapping IDs through it on page duplicate and import |
| Both, together | Turning on typed save, load and import. This is one milestone after both slices land, never one without the other |

Everything else in A1–A8 is built now, while typed load, save and import stay refused:

- The v1 snapshot check is built now, because it protects v1 files today.
- The parent-and-child consistency check is built now and unit-tested on a document. Its test through a typed file waits for v2, because v1 already refuses the typed record.

### Order of work

One commit per step, each with its tests. The suite must pass after each step, apart from the failure A6 fixes, which is fixed first. Hours are estimates for one agent, tests included.

| Step | Work | Hours | Why here |
|---|---|---|---|
| 1 | A6: actor placement; record the test baseline | 1–2 | The suite is green before the refactors |
| 2 | A3: split the four big files, with no behaviour change, plus the 400-line test | 3–5 | Smaller files make the next moves reviewable |
| 3 | A2: shared modules and the sibling-import test | 4–6 | A1's envelope reader and A4's fixes use them |
| 4 | A1: `typed_kinds!`, `TypedPages`, engine dispatch, probe rules, selection sub-roles | 8–12 | The largest change; it rests on A2 |
| 5 | A4: the Class and Use Case findings (rows 4–7, 9 and 13) | 4–6 | Each fix with a test that fails without it |
| 6 | A5: `run_quiet`, the clippy guard, the resize state | 3–4 | |
| 7 | A7: one export path for typed selections | 4–6 | Needs A1's `expand_selection`; before ERD |
| 8 | A8: native-only gate | 2–3 | Needs A1's dispatch |
|  | Total for Part A now | 29–44 | |

**Handoffs between steps**, so that no step uses something a later step makes:

- Step 1 (A6) exposes the measure that Use Case already has. Step 3 then moves it into `typed::measure` unchanged.
- Step 3 (A2) builds the envelope reader without knowing any kind. It reads `kind` and `schema` as plain values and hands back the raw `model`.
- Step 4 (A1) adds `TypedKind` and wires kind and schema dispatch onto that reader. It creates `typed::dispatch` with `specs`, `templates`, `new_page` and `regenerate` only.
- Step 7 (A7) adds `expand_selection`, and step 8 (A8) adds `available`.

The step 2–4 refactors go to Codex as one review, and steps 5–8 as a second. Each review goes up to five rounds, including the last fixes. These are then proposed as two PRs: "typed groundwork" (steps 1–4) and "typed fixes, export and gate" (steps 5–8), committed only when the owner asks.

### A6. Actor placement (step 1)

Place an element centred on the press using the size the engine measures for it: the engine exposes its measure for a new element's name, and the panel uses it. The existing test keeps its assertion. Then record the baseline: the full `cargo test` output of the affected crates.

### A1. One typed-kind declaration, dispatched in each crate

**doc (L1)** declares the kinds once, with plain metadata only:

```rust
typed_kinds! {
    UseCase => { model: use_case::Model, key: "useCase", layer: use_case::LAYER_KEY, prefix: "diagram.useCase." },
    Class   => { model: class_diagram::Model, key: "class", layer: class_diagram::LAYER_KEY, prefix: "diagram.class." },
}
```

The macro generates `TypedKind` (a `Copy` enum with `ALL` and `COUNT`) and `TypedModel` (each kind's `Arc<Model>`). It also generates `type_id`, `layer_key`, `command_prefix` and `kind_of`, all exhaustive matches.

- The gate's layer keys and command prefixes are derived from `ALL`; the old arrays are deleted.
- The archive's legacy field names (`use_cases`, `class_diagrams`) are a separate, fixed compatibility list. They are not tied to the kinds.

**format (L3)** uses the wire metadata and doc's validation.

**engine (L5)** owns behaviour as free functions in `figcraft_engine::typed::dispatch`, each an exhaustive `match kind`:

- `specs(kind) -> Vec<CommandSpec>` (command registration loops over `ALL`);
- `templates(kind)`, `new_page(kind, template)`;
- `regenerate(page, &TypedModel)`;
- `expand_selection(&TypedModel, ids)`, added in step 7 (A7);
- `available(kind, platform)`, added in step 8 (A8).

Rust allows inherent methods only in the crate that owns the type, so these are free functions, not methods on the doc enum.

**ui (L6)** reads the engine's list of available kinds and their templates. It keeps no list of its own.

**Document storage.** `Document` holds one `typed: TypedPages` in place of `use_cases` and `class_diagrams`. `TypedPages` wraps a `BTreeMap<NodeId, TypedModel>`:

- `insert(page, model)` is refused if the page has a model, or if the page cap is reached (the same cap the decoder applies, so a writer can't produce a file its reader refuses).
- `replace(page, model)` is refused if the kind would change.
- `remove` is `pub(crate)` in doc, and called only from page deletion (`Document`'s page delete, which already removes the models today) and repair. So a live page can't be emptied and refilled with another kind.

**On disk.** The format is `"typed": {"<page id>": {"kind": "...", "schema": 1, "model": {...}}}`.

- The envelope's `schema` replaces the inner `schema` fields Class and Use Case have today.
- Decoding is bounded and two-step. Step one reads the envelope with `model` as a borrowed raw value (serde_json's `raw_value` feature, the same crate and licence) and checks its byte length. A repeated envelope field, an unknown `kind` and an unknown `schema` are each refused. Step two decodes `model` with that kind's capped readers, so nothing is materialised as a `serde_json::Value`.
- A repeated page key is refused, and the page cap applies.

**Legacy keys and the probe.** The probe depends on the file version. It checks the current document, every stored version and every branch (base, head and history):

- The legacy keys `use_cases` and `class_diagrams` are refused in every version, so a file from this branch's earlier builds is refused, never silently stripped.
- `typed` is refused in a version 1 file, which covers the whole preview.
- `typed` is read in a version 2 file (after slices 8 and 9), as below.

**Load, import, restore and duplicate.**

- In the preview, typed load and import stay refused, as today.
- After slices 8 and 9, a v2 load checks every typed record in the current document, every stored version and every branch before it publishes anything, and before repair runs.
  - If any snapshot can't be checked, the whole file is refused. Examples: a history skipped at its byte budget, or a damaged branch the reader would otherwise drop.
  - The aggregate `layer_estimate` over all snapshots is checked against one limit before any regeneration. The writer refuses a file over the same limit.
- Regeneration runs inside the engine's shared load function (`cmd/file/load.rs`), before any caller can list page children. The CLI's `export`, which calls that loader directly and reads page children from what it returns, therefore always sees generated layers. Test: CLI export of a model-only v2 file from disk shows the diagram.
- Every way a snapshot reaches the canvas goes through the engine's one `regenerate`:
  - open, recovery and branch switch;
  - `version.duplicate`, which today reads the version and opens a tab directly (test: duplicate a model-only snapshot and see its layers);
  - `version.restore` and `version.preview`.
  `version.preview` gets a test that previews a model-only typed snapshot and shows its layers.
- Regeneration rebuilds the whole page. A typed page is refused on load if it has an ordinary page child, or an untagged descendant inside a generated group (D2). Parent and child links touching a typed subtree must agree, and this is checked before repair. Otherwise a node that names a generated group as its parent, without being in the group's child list, would escape the walk and be erased by repair. Test: that parent-only case is refused.
- **Every snapshot is checked, in v1 and v2.** The check is bounded, and if it can't finish, the file is refused. That includes a snapshot member whose `versions/index.json` is missing: today the reader returns empty history, and a resave would erase it. Fixture: a v1 file with a legacy key in a snapshot whose index is missing.
- **Live checks (D11).** When v2 lifts the preview refusals, `file.import` of typed pages and `version.restore` of a typed snapshot each get the D11 check before they append pages or change history. The check covers tabs that are connected, connecting, reconnecting or resumable. Today `version.restore` has only `live_read_only`, which writable live tabs pass.
- **Duplicate.** Page duplicate copies the model as it is while IDs are per model. With D5 (slice 9), duplicate and import remap every ID through the file allocator. Tests check that every reference in the copy is remapped.
- **Selection.** After every regeneration, the selection is restored by (page, semantic ID, role).
  - Roles are unique within an element. A role that repeats gets a stable sub-role, for example a Class link's `label.sourceMultiplicity` and `label.targetMultiplicity`, or `marker.source` and `marker.target`.
  - A test selects each layer of a link, forces a regeneration, and checks that the same layer is selected.

**Tests, each in its own crate so layering holds:**

- **format:** the probe refuses a page of every `TypedKind` and each legacy key, in the current document, a stored version and a branch. The envelope decoder refuses a repeated field, an unknown kind or schema, an oversized model and a repeated page key.
- **engine:** every `TypedKind` has registered commands, appears in `file.newDiagram`, and its layers are refused by the generic-edit gate. The existing typed safety tests (generic edits, `edit.repeat`, nested `command.batch`, history-only typed records, live and merge refusals) loop over `TypedKind::ALL`.
- **ui-egui:** the chooser lists exactly the engine's available kinds.

### A2. Shared modules

| Module | Takes from | Holds |
|---|---|---|
| `figcraft_doc::typed::bounded` | `use_case::bounded`, `class_diagram::bounded` | `Capped`, `Text`, `OptText`, `Pages` (refuses a repeated key), the envelope reader, `MAX_PAGES` |
| `figcraft_doc::typed::uml` | `use_case::multiplicity_ok` | The multiplicity grammar |
| `figcraft_doc::typed::graph` | `use_case::find_cycle` | Cycle search; the fixed-point closure helper |
| `figcraft_engine::typed::gate` | `use_case/gate.rs` | The generic-edit gate |
| `figcraft_engine::typed::measure` | `use_case/measure.rs` | `text_size`, `text_data`, and the new-element measure A6 uses |
| `figcraft_engine::diagram_params` (exists) | readers Use Case keeps for itself | `req_id`, `req_text`, `position`; `allowed(cmd, p, fields)`, which refuses any field outside the list (nested objects too); `nullable(cmd, p, key)`, which tells omitted, `null` and a value apart |
| `figcraft_engine::typed::commit` | both `mod.rs` | `target`, `commit` (no-op when equal, validate on a copy, store, regenerate) |
| ui `diagram_kit::glyphs` | `use_case_panel/glyphs.rs` | The pictures both panels use |

A test lists every file in each diagram module tree (doc, engine and ui) and fails if one names a sibling diagram module.

### A3. Split the big files (no behaviour change)

- doc `class_diagram/`: `model.rs`, `validate.rs`, `check.rs`, `connect.rs`, `text.rs` (exists).
- doc `use_case/`: `model.rs`, `validate.rs`, `check.rs`.
- engine `class_diagram/` and `use_case/`: `elements.rs`, `members.rs` (Class only), `links.rs`, `lifecycle.rs`, `example.rs`, `specs.rs`, plus the existing `layout.rs`, `measure.rs` and `render.rs`.
- The 400-line limit covers every file named here and every Part B file. A test holds the list and fails above the limit. The existing panel files are outside it for now (their split is recorded in gaps as follow-up work).
- A3 is accepted when there are no regressions against the A6 baseline.

### A4. Fix the Class and Use Case findings

Rows 4–7, 9 and 13 of the table. Each fix comes with a test that fails without it.

### A5. UI fixes

- `run` and `run_quiet` share one body; both panels use `run_quiet` where they need the error without a toast.
- The clippy setup:
  - `[workspace.lints.clippy] disallowed_methods = "allow"`;
  - `clippy.toml` lists `figcraft_engine::Session::execute` and `figcraft_engine::Session::execute_as`;
  - each diagram panel module starts with `#![deny(clippy::disallowed_methods)]`.
  Confirm it once by hand: a direct call in a panel fails `cargo clippy -D warnings`, and `app.run_quiet` passes. Record the result in the PR.
- Resize state per file, page and element, and keep the last preview (row 11).

### A7. One export path for typed selections (before ERD)

The engine resolves the export selection once: `typed::dispatch::expand_selection`, one pass, as D10 says.

- That prepared set is passed through sizing and encoding. The fixed-size path no longer computes bounds a second time.
- For a typed selection, the resolved set also decides what is drawn, even with `contentsOnly: false`. Today `drawn_layers` returns the whole page in that case, so an unrelated layer inside the bounds would appear. Test: a fixed-width export of `A—B` with `contentsOnly: false`, where `C` lies inside their bounds, doesn't show `C`.
- The same set drives `file.export`, the egui export and the CLI.
- Fixtures:
  - Class: `A—B—C` with only `A` selected, at a fixed width;
  - Use Case: a subject with an outside actor;
  - a full page of each kind.

### A8. Native-only gate

`typed::dispatch::available(kind, platform)` takes the platform as a parameter, and the build picks the default (`Web` on `wasm32`, `Native` otherwise). `file.newDiagram`, `diagram.*` dispatch, load and the chooser all call it.

Native tests pass `Platform::Web`, with the preview preference set on explicitly, and check that creation, dispatch and the chooser are refused. In the preview, every typed load is refused on every platform, so the load test waits for v2. After slices 8 and 9, the same valid v2 bytes must open on `Native` and be refused on `Web`. That includes a file whose only typed record is in history. `cargo xtask wasm` still has to compile.

## Part B, the Sequence diagram

### Files (none over 400 lines)

```
crates/doc/src/sequence/
  mod.rs          re-exports, caps, LAYER_KEY
  model.rs        Participant, Message, MessageKind, Activation, newtype IDs
  validate.rs     validate() and SequenceError (thiserror, one variant per rule)
  cascade.rs      the removal closure
crates/doc/src/sequence_tests.rs

crates/engine/src/sequence/
  mod.rs          specs() only
  participants.rs addParticipant, moveParticipant, removeParticipant
  messages.rs     addMessage, updateMessage, moveMessage, removeMessage
  activations.rs  setActivation, removeActivation
  read.rs         read
  layout.rs       the pure geometry, shared by the renderer, export and the UI
  render.rs       layers from the layout
  example.rs      the acceptance example as a template
crates/engine/src/sequence_tests.rs

crates/ui-egui/src/sequence_panel/
  mod.rs          state (per file and page), show
  tools.rs        the picture tools and the selected-element card
  canvas.rs       hit-testing, select, drag to reorder
  keys.rs         the keyboard
crates/ui-egui/src/sequence_panel_tests.rs
```

Registering the kind takes one new line in `typed_kinds!`. The compiler then lists every match in doc, format, engine and ui that must handle `Sequence`.

### Model

- `Participant { id, name }`, in display order.
- `Message { id, kind: MessageKind, from, to, label, of: Option<MessageId> }`, with `MessageKind` one of `Call`, `Async` or `Reply`. A self-message has `from == to`.
- `Activation { id, participant, start, end }`, where `start` and `end` are message IDs.
- Newtype IDs; `deny_unknown_fields`; the shared bounded readers.
- Caps:
  - 100 participants;
  - 500 messages;
  - 200 activations;
  - 4 nesting levels on one participant;
  - 200 characters per name or label.

### Validator, in this order

Each rule has its own `SequenceError` variant, and each test asserts the exact variant.

1. **Schema, counts and text** (`TooMany`, `TextTooLong`, `TextHasLineBreak`). This runs before anything else is built.
2. **Bounded indexes.** ID to position maps, built once and refusing a repeated ID (`DuplicateId`).
3. **Ordinary references.** A message's `from` and `to`, and an activation's `participant`, `start` and `end`, exist (`MissingParticipant`, `MissingMessage`). `of` is not checked here.
4. **`of`.** Only a `Reply` may carry it (`OfOnNonReply`). For a reply with `of`, each case has its own variant:
   - the target doesn't exist (`ReplyTargetMissing`);
   - the target isn't a `Call` (`ReplyTargetNotCall`);
   - the endpoints aren't reversed (`ReplyEndpointsNotReversed`);
   - the reply isn't later than its call (`ReplyBeforeCall`).
   A reply without `of` is allowed.
5. **Activations**, last, because they need positions:
   - `ActivationOffLifeline`: `start` and `end` each have `from` or `to` equal to `participant`.
   - `ActivationBackwards`: `start` comes strictly before `end`, as § 3 says. There is no one-event bar.
   - Intervals on one participant, as closed position intervals `[s, e]`. Two activations are either disjoint (`e1 < s2`) or nested (`s1 <= s2` and `e2 <= e1`, not identical). Identical intervals are refused (`DuplicateActivation`). Touching intervals (`e1 == s2`) are not nested, so they are a `Crossing`. Shared starts or ends are allowed only in a nesting.
   - `NestingTooDeep`: more than 4 levels.

Every edit re-runs the whole validator on a copy. An update that breaks an existing reply or activation is refused the same way. A refusal names participants and messages by name and label, not by ID, and the model is left unchanged.

### Commands (§ 3's signatures, with the details § 3 leaves open)

| Command | Details |
|---|---|
| `addParticipant {name, after?}` | Without `after`, it goes last. An `after` naming no participant is refused. Returns `{id}`. |
| `moveParticipant {id, before?\|after?}` | Exactly one anchor. An anchor naming no participant is refused, with the model unchanged. Only once the anchor is known to exist is a move to where it already is, or relative to itself, a no-op (no undo step). |
| `addMessage {kind, from, to, label, after?\|before?, of?}` | Without an anchor, it goes last. Both anchors, or an anchor naming no message, is refused. Returns `{id}`. |
| `updateMessage {id, kind?, from?, to?, label?, of?}` | An omitted field stays. Only `of` takes `null`, which clears it (read with `diagram_params::nullable`). `null` for `kind`, `from`, `to` or `label` is refused. |
| `moveMessage {id, before?\|after?}` | As `moveParticipant`: an unknown anchor is refused before the no-op check. |
| `removeMessage {id}`, `removeParticipant {id}` | The removal closure below. Returns `{removed, participants, messages, activations}`, where `removed` is the total. |
| `setActivation {participant, start, end}` | Returns `{id}`. The same participant, start and end again returns the existing ID as a no-op. |
| `removeActivation {id}` | Removes one bar. |
| `read {page?}` | Participants in display order and messages in order, with IDs. Activations are sorted by start position, then outer before inner. |

**`null` rule, for every optional argument of every command.** `updateMessage.of` is the only argument that takes `null` (it clears the link). `null` anywhere else is refused, not treated as omitted. That covers `after`, `before`, `addMessage.of`, `read.page`, and `kind`, `from`, `to` and `label` in `updateMessage`. Each command has a test that sends `null` for each of its optional arguments.

Each command declares its allowed fields through `diagram_params::allowed`. Each change is one undo step; a refusal or a no-op adds none. Undo and redo restore the whole model and the regenerated layers.

### Removal closure (`cascade.rs`, a fixed point on the shared helper)

Start from the IDs to remove, then repeat until nothing is added:

- add every message whose `from` or `to` is a removed participant;
- add every reply whose `of` is a removed message;
- add every activation whose `participant` is removed, or whose `start` or `end` is a removed message.

Each item is counted once, even when two rules reach it.

### Layout (pure, in `layout.rs`; one result for renderer, export and hit-testing)

Let `x` be a lifeline's position and `y` the top of an event's row.

**Headers:**

- A header is the name's width plus 24, at least 80 wide, and one text line plus 16 high.
- Headers sit at one pitch: the widest header plus 40. That is § 3's "40-unit gap after the widest header in the row".
- The smallest pitch is 120. A participant's centre is its header's centre.

**Lifelines:**

- They start one text line below the tallest header.
- They end 40 below the last event's row, or 40 below the headers when there are no messages.
- With no participants, nothing is drawn.

**Rows:** each event takes 48 units, and its arrow runs at `y + 16`.

**Activation bars:**

- A bar is 10 wide, with its left edge at `x + 6k` for level `k` (0-based).
- At the cap (level 3), the deepest bar spans `x + 18` to `x + 28`.
- A bar runs from its start event's arrow to its end event's arrow. When the end event is a self-message on that participant, the bar runs on to the loop's return at `y + 36`, so the returning arrowhead lands on the bar. Test: a self-message that ends the innermost activation.

**Arrow ends:**

- An arrow end uses the innermost bar covering its event on that participant, or the lifeline when none does.
- It attaches to that bar's edge facing the other end.

**Self-messages.** It leaves at `y + 16` from the right edge of its innermost bar (or the lifeline). It runs right to its tip, down to `y + 36`, and back. The tip is at `x + 32`, or 12 right of the bar's right edge when that is further, as § 3 now says. That gives `x + 40` at the cap. (§ 3's row rule now also says "at least 48".) At the smallest pitch, the tip stays 80 units short of the next lifeline.

**Drawing by kind:**

| Kind | Line | Arrowhead |
|---|---|---|
| Call | solid | filled |
| Async | solid | open |
| Reply | dashed | open |

**Labels:**

- One line, above the arrow. A name or label with a line break is refused by the validator (`TextHasLineBreak`), so a label can't wrap.
- The label's line is measured, never assumed.
  - Rows are `max(48, 2 + line height + 2 + 20 + 12)`, one height for the whole diagram.
  - The arrow sits at `y + 2 + line height + 2`, and a self-loop returns 20 below it.
  - The last 12 is room for the return stroke and arrowhead, so their painted bounds stay above the next row.
  - With the usual label size (a 12-unit line) a row is 48, the arrow is at `y + 16` and the return at `y + 36`.
  - A font with tall glyphs grows every row by the same amount, and the self-loop return and the bars move with the arrow.
  - The fixed numbers in this section are the values for the usual size.
- A message label is centred on its span and at most the span minus 8 wide. A self-message's label sits right of the tip and is at most the pitch minus the tip offset minus 8 wide.
- Text past 40 characters, or past the available width, is drawn with an ellipsis. The model keeps it whole.
- A test checks that every label's painted bounds and every self-loop return leg stay inside its row. It runs once with the usual font and once with a measure that reports tall glyphs.

### Export

Through A7's one path. A selected participant adds its messages and their other endpoints. A selected message adds its two participants.

## Tests

| Level | File | What it checks |
|---|---|---|
| Model and validator | `crates/doc/src/sequence_tests.rs` | Each rule with its exact variant and the model unchanged; identical, touching, nested and shared-end intervals; `of` on a call; each reply case; an update that breaks an existing reply or activation; serde round trip; unknown field; caps on decode; random edits never panic and agree with a brute-force check |
| Cascade | same | A message's linked reply and an activation ending on it go too; a participant's removal takes an activation on another participant; an item reached twice is counted once |
| Layout | `crates/engine/src/sequence/layout.rs` unit tests | Header size and pitch; empty diagram; lifeline start and end; the 48 row; bars up to the cap; arrows on the facing bar edge in both directions; the self loop tip with and without bars and its clearance at the smallest pitch; labels at 40 and 41 characters and at the span limit; everything inside its row |
| Renderer | same crate | Worst-case layer count within `layer_estimate`; distinct line and head per kind |
| Engine | `crates/engine/src/sequence_tests.rs` | The acceptance example through `Session::execute`; anchors (default, both, unknown, self); `of: null` and `null` refused elsewhere; `setActivation` twice; one undo step per change, none for refusal or no-op; undo and redo restore model and layers; one inapplicable field injected into each command's valid fixture is refused, state unchanged; the junk sweep; save refused during the preview |
| Registry, structure, platform | format, engine and ui-egui (A1, A2, A3, A8) | Every kind in the probe, gate, commands, `file.newDiagram` and chooser; legacy keys refused; no sibling imports; the 400-line limit; `Platform::Web` refusals |
| Headless UI | `crates/ui-egui/src/sequence_panel_tests.rs` | Place a participant; drag to reorder; add a message; drag a message; a refused reorder shows its reason and changes nothing; Delete removes a message with its cascade; Escape drops the tool, then the selection |
| Running app | control channel, then the mouse | The acceptance example (preview part); a screenshot after each step, read as an image; `diagram.sequence.read` after each step |

Each test checks one behaviour and is named for it, for example `remove_message_also_removes_an_activation_ending_on_its_reply`. Fixtures turn the diagram preview preference on explicitly, because it is off by default in release builds.

## Acceptance example (§ 3, as one test)

Participants `Requester`, `Review service`, `Store`. Messages in order:

1. A self-call on `Review service`.
2. `m1` call `Review service` → `Store`.
3. `m2` and `m3` async `Review service` → `Store`.
4. `m4` reply `Store` → `Review service` with `of: m1`.

Activations on `Store`: A = `[m1, m4]`, B = `[m2, m3]` (nested, legal).

Then:

- Legal: move `m1` above the self-call.
- Rejected, unchanged, `ReplyBeforeCall`: move `m4` above `m1`.
- Rejected, unchanged, `Crossing`: move `m1` after `m2` (`m2 < m1 < m3 < m4`; the reply order still holds).
- After slices 8 and 9: reorder participants, save, reopen, and check the IDs, the order and the activations.

## Done when

**Part A (now):**

- Steps 1–8 are done, except the items that wait for slices 8 and 9.
- No regressions against the A6 baseline at each step, and the full suite passes at the end.
- The registry, structure, export and platform tests pass.
- `cargo xtask layers` and `cargo xtask wasm` pass.

**Part B, preview:**

- Every test in the table passes, except save and reopen.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` are clean.
- `cargo xtask ci` passes, or its failing part is named with the reason.
- Screenshots of the running app are checked.

**Part B, complete:** the preview list, plus save and reopen after slices 8 and 9.

**Every part:**

- Codex reviewed it (at most five rounds), including the last fixes.
- The spec's status row, the gap entry and the revision history are updated in the same commit.
- Nothing is committed until the owner asks.

## Open questions for the owner

- Keep D0's order (ERD, then Sequence), or move Sequence first?
- Open the craft-libs format v2 PR (slice 8) now? Until it and slice 9 land, nothing typed can be saved, loaded or imported.
