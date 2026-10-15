/**
 * Ersteinrichtung beim ersten Start (überspringbar): PDF-Over-Einstellungen übernehmen →
 * Standard-Signiermittel → Signaturbild. Alles bleibt später in den Einstellungen änderbar.
 */
import { useState } from 'react';
import { SIGNER_NAME, useAppState, useDispatch } from '../../app/store';
import { useT } from '../../i18n';
import { isoWithOffset } from '../../model/format';
import type { Lang, SignerKind, SigProfile } from '../../model/types';
import { Button, RadioCards, Segmented } from '../../ui/controls';
import { DialogHeader, ModalShell } from '../../ui/Dialog';
import { IconCard, IconCheck, IconFileKey, IconFolder, IconPhone } from '../../ui/Icons';
import { SignatureBlock } from '../sigblock/SignatureBlock';

export function Onboarding() {
  const state = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  const [step, setStep] = useState(0);
  const [imported, setImported] = useState(false);
  const [signer, setSigner] = useState<SignerKind | 'ask'>(state.settings.defaultSigner);
  const [time] = useState(() => isoWithOffset(new Date()));
  const isOpen = state.overlay === 'onboarding';
  const total = 3;
  const finish = (useImport: boolean) => dispatch({ type: 'onboardingDone', imported: useImport, signer });
  const sig = state.settings.sigBlock;

  return (
    <ModalShell isOpen={isOpen} onClose={() => finish(imported)} size="md" className="onboarding-modal">
      <DialogHeader title={t('ob.title')} subtitle={t('ob.progress', { n: step + 1, total })} />
      <div className="ob-progress" aria-hidden="true">
        {Array.from({ length: total }, (_, i) => (
          <span key={i} className={`ob-dot${i <= step ? ' is-on' : ''}`} />
        ))}
      </div>

      <div className="ob-body">
        {step === 0 ? (
          <div className="step-pane">
            <span className="step-hero step-hero-info" aria-hidden="true">
              <IconFolder size={26} />
            </span>
            <h3 className="step-title">{t('ob.import.title')}</h3>
            <p className="step-text">{t('ob.import.text')}</p>
            <p className="mono-small ob-path">~/.pdf-over/PDF-Over.config</p>
            <ul className="check-list">
              {(['ob.import.item.block', 'ob.import.item.user', 'ob.import.item.folder'] as const).map((k) => (
                <li key={k}>
                  <IconCheck size={16} />
                  {t(k)}
                </li>
              ))}
            </ul>
            <div className="step-actions">
              <Button
                variant="secondary"
                onPress={() => {
                  setImported(false);
                  setStep(1);
                }}
              >
                {t('ob.import.decline')}
              </Button>
              <Button
                variant="primary"
                onPress={() => {
                  setImported(true);
                  dispatch({ type: 'toast', toast: { kind: 'success', title: t('ob.import.done') } });
                  setStep(1);
                }}
              >
                {t('ob.import.accept')}
              </Button>
            </div>
          </div>
        ) : null}

        {step === 1 ? (
          <div className="step-pane">
            <h3 className="step-title">{t('ob.signer.title')}</h3>
            <p className="step-text">{t('ob.signer.text')}</p>
            <RadioCards<SignerKind | 'ask'>
              label={t('ob.signer.title')}
              hideLabel
              value={signer}
              onChange={setSigner}
              options={[
                { value: 'idaustria', label: t('signer.idaustria'), description: t('signer.idaustria.desc'), icon: <IconPhone size={18} /> },
                { value: 'keystore', label: t('signer.keystore'), description: t('signer.keystore.desc'), icon: <IconFileKey size={18} /> },
                { value: 'card', label: t('signer.card'), description: t('signer.card.desc'), icon: <IconCard size={18} /> },
                { value: 'ask', label: t('signer.ask'), description: t('signer.ask.desc') },
              ]}
            />
            <div className="step-actions">
              <Button variant="ghost" onPress={() => setStep(0)}>
                {t('common.back')}
              </Button>
              <Button variant="primary" onPress={() => setStep(2)}>
                {t('common.next')}
              </Button>
            </div>
          </div>
        ) : null}

        {step === 2 ? (
          <div className="step-pane">
            <h3 className="step-title">{t('ob.block.title')}</h3>
            <p className="step-text">{t('ob.block.text')}</p>
            <div className="ob-preview" role="img" aria-label={t('set.block.previewAlt')}>
              <SignatureBlock sig={sig} signer={SIGNER_NAME} time={time} scale={sig.profile === 'amt' ? 0.95 : 1.55} />
            </div>
            <div className="grid-2">
              <Segmented<SigProfile>
                label={t('set.block.profile')}
                value={sig.profile}
                onChange={(v) => dispatch({ type: 'sigBlock', patch: { profile: v } })}
                options={[
                  { value: 'standard', label: t('set.block.profile.standard') },
                  { value: 'amt', label: t('set.block.profile.amt') },
                ]}
              />
              <Segmented<Lang>
                label={t('set.block.lang')}
                value={sig.lang}
                onChange={(v) => dispatch({ type: 'sigBlock', patch: { lang: v } })}
                options={[
                  { value: 'de', label: t('lang.de') },
                  { value: 'en', label: t('lang.en') },
                ]}
              />
            </div>
            <div className="step-actions">
              <Button variant="ghost" onPress={() => setStep(1)}>
                {t('common.back')}
              </Button>
              <Button variant="primary" onPress={() => finish(imported)}>
                {t('ob.finish')}
              </Button>
            </div>
          </div>
        ) : null}
      </div>
      <footer className="ob-footer">
        <Button variant="quiet" size="sm" onPress={() => finish(imported)}>
          {t('ob.skipAll')}
        </Button>
      </footer>
    </ModalShell>
  );
}
