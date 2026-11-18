import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { sound } from '../agent/sound';
import type { SoundSettings } from '../agent/soundSettings';
import { OpenWithLeagueSetting } from './OpenWithLeagueSetting';
import { ErrorReportsSetting } from './ErrorReportsSetting';
import { LogsSetting } from './LogsSetting';
import { AboutSection } from './AboutSection';
import { UpdateSetting } from './UpdateSetting';
import type { AppUpdateState } from '../agent/useAppUpdate';
import type { Theme } from '../theme/theme';
import { Segmented } from '../components/base';

/** Espera depois de mexer no volume antes de tocar o som de teste. */
const PREVIEW_DELAY_MS = 250;

type Props = {
  settings: SoundSettings;
  onChange: (settings: SoundSettings) => void;
  icon: ReactNode;
  /** Atualização do app (#35); sem ela, a seção Atualizações some. */
  update?: AppUpdateState;
  /** Se dá para o app reiniciar agora (fora da fila, da seleção e da partida). */
  canInstallUpdate?: boolean;
  /** Tema do app, no alto do menu; sem ele, a seção Aparência some. */
  theme?: Theme;
  onThemeChange?: (theme: Theme) => void;
};

/**
 * Botão de Configurações da barra do alto (#501): o tema claro ou escuro,
 * que saiu da barra para sobrar espaço em volta do Jogar; a parte de som, com
 * liga/desliga e volume, e a de abrir o app junto com o LoL (#511). Ao mexer
 * no volume, toca o som de partida encontrada para o jogador ouvir a altura.
 * Depois, a seção Atualizações (#35); um ponto no botão avisa de versão nova.
 * No fim, a seção Sobre com o aviso legal da Riot (#217).
 */
export function SoundMenu({ settings, onChange, icon, update, canInstallUpdate = true, theme, onThemeChange }: Props) {
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
  const updateVersion = update?.available?.version;
  const label = updateVersion ? `Configurações, versão ${updateVersion} disponível` : 'Configurações';
  return (
    <div ref={rootRef} className="flex flex-col items-center">
      <button type="button" aria-label={label} title={label} aria-expanded={open} aria-controls={panelId}
        onClick={() => setOpen(current => !current)}
        className="relative flex size-9 items-center justify-center rounded-full border border-control text-muted transition-colors hover:bg-raised hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent">
        {icon}
        {muted && <span aria-hidden="true" data-muted className="absolute -right-0.5 -top-0.5 rounded-full border border-line bg-surface p-0.5 text-muted"><MutedIcon /></span>}
        {updateVersion && <span aria-hidden="true" data-update className="absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full border border-surface bg-accent" />}
      </button>
      {open && (
        <section id={panelId} role="dialog" aria-label="Configurações"
          className="fixed right-3 top-[4.75rem] z-50 flex max-h-[calc(100vh-5.5rem)] w-64 flex-col gap-3 overflow-y-auto rounded-card border border-line bg-surface p-4 text-ink shadow-[0_8px_24px_-12px_var(--el-overlay)]">
          {theme && onThemeChange && <>
            <h2 className="text-section font-semibold">Aparência</h2>
            <Segmented label="Tema" value={theme} onChange={onThemeChange} className="w-full [&>button]:flex-1" options={[
              { value: 'claro', label: <><SunIcon />Claro</>, ariaLabel: 'Tema claro' },
              { value: 'escuro', label: <><MoonIcon />Escuro</>, ariaLabel: 'Tema escuro' },
            ]} />
          </>}
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
          <LogsSetting />
          <ErrorReportsSetting />
          {update && <UpdateSetting update={update} canInstall={canInstallUpdate} />}
          <AboutSection />
        </section>
      )}
    </div>
  );
}

function SunIcon() {
  return <svg aria-hidden="true" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" /></svg>;
}

function MoonIcon() {
  return <svg aria-hidden="true" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M20 15.5A8.5 8.5 0 0 1 8.5 4 8.5 8.5 0 1 0 20 15.5Z" /></svg>;
}

function MutedIcon() {
  return <svg aria-hidden="true" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
    <path d="M11 5 6 9H2v6h4l5 4V5zM22 9l-6 6M16 9l6 6" />
  </svg>;
}
