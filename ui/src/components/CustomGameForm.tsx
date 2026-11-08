import { useId } from 'react';
import {
  CUSTOM_NAME_MAX, TEAM_SIZES, customMapDescriptions, customMapNames, pickTypeNames, spectatorNames,
} from '../agent/customGame';
import type { CustomGameOptions } from '../bindings/CustomGameOptions';
import type { CustomMap } from '../bindings/CustomMap';
import type { CustomPickType } from '../bindings/CustomPickType';
import type { SpectatorPolicy } from '../bindings/SpectatorPolicy';
import { cx } from './base/cx';

type Props = {
  options: CustomGameOptions;
  onChange: (options: CustomGameOptions) => void;
  locked: boolean;
  /** Por que o formulário não pode ser enviado, mostrado embaixo do nome. */
  problem: string | null;
};

const focusRing = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';
const field = `h-10 w-full min-w-0 rounded-control border border-control bg-raised px-3 text-body text-ink disabled:cursor-not-allowed disabled:text-muted ${focusRing}`;
const labelClass = 'text-caption font-semibold uppercase tracking-wider text-muted';

/**
 * Formulário da aba Personalizada (#262): nome, senha, mapa, tipo de seleção,
 * tamanho da equipe e espectadores, as mesmas opções do client oficial. O
 * Modo de Treino fica de fora (só na aba Treinamento).
 */
export function CustomGameForm({ options, onChange, locked, problem }: Props) {
  const baseId = useId();
  const set = <K extends keyof CustomGameOptions>(key: K, value: CustomGameOptions[K]) => onChange({ ...options, [key]: value });
  return (
    <div className="grid content-start gap-4 @min-[40rem]:grid-cols-2">
      <fieldset aria-label="Mapa" disabled={locked} className="grid gap-3 @min-[40rem]:col-span-2 @min-[40rem]:grid-cols-2">
        {(Object.keys(customMapNames) as CustomMap[]).map(map => {
          const selected = options.map === map;
          return (
            <label key={map} className={cx('flex cursor-pointer flex-col gap-1 rounded-panel p-5 transition-all',
              selected ? 'bg-accent-soft ring-2 ring-accent' : 'surface shadow-soft hover:-translate-y-0.5 hover:shadow-lift',
              locked && 'cursor-not-allowed')}>
              <input type="radio" name={`${baseId}-map`} className="sr-only" checked={selected}
                onChange={() => set('map', map)} aria-describedby={`${baseId}-${map}-desc`} />
              <span className="text-section font-semibold">{customMapNames[map]}</span>
              <span id={`${baseId}-${map}-desc`} className="text-body text-muted">{customMapDescriptions[map]}</span>
            </label>
          );
        })}
      </fieldset>

      <div className="flex flex-col gap-1.5">
        <label htmlFor={`${baseId}-name`} className={labelClass}>Nome da partida</label>
        <input id={`${baseId}-name`} type="text" value={options.name} maxLength={CUSTOM_NAME_MAX} disabled={locked}
          aria-invalid={!!problem} aria-describedby={problem ? `${baseId}-problem` : undefined}
          onChange={event => set('name', event.target.value)} className={field} />
        {problem && <p id={`${baseId}-problem`} className="text-caption text-danger">{problem}</p>}
      </div>
      <div className="flex flex-col gap-1.5">
        <label htmlFor={`${baseId}-password`} className={labelClass}>Senha (opcional)</label>
        <input id={`${baseId}-password`} type="text" value={options.password} disabled={locked} autoComplete="off"
          placeholder="Sem senha" onChange={event => set('password', event.target.value)} className={field} />
      </div>
      <div className="flex flex-col gap-1.5">
        <label htmlFor={`${baseId}-pick`} className={labelClass}>Tipo de seleção</label>
        <select id={`${baseId}-pick`} value={options.pickType} disabled={locked} className={field}
          onChange={event => set('pickType', event.target.value as CustomPickType)}>
          {(Object.keys(pickTypeNames) as CustomPickType[]).map(pick => <option key={pick} value={pick}>{pickTypeNames[pick]}</option>)}
        </select>
      </div>
      <div className="grid grid-cols-2 gap-4">
        <div className="flex min-w-0 flex-col gap-1.5">
          <label htmlFor={`${baseId}-size`} className={labelClass}>Tamanho</label>
          <select id={`${baseId}-size`} value={options.teamSize} disabled={locked} className={field}
            onChange={event => set('teamSize', Number(event.target.value))}>
            {TEAM_SIZES.map(size => <option key={size} value={size}>{size}v{size}</option>)}
          </select>
        </div>
        <div className="flex min-w-0 flex-col gap-1.5">
          <label htmlFor={`${baseId}-spectators`} className={labelClass}>Espectadores</label>
          <select id={`${baseId}-spectators`} value={options.spectators} disabled={locked} className={field}
            onChange={event => set('spectators', event.target.value as SpectatorPolicy)}>
            {(Object.keys(spectatorNames) as SpectatorPolicy[]).map(policy => <option key={policy} value={policy}>{spectatorNames[policy]}</option>)}
          </select>
        </div>
      </div>
    </div>
  );
}
