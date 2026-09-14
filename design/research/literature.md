# Software design and design documentation: literature report

## Scope and evidence discipline

This report answers four different questions that are often collapsed into one “design document” question:

- **(a) What is designed?** The subject, decisions, structures, contracts, qualities, and problem/solution relationships that design work makes explicit.
- **(b) What does the document contain?** The information organization, views, viewpoints, models, decisions, rationale, and acceptance information used to represent that design.
- **(c) What does it deliver?** The people and later work products it enables, the decisions it makes reviewable, and the observable criteria by which the design can be judged.
- **(d) What methods produce it?** The elicitation, decomposition, modeling, iteration, analysis, review, and maintenance practices described by the sources.

The four-part frame above is an analytic frame for this report, not a terminology claim about any one source. Quotations are short and transcribed from the retrieved text. Page numbers refer to the PDF pagination where available; clause numbers refer to standards.

## Retrieval ledger

| ID | Source | Retrieval status and evidence used |
|---|---|---|
| **S1** | IEEE Std 1016-2009, *Standard for Information Technology—Systems Design—Software Design Descriptions* | Official IEEE SA record retrieved at <https://standards.ieee.org/ieee/1016/4502/>. The complete text was retrieved from the publicly hosted copy at <https://wildart.github.io/MISG5020/standards/IEEE-1016-2009.pdf>; the mirror is not an IEEE distribution, so normative wording is attributed to the standard and the mirror is identified as the retrieval copy. |
| **S2** | ISO/IEC/IEEE 42010:2022, *Software, systems and enterprise—Architecture description* | Official ISO record and abstract retrieved at <https://www.iso.org/standard/74393.html>; official IEEE record at <https://standards.ieee.org/ieee/42010/6846/>. A standards preview containing scope, terms, conformance, and conceptual foundations was retrieved at <https://cdn.standards.iteh.ai/samples/74393/fc7b7f103d8446a4b87a3261e31370d3/ISO-IEC-IEEE-42010-2022.pdf>. The complete commercial text, including the full Clause 6 requirements, was **not retrieved**; claims below do not silently treat the preview as the complete standard. |
| **S3** | ISO/IEC/IEEE 42010:2011, withdrawn first edition | Official status page retrieved at <https://www.iso.org/standard/50508.html> (withdrawn; replaced by 2022 edition). The preview at <https://cdn.standards.iteh.ai/samples/50508/ec3d9367f48e4b28a7ac8dddaeca5a3f/ISO-IEC-IEEE-42010-2011.pdf> contains the conceptual model and the explicit architecture-rationale definition used here. |
| **S4** | Parnas & Clements, “A Rational Design Process: How and Why to Fake It” (1986) | Full paper retrieved at <https://www.cs.tufts.edu/comp/40-2011f/readings/fake-it.pdf>. Bibliographic DOI record: <https://doi.org/10.1109/TSE.1986.6312940>. |
| **S5** | Parnas, “On the Criteria To Be Used in Decomposing Systems into Modules” (1972) | Full paper copy retrieved at <https://john.cs.olemiss.edu/~hcc/csci555/notes/localcopy/Parnas_Criteria_Decomposing.pdf>. ACM publication record retrieved at <https://doi.org/10.1145/361598.361623>. |
| **S6** | Kruchten, “Architectural Blueprints—The ‘4+1’ View Model of Software Architecture” (1995) | Full paper retrieved at the author/institution mirror <https://www.cs.ubc.ca/~gregor/teaching/papers/4%2B1view-architecture.pdf>. Bibliographic record: <https://doi.org/10.1109/52.469759>. |
| **S7** | Bass, Clements & Kazman, *Software Architecture in Practice*, 2nd ed. (2003) | Official SEI book record retrieved at <https://www.sei.cmu.edu/library/software-architecture-in-practice-2nd-edition/>. The commercial book’s complete text was **not retrieved**. The report uses the open SEI method reports S8–S10 for the ADD, quality-attribute-scenario, and ATAM details and labels the book as bibliographic/context evidence. |
| **S8** | Wojcik et al., *Attribute-Driven Design (ADD), Version 2.0* (CMU/SEI-2006-TR-023) | SEI record and full report PDF retrieved at <https://sei.cmu.edu/library/attribute-driven-design-add-version-20/> and <https://sei.cmu.edu/documents/775/2006_005_001_14795.pdf>. |
| **S9** | Barbacci et al., *Quality Attribute Workshops (QAWs), Third Edition* (CMU/SEI-2003-TR-016) | SEI record and full report PDF retrieved at <https://insights.sei.cmu.edu/library/quality-attribute-workshops-qaws-third-edition/> and <https://insights.sei.cmu.edu/documents/716/2003_005_001_14249.pdf>. |
| **S10** | Kazman, Klein & Clements, *ATAM: Method for Architecture Evaluation* (CMU/SEI-2000-TR-004) | SEI record and full report PDF retrieved at <https://sei.cmu.edu/library/atam-method-for-architecture-evaluation/> and <https://sei.cmu.edu/documents/629/2000_005_001_13706.pdf>. |
| **S11** | Brooks, *The Mythical Man-Month* (1975), “Aristocracy, Democracy, and System Design” | Full university-hosted scan retrieved at <https://www.cs.cmu.edu/~15712/papers/mythicalmanmonth00fred.pdf>. |
| **S12** | Brooks, *The Design of Design* (2010) | Official publisher record retrieved at <https://www.informit.com/store/design-of-design-essays-from-a-computer-scientist-9780321770547>. Full book text was **not retrieved**; no substantive claim below depends on it. The conceptual-integrity evidence is taken from S11. |
| **S13** | Jackson, “Problem Analysis and Structure” (2000/2001) | Full paper retrieved at <https://people.csail.mit.edu/dnj/teaching/6898/papers/mj-marktoberdorf.pdf>. Jackson’s commercial *Problem Frames* book record is available at <https://books.google.com/books/about/Problem_Frames.html?id=j6hQAAAAMAAJ>, but the complete book was **not retrieved**. |
| **S14** | Nygard, “Documenting Architecture Decisions” (2011) | Full article retrieved at <https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions>. |
| **S15** | Fowler, “Architecture Decision Record” and *Software Architecture Guide* | Articles retrieved at <https://martinfowler.com/bliki/ArchitectureDecisionRecord.html> and <https://www.martinfowler.com/architecture/>. Fowler’s “Uml As Blueprint” was also retrieved at <https://martinfowler.com/bliki/UmlAsBlueprint.html> for the caution about complete hand-off blueprints. |

The direct URL reader was unavailable for these URLs in this run, so the URLs were fetched over HTTPS and the PDF/HTML text was extracted locally. That is a retrieval limitation, not evidence that an inaccessible commercial standard or book was read. The ledger distinguishes full text, preview, official record, and unretrieved full text.

---

## (a) What is designed?

### 1. A design is a reasoned conception of a subject that can guide implementation and evaluation

IEEE 1016 defines a design as “a conceptualization of a design subject” that embodies essential characteristics, demonstrates a means of fulfilling requirements, supports analysis and evaluation, and can guide implementation (S1, §3.1, p. 3). This is more than a list of parts: the design must explain how the subject is meant to satisfy requirements and provide a basis for checking that claim. The same standard distinguishes the design subject from the SDD that represents it; the SDD is a work product that records and communicates the design (S1, §§3.1–3.2, pp. 3–4).

ISO/IEC/IEEE 42010:2022 makes the analogous architecture distinction explicit: “architecture” is the fundamental concepts or properties of an entity in its environment and the principles governing realization and evolution, while an architecture description is the “work product used to express an architecture” (S2, §§3.2–3.3, pp. 2–3 of the preview). The standard’s conceptual foundation says that an AD is “an expression of an architecture” and is made for a purpose relative to stakeholder needs; different ADs may describe the same architecture for different stakeholders, periods, or contexts (S2, §5.2.2, pp. 6–7 of the preview). Thus, a document is not identical to the architecture, and a diagram or inventory is not automatically a design.

### 2. Design decisions and the boundaries they establish

Parnas and Clements treat design as a sequence of decisions that can be made explicit in work products. They warn that ordinary development often starts without a clear desired behavior or implementation constraints and makes decisions “with no clear statement of why” (S4, §I, p. 1). Their rationalized process therefore designs requirements, module responsibilities, interfaces, dependencies, internal structures, and the rationale that connects them (S4, §§IV–V, pp. 2–6).

Parnas’s 1972 paper is especially direct about what a module decomposition is. A module is “a responsibility assignment rather than a sub-program,” and a modularization includes the system-level design decisions that must be made before independent module work can start (S5, “What Is Modularization?” pp. 1053–1054). The design is therefore not merely a component inventory. It assigns responsibility and hides decisions that other parts should not need to know.

The decomposition criterion is information hiding: “Every module in the second decomposition is characterized by its knowledge of a design decision which it hides from all others. Its interface or definition was chosen to reveal as little as possible about its inner workings” (S5, “The Criteria,” p. 1056). Parnas’s conclusion says to begin with difficult or change-prone design decisions and design modules to hide them, rather than beginning with a flowchart (S5, “Conclusion,” p. 1058). What is designed includes the change boundaries and information boundaries that make independent work, replacement, and comprehension possible.

### 3. Architecture as elements, forms, and rationale/constraints across concerns

Kruchten describes software architecture as the high-level structure assembled from architectural elements in forms chosen to satisfy major functional and non-functional requirements. He quotes the compact model “Software architecture = {Elements, Forms, Rationale/Constraints}” (S6, “An Architectural Model,” pp. 2–3). A design therefore includes not only elements and connections but also the form/pattern in which they are organized and the reasons and constraints that make that form appropriate.

The 4+1 model separates concerns rather than forcing one diagram to represent everything. Its logical view addresses functionality; the process view addresses concurrency, synchronization, integrity, and fault tolerance; the development view addresses organization, reuse, portability, and team/work allocation; the physical view addresses mapping and qualities such as performance, availability, reliability, and scalability; and scenarios connect and validate the other four views (S6, “An Architectural Model,” pp. 2–3; Table 1, p. 15). The object of design is consequently a coordinated set of decisions viewed from stakeholder-relevant perspectives.

### 4. Quality attributes are designed as concrete responses, not adjectives

The SEI ADD report defines architecture design as a recursive process driven by functional requirements, design constraints, and quality-attribute requirements (S8, §§2–3, pp. 3, 7–9). Its inputs are stakeholder-prioritized requirements; its output is a design in terms of “the roles, responsibilities, properties, and relationships among software elements” (S8, §3.2, p. 9). Quality attributes become architectural drivers that select tactics, patterns, element types, allocations, and interfaces; they are not a detached list of desired adjectives.

QAW and ATAM make this concrete through scenarios. QAW says that a quality goal alone is not definitive enough for design or evaluation; a scenario is a short story describing an interaction that exercises a quality attribute (S9, pp. 3–4). The design must therefore specify a stimulus, context, response, and measurable response criterion, such as a modification completed within two person-weeks or a failure recovered within a stated time (S9, pp. 3–4; Appendix B, pp. 19–21).

### 5. A problem/solution relationship, not only an internal solution structure

Jackson rejects solution-first reasoning: software engineering has traditionally focused on programs and solution abstractions while the problem intended to be solved is left for the reader to infer (S13, §1, pp. 1–2). Problem analysis identifies the problem world, its domains, the machine/software domain, and the phenomena shared at their interfaces. The requirement describes desired properties of the problem domain; domain properties describe what is intrinsically true there; and the machine specification describes desired machine behavior at the interface (S13, §2.3, pp. 3–4).

Jackson gives a precise success condition: `machine specification ∧ domain properties ⇒ requirement` (S13, §2.3, p. 4). This is a useful test for design content: the document should make clear what is assumed about the world, what the solution is required to do at its boundary, and why those facts imply the desired outcome. A component list with no problem-world assumptions or requirement relation cannot support that argument.

### 6. Conceptual integrity and the architecture/implementation boundary

Brooks argues that usability and simplicity require a coherent set of concepts: “Simplicity and straightforwardness proceed from conceptual integrity” (S11, ch. 4, pp. 44–45). He defines architecture as the complete, detailed user-facing specification and distinguishes it from implementation: “Where architecture tells what happens, implementation tells how it is made to happen” (S11, ch. 4, pp. 45–46). A design phase therefore decides the externally meaningful concepts and contracts while leaving room for implementation choices that do not violate them.

Brooks also gives the organizational implication: “If a system is to have conceptual integrity, someone must control the concepts” (S11, ch. 4, p. 46). Conceptual integrity does not mean that every implementation detail is centralized; it means that the externally visible design has a coherent controlling philosophy and that incompatible features are not admitted merely because they are locally attractive (S11, ch. 4, pp. 46–50).

### 7. Decisions are temporal objects, not only current-state facts

Nygard’s ADR article identifies “significant” decisions as those affecting structure, non-functional characteristics, dependencies, interfaces, or construction techniques. Each ADR records the forces and one decision made in response (S14, “Context” and “Format”). Fowler similarly defines an ADR as “a short document that captures and explains a single decision relevant to a product or ecosystem” (S15, “Architecture Decision Record”). Design includes the selected option, the rejected alternatives or trade-offs, and the conditions under which the decision may later be superseded.

---

## (b) What does the document contain?

### 1. IEEE 1016: an SDD is organized around concerns, viewpoints, views, overlays, and rationale

IEEE 1016’s required SDD contents are explicitly enumerated (S1, §4.1, p. 7):

1. identification of the SDD;
2. identified design stakeholders;
3. identified design concerns;
4. selected design viewpoints, each with allowed design-element types and design languages;
5. design views;
6. design overlays; and
7. design rationale.

The SDD identification itself includes issue date/status, scope, issuing organization, authorship, references, context, a language for each viewpoint used, body, summary, glossary, and change history (S1, §4.2, p. 8). This is not a demand for one universal template; it is a demand that the document be identifiable, scoped, attributable, referential, and maintainable.

The standard requires an SDD to identify stakeholders and each stakeholder’s concerns, and to address each concern (S1, §4.3, p. 8). It requires one or more views, each conforming to its governing viewpoint, and defines completeness as every identified concern being the topic of at least one view, all viewpoint-refined attributes being specified, and all constraints being applied (S1, §4.4, p. 8). It defines consistency as no known conflicts among view elements (S1, §4.4, p. 8).

Each viewpoint declaration must state its name, concerns, allowed design elements (entities, attributes, relationships, constraints), analytical methods/operations and evaluation criteria, and source; it may also specify completeness/consistency tests, analysis techniques, heuristics, patterns, or construction guidelines. The SDD must include the rationale for selecting each viewpoint, and each concern must be framed by at least one selected viewpoint (S1, §4.5, pp. 9–10). This is the decisive distinction between a viewpoint and a component inventory: a viewpoint defines how a concern is represented and interpreted; a view instantiates that convention for the subject.

IEEE 1016’s language-neutral viewpoints include Context, Composition, Logical, Dependency, Information, Patterns Use, Interface, Structure, Interaction, State Dynamics, Algorithm, and Resources (S1, §5.1, Table 1, pp. 13–22). The table connects each viewpoint to a concern, for example Context to services/users and system boundary; Composition to constituent parts and responsibilities; Dependency to interconnection/access; Information to persistent data and metadata; Interface to service definitions/access; Interaction to communication and synchronization; State Dynamics to states/events/transitions; and Resources to utilization/contention/availability/performance (S1, §5.1, Table 1, pp. 13–22). The standard says a listed viewpoint should be used when applicable to the identified design concerns, not that every design must contain every viewpoint (S1, §5.1, p. 13).

Design rationale is required information about the reasoning that led to the system as designed. IEEE 1016 lists issues raised, options considered, trade-offs, decisions, criteria, arguments, and justifications as possible rationale content (S1, §4.8, p. 12). A design element must have a name, type, and content; entities also have purpose, and the purpose attribute is explicitly the rationale for creating the element (S1, §§4.6–4.8, pp. 9–12).

### 2. ISO/IEC/IEEE 42010: an architecture description is an ontology of concerns and representations

The current 2022 edition’s official scope says it specifies requirements for the structure and expression of an architecture description, distinguishes an entity’s architecture from its AD, and does not prescribe the processes, methods, models, notations, techniques, tools, format, or media by which an AD is created or recorded (S2, official ISO abstract; S2, §1 of the retrieved preview). This scope matters: 42010 describes what an AD must express and the concepts it relates, but it is not a complete design method.

The 2022 definitions distinguish:

- **stakeholder:** a role, position, individual, organization, or class having an interest, right, share, or claim in the entity of interest (S2, §3.17, p. 4 of the preview);
- **concern:** a matter of relevance or importance to a stakeholder (S2, §3.10, p. 3);
- **architecture viewpoint:** conventions for creating, interpreting, and using an architecture view to frame concerns (S2, §3.8, pp. 2–3);
- **architecture view:** an information part comprising a portion of the AD (S2, §3.7, p. 2);
- **model kind:** a category of model distinguished by characteristics and modeling conventions (S2, §3.15, p. 4); and
- **correspondence:** a named relationship among AD elements, including traceability, refinement, consistency, dependency, constraint, satisfaction, or obligation (S2, §3.11, p. 3).

The 2011 conceptual model states the view/viewpoint relationship in operational terms: an AD includes one or more views; a view addresses stakeholder concerns; a view expresses the architecture according to a viewpoint; and a viewpoint establishes conventions for constructing, interpreting, and analyzing the view (S3, §4.2.4, pp. 6–7). It also states that every stakeholder, concern, viewpoint, view, model kind, model, architecture decision, and rationale is an AD element and that correspondences relate AD elements (S3, §§4.2.6–4.2.7, pp. 7–8).

For rationale, the retrieved 2011 conceptual text says: “Architecture rationale records explanation, justification or reasoning about architecture decisions that have been made.” It identifies possible contents as the decision basis, alternatives, trade-offs, potential consequences, and citations to additional information (S3, §4.2.7, p. 7). The 2022 preview’s table of contents retains a dedicated Clause 6.10, “Recording of architecture decisions and rationale,” but the full normative clause was not available in the retrieved preview; the report therefore does not claim its exact 2022 shall/should wording.

### 3. Kruchten: a view-oriented architecture document with a scenario view

Kruchten’s paper gives a concrete architecture-document outline: title page, change history, table of contents, figures, scope, references, software architecture, architectural goals and constraints, logical architecture, process architecture, development architecture, physical architecture, scenarios, size and performance, quality, and appendices for terms/principles (S6, Fig. 13, p. 14). It also proposes a separate Software Design Guidelines document for the important decisions that must be respected to maintain architectural integrity (S6, “Documenting the architecture,” p. 13).

Each of the four main views has a concern and a notation, while the “+1” scenario view is deliberately redundant: it drives discovery of architectural elements and validates/illustrates the architecture and architectural prototype (S6, “The Scenarios,” p. 9). A useful document therefore contains cross-view correspondence, not five unrelated diagrams.

### 4. Parnas and Clements: work products contain contracts, dependencies, and internal intent

The rational-process paper gives a staged content model:

- the **module guide** assigns responsibilities and records the decisions hidden by each module; it should have a tree structure rather than a flat list (S4, §V.B, p. 5);
- each **module interface specification** is a formal black-box description containing invokable access programs, parameters, externally visible effects, timing/accuracy constraints where needed, and undesired events (S4, §V.C, pp. 5–6);
- the **uses hierarchy** records which programs depend on which others and supports staged delivery, fail-soft subsets, and product families (S4, §V.D, p. 6); and
- the **module design document** records internal data structures, access-program effects, abstraction functions, undesired-event checks, and a verification argument that the implementation satisfies the module specification (S4, §V.E, pp. 6–7).

The authors explicitly say that the design document exists to enable efficient review before coding and to explain code intent to future maintainers (S4, §V.E, p. 6). A component name without responsibility, interface, hidden decisions, failure behavior, and verification argument is therefore not the content described by this method.

### 5. ADD and quality-attribute scenarios: inputs and expected outputs

ADD takes as inputs stakeholder-prioritized functional requirements, design constraints, and quality-attribute requirements (S8, §3.1, pp. 7–8). Its expected output is a system design in terms of software-element roles, responsibilities, properties, and relationships; the resulting architecture is documented using Module, Component-and-Connector, and Allocation views as appropriate (S8, §3.2, p. 9).

QAW’s refined scenario table provides a concrete content schema: business goals, relevant quality attributes, stimulus, stimulus source, environment, artifact, response, response measure, questions, and issues (S9, Appendix A, pp. 18–19). The six scenario components make “fast,” “secure,” or “modifiable” answerable rather than rhetorical. QAW’s example records the event, who/what caused it, operating environment, affected artifact, response, and a measurable one-millisecond response goal (S9, Appendix B, pp. 20–21).

ATAM uses the same scenario logic but adds architecture-evaluation records. Its output records risks and non-risks as an architectural decision (or an undecided decision), a quality-attribute response and its consequences, and the rationale for the positive or negative effect (S10, §7.1, p. 21). It also records sensitivity points, tradeoff points, and the reasoning connecting architectural properties to quality goals (S10, §§7.2–7.3, pp. 22–23).

### 6. ADR content: a decision record rather than a miniature system specification

Nygard proposes a short, modular record for one significant decision. The format has a short title, **Context** (the forces, including tensions), **Decision** (the response, in active language), **Status**, and **Consequences** (positive, negative, and neutral effects); replaced records remain and are marked superseded (S14, “Format”). He recommends numbered records in the repository and describes each as a “single decision in response to” forces (S14, “Architecture”).

Fowler’s later explanation keeps the same boundary: an ADR is short, captures one decision, context, significant ramifications, and a brief rationale; it should be linked to a superseding decision rather than rewritten when the decision changes (S15, “Architecture Decision Record”). ADRs complement a broader architecture description; they do not replace views, requirements, interfaces, or quality analysis.

### 7. Jackson: problem descriptions, domain properties, and machine specification

A problem-oriented design document should identify the problem domains and their phenomena, the machine/software domain, the requirement, domain properties, and the machine specification. Jackson’s formal implication provides the organizing trace: the machine specification and domain properties should entail the requirement (S13, §2.3, pp. 3–4). This content prevents a solution inventory from silently substituting for the problem being solved.

---

## (c) What does it deliver?

### 1. A communicable, reviewable basis for stakeholder decisions

IEEE 1016 states that an SDD is a representation used to record design information and communicate it to key design stakeholders (S1, §1.1, p. 1). Its views allow each stakeholder to focus on design details from a specific perspective and address relevant requirements (S1, §4.4, p. 8). The intended delivery is therefore not “documentation for its own sake”; it is shared information that lets stakeholders inspect whether their concerns have been addressed.

42010’s official 2022 abstract similarly treats an AD as a structured expression for stakeholders, while its 2011 introduction says architecture descriptions improve communication and cooperation among parties who create, use, and manage systems (S2, official abstract; S3, Introduction, pp. v–vi). A design document delivers a common referent for discussion and comparison, not merely an author’s memory of components.

### 2. Traceability from requirements to design and from design to later work

IEEE 1016 says an SDD’s contents can be traced to requirements, can lead to requirements changes, and can influence test plans, test cases, and test procedures (S1, §3.2.2, p. 7). Design verification checks whether the SDD addresses stakeholder concerns, is consistent with requirements, implements intended interface/algorithm/resource/error-handling decisions, achieves intended qualities, and conforms to imposed architecture; validation uses the SDD’s overview, rationale, and requirement traceability (S1, §3.2.3, p. 7). The document delivers evidence and connections that subsequent reviews and tests can use.

Parnas and Clements make the same delivery claim operational: work-product criteria let management review progress, and documentation that is used throughout construction becomes useful to later maintainers (S4, §§IV, VI.B, pp. 2, 8). They report that a carefully produced requirements document remained useful for testing and future change years after service began (S4, §VII, p. 9).

### 3. Stakeholder-specific ways to understand the architecture

Kruchten reports that the 4+1 model allowed stakeholders to find what they wanted to know: systems engineers approach physical then process views; end users, customers, and data specialists approach the logical view; project managers and configuration staff approach the development view (S6, Conclusion, p. 14). This is a concrete delivery test: a view should answer a stakeholder concern without forcing that stakeholder to infer it from an unrelated representation.

IEEE 1016 gives similar uses: Composition supports responsibility allocation, impact analysis, project/work-package planning, cost/staffing/schedule estimation, and configuration management; Dependency supports change-impact analysis, failure/resource-bottleneck isolation, integration planning, and integration tests; Interface serves as a binding agreement among designers, programmers, customers, and testers (S1, §§5.3, 5.5, 5.8, pp. 15–20). The document delivers different operational affordances through different views.

### 4. An initial architecture that can be implemented and tested

ADD’s output is an initial architecture description showing system partitioning, element types/properties/relations, and interactions and mechanisms among elements (S8, §3.2, p. 9). Its Plan–Do–Check cycle treats the design as a hypothesis: select element types using quality attributes and constraints, instantiate and allocate responsibilities, then analyze whether requirements are met (S8, §2, pp. 3–5). The delivery is a design sufficiently explicit to drive further decomposition, interface definition, implementation, and analysis.

Kruchten’s scenarios provide an additional acceptance route: after scenarios are scripted across the four blueprints, the architecture is implemented, tested, measured, and revised; lessons learned and rationale are captured in each iteration (S6, “A scenario-driven approach,” pp. 13–14). A design document that cannot support an implementation or a meaningful prototype test has not delivered the architectural hypothesis the method expects.

### 5. Risks, trade-offs, and unresolved decisions made visible

ATAM’s central goal is to uncover architectural decisions that pose risks to quality requirements, decisions not yet made, and the analyses, rationale, and guidelines needed for continuing decisions (S10, §7, p. 21). Sensitivity points tell designers where a property is critical to a quality response; tradeoff points expose properties affecting more than one quality attribute (S10, §7.2, pp. 22–23). A design document delivers a map of what must not be changed casually and what must be evaluated when assumptions change.

Parnas and Clements recommend recording alternatives considered and rejected, with reasons, because future maintainers will ask the same “why” questions (S4, §VII, p. 9). Nygard and Fowler make this historical delivery explicit: status and supersession preserve the decision trail instead of rewriting history (S14; S15).

### 6. A testable account of whether the design solves the problem

Jackson’s implication—machine specification plus domain properties entails the requirement—gives a formal shape to acceptance (S13, §2.3, p. 4). QAW’s response measure and ATAM’s scenario analysis provide a practical shape: a stimulus under stated environmental conditions should produce an observable response with a measurable criterion (S9, pp. 3–4; S10, §5.1.1, pp. 13–15). IEEE 1016’s completeness/consistency and verification criteria provide document-level acceptance (S1, §4.4; §3.2.3, pp. 7–8).

The deliverable is thus not “a plausible story.” It is an inspectable set of claims: which concern is covered, by which view, through which decisions and relations, under which assumptions, with what predicted or measured result.

### 7. A maintained reference for evolution

Parnas and Clements insist that maintenance is redesign and redevelopment; invalidated documentation must be changed, and the final documentation should remain rational and accurate (S4, §V.G, p. 7; §VII, p. 8). Nygard says small modular records have a chance of staying current, while large documents are rarely updated or read (S14, “Context”). Fowler likewise recommends short, separately stored ADRs with status and supersession links (S15, “Architecture Decision Record”). A design document delivers long-term value only if its structure lets later readers locate and revise the affected decision without reinterpreting the entire system.

---

## (d) What methods produce it?

### 1. Requirements and work-product sequencing: Parnas & Clements

Parnas and Clements propose an idealized rational process, while openly acknowledging that real projects discover facts late, change requirements, make errors, and backtrack. Their point is not that design can be perfectly deductive; “we can fake it” by producing and maintaining the work products that a rational process would have produced (S4, §§I–III, pp. 1–2).

Their method describes each stage in terms of four questions: what work product comes next, what criteria it must satisfy, what people should produce it, and what information they should use (S4, §IV, p. 2). The sequence establishes/document requirements; designs and documents module structure; designs/document module interfaces; designs/documents the uses hierarchy; designs/documents module internals; then writes programs (S4, §V, pp. 3–7). Design decisions are not considered complete until represented in the documents, and missing information is explicitly marked rather than silently invented (S4, §VII, p. 8).

Review is part of the practical method even though the paper’s ideal process section lists work products: the authors say they apply extensive systematic reviews to every work product and test executable code (S4, §V, p. 3). Documentation is designed by first stating the questions it must answer, refining those questions into sections, putting each fact in one place, and reviewing organization as well as accuracy (S4, §VI.B, p. 8). This produces a reference document rather than a stream-of-consciousness narrative.

### 2. Information hiding and change-oriented decomposition: Parnas (1972)

The decomposition method begins by listing difficult or likely-to-change design decisions, then assigns each to a module whose interface reveals as little as possible (S5, “The Criteria” and “Conclusion,” pp. 1056, 1058). The resulting modules need not correspond to execution phases. The method evaluates candidate decompositions against independent development, flexibility, comprehensibility, and efficiency, and it separately considers hierarchy and clean decomposition rather than conflating them (S5, pp. 1055–1058).

The practical method therefore asks “what decision should be hidden, what can clients rely on, and what change should stay local?” It does not ask only “what functions run first?” This is a method for producing meaningful module guides and interface contracts, not for naming every runtime component.

### 3. View selection and scenario-driven iteration: Kruchten

Kruchten’s process is architecture-centered, scenario-driven, and iterative (S6, abstract). An iteration selects a small number of critical or risky scenarios, places a strawman architecture, scripts the scenarios to discover abstractions/processes/subsystems, lays those elements onto logical/process/development/physical blueprints, implements and tests an architectural prototype, measures it where possible, and captures lessons (S6, “A scenario-driven approach,” pp. 13–14). The next iteration reassesses risk, adds scenarios, updates all views, tests again, and updates rationale/guidelines.

The method treats the scenario view as both a design driver and a validation/illustration mechanism (S6, “The Scenarios,” p. 9). A document is produced alongside the iterations: architecture views plus design guidelines for the decisions that preserve integrity (S6, “Documenting the architecture,” pp. 13–14).

### 4. Attribute-Driven Design: recursive Plan–Do–Check

ADD starts by confirming that requirements are sufficient and prioritized, chooses an element to decompose, identifies candidate architectural drivers, chooses a design concept satisfying those drivers, instantiates elements and allocates responsibilities, defines interfaces, verifies/refines requirements as constraints, and repeats for the next element (S8, Table of Contents and §§4–11). The method’s concise cycle is:

- **Plan:** use quality attributes and constraints to select element types;
- **Do:** instantiate elements satisfying quality and functional requirements; and
- **Check:** analyze the resulting design to determine whether requirements are met (S8, §2, pp. 3–5).

The method uses quality-attribute requirements as design drivers rather than appending quality claims after structural decisions are made (S8, Abstract and §2). Its output is a set of views grounded in explicit responsibilities, properties, relationships, and design decisions (S8, §3.2).

### 5. Quality Attribute Workshops: stakeholder elicitation before architecture is complete

QAW is a facilitated, early intervention method that engages stakeholders before the architecture is complete to generate, prioritize, and refine quality-attribute scenarios (S9, Abstract and §3, pp. vii, 7). Its eight steps are presentation/introductions, business/mission presentation, architectural-plan presentation, identification of architectural drivers, scenario brainstorming, scenario consolidation, scenario prioritization, and scenario refinement (S9, §3, p. 7).

The method makes the vague concrete: each scenario receives stimulus/source, environment, artifact, response, and response measure, plus questions and issues (S9, Appendix A, pp. 18–19). The resulting prioritized/refined scenarios become usable requirements and inputs to architecture design and evaluation; they are not generic “non-functional requirements” labels.

### 6. ATAM: structured architecture evaluation and trade-off analysis

ATAM is a repeatable evaluation method for understanding the consequences of architecture decisions with respect to quality attributes and their interactions (S10, §§1, 3, pp. 1, 7–8). It presents business drivers, presents the architecture, identifies approaches, builds a quality-attribute utility tree, analyzes approaches, brainstorms/prioritizes scenarios, analyzes again, and presents results (S10, §8, pp. 25–37).

Utility trees translate business drivers into concrete quality scenarios and prioritize them by importance and perceived risk (S10, §5.3, pp. 16–18). Evaluation records risks, non-risks, sensitivity points, tradeoff points, assumptions, consequences, and rationale (S10, §7, pp. 21–23). ATAM is not primarily a construction method; it evaluates a proposed or existing architecture and supplies evidence about where architectural reasoning is weak or where quality goals conflict.

### 7. Problem analysis and frame decomposition: Jackson

Jackson’s method starts in the problem world: identify domains and phenomena, distinguish the machine domain from problem domains, state domain properties and requirements, and specify the machine’s behavior at shared interfaces (S13, §§2–3, pp. 2–6). Problem frames classify recurring elementary problems and expose frame-specific concerns; decomposition into frames guides analysis rather than allowing the solution structure to define the problem after the fact (S13, Abstract and §1, pp. 1–2).

The validation method is the implication between machine specification, domain properties, and requirement (S13, §2.3, p. 4). This is a method for ensuring the design is solving the intended problem, not just producing a coherent internal architecture.

### 8. Conceptual integrity as an architectural governance method: Brooks

Brooks’s method is organizational as well as representational: separate architecture from implementation, give a small number of agreeing minds control over externally visible concepts, and let implementers creatively solve the implementation under a coherent specification (S11, ch. 4, pp. 45–50). He argues for interactive communication and cost/performance feedback rather than isolating the architecture team from implementation realities (S11, ch. 5, pp. 54–56).

The method also supplies a selection rule: a feature that does not integrate with the system’s basic concepts should be omitted, or the system should be reconsidered around a new coherent set of concepts (S11, ch. 4, pp. 46–47). Conceptual integrity is therefore tested against the whole user-facing design, not against local elegance of a component.

### 9. ADRs as lightweight, continuous decision capture: Nygard and Fowler

Nygard’s method is to record significant decisions close to when they are made, in small numbered text files kept in the project repository, with context, decision, status, and consequences. The records are intentionally modular so they can remain current; a reversed decision is kept and marked superseded rather than rewritten (S14, “Context,” “Architecture,” and “Format”).

Fowler adds that writing an ADR has two purposes: preserving a record so people can later understand why the system is constructed as it is, and clarifying the thinking of the group while the decision is made (S15, “Architecture Decision Record”). Fowler’s architecture guide similarly treats architecture as deciding what is important and keeping those important elements in good condition, while warning against separating architecture from programming (S15, *Software Architecture Guide*). ADRs are therefore one method within a design practice, not a replacement for an architecture description.

### 10. What the standards deliberately leave open

IEEE 1016 specifies SDD information content and organization but explicitly does not prescribe design methodology, configuration management, or quality-assurance methodology (S1, §1.1, p. 1). ISO/IEC/IEEE 42010:2022 likewise specifies AD concepts and conformance properties but does not specify the processes, architecting methods, models, notations, techniques, tools, format, or media by which an AD is created or managed (S2, official abstract and §1 of the preview). The field therefore distinguishes **what a design description must make expressible and reviewable** from **the method used to discover and validate it**. ADD, QAW, ATAM, scenario-driven iteration, information hiding, problem frames, and ADRs are complementary methods, not competing mandatory templates.

---

## synthesis: 10 concrete rules for what makes a document a design rather than notes

Each rule is a cross-source synthesis; the citations identify the source requirements or arguments from which the rule follows.

1. **Name the subject, purpose, scope, status, audience, and context.** A design description identifies its subject and purpose, and IEEE 1016 requires scope, status, authorship, context, references, and change history; 42010 treats an AD as a work product made for a particular purpose and stakeholder need (S1, §4.2; S2, §§3.3, 5.2.2).

2. **Start with stakeholders’ concerns and requirements, not with a component inventory.** IEEE 1016 requires identifying stakeholders and each concern and addressing every concern; 42010 defines concerns as matters important to stakeholders; ADD begins from prioritized functional, constraint, and quality requirements (S1, §§4.3–4.5; S2, §§3.10, 5.2.3; S8, §3.1).

3. **State the design decisions that resolve those concerns.** A design assigns responsibilities, selects forms, boundaries, interfaces, and quality tactics; Parnas treats modules as responsibility assignments hiding decisions, while ADD defines output in roles, responsibilities, properties, and relationships (S5, “What Is Modularization” and “Conclusion”; S6, “An Architectural Model”; S8, §3.2).

4. **Use explicit viewpoints and views, and map each concern to its governing representation.** A viewpoint specifies conventions and analysis; a view instantiates them. IEEE 1016 requires one governing viewpoint per view, concern coverage, and viewpoint-selection rationale; 42010 and Kruchten make multiple coordinated views a stakeholder-facing architecture description (S1, §§4.4–4.5; S2, §§3.7–3.8; S3, §4.2.4; S6, pp. 2–3).

5. **Describe boundaries and externally observable contracts before internal detail.** Context and Interface viewpoints define services, actors, boundaries, and interaction rules; Parnas’s interface specification is a black-box contract; Jackson separates problem-domain properties from machine behavior at shared interfaces (S1, §§5.2, 5.8; S4, §V.C; S13, §2.3).

6. **Turn quality adjectives into scenarios with measurable responses.** QAW requires stimulus, source, environment, artifact, response, and response measure; ATAM uses scenarios to elicit, concretize, prioritize, and test quality goals (S9, pp. 3–4, 18–21; S10, §§5, 7).

7. **Make assumptions, constraints, dependencies, failure behavior, and change boundaries explicit.** IEEE 1016 requires design elements and constraints; Parnas’s information-hiding criterion puts likely-to-change decisions behind interfaces; Parnas & Clements require undesired events, uses hierarchy, and verification arguments (S1, §§4.5–4.6; S5, “The Criteria”; S4, §§V.B–V.E).

8. **Record rationale, alternatives, trade-offs, consequences, and unresolved decisions.** IEEE 1016 requires rationale; ISO 42010’s conceptual text defines rationale in terms of basis, alternatives, trade-offs, consequences, and sources; ATAM records risks/non-risks and their reasoning; ADRs preserve context and superseded decisions (S1, §4.8; S3, §4.2.7; S10, §7; S14; S15).

9. **Attach a review or validation argument to the design claims.** IEEE 1016 defines completeness, consistency, verification, validation, and traceability; ADD’s Check step analyzes whether requirements are met; Kruchten tests an architectural prototype; Jackson gives the implication from machine specification and domain properties to requirement (S1, §§3.2.2–3.2.3, 4.4; S8, §2; S6, pp. 13–14; S13, §2.3).

10. **Keep the document/reference set structured, current, and evolvable.** Parnas & Clements design documents by questions, place each fact once, review organization and accuracy, and update invalidated documents; Nygard and Fowler favor small modular records with status and supersession rather than a rewritten monolith (S4, §§VI.B–VII; S14, “Context” and “Format”; S15, “Architecture Decision Record”). A document that only records current nouns but cannot preserve why, what changed, and what must be rechecked is notes, not a maintained design.

---

## Bibliography

- Barbacci, M. R., Ellison, R., Lattanze, A. J., Stafford, J. A., Weinstock, C. B., & Wood, W. G. (2003). *Quality Attribute Workshops (QAWs), Third Edition*. CMU/SEI-2003-TR-016. SEI. <https://insights.sei.cmu.edu/documents/716/2003_005_001_14249.pdf>
- Bass, L., Clements, P., & Kazman, R. (2003). *Software Architecture in Practice*, 2nd ed. Addison-Wesley. Official SEI record: <https://www.sei.cmu.edu/library/software-architecture-in-practice-2nd-edition/>.
- Brooks, F. P., Jr. (1975). *The Mythical Man-Month: Essays on Software Engineering*. Addison-Wesley. Retrieved scan: <https://www.cs.cmu.edu/~15712/papers/mythicalmanmonth00fred.pdf>.
- Brooks, F. P., Jr. (2010). *The Design of Design: Essays from a Computer Scientist*. Addison-Wesley Professional. Official record: <https://www.informit.com/store/design-of-design-essays-from-a-computer-scientist-9780321770547> (full text not retrieved).
- Fowler, M. (2003). “Uml As Blueprint.” <https://martinfowler.com/bliki/UmlAsBlueprint.html>.
- Fowler, M. (2026). “Architecture Decision Record.” <https://martinfowler.com/bliki/ArchitectureDecisionRecord.html>.
- Fowler, M. “Software Architecture Guide.” <https://www.martinfowler.com/architecture/>.
- IEEE. (2009). *IEEE Std 1016-2009: IEEE Standard for Information Technology—Systems Design—Software Design Descriptions*. Official record: <https://standards.ieee.org/ieee/1016/4502/>; retrieved text copy: <https://wildart.github.io/MISG5020/standards/IEEE-1016-2009.pdf>.
- ISO/IEC/IEEE. (2011). *ISO/IEC/IEEE 42010:2011: Systems and software engineering—Architecture description* (withdrawn). Official status: <https://www.iso.org/standard/50508.html>; retrieved preview: <https://cdn.standards.iteh.ai/samples/50508/ec3d9367f48e4b28a7ac8dddaeca5a3f/ISO-IEC-IEEE-42010-2011.pdf>.
- ISO/IEC/IEEE. (2022). *ISO/IEC/IEEE 42010:2022: Software, systems and enterprise—Architecture description*. Official record: <https://www.iso.org/standard/74393.html>; retrieved preview: <https://cdn.standards.iteh.ai/samples/74393/fc7b7f103d8446a4b87a3261e31370d3/ISO-IEC-IEEE-42010-2022.pdf>.
- Jackson, M. (2001 [paper presented 2000]). “Problem Analysis and Structure.” In *Proceedings of the NATO Summer School, Marktoberdorf*. Retrieved paper: <https://people.csail.mit.edu/dnj/teaching/6898/papers/mj-marktoberdorf.pdf>.
- Kazman, R., Klein, M., & Clements, P. (2000). *ATAM: Method for Architecture Evaluation*. CMU/SEI-2000-TR-004. <https://sei.cmu.edu/documents/629/2000_005_001_13706.pdf>.
- Kruchten, P. (1995). “Architectural Blueprints—The ‘4+1’ View Model of Software Architecture.” *IEEE Software*, 12(6), 42–50. Retrieved paper: <https://www.cs.ubc.ca/~gregor/teaching/papers/4%2B1view-architecture.pdf>. DOI: <https://doi.org/10.1109/52.469759>.
- Nygard, M. (2011). “Documenting Architecture Decisions.” Cognitect Blog. <https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions>.
- Parnas, D. L. (1972). “On the Criteria To Be Used in Decomposing Systems into Modules.” *Communications of the ACM*, 15(12), 1053–1058. Retrieved copy: <https://john.cs.olemiss.edu/~hcc/csci555/notes/localcopy/Parnas_Criteria_Decomposing.pdf>. DOI: <https://doi.org/10.1145/361598.361623>.
- Parnas, D. L., & Clements, P. C. (1986). “A Rational Design Process: How and Why to Fake It.” *IEEE Transactions on Software Engineering*, SE-12(2), 251–257. Retrieved paper: <https://www.cs.tufts.edu/comp/40-2011f/readings/fake-it.pdf>. DOI: <https://doi.org/10.1109/TSE.1986.6312940>.
- Wojcik, R., Bachmann, F., Bass, L., Clements, P., Merson, P., Nord, R., & Wood, W. (2006). *Attribute-Driven Design (ADD), Version 2.0*. CMU/SEI-2006-TR-023. <https://sei.cmu.edu/documents/775/2006_005_001_14795.pdf>.
