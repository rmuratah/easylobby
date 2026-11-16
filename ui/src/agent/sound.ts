/**
 * Sons do app, gerados na hora com a Web Audio API (sem arquivo de áudio).
 * Cada som é uma sequência curta de notas; todos passam pelo volume geral
 * escolhido pelo jogador (#501).
 */

/** Uma nota: frequência (Hz), início e duração, em segundos. */
type Note = readonly [frequency: number, offset: number, seconds: number];
type Cue = { type: OscillatorType; volume: number; notes: ReadonlyArray<Note>; delay?: number };

/**
 * Os momentos que tocam som. Os de `useLockInSound`, da contagem e da partida
 * encontrada vieram antes (#135, #475, #498); os outros são da #501.
 */
export type SoundCue =
  | 'matchFound'
  | 'tick'
  | 'tickUrgent'
  | 'banLockIn'
  | 'pickLockIn'
  /** Começou a sua vez de banir ou escolher. */
  | 'turnStart'
  /** Todos aceitaram: entrou na seleção de campeões. */
  | 'champSelectStart'
  /** Convite de lobby recebido. */
  | 'invite'
  /** Alguém recusou o aceite ou saiu da seleção. */
  | 'cancelled'
  /** Mensagem recebida (conversa privada ou chat da seleção). */
  | 'message'
  /** Pedido de troca recebido na seleção. */
  | 'swapRequest'
  /** Um aliado confirmou o campeão. */
  | 'allyLockIn'
  /** A seleção acabou e a partida está carregando. */
  | 'gameStart'
  | 'memberJoin'
  | 'memberLeave'
  /** Clique e hover nos botões principais. */
  | 'click'
  | 'hover'
  /** Clique de aceitar ou recusar a partida encontrada. */
  | 'accept'
  | 'decline';

const CUES: Record<SoundCue, Cue> = {
  // Três notas subindo, para chamar a atenção sem assustar.
  matchFound: { type: 'sine', volume: 0.25, delay: 0.05, notes: [[659.25, 0, 0.45], [880, 0.16, 0.45], [1318.5, 0.32, 0.45]] },
  tick: { type: 'triangle', volume: 0.18, notes: [[784, 0, 0.09]] },
  tickUrgent: { type: 'triangle', volume: 0.18, notes: [[1046.5, 0, 0.09]] },
  // Ban: duas notas descendo, mais graves. Escolha: três notas subindo.
  banLockIn: { type: 'square', volume: 0.096, delay: 0.02, notes: [[392, 0, 0.12], [261.63, 0.1, 0.3]] },
  pickLockIn: { type: 'triangle', volume: 0.16, delay: 0.02, notes: [[523.25, 0, 0.14], [659.25, 0.08, 0.14], [1046.5, 0.16, 0.4]] },
  turnStart: { type: 'sine', volume: 0.22, notes: [[783.99, 0, 0.18], [1174.66, 0.12, 0.35]] },
  champSelectStart: { type: 'triangle', volume: 0.18, notes: [[392, 0, 0.3], [523.25, 0.12, 0.3], [659.25, 0.24, 0.3], [783.99, 0.36, 0.55]] },
  invite: { type: 'sine', volume: 0.2, notes: [[880, 0, 0.22], [1318.5, 0.14, 0.4]] },
  cancelled: { type: 'triangle', volume: 0.2, notes: [[523.25, 0, 0.2], [392, 0.15, 0.2], [311.13, 0.3, 0.45]] },
  message: { type: 'sine', volume: 0.16, notes: [[1046.5, 0, 0.1], [1568, 0.06, 0.16]] },
  swapRequest: { type: 'sine', volume: 0.18, notes: [[987.77, 0, 0.12], [987.77, 0.14, 0.12], [1318.5, 0.28, 0.25]] },
  allyLockIn: { type: 'triangle', volume: 0.1, notes: [[659.25, 0, 0.12], [987.77, 0.08, 0.25]] },
  gameStart: { type: 'triangle', volume: 0.18, notes: [[523.25, 0, 0.25], [783.99, 0.15, 0.25], [1046.5, 0.3, 0.25], [1318.5, 0.45, 0.7]] },
  memberJoin: { type: 'sine', volume: 0.14, notes: [[587.33, 0, 0.12], [880, 0.09, 0.2]] },
  memberLeave: { type: 'sine', volume: 0.12, notes: [[880, 0, 0.12], [587.33, 0.09, 0.2]] },
  click: { type: 'triangle', volume: 0.1, notes: [[1200, 0, 0.05]] },
  hover: { type: 'sine', volume: 0.035, notes: [[1760, 0, 0.035]] },
  accept: { type: 'triangle', volume: 0.18, notes: [[659.25, 0, 0.1], [987.77, 0.07, 0.2]] },
  decline: { type: 'triangle', volume: 0.16, notes: [[493.88, 0, 0.1], [329.63, 0.07, 0.22]] },
};

/**
 * Volume geral de partida: som ligado em 70%. Nesse ponto os sons tocam na
 * altura de antes da #501; acima dele ficam mais altos.
 */
export const DEFAULT_VOLUME = 0.7;

export type SoundPlayer = {
  /** Libera o áudio; chame num clique ou tecla, que o navegador exige. */
  unlock(): void;
  /** Volume geral, de 0 (mudo) a 1 (#501). */
  setVolume(volume: number): void;
  play(cue: SoundCue): void;
  playMatchFound(): void;
  /** Tique da contagem regressiva da sua vez na seleção (#475); `urgent` nos últimos segundos. */
  playTick(urgent?: boolean): void;
  /** Confirmação do ban (duas notas descendo, mais graves) ou da escolha (três notas subindo) (#498). */
  playLockIn(kind: 'ban' | 'pick'): void;
};

export function createSoundPlayer(create: () => AudioContext | null): SoundPlayer {
  let context: AudioContext | null = null;
  let master = DEFAULT_VOLUME;
  const audio = () => {
    if (!context) {
      try { context = create(); } catch { context = null; }
    }
    if (context?.state === 'suspended') context.resume().catch(() => {});
    return context;
  };
  const play = (cue: SoundCue) => {
    if (master <= 0) return;
    const ctx = audio();
    if (!ctx) return;
    const { type, volume, notes, delay = 0.01 } = CUES[cue];
    const peak = volume * master / DEFAULT_VOLUME;
    const start = ctx.currentTime + delay;
    for (const [frequency, offset, seconds] of notes) {
      const at = start + offset;
      const oscillator = ctx.createOscillator();
      const gain = ctx.createGain();
      oscillator.type = type;
      oscillator.frequency.value = frequency;
      gain.gain.setValueAtTime(0.0001, at);
      gain.gain.exponentialRampToValueAtTime(peak, at + Math.min(0.02, seconds / 4));
      gain.gain.exponentialRampToValueAtTime(0.0001, at + seconds);
      oscillator.connect(gain).connect(ctx.destination);
      oscillator.start(at);
      oscillator.stop(at + seconds + 0.05);
    }
  };
  return {
    unlock() { audio(); },
    setVolume(volume) { master = Math.min(1, Math.max(0, Number.isFinite(volume) ? volume : 0)); },
    play,
    playMatchFound() { play('matchFound'); },
    playTick(urgent = false) { play(urgent ? 'tickUrgent' : 'tick'); },
    playLockIn(kind) { play(kind === 'ban' ? 'banLockIn' : 'pickLockIn'); },
  };
}

export const sound = createSoundPlayer(() => typeof AudioContext === 'undefined' ? null : new AudioContext());
