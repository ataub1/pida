/**
 * Platzhalter gefunden (docs/analyse/01 §4.2, F-20/F-21): Frage Ja/Nein/Abbrechen; bei mehreren
 * leeren Signaturfeldern Auswahl (erstes vorausgewählt, keine freie Eingabe – B-Korrektur).
 */
import { useState } from 'react';
import { Label, Radio, RadioGroup } from 'react-aria-components';
import { useApp } from '../app/context';
import type { DocInfo, FieldInfo, QrInfo } from '../core/types';
import '../styles/legacy/screens.css';
import { AccessButton, ScreenHeading } from '../ui/controls';

export function PlaceholderScreen({ doc, fields, qr }: { doc: DocInfo; fields: FieldInfo[]; qr: QrInfo | null }) {
  const { t, dispatch } = useApp();
  const [selected, setSelected] = useState('0');
  const answer = (a: 'Yes' | 'No' | 'Cancel') =>
    dispatch({
      type: 'placeholder_answer',
      answer: a,
      field_index: a === 'Yes' && fields.length > 0 ? Number(selected) : null,
    });
  return (
    <div className="screen screen-placeholder">
      <ScreenHeading subtitle={doc.name}>{t.t('ng-ui-placeholder-title')}</ScreenHeading>
      <div className="card">
        <p>{t.t('dataSourceSelection-usePlaceholderText')}</p>
        {qr ? <p className="hint">{t.t('ng-ui-placeholder-qr', { page: qr.page + 1 })}</p> : null}
        {fields.length === 1 ? (
          <p className="hint">{t.t('ng-ui-placeholder-field', { name: fields[0]?.name ?? '' })}</p>
        ) : null}
        {fields.length > 1 ? (
          <RadioGroup className="radio-group" value={selected} onChange={setSelected}>
            <Label className="field-label">{t.t('positioning-placeholder')}</Label>
            {fields.map((f, i) => (
              <Radio key={i} value={String(i)} className="radio">
                <span className="radio-mark" aria-hidden="true" />
                <span>
                  {f.name}
                  {f.page != null ? <span className="hint"> – {t.t('ng-ui-page-n', { page: f.page + 1 })}</span> : null}
                </span>
              </Radio>
            ))}
          </RadioGroup>
        ) : null}
        <div className="actions">
          <AccessButton msg="common-Cancel" variant="quiet" onPress={() => answer('Cancel')} />
          <AccessButton msg="ng-ui-no" onPress={() => answer('No')} />
          <AccessButton msg="ng-ui-yes" variant="primary" onPress={() => answer('Yes')} />
        </div>
      </div>
    </div>
  );
}
