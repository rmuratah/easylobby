import type { RuneCatalog } from '../bindings/RuneCatalog';
import type { RunePage } from '../bindings/RunePage';
import type { RuneSlot } from '../bindings/RuneSlot';
import type { RuneStyle } from '../bindings/RuneStyle';

/** Fragmentos como os do client: 5008 e 5001 aparecem em duas linhas. */
const statSlots: RuneSlot[] = [
  { kind: 'statMod', label: 'Ofensiva', perks: [5008, 5005, 5007] },
  { kind: 'statMod', label: 'Flex', perks: [5008, 5010, 5001] },
  { kind: 'statMod', label: 'Defesa', perks: [5011, 5013, 5001] },
];

/** Árvore enxuta como a dos testes do agente: runas `base + 1`, `base + 11`... */
function style(id: number, name: string, base: number, allowedSubStyles: number[]): RuneStyle {
  return {
    id, name, allowedSubStyles,
    slots: [
      { kind: 'keystone', label: '', perks: [base + 1, base + 2] },
      { kind: 'regular', label: '', perks: [base + 11, base + 12] },
      { kind: 'regular', label: '', perks: [base + 21, base + 22] },
      { kind: 'regular', label: '', perks: [base + 31, base + 32] },
      ...statSlots,
    ],
  };
}

const names: Record<number, string> = {
  101: 'Pressione o Ataque', 102: 'Ritmo Fatal', 111: 'Triunfo', 112: 'Presença de Espírito',
  121: 'Lenda: Espontaneidade', 122: 'Lenda: Linhagem', 131: 'Golpe de Misericórdia', 132: 'Dilacerar',
  201: 'Eletrocutar', 202: 'Colheita Sombria', 211: 'Golpe Desleal', 212: 'Gosto de Sangue',
  221: 'Globos Oculares', 222: 'Sentinela Zumbi', 231: 'Caçador Voraz', 232: 'Caçador Engenhoso',
  301: 'Aery', 302: 'Cometa Arcano', 311: 'Faixa de Fluxo de Mana', 312: 'Manto Nimbus',
  321: 'Transcendência', 322: 'Celeridade', 331: 'Chamuscar', 332: 'Caminhar sobre as Águas',
  5005: 'Velocidade de Ataque', 5007: 'Aceleração de Habilidade', 5008: 'Força Adaptativa',
  5001: 'Vida Escalável', 5010: 'Velocidade de Movimento', 5011: 'Vida', 5013: 'Tenacidade',
};

export const testFullRuneCatalog: RuneCatalog = {
  styles: [
    style(8000, 'Precisão', 100, [8100, 8200]),
    style(8100, 'Dominação', 200, [8000, 8200]),
    style(8200, 'Feitiçaria', 300, [8000]),
  ],
  perks: Object.entries(names).map(([id, name]) => ({ id: Number(id), name, shortDesc: `Descrição de ${name}`, longDesc: `Descrição completa de ${name}.\n\nTempo de Recarga: 10s` })),
};

/** Página válida: Precisão com Dominação. */
export const testRunePage: RunePage = {
  id: 10, name: 'Mid', primaryStyleId: 8000, subStyleId: 8100,
  selectedPerkIds: [101, 111, 121, 131, 211, 231, 5008, 5010, 5011],
  isEditable: true, isDeletable: true, isTemporary: false, isValid: true,
};
