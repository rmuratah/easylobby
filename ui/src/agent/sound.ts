/**
 * Som de partida encontrada, gerado na hora com a Web Audio API (sem arquivo
 * de áudio). Três notas curtas e subindo, para chamar a atenção sem assustar.
 */
const NOTES: ReadonlyArray<readonly [frequency: number, offset: number]> = [[659.25, 0], [880, 0.16], [1318.5, 0.32]];
const NOTE_SECONDS = 0.45;
const VOLUME = 0.25;

export type SoundPlayer = {
  /** Libera o áudio; chame num clique ou tecla, que o navegador exige. */
  unlock(): void;
  playMatchFound(): void;
};

export function createSoundPlayer(create: () => AudioContext | null): SoundPlayer {
  let context: AudioContext | null = null;
  const audio = () => {
    if (!context) {
      try { context = create(); } catch { context = null; }
    }
    if (context?.state === 'suspended') context.resume().catch(() => {});
    return context;
  };
  return {
    unlock() { audio(); },
    playMatchFound() {
      const ctx = audio();
      if (!ctx) return;
      const start = ctx.currentTime + 0.05;
      for (const [frequency, offset] of NOTES) {
        const at = start + offset;
        const oscillator = ctx.createOscillator();
        const gain = ctx.createGain();
        oscillator.type = 'sine';
        oscillator.frequency.value = frequency;
        gain.gain.setValueAtTime(0.0001, at);
        gain.gain.exponentialRampToValueAtTime(VOLUME, at + 0.02);
        gain.gain.exponentialRampToValueAtTime(0.0001, at + NOTE_SECONDS);
        oscillator.connect(gain).connect(ctx.destination);
        oscillator.start(at);
        oscillator.stop(at + NOTE_SECONDS + 0.05);
      }
    },
  };
}

export const sound = createSoundPlayer(() => typeof AudioContext === 'undefined' ? null : new AudioContext());
