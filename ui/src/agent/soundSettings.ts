import { useCallback, useEffect, useState } from 'react';
import { DEFAULT_VOLUME, sound } from './sound';

/** Som ligado ou desligado e o volume geral, de 0 a 1 (#501). */
export type SoundSettings = { enabled: boolean; volume: number };

const SOUND_STORAGE_KEY = 'easylobby.som';
const DEFAULT_SOUND_SETTINGS: SoundSettings = { enabled: true, volume: DEFAULT_VOLUME };

export function readSoundSettings(): SoundSettings {
  try {
    const stored = localStorage.getItem(SOUND_STORAGE_KEY);
    if (!stored) return DEFAULT_SOUND_SETTINGS;
    const parsed: unknown = JSON.parse(stored);
    if (typeof parsed !== 'object' || parsed === null) return DEFAULT_SOUND_SETTINGS;
    const { enabled, volume } = parsed as Partial<SoundSettings>;
    return {
      enabled: typeof enabled === 'boolean' ? enabled : DEFAULT_SOUND_SETTINGS.enabled,
      volume: typeof volume === 'number' && Number.isFinite(volume) ? Math.min(1, Math.max(0, volume)) : DEFAULT_SOUND_SETTINGS.volume,
    };
  } catch {
    return DEFAULT_SOUND_SETTINGS;
  }
}

function saveSoundSettings(settings: SoundSettings) {
  try {
    localStorage.setItem(SOUND_STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // Sem armazenamento, a escolha vale só até fechar o app.
  }
}

/** Volume que de fato toca: zero com o som desligado. */
function effectiveVolume(settings: SoundSettings): number {
  return settings.enabled ? settings.volume : 0;
}

/**
 * Som ligado/desligado e volume, salvos como o tema. Aplica no player de
 * sons de todo o app.
 */
export function useSoundSettings() {
  const [settings, setSettings] = useState<SoundSettings>(readSoundSettings);
  useEffect(() => { sound.setVolume(effectiveVolume(settings)); }, [settings]);
  const update = useCallback((next: SoundSettings) => {
    setSettings(next);
    saveSoundSettings(next);
  }, []);
  return { settings, update };
}
