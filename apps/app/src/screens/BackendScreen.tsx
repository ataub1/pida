/**
 * Auswahl des Signiermittels (docs/analyse/01 §4.4, F-50…F-56) mit Datenschutzhinweis vor dem ersten
 * Upload (`document_leaves_device`, Plan §4.2/§7.8).
 */
import type { ReactElement } from 'react';
import { Button } from 'react-aria-components';
import { useApp } from '../app/context';
import { IconCard, IconKey, IconPhone, IconShield } from '../components/icons';
import { AccessButton, ConfirmDialog, Markup, Mnemonic, ScreenHeading } from '../components/ui';
import type { BackendId, BackendOptionView, PrivacyNoticeView } from '../core/types';

interface BackendMeta {
  title: string;
  access: string;
  desc: string;
  unavailable: string;
  icon: (p: { size?: number }) => ReactElement;
}

const META: Partial<Record<BackendId, BackendMeta>> = {
  atrust_native: {
    title: 'BKU-MOBILE',
    access: 'bku_selection-mobile',
    desc: 'ng-ui-backend-atrust-desc',
    unavailable: 'ng-ui-backend-unavailable',
    icon: IconPhone,
  },
  local_bku: {
    title: 'BKU-LOCAL',
    access: 'bku_selection-card',
    desc: 'ng-ui-backend-local-desc',
    unavailable: 'ng-ui-backend-local-unavailable',
    icon: IconCard,
  },
  keystore: {
    title: 'BKU-KS',
    access: 'bku_selection-ks',
    desc: 'ng-ui-backend-keystore-desc',
    unavailable: 'ng-ui-backend-keystore-unavailable',
    icon: IconKey,
  },
};

function BackendCard({ option }: { option: BackendOptionView }) {
  const { t, dispatch, isMac } = useApp();
  const meta = META[option.id];
  if (!meta) return null;
  const caps = option.capabilities;
  const key = t.accessKey(meta.access);
  const title = t.t(meta.title);
  const Icon = meta.icon;
  const descId = `backend-desc-${option.id}`;
  return (
    <li className="backend-item">
      <Button
        className="backend-card"
        isDisabled={!option.available}
        onPress={() => dispatch({ type: 'choose_backend', backend: option.id })}
        aria-describedby={descId}
        data-accesskey={key ?? undefined}
        ref={(el) => {
          if (el && key) el.setAttribute('aria-keyshortcuts', `${isMac ? 'Control+Alt' : 'Alt'}+${key.toUpperCase()}`);
        }}
      >
        <span className="backend-icon" aria-hidden="true">
          <Icon size={34} />
        </span>
        <span className="backend-text">
          <span className="backend-title">
            <Mnemonic text={title} accessKey={key} />
          </span>
          <span id={descId} className="backend-desc">
            {option.available ? t.t(meta.desc) : t.t(meta.unavailable)}
            {caps?.document_leaves_device ? <span className="backend-note"> {t.t('ng-ui-backend-leaves-device')}</span> : null}
          </span>
          {caps ? (
            <span className="badges">
              {caps.qualification === 'Qes' ? <span className="badge">{t.t('ng-ui-backend-qes')}</span> : null}
              {caps.factors.app ? <span className="badge badge-muted">{t.t('ng-ui-factor-app')}</span> : null}
              {caps.factors.sms ? <span className="badge badge-muted">{t.t('ng-ui-factor-sms')}</span> : null}
              {caps.factors.fido2 ? <span className="badge badge-muted">FIDO2</span> : null}
              {caps.factors.card ? <span className="badge badge-muted">{t.t('ng-ui-factor-card')}</span> : null}
            </span>
          ) : null}
        </span>
      </Button>
    </li>
  );
}

function PrivacyDialog({ notice }: { notice: PrivacyNoticeView }) {
  const { t, dispatch } = useApp();
  const text = notice.custom_text ?? t.t('config-DataProtectionStatement');
  return (
    <ConfirmDialog
      title={t.t('config-DataProtection')}
      onCancel={() => dispatch({ type: 'acknowledge_privacy', accepted: false })}
      actions={
        <>
          <AccessButton msg="ng-ui-privacy-decline" variant="quiet" onPress={() => dispatch({ type: 'acknowledge_privacy', accepted: false })} />
          <AccessButton msg="ng-ui-privacy-accept" variant="primary" onPress={() => dispatch({ type: 'acknowledge_privacy', accepted: true })} />
        </>
      }
    >
      <p className="privacy-lead">
        <IconShield size={20} /> {t.t('ng-ui-privacy-lead')}
      </p>
      <p>
        <Markup text={text} targets={[{ href: t.t('config-DataProtectionURL') }]} />
      </p>
    </ConfirmDialog>
  );
}

export function BackendScreen({ options, privacyNotice }: { options: BackendOptionView[]; privacyNotice: PrivacyNoticeView | null }) {
  const { t } = useApp();
  // Reservierte Backends (Szenario B/C/D) haben in v1 keine Oberfläche.
  const shown = options.filter((o) => META[o.id] !== undefined);
  return (
    <div className="screen screen-backend">
      <ScreenHeading>{t.t('ng-ui-backend-title')}</ScreenHeading>
      <ul className="backend-list">
        {shown.map((o) => (
          <BackendCard key={o.id} option={o} />
        ))}
      </ul>
      <p className="hint">{t.t('ng-ui-backend-hint')}</p>
      {privacyNotice ? <PrivacyDialog notice={privacyNotice} /> : null}
    </div>
  );
}
