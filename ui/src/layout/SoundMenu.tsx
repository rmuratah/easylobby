import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { sound } from '../agent/sound';
import type { SoundSettings } from '../agent/soundSettings';
import { OpenWithLeagueSetting } from './OpenWithLeagueSetting';

/** Espera depois de mexer no volume antes de tocar o som de teste. */
const PREVIEW_DELAY_MS = 250;

type Props = {
  settings: SoundSettings;
  onChange: (settings: SoundSettings) => void;
  icon: ReactNode;
};

/**
 * Botão de Configurações da barra lateral (#501): a parte de som, com
 * liga/desliga e volume, e a de abrir o app junto com o LoL (#511). Ao mexer
 * no volume, toca o som de partida encontrada para o jogador ouvir a altura.
 */
export function SoundMenu({ settings, onChange, icon }: Props) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const preview = useRef<ReturnType<typeof setTimeout> | null>(null);
  const panelId = useId();
  const percent = Math.round(settings.volume * 100);

  useEffect(() => {
    if (!open) return;
    function onPointer(event: PointerEvent) {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    }
    function onKey(event: KeyboardEvent) {
      if (event.key === 'Escape') setOpen(false);
    }
    window.addEventListener('pointerdown', onPointer);
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('pointerdown', onPointer);
      window.removeEventListener('keydown', onKey);
    };
  }, [open]);

  useEffect(() => () => { if (preview.current) clearTimeout(preview.current); }, []);

  function playPreview() {
    if (preview.current) clearTimeout(preview.current);
    preview.current = setTimeout(() => sound.play('matchFound'), PREVIEW_DELAY_MS);
  }

  function setVolume(value: number) {
    const volume = Math.min(1, Math.max(0, value / 100));
    // Mexer no volume com o som desligado liga o som de novo.
    onChange({ enabled: volume > 0 ? true : settings.enabled, volume });
    if (volume > 0) playPreview();
  }

  const muted = !settings.enabled || settings.volume === 0;
  return (
    <div ref={rootRef} className="flex flex-col items-center">
      <button type="button" aria-label="Configurações" title="Configurações" aria-expanded={open} aria-controls={panelId}
        onClick={() => setOpen(current => !current)}
        className="relative flex size-11 items-center justify-center rounded-full border border-control text-muted transition-colors hover:bg-raised hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent">
        {icon}
        {muted && <span aria-hidden="true" data-muted className="absolute -right-0.5 -top-0.5 rounded-full border border-line bg-surface p-0.5 text-muted"><MutedIcon /></span>}
      </button>
      {open && (
        <section id={panelId} role="dialog" aria-label="Configurações"
          className="fixed bottom-4 left-[7.25rem] z-50 flex w-64 flex-col gap-3 rounded-card border border-line bg-surface p-4 text-ink shadow-[0_8px_24px_-12px_var(--el-overlay)]">
          <h2 className="text-section font-semibold">Som</h2>
          <label className="flex items-center justify-between gap-3 text-body">
            Sons do app
            <input type="checkbox" role="switch" checked={settings.enabled} className="size-4 accent-[var(--el-accent)]"
              onChange={event => onChange({ ...settings, enabled: event.target.checked })} />
          </label>
          <label className="flex flex-col gap-1 text-body">
            <span className="flex items-center justify-between">Volume <span className="tabular-nums text-muted">{percent}%</span></span>
            <input type="range" min={0} max={100} step={5} value={percent}
              aria-valuetext={`${percent}%`} className="w-full accent-[var(--el-accent)]"
              onChange={event => setVolume(Number(event.target.value))} />
          </label>
          <OpenWithLeagueSetting />
        </section>
      )}
    </div>
  );
}

function MutedIcon() {
  return <svg aria-hidden="true" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
    <path d="M11 5 6 9H2v6h4l5 4V5zM22 9l-6 6M16 9l6 6" />
  </svg>;
}
