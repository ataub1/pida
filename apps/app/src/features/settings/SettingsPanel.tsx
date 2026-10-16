/**
 * Einstellungen als Seitenleiste über der App (ADR 0004 §2.8, §3.11; Gestalt aus dem Prototyp
 * `prototypes/pida-workspace/src/features/settings/SettingsPanel.tsx`): Änderungen gelten **sofort**
 * (`apply_settings`, kein Speichern/Abbrechen), Suche über alle Bereiche mit Synonymen (`.keywords`),
 * Bereichsnavigation links (schmal und auf Mobilgeräten waagrecht, im Vollbild). Bereiche: Signaturbild
 * (Live-Editor mit Vorschau, die beim Bearbeiten oben sichtbar bleibt), Dokument (PDF/A, Platzhaltersuche),
 * Signiermittel, Speichern, Darstellung, Datenschutz, Erweitert, Über.
 *
 * Schließen (×, Escape, Klick daneben) sendet zuerst wartende Texteingaben und dann `close_settings` – es gibt
 * nichts zu verwerfen. Schreibgeschützte Einstellungen (`read_only`, `-c`) zeigen ein Banner; Änderungen
 * gelten dann bis zum Programmende (Kern). Mobile: ohne Proxy, Ordner, Update-Prüfung und Protokolle;
 * Keystore nur über den PKCS#12-Import.
 */
import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Button as AriaButton, Input, SearchField } from 'react-aria-components';
import type { SettingsSectionId } from '../../app/context';
import { useApp } from '../../app/context';
import { EMBLEM_DROP_EVENT } from '../../app/events';
import { isMobile, type SettingsView, type SignatureProfile } from '../../core/types';
import { DialogHeader, ModalShell } from '../../ui/Dialog';
import { IconClose, IconFileText, IconFolder, IconInfo, IconPhone, IconSearch, IconSettings, IconShield, IconSignature, IconSun } from '../../ui/Icons';
import { AboutRow, CertCacheRow, LogsRow, PRIVACY_HEADING_ID, PrivacyInfoRow, ProxyRow, RecentRow, ResetRow, ThemeRow, UiLangRow, UpdatesRow } from './OtherRows';
import { ROWS, SECTIONS, search, visibleRows, type RowDef, type RowId, type SectionId } from './rows';
import { BlockLangRow, BlockPreview, EmblemRow, LogoSizeRow, NoteRow, PdfaRow, PlaceholderRow, ProfileRow } from './SignatureRows';
import { CardRow, DefaultSignerRow, Fido2Row, IdAustriaRow, KeystoreRow, PostfixRow, SaveTargetRow } from './SignerRows';
import { SettingsProvider, useSettings } from './state';
import './settings.css';

/** Props des Einstellungs-Overlays (in E0 festgelegt). */
export interface SettingsPanelProps {
  settingsView: SettingsView;
  /** Abschnitt beim Öffnen */
  section: SettingsSectionId;
}

const SECTION_ICON: Record<SectionId, ReactNode> = {
  signature: <IconSignature size={18} />,
  document: <IconFileText size={18} />,
  signers: <IconPhone size={18} />,
  save: <IconFolder size={18} />,
  appearance: <IconSun size={18} />,
  privacy: <IconShield size={18} />,
  advanced: <IconSettings size={18} />,
  about: <IconInfo size={18} />,
};

export function SettingsPanel({ settingsView, section }: SettingsPanelProps) {
  const { view } = useApp();
  return (
    <SettingsProvider view={settingsView} mobile={isMobile(view)}>
      <Panel initial={section} />
    </SettingsProvider>
  );
}

function SettingRow({ id, goto }: { id: RowId; goto: (section: SectionId, focus?: string) => void }) {
  switch (id) {
    case 'signature.preview':
      return <BlockPreview />;
    case 'signature.profile':
      return <ProfileRow />;
    case 'signature.language':
      return <BlockLangRow />;
    case 'signature.emblem':
      return <EmblemRow />;
    case 'signature.logo_size':
      return <LogoSizeRow />;
    case 'signature.note':
      return <NoteRow />;
    case 'document.pdfa':
      return <PdfaRow />;
    case 'document.placeholder':
      return <PlaceholderRow />;
    case 'signers.default':
      return <DefaultSignerRow />;
    case 'signers.id_austria':
      return <IdAustriaRow />;
    case 'signers.fido2':
      return <Fido2Row />;
    case 'signers.keystore':
      return <KeystoreRow />;
    case 'signers.card':
      return <CardRow />;
    case 'save.target':
      return <SaveTargetRow />;
    case 'save.postfix':
      return <PostfixRow />;
    case 'appearance.theme':
      return <ThemeRow />;
    case 'appearance.language':
      return <UiLangRow />;
    case 'privacy.info':
      return <PrivacyInfoRow />;
    case 'privacy.certcache':
      return <CertCacheRow />;
    case 'privacy.recent':
      return <RecentRow />;
    case 'advanced.proxy':
      return <ProxyRow />;
    case 'advanced.updates':
      return <UpdatesRow />;
    case 'advanced.reset':
      return <ResetRow />;
    case 'about.info':
      return <AboutRow onPrivacy={() => goto('privacy', PRIVACY_HEADING_ID)} />;
    case 'about.logs':
      return <LogsRow />;
  }
}

const PREVIEW_ROW = ROWS.find((r) => r.id === 'signature.preview') as RowDef;

function Panel({ initial }: { initial: SectionId }) {
  const { t, dispatch } = useApp();
  const s = useSettings();
  const [current, setCurrent] = useState<SectionId>(initial);
  const [query, setQuery] = useState('');
  const content = useRef<HTMLDivElement>(null);

  const close = () => {
    s.flush();
    void dispatch({ type: 'close_settings' });
  };

  // Auf Bildmarke bzw. Vorschau gezogene Bilddatei (Fenster-Ereignisse der Shell, `App.tsx`) → Bildmarke
  const apply = useRef(s.apply);
  apply.current = s.apply;
  useEffect(() => {
    const onDrop = (e: Event) => {
      const path = (e as CustomEvent<string>).detail;
      if (typeof path === 'string' && path) apply.current({ 'signature.emblem': path });
    };
    window.addEventListener(EMBLEM_DROP_EVENT, onDrop);
    return () => window.removeEventListener(EMBLEM_DROP_EVENT, onDrop);
  }, []);

  const goto = (section: SectionId, focus?: string) => {
    setQuery('');
    setCurrent(section);
    content.current?.scrollTo?.({ top: 0 });
    if (focus) setTimeout(() => document.getElementById(focus)?.focus(), 0);
  };

  const rows = visibleRows({ mobile: s.mobile, profile: s.get<SignatureProfile>('signature.profile') });
  const q = query.trim();
  const hits = q ? search(t, rows, q) : rows.filter((r) => r.section === current);
  // Signaturbild-Treffer zeigen die Live-Vorschau mit (Änderungen gelten sofort)
  const shown = q && hits.some((r) => r.section === 'signature') && !hits.some((r) => r.id === 'signature.preview') ? [PREVIEW_ROW, ...hits] : hits;
  const sections = SECTIONS.filter((sec) => shown.some((r) => r.section === sec.id));

  return (
    <ModalShell onClose={close} variant="drawer" size="lg" className="settings-modal">
      <DialogHeader title={t.t('ng-ui-set-title')} subtitle={t.t('ng-ui-set-instant')} onClose={close} icon={<IconSettings size={20} />} />
      {s.view.read_only ? (
        <p className="settings-readonly" role="note" data-testid="settings-read-only">
          <IconInfo size={16} />
          <span>{t.t('ng-ui-settings-read-only')}</span>
        </p>
      ) : null}
      <div className="settings-search">
        <SearchField aria-label={t.t('ng-ui-set-search')} value={query} onChange={setQuery} className="search-field">
          <IconSearch size={17} className="search-icon" />
          <Input className="input search-input" placeholder={t.t('ng-ui-set-search')} />
          <AriaButton className="search-clear" aria-label={t.t('ng-ui-set-search-clear')}>
            <IconClose size={14} />
          </AriaButton>
        </SearchField>
      </div>
      <div className={`settings-layout${q ? ' is-searching' : ''}`}>
        {q ? null : (
          <nav className="settings-nav" aria-label={t.t('ng-ui-set-sections')}>
            {SECTIONS.map((sec) => (
              <AriaButton
                key={sec.id}
                className={`settings-nav-item${current === sec.id ? ' is-current' : ''}`}
                aria-current={current === sec.id ? 'true' : undefined}
                onPress={() => goto(sec.id)}
              >
                {SECTION_ICON[sec.id]}
                <span>{t.t(sec.label)}</span>
              </AriaButton>
            ))}
          </nav>
        )}
        <div className="settings-content" ref={content}>
          <p className="sr-only" role="status">
            {q ? t.t('ng-ui-set-search-hits', { n: hits.length }) : ''}
          </p>
          {q && shown.length === 0 ? <p className="muted settings-empty">{t.t('ng-ui-set-search-empty', { q })}</p> : null}
          {sections.map((sec) => (
            <section key={sec.id} className="settings-section" aria-labelledby={`set-h-${sec.id}`}>
              <h3 className="settings-section-title" id={`set-h-${sec.id}`}>
                {t.t(sec.label)}
              </h3>
              {shown
                .filter((r) => r.section === sec.id)
                .map((r) => (
                  <div key={r.id} className="settings-row" data-row={r.id}>
                    <SettingRow id={r.id} goto={goto} />
                  </div>
                ))}
            </section>
          ))}
        </div>
      </div>
    </ModalShell>
  );
}
