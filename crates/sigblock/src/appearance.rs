//! Appearance-XObjects als lopdf-Objektgraph (docs/analyse/03 §4.7, docs/analyse/04 §3.7).
//!
//! Der Graph verwendet **lokale Referenzen** `Object::Reference((i, LOCAL_GENERATION))` mit
//! `i` = Index in [`Appearance::objects`]. [`Appearance::materialize`] ersetzt sie durch echte IDs
//! des `IncrementBuilder` (`pdfover-pdf-incremental`).

use pdfover_pdf_cos::{Object, ObjectId};

use crate::Result;
use crate::fonts::FontProvider;
use crate::input::{PdfaMode, SigBlockInput};
use crate::layout::Layout;
use crate::profile::{Profile, Resources};

/// Generationsnummer lokaler Referenzen (kommt in neuen Objekten nie vor).
pub const LOCAL_GENERATION: u16 = u16::MAX;

/// Optionen für [`Appearance::build`].
#[derive(Debug, Clone, PartialEq)]
pub struct BuildOptions {
    /// Blockbreite aus der Positionierung.
    pub width: f32,
    /// Rotation des Inhalts: Blockrotation + Seitenrotation (Grad, Vielfaches von 90) → `/Matrix`.
    pub rotation: i32,
    /// PDF/A-Behandlung.
    pub pdfa: PdfaMode,
    /// Nur Vorschau: Holder ohne `Do` (wie `buildPreviewOnly`), Tabelle als Seiteninhalt im Mini-PDF.
    pub preview_only: bool,
}

/// Fertiges Erscheinungsbild.
#[derive(Debug, Clone, PartialEq)]
pub struct Appearance {
    /// Breite W in pt.
    pub width: f32,
    /// Höhe H in pt.
    pub height: f32,
    /// Layout (für Vorschau, `/TU`, Tests).
    pub layout: Option<Layout>,
    /// Objekte (Holder, Inner, Fonts, Deskriptoren, FontFile2, ToUnicode, Bilder, SMasks …).
    pub objects: Vec<Object>,
    /// Index des Holder-Form-XObjects (Ziel von `/AP /N`).
    pub holder: usize,
    /// Index des Inner-Form-XObjects (`/DR /XObject /FRM1`).
    pub inner: usize,
    /// Ressourcenname des Inner-Forms (`FRM1`).
    pub inner_name: String,
    /// `/TU`-Text.
    pub tu_text: String,
}

/// Appearance mit echten Objekt-IDs.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterializedAppearance {
    /// ID des Holder-Forms.
    pub holder_id: ObjectId,
    /// ID des Inner-Forms.
    pub inner_id: ObjectId,
    /// Ressourcenname des Inner-Forms.
    pub inner_name: String,
    /// Alle Objekte mit ihren IDs (in [`Appearance::objects`]-Reihenfolge).
    pub objects: Vec<(ObjectId, Object)>,
}

impl Appearance {
    /// Baut das Erscheinungsbild eines sichtbaren Profils.
    pub fn build(
        profile: &Profile,
        input: &SigBlockInput,
        opts: &BuildOptions,
        fonts: &FontProvider,
        resources: &dyn Resources,
    ) -> Result<Appearance> {
        todo!("Paket C")
    }

    /// Unsichtbare Signatur: leeres Form-XObject mit `/BBox [0 0 0 0]` (docs/analyse/03 §4.6).
    pub fn invisible() -> Appearance {
        todo!("Paket C")
    }

    /// Ersetzt lokale Referenzen; `alloc` liefert neue IDs in Objektreihenfolge.
    pub fn materialize(&self, alloc: &mut dyn FnMut() -> ObjectId) -> MaterializedAppearance {
        todo!("Paket C")
    }

    /// Eigenständiges Mini-PDF mit genau diesem Block (Blockvorschau über denselben Renderer,
    /// Plan §6.6); Seitengröße `W × H`.
    pub fn to_standalone_pdf(&self) -> Result<Vec<u8>> {
        todo!("Paket C")
    }
}
