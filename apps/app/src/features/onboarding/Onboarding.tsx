/**
 * Ersteinrichtung (ADR 0004 §2.2, §3.12; Gestalt aus dem Prototyp `features/onboarding/Onboarding.tsx`) – nur
 * beim ersten Start (`ViewState::first_run`, fehlende Einstellungsdatei), Bestandsnutzer sehen sie nie:
 *
 * 1. **PDF-Over-Einstellungen übernehmen** – nur mit Altkonfiguration (`first_run.migration`), mit der Liste
 *    der Übernahmen; ohne Altkonfiguration beginnt die Einrichtung direkt mit Schritt 2.
 * 2. **Standard-Signiermittel** (ID Austria, Keystore – wählbar, sobald eingerichtet –, Chipkarte am Desktop,
 *    „Jedes Mal fragen“) → `update_first_run`, sofort gespeichert.
 * 3. **Signaturbild** (Profil, Blocksprache) mit der Blockvorschau des Kerns (`first_run.preview_revision`)
 *    → `update_first_run`. Die Bildmarke bleibt das PIDA-Siegel, sofern PDF-Over keine eigene gesetzt hatte.
 *
 * „Los geht’s“ → `finish_first_run` (Hinweis „Einrichtung abgeschlossen“), „Später einrichten“ (auch Esc) →
 * `postpone_first_run` (Hinweis, wo es weitergeht; ADR 0004 Nachtrag 12); ein noch offenes Migrationsangebot wird
 * bei „Später“ abgelehnt (erneut über Einstellungen › Erweitert). Ohne Ersteinrichtung erscheint ein
 * Migrationsangebot (`ViewState::migration`, z. B. nach „Suchen …“ in den Einstellungen) als eigener Dialog.
 */
import { useEffect, useRef, useState } from 'react';
import { useApp } from '../../app/context';
import { isMobile, type BackendChoice, type FirstRunView, type Lang, type MigrationOfferView, type SignatureProfile } from '../../core/types';
import { Button, RadioCards, Segmented, type ChoiceOption } from '../../ui/controls';
import { DialogHeader, ModalShell } from '../../ui/Dialog';
import { IconCard, IconFileKey, IconPhone } from '../../ui/Icons';
import { MigrationOfferPane } from './MigrationOffer';
import './onboarding.css';

/** Props der Ersteinrichtung (in E0 festgelegt). */
export interface OnboardingProps {
  /** Migrationsangebot (`ViewState::migration`) */
  migration: MigrationOfferView | null;
  /** Ersteinrichtung (`ViewState::first_run`) */
  firstRun: FirstRunView | null;
}

export function Onboarding({ migration, firstRun }: OnboardingProps) {
  if (firstRun) return <FirstRunDialog firstRun={firstRun} />;
  if (migration) return <MigrationOfferDialog offer={migration} />;
  return null;
}

/** Migrationsangebot ohne Ersteinrichtung: ausdrückliche Wahl, kein Schließen nebenbei. */
function MigrationOfferDialog({ offer }: { offer: MigrationOfferView }) {
  const { t } = useApp();
  return (
    <ModalShell onClose={() => undefined} isDismissable={false} isKeyboardDismissDisabled size="md" className="onboarding-modal">
      <DialogHeader title={t.t('ng-ui-ob-offer-title')} />
      <div className="ob-body">
        <MigrationOfferPane offer={offer} />
      </div>
    </ModalShell>
  );
}

type Step = 0 | 1 | 2;
type SignerChoice = BackendChoice | 'ask';
type LangChoice = Lang | 'system';

const PROFILE_LABEL: Record<SignatureProfile, string> = {
  SIGNATURBLOCK_SMALL: 'ng-ui-set-profile-small',
  AMTSSIGNATURBLOCK: 'ng-ui-set-profile-amt',
  BASE_LOGO: 'ng-ui-set-profile-logo',
  INVISIBLE: 'ng-ui-set-profile-invisible',
};

const LANG_LABEL: Record<LangChoice, string> = {
  system: 'ng-ui-set-lang-system',
  de: 'ng-ui-set-lang-de',
  en: 'ng-ui-set-lang-en',
};

function FirstRunDialog({ firstRun }: { firstRun: FirstRunView }) {
  const { t, dispatch } = useApp();
  // Schritt 1 nur, wenn beim Öffnen ein Angebot vorlag (danach ist es beantwortet und verschwindet)
  const [hadMigration] = useState(() => firstRun.migration !== null);
  const [step, setStep] = useState<Step>(() => (firstRun.migration ? 0 : 1));
  const [busy, setBusy] = useState(false);
  const titleRef = useRef<HTMLDivElement>(null);
  const firstStep = useRef(true);
  const total = hadMigration ? 3 : 2;
  const n = hadMigration ? step + 1 : step;

  // Angebot anderswo beantwortet → weiter mit Schritt 2
  useEffect(() => {
    if (step === 0 && !firstRun.migration) setStep(1);
  }, [step, firstRun.migration]);

  // Fokus auf die Überschrift des neuen Schritts (Screenreader hören den Wechsel)
  useEffect(() => {
    if (firstStep.current) {
      firstStep.current = false;
      return;
    }
    titleRef.current?.querySelector<HTMLElement>('.step-title')?.focus();
  }, [step]);

  // „Fertig“ = `finish_first_run` („Einrichtung abgeschlossen“), „Später einrichten“ = `postpone_first_run` (der
  // Kern sagt, wo es weitergeht; Nachtrag 12)
  const finish = async (later: boolean) => {
    if (busy) return;
    setBusy(true);
    if (later && firstRun.migration) await dispatch({ type: 'decline_migration' });
    const ok = await dispatch({ type: later ? 'postpone_first_run' : 'finish_first_run' });
    if (!ok) setBusy(false);
  };
  const later = () => void finish(true);

  return (
    <ModalShell onClose={later} isDismissable={false} size="md" className="onboarding-modal">
      <DialogHeader title={t.t('ng-ui-ob-title')} subtitle={t.t('ng-ui-ob-progress', { n, total })} />
      <div className="ob-progress" aria-hidden="true">
        {Array.from({ length: total }, (_, i) => (
          <span key={i} className={`ob-dot${i < n ? ' is-on' : ''}`} />
        ))}
      </div>
      <div className="ob-body" ref={titleRef}>
        {step === 0 && firstRun.migration ? <MigrationOfferPane offer={firstRun.migration} showSeal titleId="ob-step-title" onDone={() => setStep(1)} /> : null}
        {step === 1 ? <SignerStep firstRun={firstRun} onNext={() => setStep(2)} /> : null}
        {step === 2 ? <BlockStep firstRun={firstRun} onBack={() => setStep(1)} onFinish={() => void finish(false)} busy={busy} /> : null}
      </div>
      <footer className="ob-footer">
        <Button variant="quiet" size="sm" onPress={later} isDisabled={busy}>
          {t.t('ng-ui-ob-later')}
        </Button>
      </footer>
    </ModalShell>
  );
}

/** Sofort übernehmen (`update_first_run`); bis der Kern antwortet, gilt die eigene Wahl. */
function useFirstRunUpdate(firstRun: FirstRunView) {
  const { dispatch } = useApp();
  const [pending, setPending] = useState<Partial<Pick<FirstRunView, 'default_backend' | 'profile' | 'block_language'>> | null>(null);
  useEffect(() => setPending(null), [firstRun]);
  const current = { default_backend: firstRun.default_backend, profile: firstRun.profile, block_language: firstRun.block_language, ...pending };
  const update = (patch: NonNullable<typeof pending>) => {
    const next = { ...current, ...patch };
    setPending(next);
    void dispatch({ type: 'update_first_run', default_backend: next.default_backend, profile: next.profile, block_language: next.block_language });
  };
  return { current, update };
}

function SignerStep({ firstRun, onNext }: { firstRun: FirstRunView; onNext: () => void }) {
  const { t, view } = useApp();
  const { current, update } = useFirstRunUpdate(firstRun);
  const mobile = isMobile(view);
  const option = (id: 'atrust_native' | 'keystore' | 'local_bku') => view.signers.options.find((o) => o.backend === id) ?? null;
  const ida = option('atrust_native');
  const keystore = option('keystore');
  const value: SignerChoice = current.default_backend ?? 'ask';
  const keystoreReady = !!keystore?.available || value === 'keystore';
  const options: ChoiceOption<SignerChoice>[] = [
    {
      value: 'id_austria',
      label: t.t('ng-ui-set-signer-ida'),
      description: ida?.account ?? t.t(mobile ? 'ng-ui-set-signer-ida-desc-mobile' : 'ng-ui-set-signer-ida-desc'),
      icon: <IconPhone size={18} />,
    },
    {
      value: 'keystore',
      label: t.t('ng-ui-set-signer-keystore'),
      description: keystoreReady ? (keystore?.account ?? t.t('ng-ui-set-signer-keystore-desc')) : t.t('ng-ui-ob-signer-keystore-off'),
      icon: <IconFileKey size={18} />,
      isDisabled: !keystoreReady,
    },
  ];
  // Mobile: keine lokale Chipkarten-Software
  if (!mobile) options.push({ value: 'local_bku', label: t.t('ng-ui-set-signer-card'), description: t.t('ng-ui-set-signer-card-desc'), icon: <IconCard size={18} /> });
  options.push({ value: 'ask', label: t.t('ng-ui-set-signer-ask'), description: t.t('ng-ui-set-signer-ask-desc') });
  return (
    <div className="step-pane">
      <h3 className="step-title" id="ob-step-title" tabIndex={-1}>
        {t.t('ng-ui-ob-signer-title')}
      </h3>
      <p className="step-text">{t.t('ng-ui-ob-signer-text')}</p>
      <RadioCards<SignerChoice> label={t.t('ng-ui-ob-signer-title')} hideLabel value={value} onChange={(v) => update({ default_backend: v === 'ask' ? null : v })} options={options} />
      <div className="step-actions">
        <Button variant="primary" onPress={onNext}>
          {t.t('ng-ui-ob-next')}
        </Button>
      </div>
    </div>
  );
}

function BlockStep({ firstRun, onBack, onFinish, busy }: { firstRun: FirstRunView; onBack: () => void; onFinish: () => void; busy: boolean }) {
  const { t, bridge } = useApp();
  const { current, update } = useFirstRunUpdate(firstRun);
  const profile = current.profile;
  const lang: LangChoice = current.block_language ?? 'system';
  const profiles: SignatureProfile[] = ['SIGNATURBLOCK_SMALL', 'AMTSSIGNATURBLOCK'];
  // übernommenes Sonderprofil (Nur Bildmarke, Unsichtbar) bleibt wählbar
  if (!profiles.includes(profile)) profiles.push(profile);
  const textBlock = profile === 'SIGNATURBLOCK_SMALL' || profile === 'AMTSSIGNATURBLOCK';
  const alt =
    profile === 'BASE_LOGO'
      ? t.t('ng-ui-set-preview-alt-logo', { profile: t.t(PROFILE_LABEL[profile]) })
      : t.t('ng-ui-set-preview-alt', { profile: t.t(PROFILE_LABEL[profile]), lang: t.t(LANG_LABEL[lang]) });
  return (
    <div className="step-pane">
      <h3 className="step-title" id="ob-step-title" tabIndex={-1}>
        {t.t('ng-ui-ob-block-title')}
      </h3>
      <p className="step-text">{t.t('ng-ui-ob-block-text')}</p>
      <div className="ob-preview" data-testid="ob-preview">
        {profile === 'INVISIBLE' ? (
          <p className="muted">{t.t('ng-ui-set-preview-invisible')}</p>
        ) : firstRun.preview_revision > 0 ? (
          <img className="ob-preview-img" src={bridge.blockPreviewUrl(2, firstRun.preview_revision)} alt={alt} draggable={false} />
        ) : null}
      </div>
      <div className="ob-choices">
        <Segmented<SignatureProfile>
          label={t.t('ng-ui-set-profile')}
          value={profile}
          onChange={(v) => update({ profile: v })}
          options={profiles.map((p) => ({ value: p, label: t.t(PROFILE_LABEL[p]) }))}
        />
        {textBlock ? (
          <Segmented<LangChoice>
            label={t.t('ng-ui-set-block-lang')}
            value={lang}
            onChange={(v) => update({ block_language: v === 'system' ? null : v })}
            options={(['system', 'de', 'en'] as const).map((l) => ({ value: l, label: t.t(LANG_LABEL[l]) }))}
          />
        ) : null}
      </div>
      <div className="step-actions">
        <Button variant="ghost" onPress={onBack}>
          {t.t('ng-ui-ob-back')}
        </Button>
        <Button variant="primary" onPress={onFinish} isDisabled={busy}>
          {t.t('ng-ui-ob-finish')}
        </Button>
      </div>
    </div>
  );
}
