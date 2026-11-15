/**
 * Som de partida encontrada, gerado na hora com a Web Audio API (sem arquivo
 * de áudio). Três notas curtas e subindo, para chamar a atenção sem assustar.
 */
const NOTES: ReadonlyArray<readonly [frequency: number, offset: number]> = [[659.25, 0], [880, 0.16], [1318.5, 0.32]];
const NOTE_SECONDS = 0.45;
const VOLUME = 0.25;
/** Tique curto da contagem da seleção. */
const TICK_SECONDS = 0.09;
const TICK_VOLUME = 0.18;

export type SoundPlayer = {
  /** Libera o áudio; chame num clique ou tecla, que o navegador exige. */
  unlock(): void;
  playMatchFound(): void;
  /** Tique da contagem regressiva da sua vez na seleção (#475); `urgent` nos últimos segundos. */
  playTick(urgent?: boolean): void;
  /** Confirmação do ban (duas notas descendo, mais graves) ou da escolha (três notas subindo) (#498). */
  playLockIn(kind: 'ban' | 'pick'): void;
};

/** Notas da confirmação: frequência, início e duração, em segundos. */
const LOCK_IN: Record<'ban' | 'pick', { type: OscillatorType; notes: ReadonlyArray<readonly [number, number, number]> }> = {
  ban: { type: 'square', notes: [[392, 0, 0.12], [261.63, 0.1, 0.3]] },
  pick: { type: 'triangle', notes: [[523.25, 0, 0.14], [659.25, 0.08, 0.14], [1046.5, 0.16, 0.4]] },
};
const LOCK_IN_VOLUME = 0.16;

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
    playTick(urgent = false) {
      const ctx = audio();
      if (!ctx) return;
      const at = ctx.currentTime + 0.01;
      const oscillator = ctx.createOscillator();
      const gain = ctx.createGain();
      oscillator.type = 'triangle';
      oscillator.frequency.value = urgent ? 1046.5 : 784;
      gain.gain.setValueAtTime(0.0001, at);
      gain.gain.exponentialRampToValueAtTime(TICK_VOLUME, at + 0.005);
      gain.gain.exponentialRampToValueAtTime(0.0001, at + TICK_SECONDS);
      oscillator.connect(gain).connect(ctx.destination);
      oscillator.start(at);
      oscillator.stop(at + TICK_SECONDS + 0.02);
    },
    playLockIn(kind) {
      const ctx = audio();
      if (!ctx) return;
      const { type, notes } = LOCK_IN[kind];
      const start = ctx.currentTime + 0.02;
      for (const [frequency, offset, seconds] of notes) {
        const at = start + offset;
        const oscillator = ctx.createOscillator();
        const gain = ctx.createGain();
        oscillator.type = type;
        oscillator.frequency.value = frequency;
        gain.gain.setValueAtTime(0.0001, at);
        gain.gain.exponentialRampToValueAtTime(kind === 'ban' ? LOCK_IN_VOLUME * 0.6 : LOCK_IN_VOLUME, at + 0.01);
        gain.gain.exponentialRampToValueAtTime(0.0001, at + seconds);
        oscillator.connect(gain).connect(ctx.destination);
        oscillator.start(at);
        oscillator.stop(at + seconds + 0.02);
      }
    },
  };
}

export const sound = createSoundPlayer(() => typeof AudioContext === 'undefined' ? null : new AudioContext());
