//! „QR-Platzhalter erstellen“ mit echtem PDFium (nur mit `PIDA_PDFIUM_DIR`, sonst übersprungen): das PNG trägt
//! 360 dpi und wird von PIDAs eigener Platzhaltersuche als derselbe Platzhalter erkannt.

use pida_core_api::qr_placeholder::{self, QrPlaceholderFormat, QrPlaceholderProfile, QrPlaceholderSpec};
use pida_i18n::Lang;

fn backend() -> Option<pida_render::PdfiumBackend> {
    let dir = std::env::var_os("PIDA_PDFIUM_DIR").filter(|d| !d.is_empty())?;
    Some(pida_render::PdfiumBackend::bind(Some(std::path::Path::new(&dir))).expect("PDFium bindbar"))
}

#[test]
fn png_is_recognised_as_the_same_placeholder() {
    let Some(render) = backend() else { return };
    for (id, profile, language, want) in [
        ("0001", QrPlaceholderProfile::Standard, Lang::De, Some("SIGNATURBLOCK_SMALL_DE")),
        ("42", QrPlaceholderProfile::Amtssignatur, Lang::En, Some("AMTSSIGNATURBLOCK_EN_RECOMMENDED")),
        ("7", QrPlaceholderProfile::Signer, Lang::De, None),
    ] {
        let spec = QrPlaceholderSpec { id: id.to_owned(), profile, language };
        let png = qr_placeholder::image(&spec, QrPlaceholderFormat::Png, Some(&render)).expect("PNG");
        let decoder = png::Decoder::new(std::io::Cursor::new(&png));
        let mut reader = decoder.read_info().expect("PNG lesbar");
        let info = reader.info();
        // 240 × 48 pt bei 5 px/pt, 360 dpi
        assert_eq!((info.width, info.height), (1200, 240));
        let dims = info.pixel_dims.expect("pHYs");
        assert_eq!((dims.xppu, dims.unit), (14173, png::Unit::Meter));
        let mut buf = vec![0; reader.output_buffer_size().expect("Größe")];
        let frame = reader.next_frame(&mut buf).expect("Frame");
        let rgba: Vec<u8> =
            buf[..frame.buffer_size()].as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
        let image = pida_render::RgbaImage::from_raw(frame.width, frame.height, rgba).unwrap();
        let payload = pida_placement::decode_qr(&image)
            .iter()
            .find_map(|t| pida_placement::parse_qr_payload(t))
            .unwrap_or_else(|| panic!("{id}: kein QR-Platzhalter erkannt"));
        assert_eq!(payload.id.as_deref(), Some(id));
        assert_eq!(payload.profile.as_deref(), want);
    }
}

#[test]
fn svg_needs_no_renderer_png_does() {
    let spec = QrPlaceholderSpec { id: "1".into(), profile: QrPlaceholderProfile::Standard, language: Lang::De };
    assert!(qr_placeholder::image(&spec, QrPlaceholderFormat::Svg, None).is_ok());
    assert_eq!(qr_placeholder::image(&spec, QrPlaceholderFormat::Png, None), Err(qr_placeholder::MSG_FAILED));
}
