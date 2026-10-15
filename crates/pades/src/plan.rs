//! Signierplan (was in das Increment kommt; Plan §6.4).

use std::sync::Arc;

use pida_cades::CmsProfile;
use pida_pdf_cos::{ObjectId, Rect};
use pida_sigblock::Appearance;
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
    /// Modell B mit Mindestreserve des Backends (`Capabilities::cms_reserve`); wie
    /// [`Reserve::ModelB`] vergrößert, wenn die Schätzung 50 % übersteigt (additiv, Welle 2).
    ModelBAtLeast(usize),
}

impl Reserve {
    /// Modell-B-Reserve aus `Capabilities::cms_reserve` (`None` = 16 KiB).
    pub fn model_b(cms_reserve: Option<usize>) -> Reserve {
        match cms_reserve {
            Some(n) => Reserve::ModelBAtLeast(n),
            None => Reserve::ModelB,
        }
    }
}

/// Sichtbarer Block auf einer Seite.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockPlacement {
    /// Erscheinungsbild (aus `pida-sigblock`)
    pub appearance: Arc<Appearance>,
    /// 0-basierte Seite (bei `new_page` = Index der neuen Seite)
    pub page_index: usize,
    /// neue Seite anhängen (gleiche MediaBox und `/Rotate` wie die letzte)
    pub new_page: bool,
    /// Widget-`/Rect` (aus `pida_placement::widget_rect`)
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

/// Zielfeld. `Debug` zeigt den Feldnamen nur als Länge (Dokumentinhalt, Regel 9).
#[derive(Clone, PartialEq, Eq)]
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

/// Plan für `prepare()`. `Debug` zeigt `/Name` und `/Reason` nur als Länge (Regel 9).
#[derive(Clone, PartialEq)]
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
    /// Zwischenzertifikate ins CMS einbetten (Modell A; PDF-AS: nein, Entscheidung S3, Plan §6.10) –
    /// wirkt auf `Reserve::ModelA` (`cades::estimate_cms_len`) und `Prepared::finish_raw`
    /// (`AssembleOptions::include_intermediates`).
    pub include_intermediates: bool,
    /// sRGB-OutputIntent ergänzen, falls keiner vorhanden (PDF/A, Plan §6.9)
    pub add_output_intent_if_missing: bool,
    /// PDF/UA-Strukturelemente fortschreiben (StructTreeRoot vorhanden)
    pub pdfua: bool,
    /// Profil-Key `signatureSize` (Mindestreserve in Bytes; additiv, Welle 2)
    pub signature_size: Option<usize>,
}

impl std::fmt::Debug for FieldTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = |s: &str| s.chars().count();
        match self {
            FieldTarget::New { name } => f.debug_struct("New").field("name_len", &name.as_deref().map(len)).finish(),
            FieldTarget::Existing { field_id, widget_id, qualified_name } => f
                .debug_struct("Existing")
                .field("field_id", field_id)
                .field("widget_id", widget_id)
                .field("qualified_name_len", &len(qualified_name))
                .finish(),
        }
    }
}

impl std::fmt::Debug for SignPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = |v: &Option<String>| v.as_ref().map(|s| s.chars().count());
        f.debug_struct("SignPlan")
            .field("profile_id", &self.profile_id)
            .field("visibility", &self.visibility)
            .field("field", &self.field)
            .field("signing_time", &self.signing_time)
            .field("name_len", &len(&self.name))
            .field("reason_len", &len(&self.reason))
            .field("location", &self.location)
            .field("reserve", &self.reserve)
            .field("cms_profile", &self.cms_profile)
            .field("include_intermediates", &self.include_intermediates)
            .field("add_output_intent_if_missing", &self.add_output_intent_if_missing)
            .field("pdfua", &self.pdfua)
            .field("signature_size", &self.signature_size)
            .finish()
    }
}

impl SignPlan {
    /// Unsichtbarer Plan mit neuem Feld und Defaults (Modell A, `PdfasCompat`, ohne Kette) – Basis
    /// für Tests und einfache Aufrufer.
    pub fn invisible(signing_time: OffsetDateTime) -> SignPlan {
        SignPlan {
            profile_id: "INVISIBLE".into(),
            visibility: Visibility::Invisible,
            field: FieldTarget::New { name: None },
            signing_time,
            name: None,
            reason: Some("PAdES Signature".into()),
            location: None,
            reserve: Reserve::ModelA,
            cms_profile: CmsProfile::PdfasCompat,
            include_intermediates: false,
            add_output_intent_if_missing: false,
            pdfua: false,
            signature_size: None,
        }
    }

    /// Sichtbar?
    pub fn is_visible(&self) -> bool {
        matches!(self.visibility, Visibility::Visible(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_name_reason_and_field_name() {
        let mut p = SignPlan::invisible(OffsetDateTime::UNIX_EPOCH);
        p.name = Some("Max Mustermann".into());
        p.reason = Some("Vertrag 4711".into());
        p.field =
            FieldTarget::Existing { field_id: (1, 0), widget_id: (1, 0), qualified_name: "Unterschrift Max".into() };
        let d = format!("{p:?}");
        assert!(!d.contains("Mustermann") && !d.contains("4711") && !d.contains("Unterschrift"), "{d}");
        assert!(d.contains("name_len: Some(14)"), "{d}");
    }
}
