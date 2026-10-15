/**
 * Dezente, einklappbare Demo-Steuerung (unten links): Szenario wählen und die Antwort des simulierten
 * Signiermittels festlegen, damit alle Wege durchgeklickt werden können. Nur im Prototyp.
 */
import { useEffect, useRef, useState } from 'react';
import { Button as AriaButton, Label, ListBox, ListBoxItem, Popover, Radio, RadioGroup, Select, SelectValue } from 'react-aria-components';
import { useAppState, useDispatch } from '../../app/store';
import { useT, type MsgKey } from '../../i18n';
import type { DemoResponse, ScenarioId, SignerKind } from '../../model/types';
import { Button, Segmented } from '../../ui/controls';
import { IconCheck, IconChevronDown, IconClose, IconRefresh, IconSliders } from '../../ui/Icons';

const SCENARIOS: ScenarioId[] = ['start', 'blank', 'contract', 'field', 'signed', 'batch', 'verify', 'error-password', 'error-atrust'];
const RESPONSES: DemoResponse[] = ['app', 'fido2', 'sms', 'cancel', 'timeout'];
const STORAGE_KEY = 'pida.demoPanel';

function readOpen(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) === 'open';
  } catch {
    return false;
  }
}

export function DemoPanel() {
  const state = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  const [open, setOpen] = useState(readOpen);
  // Fokus beim Auf-/Zuklappen mitnehmen (sonst landet er auf <body>, WCAG 2.4.3)
  const toggled = useRef(false);
  const pillRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const toggle = (next: boolean) => {
    toggled.current = true;
    setOpen(next);
  };
  useEffect(() => {
    try {
      localStorage.setItem(STORAGE_KEY, open ? 'open' : 'closed');
    } catch {
      /* kein Speicher */
    }
    if (!toggled.current) return;
    toggled.current = false;
    if (open) panelRef.current?.querySelector<HTMLElement>('.select-button')?.focus();
    else pillRef.current?.focus();
  }, [open]);

  if (!open) {
    return (
      <aside className="demo-dock" aria-label={t('demo.label')}>
        <AriaButton ref={pillRef} className="demo-pill" onPress={() => toggle(true)} aria-expanded={false}>
          <IconSliders size={16} />
          {t('demo.toggle')}
        </AriaButton>
      </aside>
    );
  }

  return (
    <aside className="demo-dock" aria-label={t('demo.label')}>
      <div className="demo-panel" ref={panelRef}>
        <div className="demo-head">
          <p className="demo-title">
            <IconSliders size={16} />
            {t('demo.label')}
          </p>
          <AriaButton className="icon-btn icon-btn-ghost icon-btn-sm" aria-label={t('demo.collapse')} aria-expanded onPress={() => toggle(false)}>
            <IconClose size={16} />
          </AriaButton>
        </div>

        <Select
          className="demo-select"
          selectedKey={state.demo.scenario}
          onSelectionChange={(k) => {
            if (k) dispatch({ type: 'scenario', id: k as ScenarioId });
          }}
        >
          <Label className="field-label">{t('demo.scenario')}</Label>
          <AriaButton className="select-button">
            <SelectValue className="select-value" />
            <IconChevronDown size={16} />
          </AriaButton>
          <Popover className="popover" placement="top start" offset={6}>
            <ListBox className="menu">
              {SCENARIOS.map((s) => (
                <ListBoxItem key={s} id={s} className="menu-item" textValue={t(`demo.sc.${s}` as MsgKey)}>
                  {({ isSelected }) => (
                    <>
                      <span className="menu-item-label">{t(`demo.sc.${s}` as MsgKey)}</span>
                      {isSelected ? <IconCheck size={16} className="menu-item-check" /> : null}
                    </>
                  )}
                </ListBoxItem>
              ))}
            </ListBox>
          </Popover>
        </Select>

        <RadioGroup className="demo-radios" value={state.demo.response} onChange={(v) => dispatch({ type: 'demo', patch: { response: v as DemoResponse } })}>
          <Label className="field-label">{t('demo.response')}</Label>
          {RESPONSES.map((r) => (
            <Radio key={r} value={r} className="demo-radio">
              <span className="demo-radio-dot" aria-hidden="true" />
              {t(`demo.resp.${r}` as MsgKey)}
            </Radio>
          ))}
        </RadioGroup>

        <Segmented<SignerKind>
          label={t('demo.signer')}
          size="sm"
          value={state.signer}
          onChange={(v) => dispatch({ type: 'setSigner', signer: v })}
          options={[
            { value: 'idaustria', label: t('signer.idaustria') },
            { value: 'keystore', label: t('signer.keystore') },
            { value: 'card', label: t('signer.card') },
          ]}
        />

        <div className="demo-buttons">
          <Button variant="secondary" size="sm" onPress={() => dispatch({ type: 'scenario', id: state.demo.scenario })}>
            <IconRefresh size={14} />
            {t('demo.reload')}
          </Button>
          <Button variant="secondary" size="sm" onPress={() => dispatch({ type: 'overlay', overlay: 'onboarding' })}>
            {t('demo.onboarding')}
          </Button>
          <Button
            variant="secondary"
            size="sm"
            onPress={() => {
              dispatch({ type: 'resetPrivacy' });
              dispatch({ type: 'toast', toast: { kind: 'info', title: t('set.adv.privacy.done') } });
            }}
          >
            {t('demo.privacy')}
          </Button>
        </div>
        <p className="demo-note">{t('demo.note')}</p>
      </div>
    </aside>
  );
}
