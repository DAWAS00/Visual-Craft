# Diagram page: UI design spec

Status: proposal, 2026-11-18. Nothing here is built. It turns the owner's answers to a design review
into screens, flows and slices. The behaviour and engine plan stay in
[diagram-implementation-plan.md](diagram-implementation-plan.md); the catalog of diagram types and
the vendor workflow notes stay in [diagram-feature-review.md](diagram-feature-review.md). The
reference product is Visual Paradigm CE 18.1, studied from its public guides only (clean room, see
[AGENTS.md](../AGENTS.md)): nothing from its install, icons or symbols is used.

## Decisions taken

| # | Question | Decision | Why |
|---|---|---|---|
| D1 | Where it lives | A new **Diagram** file kind the user sees: File ▸ New diagram, its own mark on tabs and the home screen | A page of its own, as asked; the Boards objects underneath give save, undo and live sessions for free |
| D2 | First screen | A **start dialog** (type and template), then the canvas | The first choice is the diagram type; a blank canvas hides templates |
| D3 | First release | **Flowchart and mind map** first, then the typed UML Class, ERD and UML Sequence diagrams (owner decision, 2026-11-19) | Flowchart and mind map run on plain Board layers; the typed diagrams need their own model, specified in [diagram-type-specifications](diagram-type-specifications.md) |
| D4 | Input | **Visual first**; text import (**Insert diagram…**) is a power feature | Most users will place and connect; text is the fast path for people who already have a flow written |
| D5 | Layout of the screen | **Reuse the Board chrome** and add one left **shape library** | Fewest new widgets, consistent with the rest of the app |
| D6 | Auto layout | A **manual Arrange diagram** action, left to right or top to bottom | Hand placement is never undone behind the user's back; one undo step |

## Non-goals for the first release

- SysML, BPMN and the UML diagrams other than Class and Sequence. Class, Sequence and ERD come after the flowchart slices, as typed diagrams ([diagram-type-specifications](diagram-type-specifications.md)).
- A model shared between views (rename here, update there). Each diagram owns its layers.
- Live auto layout after every edit (mind maps keep their own).
- Importing Visual Paradigm projects. SVG artwork import already exists.
- Any symbol, icon or template copied or traced from another product.

## How it is stored (the one engineering decision that needs the owner)

`FileKind` is a closed enum (`Design`, `Board`; `crates/doc/src/board.rs:18`). `is_board` is read in
28 places across 15 files (26 outside tests; the whole word, not names that merely contain it). A third variant would need every one of them reviewed, and an older
FigCraft opening the file would fail to read an unknown variant.

**Proposal:** the user-facing kind is Diagram; the stored kind stays `Board`, with an optional,
skipped-when-default document field `board_style: "diagram"` (name to be settled). Effects:

- Board commands and the normalize pass keep working unchanged. The field itself must be carried
  by the native save; the `.fig` and other importers leave it at its default. The style is local
  to the creator in v1 (the live protocol does not change; a peer sees a plain board, and the
  creator keeps the style after a live replacement), and Recents keeps the Board mark because its
  reader is an external crate. Slice 2 adds a save round trip, a legacy-file test and the
  two-client test described in the plan.
- Older versions open a diagram as a plain board: the shapes, connectors and labels are intact,
  and only the Diagram mark and shape library are missing there. A diagram saved again by an older
  version loses the style.
- Only the UI asks `is_diagram`: the tab mark, the home card, the start dialog, the shape library,
  and the toolbar's extra entries.
- A later real `FileKind::Diagram` stays possible; the field migrates to it.

This keeps D1 as the owner chose it and avoids the 15-file audit.

## Screens

Sizes follow the existing Board chrome. Colours come only from `theme::Tokens`; nothing is hard
coded. Glyphs are drawn in code (`crates/ui-egui/src/icons.rs`), designed from scratch.

### 1. Home screen card

```
 New file
 +----------+ +----------+ +----------+
 |  design  | |  board   | | diagram  |   <- new card, same size as the others
 |  [mark]  | |  [note]  | | [nodes]  |
 +----------+ +----------+ +----------+
```

The card raises `dialog.newDiagram`, which opens the start dialog (a UI request, never a dialog
from a programmatic call). File ▸ New diagram does the same.

### 2. Start dialog

```
+--------------------------------------------------------------+
| New diagram                                              [x] |
|--------------------------------------------------------------|
|  Type              Template                                  |
|  (•) Flowchart     +--------+ +--------+ +--------+          |
|  ( ) Mind map      | blank  | | flow   |                      |
|    (slice 6)       |        | | chart  |                      |
|  Name              +--------+ +--------+                      |
|  [Untitled diagram____________]                              |
|                                       [Cancel]  [Create]     |
+--------------------------------------------------------------+
```

- Left: type list (two entries in v1). Right: template cards drawn from a real preview, as the
  Board templates picker already does (`widgets/board_picker.rs`).
- Blank is first and selected. Enter creates, Escape cancels. Focus starts on the Name field.
- Templates in slice 2: Blank and the existing Flow chart. More arrive with their own slices. The
  Mind map entry is hidden until slice 6.
- Create makes the file with the template as its starting content (the new tab's initial state, so
  undo does not empty it). A failed template leaves the current tab active.
- The window narrows to a single column below 640 px.

### 3. The editor

```
+----------------------------------------------------------------------+
| tab bar:  [diagram mark] Review flow                                 |
+--------+-------------------------------------------------------------+
| SHAPES |                                                             |
| [find] |        (canvas: dot grid, as boards)                        |
| Flow   |                                                             |
|  [ ] [ ]   [Review] ---> <Approved?> --Yes--> (Ship)                 |
|  [ ] [ ]                    |                                        |
| Basic  |                    No                                       |
|  [ ] [ ]                    v                                        |
|        |                 [Revise]                                    |
| (hide) |                                                             |
+--------+-------------------------------------------------------------+
|     bottom toolbar (Board toolbar + Insert diagram + Arrange)        |
+----------------------------------------------------------------------+
```

- **Left shape library**: collapsible, 200 px, searchable, grouped. Flow: Process (rectangle),
  Decision (diamond), Terminator (rounded rectangle), Data (parallelogram), Document, Database,
  Predefined process, Manual input, Internal storage. Basic: the other board shapes (ellipse,
  triangle, chevron, and so on). Click arms the tool; drag places. Every entry is one of the 23
  existing `BoardShape` variants, regrouped and named, so no new shape art is needed. A "stored
  data" symbol is not among them and would be a new shape (rendering, a new variant kept at the end
  of the binary encoding, an ATTRIBUTION row), so it is out of this release.
- **Bottom toolbar**: the Board toolbar unchanged, plus two buttons next to Templates:
  **Insert diagram…** and **Arrange**.
- **Contextual bar** over the selection: unchanged, plus a shape-type menu (Process, Decision,
  Terminator) that swaps the type in place.
- The Design panel stays hidden until asked for, as on boards.

### 4. The "+" handle and the shape chooser

A click on a selected shape's **+** handle still makes the next shape of the same kind in one step
(existing behaviour, `canvas/board_connect.rs`). A **drag** that ends on empty canvas opens a
small chooser at the drop point:

```
          drop point
              •
        +---------------+
        | Same shape    |
        | Process       |
        | Decision      |
        | Terminator    |
        +---------------+
```

- Up/Down moves, Enter confirms, Escape cancels with no change; a click outside cancels. The
  chooser takes its keys before the canvas shortcuts run (the selected source is still a canvas
  target, and an unconsumed Up or Down would nudge it): a test presses Down, then Escape, and
  asserts the source's geometry and the undo depth are unchanged.
- Confirming creates the shape and the arrow as one undo step and starts text editing in the new
  shape. Dropping on an existing shape keeps the existing join behaviour.
- The pending choice is cleared if the source, page or file disappears, or editing access changes.

### 5. Insert diagram dialog

```
+--------------------------------------------------+
| Insert diagram                               [x] |
|--------------------------------------------------|
| flowchart LR                                     |
|   review["Review request"] --> decision{"Approved?"} |
|   ...                                            |
| [multiline text field]                           |
|                                                  |
| Line 3, column 12: expected a node after `-->`   |  <- error, field keeps its text
|                          [Example] [Cancel] [Insert] |
+--------------------------------------------------+
```

- Opens only in the interactive UI. The parameterised command `board.insertFlowchart` stays callable
  from the CLI, MCP and control channel.
- The text stays on any error. Nothing is inserted on error: zero layers, no undo entry.
- Insert places the result on the active page, selects it, and the whole insertion is one undo step.

### 6. Arrange

A split button: **Arrange ▾** with *Left to right* and *Top to bottom*. It is disabled, with a
tooltip that says why, when the selection has fewer than two shapes, holds any object other than
unlocked board shapes, or has no connection between the selected shapes. Connectors and connector
labels in the selection are ignored, so Select all then Arrange works; the engine and the button
share one eligibility function. It runs `board.arrangeDiagram` and
reports route fallbacks in a toast ("1 connector couldn't avoid a shape") with the usual undo.

### 7. Empty and first-use state

A blank diagram shows a dismissible card at the canvas centre: **Place a shape from the left, then
drag its + handle to continue the flow.** Plus two buttons: *Insert from text* and *Start from a
template*. The card goes away on the first edit and never returns for that file.

## The workflow, end to end

1. Home ▸ **Diagram** ▸ choose **Flowchart**, **Blank**, **Create**.
2. Drag **Process** from the library; type "Review request".
3. Drag its **+** handle to an empty spot; choose **Decision**; type "Approved?".
4. Drag from the diamond to **Terminator** "Ship"; double-click the arrow, type "Yes".
5. Drag from the diamond downward to **Process** "Revise"; label it "No"; join "Revise" back to
   "Review request".
6. Select the four shapes (Select all also works when the page holds only shapes, connectors and
   labels; any other object in the selection disables Arrange with a reason), then
   **Arrange ▾ ▸ Left to right**. The retry arrow takes a visible return channel.
7. Move the decision: its arrows and labels follow. Undo takes back the move.
8. Save as `.figcraft`, reopen, export SVG or PDF with both labels.

The same graph comes from **Insert diagram…** in one step.

## States, keyboard, accessibility

| Topic | Rule |
|---|---|
| Focus | Dialogs focus their first field. Closing returns focus to the canvas. Escape cancels before any document edit. |
| Keyboard | Library: arrow keys move, Enter arms the tool, `/` focuses search. Chooser: arrows, Enter, Escape. Arrange: a command with a shortcut to be assigned in the shortcut table. |
| Screen readers | Every control has a name and role through `a11y`; the chooser and library are lists with selected state. (Gap 22 says screen readers have never been run; this spec does not claim they work.) |
| Targets | 32 px toolbar cells and at least 24 px list rows, as elsewhere in the app. |
| Theme | Light and dark through `Tokens`; contrast at least what the neighbouring panels meet. |
| Languages | Every string through `tr`; right-to-left mirrors the library to the right. |
| Sizes | Check 800×500, 1280×800 and 1440×900. Below 900 px wide the library starts collapsed. |
| Motion | None beyond existing popovers. |
| Empty and error | Parse error in the dialog; arrange refusal as a disabled button with a reason; a failed insert changes nothing. |

## Commands (from the implementation plan)

| Command | Change |
|---|---|
| `file.newDiagram` | New. `{name?, type?: flowchart\|mindMap (default flowchart), template?}`. Always creates, as `file.newBoard` does. |
| `dialog.newDiagram` | New UI request raised by File ▸ New diagram and the home card; the control channel drops it. The dialog submits `file.newDiagram` with explicit parameters. |
| `board.addConnected` | Add optional `shape` and `label`. |
| `board.insertFlowchart` | New. `{source, x?, y?}`. |
| `board.arrangeDiagram` | New. `{ids?, direction?}`. |

Each is registered once and reaches the menus, palette, CLI, control channel and MCP together. The
junk-parameter sweep in `crates/engine/src/tests.rs` is extended for all new commands. None of them
opens a dialog when called programmatically.

## Slices

Hours are one agent working sequentially, tests included, **estimated** (not measured), in the
convention of [gaps.md](gaps.md).

| Slice | Deliverable | Hours |
|---|---|---|
| 1 | `board.addConnected` shape and label, the drag chooser, the updated drag tests | 6–10 |
| 2 | Diagram identity: the field through save and local re-apply after a live replacement (no wire change), `file.newDiagram`, `dialog.newDiagram`, start dialog, templates | 10–18 |
| 3 | Left shape library: grouping, search, drag and click | 6–10 |
| 4 | `figcraft-diagram` graph and parser, rank layout, `board.insertFlowchart`, the dialog, limits, errors, live-payload check | 10–18 |
| 5 | Routing and full planner (return channels, obstacles), `board.arrangeDiagram`, determinism tests | 14–26 |
| 6 | Mind-map type in the start dialog on the existing `board.mindMap`, with acceptance checks (toolbar entry and drag to re-parent stay in gap 24) | 2–4 |
| 7 | Hand test on desktop and web, three window sizes, both themes, docs, parity rows | 6–10 |
| Total | | **54–96** |

Order follows dependency. Slice 1 is useful on every Board before the Diagram kind exists. Import
(slice 4) ships with a simple rank layout and the existing elbow router; slice 5 adds obstacle
avoidance. The slice numbers here are not the numbers in the plan's "Slice order, revised", which
groups the same work into five steps.

Slices 8–14 (format version 2, the typed core, Class, ERD, Sequence and their hand test, 84–146 h)
are listed in [diagram-type-specifications](diagram-type-specifications.md#tasks).

Slice 6 acceptance: choosing Mind map in the start dialog creates a file whose starting content is
the first node and its children (as `board.mindMap` makes them), and editing a node's text lays
the map out again.

## Risks

- **Layout quality.** Routing around other shapes (not only the two endpoints) is new work and the
  largest risk in slice 5. Fallback routes are reported, not hidden.
- **Payload limits in live sessions.** The proposed 64 KiB, 100 nodes and 200 edges need checking
  against the real session budget before they are documented.
- **Naming.** The interface says **Diagram**, never another product's name.
- **Scope drift.** FigCraft targets Figma. Anything beyond flowchart and mind map needs its own
  owner decision and a shared-model design first.
- **Not yet observed.** The reference editor's live behaviour (which choices its connection handle
  offers, how it lays out a return edge) is not observed: its activation reminder blocked the
  session. Public guides only, until someone with a licence runs the check with a synthetic file.

## Acceptance

The acceptance table in the [implementation plan](diagram-implementation-plan.md#acceptance-checks)
applies to the finished feature, staged by slice: the Connected choice row belongs to slice 1; the
Input and Save rows to the import slice; the Layout and Selection scope rows to the routing and
Arrange slice (the import slice has no obstacle or return-channel checks); the live-session rows
for Board content run from slice 1 on, because Board layer metadata syncs independently of the
style, and only the style's own sync waits on its open decision. Each slice states which rows it
claims. This spec adds: the start dialog creates a diagram in one step; Escape on the
chooser and on both dialogs leaves the document as it was when that control opened (opening the
Insert dialog commits an open text draft first, so the state to compare is the one after that);
every new control has a name and a
keyboard path; a screenshot of each screen above is read at the three window sizes before a slice
lands.
