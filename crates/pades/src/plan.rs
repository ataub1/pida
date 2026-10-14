//! Signierplan (was in das Increment kommt; Plan §6.4).

use std::sync::Arc;

use pdfover_cades::CmsProfile;
use pdfover_pdf_cos::{ObjectId, Rect};
use pdfover_sigblock::Appearance;
use time::OffsetDateTime;

/// Reserve für `/Contents`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reserve {
    /// Modell B: `max(16 KiB, 2 × größtes gemessenes CMS)`; vor `prepare` vergrößert, wenn die
    /// Schätzung 50 % übersteigt.
    ModelB,
    /// Modell A: exakt berechnet + 1 KiB, mindestens 4 KiB; Profil-Key `signatureSize` respektiert.
    ModelA,
    /// fester Wert in Bytes (z. B. Neuvorbereitung nach `CmsTooLarge`)
    Bytes(usize),
}

/// Sichtbarer Block auf einer Seite.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockPlacement {
    /// Erscheinungsbild (aus `pdfover-sigblock`)
    pub appearance: Arc<Appearance>,
    /// 0-basierte Seite (bei `new_page` = Index der neuen Seite)
    pub page_index: usize,
    /// neue Seite anhängen (gleiche MediaBox und `/Rotate` wie die letzte)
    pub new_page: bool,
    /// Widget-`/Rect` (aus `pdfover_placement::widget_rect`)
    pub rect: Rect,
}

/// Sichtbarkeit.
#[derive(Debug, Clone, PartialEq)]
pub enum Visibility {
    /// Sichtbarer Block
    Visible(BlockPlacement),
    /// unsichtbar: `/Rect [0 0 0 0]`, erste Seite, leeres AP
    Invisible,
}

/// Zielfeld.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldTarget {
    /// Neues Feld+Widget (Name Default `PDF-AS Signatur<n>`, E-07)
    New {
        /// expliziter Name (sonst Default)
        name: Option<String>,
    },
    /// Vorhandenes leeres Feld (Rect bleibt, AP ersetzt, `/F 4`, `/V` angehängt; Widget in `/Kids`
    /// wird mitgeschrieben)
    Existing {
        /// Feld-ID
        field_id: ObjectId,
        /// Widget-ID
        widget_id: ObjectId,
        /// qualifizierter Name (Anzeige/Fehler)
        qualified_name: String,
    },
}

/// Plan für `prepare()`.
#[derive(Debug, Clone, PartialEq)]
pub struct SignPlan {
    /// Profil-ID (12 PDF-Over-IDs oder eigenes Profil)
    pub profile_id: String,
    /// Sichtbarkeit
    pub visibility: Visibility,
    /// Zielfeld
    pub field: FieldTarget,
    /// `/M` und signing-time (vor der Nutzerinteraktion, wie PDF-AS)
    pub signing_time: OffsetDateTime,
    /// `/Name` (aufgelöster `SIG_SUBJECT`; entfällt bei unsichtbar ohne Zertifikat)
    pub name: Option<String>,
    /// `/Reason` (`adobeSignReasonValue` bzw. „PAdES Signature“)
    pub reason: Option<String>,
    /// `/Location` (Ressourcenname des QR-Platzhalters)
    pub location: Option<String>,
    /// Reserve
    pub reserve: Reserve,
    /// CMS-Profil (Modell A)
    pub cms_profile: CmsProfile,
    /// sRGB-OutputIntent ergänzen, falls keiner vorhanden (PDF/A, Plan §6.9)
    pub add_output_intent_if_missing: bool,
    /// PDF/UA-Strukturelemente fortschreiben (StructTreeRoot vorhanden)
    pub pdfua: bool,
}
