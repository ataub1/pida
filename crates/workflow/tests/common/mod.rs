//! Gemeinsame Bausteine der Reducer-Tests.
#![allow(dead_code)]

use pdfover_backend_api::{
    BackendId, Capabilities, CertNeed, FactorSet, Interaction, Qualification, ServerNeed, SignInput, SignOutput,
};
use pdfover_pades::{AcceptanceClass, OutputInfo};
use pdfover_pdf_cos::Rect;
use pdfover_placement::{EmptyFieldCandidate, Placement, QrPayload, QrPlaceholder, TablePos};
use pdfover_workflow::{
    BackendOption, Context, DocFacts, DocId, Effect, Event, InputDocument, PlaceholderFound, PositionChoice, Queue,
    SignFlow, SigningPhase, State, Transition, reduce,
};

pub fn doc(id: u64) -> InputDocument {
    InputDocument::new(DocId(id), format!("/in/doc{id}.pdf"))
}

pub fn queue(ids: &[u64]) -> Queue {
    let mut q = Queue::default();
    q.add(ids.iter().map(|i| doc(*i)));
    q
}

pub fn start_placement(page_count: usize) -> Placement {
    Placement { page_index: page_count - 1, x: 297.5, y: 210.5, width: 230.0, height: 49.0 }
}

pub fn facts(id: u64, page_count: usize) -> DocFacts {
    DocFacts {
        id: DocId(id),
        page_count,
        signed: false,
        class: AcceptanceClass::K1Regular,
        start: Some(start_placement(page_count)),
    }
}

pub fn analyzed(id: u64, page_count: usize, placeholder: Option<PlaceholderFound>, signed: bool) -> Event {
    Event::Analyzed {
        doc: DocId(id),
        class: AcceptanceClass::K1Regular,
        placeholder,
        signed,
        page_count,
        start: Some(start_placement(page_count)),
        warnings: Vec::new(),
    }
}

pub fn field(name: &str) -> EmptyFieldCandidate {
    EmptyFieldCandidate {
        qualified_name: name.into(),
        field_id: (10, 0),
        widget_id: (10, 0),
        page_index: Some(0),
        rect: Rect { llx: 10.0, lly: 10.0, urx: 210.0, ury: 60.0 },
    }
}

pub fn qr() -> QrPlaceholder {
    QrPlaceholder {
        page_index: 0,
        resource_name: "Im1".into(),
        payload: QrPayload { id: Some("1".into()), raw: "PDF-AS-POS;id=1".into(), ..Default::default() },
        pos: TablePos::manual(1, 100.0, 700.0),
        ctm: [100.0, 0.0, 0.0, 100.0, 100.0, 600.0],
    }
}

pub fn caps_atrust() -> Capabilities {
    Capabilities {
        input: SignInput::Document,
        output: SignOutput::Cms,
        certificate: CertNeed::OnlyForVisibleBlock,
        interaction: Interaction::NativeForms,
        server: ServerNeed::None,
        max_batch: 1,
        login_once_per_batch: true,
        document_leaves_device: true,
        qualification: Qualification::Qes,
        factors: FactorSet { app: true, sms: true, qr: true, fido2: false, card: false },
        cms_reserve: Some(16 * 1024),
    }
}

pub fn caps_keystore() -> Capabilities {
    Capabilities {
        input: SignInput::Digest { algs: vec![] },
        output: SignOutput::RawSignature,
        certificate: CertNeed::Required,
        interaction: Interaction::Secret,
        server: ServerNeed::None,
        max_batch: u16::MAX,
        login_once_per_batch: true,
        document_leaves_device: false,
        qualification: Qualification::AdES,
        factors: FactorSet::default(),
        cms_reserve: None,
    }
}

pub fn caps_localbku() -> Capabilities {
    Capabilities {
        input: SignInput::Document,
        output: SignOutput::Cms,
        certificate: CertNeed::Required,
        interaction: Interaction::External,
        server: ServerNeed::None,
        max_batch: 1,
        login_once_per_batch: false,
        document_leaves_device: false,
        qualification: Qualification::Qes,
        factors: FactorSet { card: true, ..FactorSet::default() },
        cms_reserve: Some(16 * 1024),
    }
}

/// Optionen: ID Austria verfügbar, lokale BKU und Keystore wie angegeben.
pub fn options(localbku: bool, keystore: bool) -> Vec<BackendOption> {
    vec![
        BackendOption { id: BackendId::AtrustNative, capabilities: Some(caps_atrust()) },
        BackendOption { id: BackendId::LocalBku, capabilities: localbku.then(caps_localbku) },
        BackendOption { id: BackendId::Keystore, capabilities: keystore.then(caps_keystore) },
    ]
}

pub fn output_info(id: u64) -> OutputInfo {
    OutputInfo { path: format!("/in/doc{id}_signed.pdf").into(), len: 1000, sha256: "ab".into() }
}

/// Reducer mit Kontext.
pub fn step(state: State, event: Event, ctx: &Context) -> Transition {
    reduce(state, event, ctx)
}

/// Folge von Ereignissen; liefert den Endzustand und alle Effekte.
pub fn run(state: State, events: impl IntoIterator<Item = Event>, ctx: &Context) -> (State, Vec<Effect>) {
    let mut state = state;
    let mut effects = Vec::new();
    for e in events {
        let t = reduce(state, e, ctx);
        state = t.state;
        effects.extend(t.effects);
    }
    (state, effects)
}

pub fn signing(ids: &[u64], phase: SigningPhase, backend: BackendId, position: PositionChoice) -> State {
    let flow = SignFlow { needs_certificate: true, min_reserve: None };
    State::Signing { queue: queue(ids), position, backend, phase, doc: facts(ids[0], 3), flow }
}

pub fn output_state(ids: &[u64]) -> State {
    State::Output {
        queue: queue(ids),
        saved: None,
        decision: None,
        doc: facts(ids[0], 3),
        position: PositionChoice::Auto,
        backend: BackendId::Keystore,
    }
}

pub fn choosing(ids: &[u64], position: PositionChoice, auto_choice: bool) -> State {
    State::ChoosingBackend { queue: queue(ids), position, options: Vec::new(), doc: facts(ids[0], 3), auto_choice }
}

pub fn positioning(ids: &[u64]) -> State {
    State::Positioning {
        queue: queue(ids),
        page: 2,
        placement: Some(start_placement(3)),
        new_page: false,
        doc: facts(ids[0], 3),
    }
}
