/** Fehler in Alltagssprache mit Aktion: kennwortgeschütztes PDF → Kennwort eingeben (Demo-Kennwort „demo“). */
import { useState, type FormEvent } from 'react';
import { useDispatch } from '../../app/store';
import { useT } from '../../i18n';
import type { DocState } from '../../model/types';
import { Button, Details, TextInput } from '../../ui/controls';
import { IconLock } from '../../ui/Icons';

export function LockedDocument({ doc, onOther }: { doc: DocState; onOther: () => void }) {
  const t = useT();
  const dispatch = useDispatch();
  const [pw, setPw] = useState('');
  const [error, setError] = useState<string | null>(null);
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!pw.trim()) {
      setError(t('locked.empty'));
    } else if (pw.trim().toLowerCase() === 'demo') {
      dispatch({ type: 'unlock', id: doc.spec.id });
      dispatch({ type: 'toast', toast: { kind: 'success', title: t('locked.unlocked'), text: doc.spec.fileName } });
    } else {
      setError(t('locked.wrong'));
    }
  };
  return (
    <div className="state-card-wrap">
      <section className="state-card" aria-labelledby="locked-title">
        <span className="state-icon state-icon-warning" aria-hidden="true">
          <IconLock size={26} />
        </span>
        <h1 className="state-title" id="locked-title">
          {t('locked.title')}
        </h1>
        <p className="state-text">{t('locked.text')}</p>
        <form className="state-form" onSubmit={submit} noValidate>
          <TextInput
            label={t('locked.password')}
            type="password"
            value={pw}
            onChange={(v) => {
              setPw(v);
              setError(null);
            }}
            error={error}
            autoFocus
            autoComplete="off"
            description={t('locked.hint')}
          />
          <div className="state-actions">
            <Button variant="primary" type="submit">
              {t('locked.open')}
            </Button>
            <Button variant="ghost" onPress={onOther}>
              {t('locked.other')}
            </Button>
          </div>
        </form>
        <Details label={t('common.details')} level={2}>
          <p className="mono-small">{t('locked.details', { file: doc.spec.fileName, size: doc.spec.sizeKb })}</p>
        </Details>
      </section>
    </div>
  );
}
