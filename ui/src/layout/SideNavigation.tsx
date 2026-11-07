import type { ReactNode, RefObject } from 'react';
import type { Theme } from '../theme/theme';
import { Segmented } from '../components/base';
import { cx } from '../components/base/cx';

export type NavigationPage = 'flow' | 'home' | 'profile';

type Props = {
  active: NavigationPage;
  flowLabel: string;
  flowButtonRef: RefObject<HTMLButtonElement | null>;
  theme: Theme;
  onNavigate: (page: NavigationPage) => void;
  onThemeChange: (theme: Theme) => void;
};

const themeOptions: { value: Theme; label: ReactNode; ariaLabel: string }[] = [
  { value: 'claro', label: <SunIcon />, ariaLabel: 'Tema claro' },
  { value: 'escuro', label: <MoonIcon />, ariaLabel: 'Tema escuro' },
];

/**
 * Navegação permanente da janela (#160); não controla nem remonta o conteúdo.
 * Fica estreita, só com os ícones das telas, e se expande sobre o conteúdo
 * ao passar o mouse ou ao navegar pelo teclado.
 */
export function SideNavigation({ active, flowLabel, flowButtonRef, theme, onNavigate, onThemeChange }: Props) {
  return (
    <div className="app-navigation-slot">
      <aside className="app-navigation surface shadow-soft" aria-label="Navegação do aplicativo">
        <h1 data-tauri-drag-region className="app-logo" title="easylobby">
          <span className="sr-only">easylobby</span>
          <span aria-hidden="true" className="app-logo-short">e<span>l</span></span>
          <span aria-hidden="true" className="app-logo-full">easy<span>lobby</span></span>
        </h1>
        <nav aria-label="Funcionalidades" className="flex flex-col items-stretch gap-2">
          <NavigationButton active={active === 'home'} label="Início" icon={<HomeIcon />}
            onClick={() => onNavigate('home')} />
          <NavigationButton ref={flowButtonRef} active={active === 'flow'} label={flowLabel} primary
            icon={<SwordsIcon />} onClick={() => onNavigate('flow')} />
          <div aria-hidden="true" className="mx-1 my-1 border-t border-line" />
          <NavigationButton active={active === 'profile'} label="Perfil" icon={<ProfileIcon />}
            onClick={() => onNavigate('profile')} />
        </nav>
        <div className="mt-auto flex flex-col items-center gap-3">
          <Segmented label="Tema" value={theme} onChange={onThemeChange} options={themeOptions}
            appearance="embedded" className="theme-switch max-w-full flex-col rounded-full p-1" />
          <button type="button" disabled aria-label="Configurações — em breve" title="Configurações — em breve"
            className="flex size-11 items-center justify-center rounded-full border border-control text-muted disabled:cursor-not-allowed disabled:opacity-60">
            <SettingsIcon />
          </button>
        </div>
      </aside>
    </div>
  );
}

type ButtonProps = {
  ref?: RefObject<HTMLButtonElement | null>;
  active: boolean;
  label: string;
  icon: ReactNode;
  primary?: boolean;
  onClick: () => void;
};

function NavigationButton({ ref, active, label, icon, primary = false, onClick }: ButtonProps) {
  return (
    <button ref={ref} type="button" aria-current={active ? 'page' : undefined} onClick={onClick} aria-label={label} title={label}
      className={cx('app-navigation-button flex flex-col items-center justify-center gap-1 rounded-tile border px-1 py-2 text-caption font-semibold transition-colors',
        'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent',
        active ? 'border-accent bg-accent-soft text-accent-ink' : 'border-transparent text-muted hover:bg-raised hover:text-ink',
        primary && !active && 'border-line bg-raised text-ink')}>
      {icon}<span aria-hidden="true" className="app-navigation-label">{label}</span>
    </button>
  );
}

function BaseIcon({ children }: { children: ReactNode }) {
  return <svg aria-hidden="true" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor"
    strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">{children}</svg>;
}

const SwordsIcon = () => <BaseIcon><path d="M14.5 17.5 3 6V3h3l11.5 11.5M13 19l6-6M16 16l4 4M19 21l2-2M9.5 17.5 21 6V3h-3L6.5 14.5M11 19l-6-6M8 16l-4 4M5 21l-2-2" /></BaseIcon>;
const ProfileIcon = () => <BaseIcon><circle cx="12" cy="8" r="4" /><path d="M4 21a8 8 0 0 1 16 0" /></BaseIcon>;
const HomeIcon = () => <BaseIcon><path d="m3 11 9-8 9 8M5 10v10h14V10M9 20v-6h6v6" /></BaseIcon>;
const SettingsIcon = () => <BaseIcon><circle cx="12" cy="12" r="3.5" /><circle cx="12" cy="12" r="7" /><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9 7 7M17 17l2.1 2.1M19.1 4.9 17 7M7 17l-2.1 2.1" /></BaseIcon>;

function SunIcon() {
  return <svg aria-hidden="true" width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" /></svg>;
}

function MoonIcon() {
  return <svg aria-hidden="true" width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M20 15.5A8.5 8.5 0 0 1 8.5 4 8.5 8.5 0 1 0 20 15.5Z" /></svg>;
}
