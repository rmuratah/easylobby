import type { ReactNode, RefObject } from 'react';
import type { Theme } from '../theme/theme';
import { cx } from '../components/base/cx';
import type { SoundSettings } from '../agent/soundSettings';
import { SoundMenu } from './SoundMenu';
import type { AppUpdateState } from '../agent/useAppUpdate';

export type NavigationPage = 'flow' | 'home' | 'profile' | 'collection' | 'conversations';

type Props = {
  active: NavigationPage;
  flowLabel: string;
  flowButtonRef: RefObject<HTMLButtonElement | null>;
  theme: Theme;
  /** Mensagens não lidas: um ponto de destaque no botão Conversas. */
  unread?: number;
  onNavigate: (page: NavigationPage) => void;
  onThemeChange: (theme: Theme) => void;
  /** Som do app (#501), no botão de Configurações. */
  sound: SoundSettings;
  onSoundChange: (settings: SoundSettings) => void;
  /** Atualização do app (#35), também no botão de Configurações. */
  update?: AppUpdateState;
  canInstallUpdate?: boolean;
  /** Recebe o lugar do botão Jogar, onde a busca de partida põe o próprio botão (#600). */
  queueSlotRef?: (element: HTMLDivElement | null) => void;
  /** A busca de partida está no lugar do Jogar: o Jogar some e o espaço alarga. */
  queueInSlot?: boolean;
};

/**
 * Navegação permanente da janela, na barra do alto (#160): as telas à
 * esquerda e o botão Jogar no centro, em destaque, saindo um pouco para fora
 * da barra. Não controla nem remonta o conteúdo. Com a janela estreita, os
 * destinos ficam só com os ícones.
 */
export function TopNavigation({ active, flowLabel, flowButtonRef, unread = 0, onNavigate, queueSlotRef, queueInSlot = false }: Pick<Props, 'active' | 'flowLabel' | 'flowButtonRef' | 'unread' | 'onNavigate' | 'queueSlotRef' | 'queueInSlot'>) {
  const conversationsLabel = unread > 0 ? `Conversas, ${unread} não ${unread === 1 ? 'lida' : 'lidas'}` : 'Conversas';
  return (
    <nav aria-label="Funcionalidades" className="contents">
      <div data-tauri-drag-region className="app-toolbar-start">
        <h1 data-tauri-drag-region className="app-logo" title="easylobby">
          <span className="sr-only">easylobby</span>
          <span aria-hidden="true" className="app-logo-short">e<span>l</span></span>
          <span aria-hidden="true" className="app-logo-full">easy<span>lobby</span></span>
        </h1>
        <div className="flex items-center gap-1">
          <NavigationButton active={active === 'home'} label="Início" icon={<HomeIcon />}
            onClick={() => onNavigate('home')} />
          <NavigationButton active={active === 'profile'} label="Perfil" icon={<ProfileIcon />}
            onClick={() => onNavigate('profile')} />
          <NavigationButton active={active === 'collection'} label="Coleção" icon={<CollectionIcon />}
            onClick={() => onNavigate('collection')} />
          <NavigationButton active={active === 'conversations'} label="Conversas" ariaLabel={conversationsLabel}
            icon={<ChatIcon />} badge={unread > 0} onClick={() => onNavigate('conversations')} />
          <NavigationButton active={false} label="Loja" ariaLabel="Loja — em breve" icon={<StoreIcon />} disabled />
        </div>
      </div>
      <div className={cx('app-play-slot', queueInSlot && 'is-queue')}>
        {/* No lobby (#600), o botão da busca de partida entra aqui, no lugar do Jogar. */}
        <div ref={queueSlotRef} className="contents" />
        <button ref={flowButtonRef} type="button" hidden={queueInSlot} aria-current={active === 'flow' ? 'page' : undefined}
          aria-label={flowLabel} title={flowLabel} onClick={() => onNavigate('flow')} className="app-play-button">
          <span className="app-play-button-face">
            <SwordsIcon />
            <span aria-hidden="true">{flowLabel}</span>
          </span>
        </button>
      </div>
    </nav>
  );
}

/** Configurações, com o tema dentro, no lado direito da barra do alto. */
export function ToolbarSettings({ theme, onThemeChange, sound, onSoundChange, update, canInstallUpdate }: Pick<Props, 'theme' | 'onThemeChange' | 'sound' | 'onSoundChange' | 'update' | 'canInstallUpdate'>) {
  return <SoundMenu settings={sound} onChange={onSoundChange} icon={<SettingsIcon />}
    update={update} canInstallUpdate={canInstallUpdate} theme={theme} onThemeChange={onThemeChange} />;
}

type ButtonProps = {
  ref?: RefObject<HTMLButtonElement | null>;
  active: boolean;
  label: string;
  icon: ReactNode;
  /** Nome lido pelo leitor de tela, quando difere do rótulo (por exemplo, com as não lidas). */
  ariaLabel?: string;
  /** Ponto de destaque no canto do ícone. */
  badge?: boolean;
  /** Reserva de uma tela que ainda não existe ("em breve"). */
  disabled?: boolean;
  onClick?: () => void;
};

function NavigationButton({ ref, active, label, ariaLabel = label, icon, badge = false, disabled = false, onClick }: ButtonProps) {
  return (
    <button ref={ref} type="button" disabled={disabled} aria-current={active ? 'page' : undefined} onClick={onClick} aria-label={ariaLabel} title={ariaLabel}
      className={cx('app-navigation-button relative flex h-9 items-center justify-center gap-2 rounded-control border px-2.5 text-body font-semibold transition-colors',
        'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent',
        active ? 'border-accent bg-accent-soft text-accent-ink' : 'border-transparent text-muted hover:bg-raised hover:text-ink',
        'disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:bg-transparent disabled:hover:text-muted')}>
      {icon}<span aria-hidden="true" className="app-navigation-label">{label}</span>
      {badge && <span aria-hidden="true" data-unread className="absolute right-1 top-1 h-2 w-2 rounded-full bg-accent" />}
    </button>
  );
}

function BaseIcon({ children }: { children: ReactNode }) {
  return <svg aria-hidden="true" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor"
    strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">{children}</svg>;
}

const SwordsIcon = () => <BaseIcon><path d="M14.5 17.5 3 6V3h3l11.5 11.5M13 19l6-6M16 16l4 4M19 21l2-2M9.5 17.5 21 6V3h-3L6.5 14.5M11 19l-6-6M8 16l-4 4M5 21l-2-2" /></BaseIcon>;
const ProfileIcon = () => <BaseIcon><circle cx="12" cy="8" r="4" /><path d="M4 21a8 8 0 0 1 16 0" /></BaseIcon>;
const ChatIcon = () => <BaseIcon><path d="M21 12a8 8 0 0 1-11.6 7.1L4 20l1-4.6A8 8 0 1 1 21 12z" /></BaseIcon>;
const CollectionIcon = () => <BaseIcon><rect x="3" y="3" width="7" height="7" rx="1.5" /><rect x="14" y="3" width="7" height="7" rx="1.5" /><rect x="3" y="14" width="7" height="7" rx="1.5" /><rect x="14" y="14" width="7" height="7" rx="1.5" /></BaseIcon>;
const StoreIcon = () => <BaseIcon><path d="M6 8h12l-1 12H7L6 8z" /><path d="M9 10V7a3 3 0 0 1 6 0v3" /></BaseIcon>;
const HomeIcon = () => <BaseIcon><path d="m3 11 9-8 9 8M5 10v10h14V10M9 20v-6h6v6" /></BaseIcon>;
const SettingsIcon = () => <BaseIcon><circle cx="12" cy="12" r="3.5" /><circle cx="12" cy="12" r="7" /><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9 7 7M17 17l2.1 2.1M19.1 4.9 17 7M7 17l-2.1 2.1" /></BaseIcon>;
