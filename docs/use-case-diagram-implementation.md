# UML Use Case Diagram in FigCraft

Design proposal, 2026-11-19. Add **Use Case Diagram** to FigCraft’s Diagram chooser. Users should be able to start with a blank canvas, define a system and its actors, add use cases, and create UML relationships whose meaning and direction survive editing, save, and export.

This uses the per-page model, generated canvas layers, and native-file rules in the [typed diagram specification](diagram-type-specifications.md). The existing Diagram shell and connected-shape gesture are the starting points. Use Case Diagram is a new typed page, `useCase`.

> **Status.** The agreed rules are in [section 5 of the typed diagram specification](diagram-type-specifications.md), slices 15–17. Where they differ from this proposal, the specification wins: sizes are stored with a measured minimum; geometry rules keep members inside a subject and other elements outside it with fixed gaps; `updateElement` changes names and presentation only, with separate `move` and `resize`; `connect` can create an extension point for an Extend; `diagram.useCase.validate` is named `check`; Dependency and notes are in slice 17. This file keeps the workflow research and the example.

## What Visual Paradigm Online exposes

I opened **Unified Platform → Web Diagram Studio → Use Case Diagram** in a guest browser. The blank editor loaded with a dedicated Use Case palette. I observed the search field, **+ Shapes**, a use-case oval, an actor, a system shape, line and arrow tools, page controls, grid and guide settings, and export. The editor also exposes settings for connection arrows and connection points. REA evidence `ev_90810fe34ed10e6be17cb795a36bc19f36088d2198ef225c272b56a4aa890c56` records the blank editor and palette. [Unified Platform](https://online.visual-paradigm.com/unified-platform/) · [Blank Use Case editor](https://online.visual-paradigm.com/app/diagrams/#diagram:proj=0&type=UseCaseDiagram&width=11&height=8.5&unit=inch)

| Capability | Reference behavior or evidence | FigCraft plan |
| --- | --- | --- |
| Start from scratch or from an example | The guest chooser creates a blank typed canvas. The vendor also publishes use-case templates. | Offer **Blank** first and one original example when the core workflow is ready. |
| Add, rename, and resize objects | The online guides describe placing shapes from a palette, inline name editing, and drag-to-resize. The vendor’s use-case drawing guide describes label wrapping and placing systems, actors, and cases. | Use a searchable **Actors, Use cases, Boundaries, Relationships** palette. Support typed placement, inline naming, and reflow of long labels. |
| Create a shape and connect it in one gesture | VP Online documents its Resource Catalog. The vendor drawing guide illustrates creating a connected use case and creating include and extend relationships from a shape. | Extend FigCraft’s existing connection affordance with typed choices and one undo step for both objects. |
| Add detail to a relationship | The vendor documents include, extend, generalization, and dependency. The drawing and notation guides describe extension points on the receiving base case. | Use a relationship inspector for type, endpoint roles, and extension condition and points. Derive the UML line and keyword from the selected type. |
| Organize a complex drawing | The vendor documents multiple system boundaries, packages, and business-use-case notation. | Support multiple named system boundaries first. Add package grouping and business variants in a later slice. |
| Format and arrange | The online editor guide describes shape and line formatting, snapping, alignment guides, and layout tools. | Use FigCraft styling and placement controls. Keep UML markers and dash patterns tied to the relationship type. Add typed alignment when needed so moving a generated shape updates its model. |
| Edit the canvas and export | The live editor exposed page controls, connection points, guides, and an export control. The online feature page lists the output formats. | Reuse FigCraft’s zoom, pan, snapping, selection, undo, native save, and PNG/SVG/PDF export. |
| Describe a scenario or add a linked view | Visual Paradigm’s product guides document use-case specifications, flow of events, and sub-diagrams. Availability in this guest editor was not verified. | Add scenario steps and linked diagram views after the visual relationship editor. |

The live check covered blank creation and the exposed stencil and controls. Connection gestures, editing an extension point, and export were checked against the public guides. Some stencil icons had no accessible names, so their exact functions remain unverified. [Online use-case tutorial](https://online.visual-paradigm.com/diagrams/tutorials/use-case-diagram-tutorial/) · [Online use-case tool](https://online.visual-paradigm.com/diagrams/features/use-case-diagram-software/) · [Online UML tools](https://online.visual-paradigm.com/diagrams/features/uml-tool/) · [Online editor guide](https://online.visual-paradigm.com/diagrams/features/online-diagram-editor/) · [Desktop drawing guide](https://circle.visual-paradigm.com/docs/uml-and-sysml/use-case-diagram/how-to-draw-a-use-case-diagram-in-uml/) · [Desktop notation guide](https://circle.visual-paradigm.com/docs/uml-and-sysml/use-case-diagram/use-case-diagram-notations-guide/)

## Objects and relationship rules

An **actor** is a role outside the system, such as a customer, another service, or a device. A **use case** is a goal or behavior the system provides. A **system boundary** shows which goals belong to that subject. For an ordered sequence of actions, use the scenario description, an Activity Diagram, or a Sequence Diagram. A Use Case Diagram’s arrows describe relationships between goals and roles. [Online use-case tutorial](https://online.visual-paradigm.com/diagrams/tutorials/use-case-diagram-tutorial/) · [OMG UML 2.5.1](https://www.omg.org/spec/UML/2.5.1)

| Relationship | Valid endpoints | Rendering and arrow target | When to use it |
| --- | --- | --- | --- |
| **Association** | Actor and use case, drag in either direction | Solid line with no arrow in FigCraft’s first version | The actor participates in that use case. Store a single canonical actor-to-case pair for duplicate detection. |
| **Include** | Including use case → included use case | Dashed line with an open arrow at the included case and a `«include»` label | The including case incorporates required behavior defined by the included case. |
| **Extend** | Extending use case → extended base use case | Dashed line with an open arrow at the base case and a `«extend»` label | The extending case adds behavior at one or more named points in the base. Store an optional condition and references to points owned by the base. |
| **Generalization** | Specialized actor → general actor, or specialized use case → general use case | Solid line with a hollow triangle at the general element | The child is a specialization of the parent. Keep actor and use-case hierarchies separate. |
| **Dependency** | Client actor or use case → supplier actor or use case | Dashed line with an open arrow at the supplier | A generic dependency with no include or extend semantics. Add it after the core relationship tools. |
| **Note attachment** | Note → actor, use case, or relationship | Dotted annotation line | Add explanatory text without changing the UML model relationships. Implement with notes in the follow-up slice. |

The notation rules come from the public UML specification and the vendor’s notation guide. Restricting generic dependency endpoints and using a directionless association are first-version FigCraft choices. [OMG UML 2.5.1](https://www.omg.org/spec/UML/2.5.1) · [Vendor notation guide](https://circle.visual-paradigm.com/docs/uml-and-sysml/use-case-diagram/use-case-diagram-notations-guide/)

### Connecting existing shapes and creating a related shape

1. Place a **System boundary**, give it a name, and place use cases inside it. Place actors around it. Allow a case to be unassigned while drafting. Each case has at most one system boundary in this editor. Multiple boundaries can appear side by side. A move between systems changes membership only when the user drops the case into a new boundary or updates its membership in the inspector.
2. Select a source and drag its connection handle. When the cursor is over an existing target, highlight it and show the relationship types valid for that pair. Show a short explanation when there is no valid connection. Keep the preview outside the document.
3. On blank canvas, offer the valid relationship and shape combinations. From an actor, **Add connected use case** creates an association. From a use case, **Add included use case** creates an include arrow toward the new case. **Add extension** creates the new extending case and an arrow **back to** the source base case. **Add parent** creates a generalization arrow to the new parent.
4. For **Extend**, pick one or more extension points on the base or create a point there as part of the same operation. The condition is ordinary editable text. Confirming creates the new shape, point if needed, and relationship in one undo action. Escape discards the draft.
5. Selecting a relationship opens its inspector. Show endpoint roles as words such as **Extending case** and **Base case**. Reattaching an end validates those roles and any referenced points again. If a new base is selected, require the user to choose points on that base before committing.

Use the existing connection gesture as a UI precedent. The use-case command owns the object and relationship creation so the canvas, CLI, and MCP share validation. [FigCraft connected-shape overlay](../crates/ui-egui/src/canvas/board_connect.rs) · [Typed diagram transactions](diagram-type-specifications.md#decisions)

### Example to build and test

Draw a **Room booking** boundary. Put `Reserve room`, `Check availability`, `Apply member discount`, and `Reserve meeting room` inside. Put `Guest` and `Member` outside. Add these connections in order.

| From | Relationship | To | Meaning |
| --- | --- | --- | --- |
| Guest | Association | Reserve room | Guest participates in the reservation. |
| Reserve room | Include | Check availability | Every reservation uses the availability check. |
| Apply member discount | Extend | Reserve room | At the base case’s `Before total` point, apply the discount if the member is eligible. |
| Member | Generalization | Guest | Member is a specialized actor role. |
| Reserve meeting room | Generalization | Reserve room | Meeting-room reservation specializes room reservation. |

The discount example is an extension of reservation. Its arrow points toward **Reserve room** even if the user started the drag from **Reserve room** to create the new extension.

To describe the actions inside **Reserve room**, give it a basic scenario such as choosing a date, checking availability, and confirming the reservation. Add an alternative that asks for a different date when the room is unavailable. When implementing the scenario editor, store ordered steps and let an alternative branch from a specific step ID. Keep these step links in the scenario and display their order there or in a linked Activity Diagram. The visible `«include»` and `«extend»` relationships still connect use-case goals. [Use-case behavior and scenarios](https://circle.visual-paradigm.com/docs/uml-and-sysml/use-case-diagram/)

## Data and engine functions

Store a diagram-local model on the typed page and use the shared semantic ID allocator. Store named boundaries with bounds, actors with positions and a person or external-system presentation, use cases with positions and optional boundary IDs, and ordered extension points with stable IDs. Store the relationship kind and endpoint IDs separately from its generated shape. Extend stores the base’s point IDs and optional condition. Labels, arrowheads, and paths are generated for the renderer and export. Actor and case names can repeat where meaningful because connections resolve by IDs. [Typed file, allocator, and projection rules](diagram-type-specifications.md#decisions)

The function names below are proposed public FigCraft commands.

| Function | Parameters and result |
| --- | --- |
| `file.newDiagram` | Add `type: "useCase"` with an optional name. Blank creates a new typed page and opens its palette. |
| `diagram.useCase.addBoundary` | `{name, x, y, width, height}` → boundary ID. |
| `diagram.useCase.addActor` | `{name, x, y, presentation?}` → actor ID. Presentation defaults to the actor glyph. Offer a labeled external-system presentation. |
| `diagram.useCase.addCase` | `{name, x, y, boundary?}` → use-case ID. A canvas drop inside a boundary supplies its ID explicitly. |
| `diagram.useCase.updateElement` | `{id, name?, x?, y?, width?, height?, boundary?}` → element ID. Validate fields by element kind. For a use case, `boundary: null` clears membership. Moving a boundary translates its members together. |
| `diagram.useCase.addExtensionPoint` | `{caseId, name}` → point ID. Add rename and remove operations by point ID. |
| `diagram.useCase.connect` | `{kind, from, to, extensionPoints?, condition?}` → relationship ID. Require point IDs for Extend. |
| `diagram.useCase.addConnected` | `{source, relation, targetKind, name, x, y, boundary?, extensionPoints?, newExtensionPoint?, condition?}` → element and relationship IDs and newly created point ID, if any. The requested relation carries direction through named choices such as `addExtension` and `addParent`. |
| `diagram.useCase.updateRelationship` | `{id, kind?, from?, to?, extensionPoints?, condition?}` → relationship ID. Revalidate the resulting complete relationship. Require an explicit endpoint order when converting to a directed relation. |
| `diagram.useCase.remove` | `{ids}` removes selected elements or relationships. Removing an actor or case removes its incident links in the same transaction. Removing a boundary clears member assignments and keeps their positions. |
| `diagram.useCase.validate` | Read-only. Return IDs and actionable messages for incomplete drafting, such as an unassigned use case, separate from errors that block edits. |

Validate current-page ownership, endpoint kinds, distinct IDs, duplicates, and finite geometry before committing. Reject include and generalization cycles. Allow multiple independent specializations and multiple extensions of one base. Require Extend points to belong to its base and refuse deleting a point still referenced by an Extend link. Changing a relationship to another kind drops fields specific to Extend. Connectors never persist with free ends. An invalid edit leaves the old relationship and undo history intact.

Use the existing typed projection rules to render an actor glyph, an oval with wrapped text for a use case, and an outlined frame for a boundary. Route lines to the outline of the actor or oval, using the existing straight and elbow geometry where applicable. Derive the relationship marker and dash pattern from its kind. Reuse open arrows where possible and draw a hollow triangle with original vector geometry for generalization. Stop the line at the triangle base so it stays hollow in a transparent export. Give multiple links between the same pair distinct routes and label positions. Export a selected relationship with both endpoints, its label, and marker. [Existing Board connector geometry](../crates/doc/src/board_route.rs) · [Typed export rules](diagram-type-specifications.md#decisions)

## Delivery and acceptance

| Slice | Scope | Checks |
| --- | --- | --- |
| 1. Typed page and palette | Add `useCase` to the Diagram chooser after the shared typed-file foundation. Implement boundaries, actors, cases, inline naming, semantic selection, and rendering. | Create two systems. Move one system and verify only its assigned cases move. Rename an actor and resize a case. Save and reopen. |
| 2. Core connections | Add Association, Include, Extend, Generalization, the connected creation gesture, and the relationship inspector. | Build the room booking example. Verify all arrow directions and markers. Test a drag in either direction for Association, and verify **Add extension** places the arrow on the base. Undo or redo each connected creation in one step. |
| 3. Validation and native delivery | Finish reattach, delete and invalid-operation behavior, native save, and SVG/PNG/PDF selection export. Enforce the existing typed-page live-session and branching rules. | Reject actor-to-actor Association, actor-to-case Include, mismatched Generalization, self-links, duplicate links, and generalization or include cycles. Reject an extension point from a different base. Move and delete elements, reopen, and check all relation IDs and labels. Test the UI and a headless command path. |
| 4. Additional authoring tools | Add generic Dependency, notes and their attachments, packages, business actor and use-case variants, and optional use-case scenario details. Add an original starter template. | Keep notes distinct from UML relationships. Verify grouping preserves IDs and connections. Represent scenario steps explicitly and link to an Activity or Sequence Diagram when implemented. |

Run the shared typed-framework tests and the FigCraft desktop and web checks before release. Use original symbols, examples, and UI. The browser research establishes the reference workflow. FigCraft’s acceptance cases establish how our editor should behave.
