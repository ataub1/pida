/**
 * „QR-Platzhalter erstellen“ (ADR 0004 Nachtrag 24, Analyse 06 E-1; Vorbild PDF-AS-WAI `qrCodeGeneration`): für die
 * Person, die ein Dokument für andere vorbereitet. Nummer, Signaturbild und Sprache → Vorschau des Kerns
 * (`pida-render://qr/…`) → Speichern als PNG bzw. SVG (`export_qr_placeholder`: am Desktop mit Speichern-Dialog,
 * auf Mobilgeräten über „Teilen“, im Web als Download). Offline, nichts geht an einen Server.
 */
import { useId, useState } from 'react';
import { useApp } from '../../app/context';
import { isMobile, type Lang, type QrPlaceholderFormat, type QrPlaceholderProfile, type QrPlaceholderSpec } from '../../core/types';
import { webProfile } from '../../core/web-profile';
import { Button, RadioCards, Segmented, TextInput } from '../../ui/controls';
import { DialogHeader, ModalShell } from '../../ui/Dialog';
import { IconDownload, IconQr, IconShare } from '../../ui/Icons';
import './qr.css';

/** Nummer wie im Kern: nur Ziffern, 1 bis 10 Stellen. */
export function qrIdValid(id: string): boolean {
  return /^\d{1,10}$/.test(id);
}

function fileName(spec: QrPlaceholderSpec, format: QrPlaceholderFormat): string {
  return `${spec.language === 'en' ? 'qr-placeholder' : 'qr-platzhalter'}-${spec.id}.${format}`;
}

export function QrPlaceholderDialog({ onClose }: { onClose: () => void }) {
  const { t, view, bridge, dispatch, announce, toast } = useApp();
  const hintId = useId();
  const [id, setId] = useState('0001');
  const [profile, setProfile] = useState<QrPlaceholderProfile>('standard');
  const [language, setLanguage] = useState<Lang>(view.language);
  const [error, setError] = useState<string | null>(null);
  // letzte gültige Nummer für die Vorschau (beim Tippen kein kaputtes Bild)
  const [shown, setShown] = useState('0001');
  const mobile = isMobile(view);
  const web = webProfile() !== null;
  const spec: QrPlaceholderSpec = { id, profile, language };
  const preview: QrPlaceholderSpec = { id: shown, profile, language };
  const profileLabel = t.t(`ng-ui-shell-qr-profile-${profile}`);
  const langLabel = language === 'en' ? 'English' : 'Deutsch';

  const save = async (format: QrPlaceholderFormat) => {
    if (!qrIdValid(id)) {
      const msg = t.t('ng-qr-invalid-id');
      setError(msg);
      announce(msg);
      return;
    }
    let path: string | null = null;
    // Desktop-App: Speichern-Dialog; Mobile teilt, das Web lädt herunter (der Kern entscheidet ohne Pfad)
    if (!mobile && !web) {
      try {
        path = await bridge.pickSavePath({
          title: t.t('ng-ui-shell-qr-title'),
          filters: [{ name: format.toUpperCase(), extensions: [format] }],
          defaultPath: fileName(spec, format),
        });
      } catch {
        toast(t.t('ng-ui-dialog-failed'), 'error');
        return;
      }
      if (!path) return;
    }
    void dispatch({ type: 'export_qr_placeholder', spec, format, path });
  };

  return (
    <ModalShell onClose={onClose} size="md" className="qr-dialog">
      <DialogHeader title={t.t('ng-ui-shell-qr-title')} subtitle={t.t('ng-ui-shell-qr-subtitle')} icon={<IconQr size={20} />} onClose={onClose} />
      <div className="qr-body">
        {/* Vorschau oben: bleibt beim Ändern der Felder im Blick */}
        <figure className="qr-preview">
          <img
            src={bridge.qrPlaceholderUrl(preview, 'svg')}
            alt={t.t('ng-ui-shell-qr-alt', { id: shown, profile: profileLabel, lang: langLabel })}
            width={240}
            height={48}
            data-testid="qr-preview"
          />
          <figcaption className="qr-hint" id={hintId}>
            {t.t('ng-ui-shell-qr-howto')}
          </figcaption>
        </figure>
        <div className="qr-fields">
          <TextInput
            label={t.t('ng-ui-shell-qr-id')}
            description={t.t('ng-ui-shell-qr-id-hint')}
            value={id}
            onChange={(v) => {
              setId(v);
              setError(null);
              if (qrIdValid(v)) setShown(v);
            }}
            inputMode="numeric"
            autoComplete="off"
            maxLength={10}
            error={error}
          />
          <RadioCards<QrPlaceholderProfile>
            label={t.t('ng-ui-shell-qr-profile')}
            value={profile}
            onChange={setProfile}
            options={(['standard', 'amtssignatur', 'signer'] as const).map((p) => ({
              value: p,
              label: t.t(`ng-ui-shell-qr-profile-${p}`),
              description: t.t(`ng-ui-shell-qr-profile-${p}-desc`),
            }))}
          />
          <Segmented<Lang>
            // „Wie bei der unterzeichnenden Person“: die Sprache betrifft nur die Beschriftung des Platzhalters
            label={t.t(profile === 'signer' ? 'ng-ui-shell-qr-lang-caption' : 'ng-ui-shell-qr-lang')}
            value={language}
            onChange={setLanguage}
            options={[
              { value: 'de', label: 'Deutsch', lang: 'de' },
              { value: 'en', label: 'English', lang: 'en' },
            ]}
          />
        </div>
      </div>
      <div className="confirm-actions dialog-actions">
        <Button variant="secondary" onPress={() => void save('svg')} aria-describedby={hintId}>
          {mobile ? <IconShare size={16} /> : <IconDownload size={16} />}
          {t.t(mobile ? 'ng-ui-shell-qr-share-svg' : 'ng-ui-shell-qr-save-svg')}
        </Button>
        <Button variant="primary" onPress={() => void save('png')} aria-describedby={hintId}>
          {mobile ? <IconShare size={16} /> : <IconDownload size={16} />}
          {t.t(mobile ? 'ng-ui-shell-qr-share-png' : 'ng-ui-shell-qr-save-png')}
        </Button>
      </div>
    </ModalShell>
  );
}
