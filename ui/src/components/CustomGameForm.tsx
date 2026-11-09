import { useId } from 'react';
import {
  CUSTOM_NAME_MAX, FORM_MAPS, TEAM_SIZES, aramMapNames, formMapNames, pickTypeName, pickTypesByMap, spectatorNames, withMap,
} from '../agent/customGame';
import type { AramMap } from '../bindings/AramMap';
import type { CustomGameOptions } from '../bindings/CustomGameOptions';
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
const choice = 'flex cursor-pointer items-center gap-2 text-body has-[:disabled]:cursor-not-allowed';
const radio = 'size-4 shrink-0 accent-[var(--el-accent)]';

/** Sigla no lugar do ícone do modo, até as imagens finais entrarem. */
const mapIcons = { summonersRift: 'SR', howlingAbyss: 'MA', classicRift: 'RC' } as const;

/**
 * Formulário da aba Personalizada, como a tela "Criar personalizada" do
 * client oficial (#270): o modo no topo e, embaixo, duas colunas. À esquerda,
 * nome, tamanho da equipe, senha e "Esconder da lista de saguões públicos"; à
 * direita, tipo de jogo, seleção de mapas (só no Mapa aleatório, #271),
 * espectadores e intervalo de espectador. O Modo de Treino fica na aba
 * Treinamento.
 */
export function CustomGameForm({ options, onChange, locked, problem }: Props) {
  const baseId = useId();
  const set = <K extends keyof CustomGameOptions>(key: K, value: CustomGameOptions[K]) => onChange({ ...options, [key]: value });
  return (
    <div className="flex flex-col gap-5">
      <fieldset aria-label="Modo" disabled={locked} className="grid gap-3 border-b border-line pb-5 @min-[40rem]:grid-cols-3">
        {FORM_MAPS.map(map => {
          const selected = options.map === map;
          return (
            <label key={map} className={cx('flex cursor-pointer items-center gap-4 rounded-panel p-4 transition-all',
              selected ? 'bg-accent-soft ring-2 ring-accent' : 'surface shadow-soft hover:-translate-y-0.5 hover:shadow-lift',
              locked && 'cursor-not-allowed')}>
              <input type="radio" name={`${baseId}-map`} className="sr-only" checked={selected}
                onChange={() => onChange(withMap(options, map))} />
              <span aria-hidden="true" className={cx('flex size-12 shrink-0 items-center justify-center rounded-tile text-body font-semibold tracking-wide',
                selected ? 'bg-primary-gradient text-on-accent' : 'bg-raised text-muted')}>{mapIcons[map]}</span>
              <span className="text-section font-semibold uppercase tracking-wide">{formMapNames[map]}</span>
            </label>
          );
        })}
      </fieldset>

      <div className="grid gap-x-8 gap-y-4 @min-[40rem]:grid-cols-2">
        <div className="flex flex-col gap-4">
          <div className="flex flex-col gap-1.5">
            <label htmlFor={`${baseId}-name`} className={labelClass}>Nome</label>
            <input id={`${baseId}-name`} type="text" value={options.name} maxLength={CUSTOM_NAME_MAX} disabled={locked}
              aria-invalid={!!problem} aria-describedby={problem ? `${baseId}-problem` : undefined}
              onChange={event => set('name', event.target.value)} className={field} />
            {problem && <p id={`${baseId}-problem`} className="text-caption text-danger">{problem}</p>}
          </div>
          <div className="flex flex-col gap-1.5">
            <label htmlFor={`${baseId}-size`} className={labelClass}>Tamanho da equipe</label>
            <select id={`${baseId}-size`} value={options.teamSize} disabled={locked} className={field}
              onChange={event => set('teamSize', Number(event.target.value))}>
              {TEAM_SIZES.map(size => <option key={size} value={size}>{size}</option>)}
            </select>
          </div>
          <div className="flex flex-col gap-1.5">
            <label htmlFor={`${baseId}-password`} className={labelClass}>Senha (opcional)</label>
            <input id={`${baseId}-password`} type="text" value={options.password} disabled={locked} autoComplete="off"
              onChange={event => set('password', event.target.value)} className={field} />
          </div>
          <label className={cx(choice, 'font-semibold uppercase tracking-wide text-caption')}>
            <input type="checkbox" checked={options.hideFromPublicList} disabled={locked} className={radio}
              onChange={event => set('hideFromPublicList', event.target.checked)} />
            Esconder da lista de saguões públicos
          </label>
        </div>

        <div className="flex flex-col gap-4">
          <div className="flex flex-col gap-1.5">
            <label htmlFor={`${baseId}-pick`} className={labelClass}>Tipo de jogo</label>
            <select id={`${baseId}-pick`} value={options.pickType} disabled={locked} className={field}
              onChange={event => set('pickType', event.target.value as CustomPickType)}>
              {pickTypesByMap[options.map].map(pick => <option key={pick} value={pick}>{pickTypeName(options.map, pick)}</option>)}
            </select>
          </div>
          {options.map === 'howlingAbyss' && (
            <div className="flex flex-col gap-1.5">
              <label htmlFor={`${baseId}-aram-map`} className={labelClass}>Seleção de mapas</label>
              <select id={`${baseId}-aram-map`} value={options.aramMap} disabled={locked} className={field}
                onChange={event => set('aramMap', event.target.value as AramMap)}>
                {(Object.keys(aramMapNames) as AramMap[]).map(map => <option key={map} value={map}>{aramMapNames[map]}</option>)}
              </select>
            </div>
          )}
          <fieldset disabled={locked} className="flex flex-col gap-2">
            <legend className={cx(labelClass, 'mb-2')}>Permitir espectadores</legend>
            <div className="grid grid-cols-2 gap-x-4 gap-y-2">
              {(Object.keys(spectatorNames) as SpectatorPolicy[]).map(policy => (
                <label key={policy} className={choice}>
                  <input type="radio" name={`${baseId}-spectators`} checked={options.spectators === policy} className={radio}
                    onChange={() => set('spectators', policy)} />
                  {spectatorNames[policy]}
                </label>
              ))}
            </div>
          </fieldset>
          <fieldset disabled={locked} className="flex flex-col gap-2">
            <legend className={cx(labelClass, 'mb-2')}>Intervalo de espectador</legend>
            <div className="grid grid-cols-2 gap-x-4">
              {([[true, 'Adicionar intervalo'], [false, 'Sem intervalo']] as const).map(([delay, label]) => (
                <label key={label} className={choice}>
                  <input type="radio" name={`${baseId}-delay`} checked={options.spectatorDelay === delay} className={radio}
                    onChange={() => set('spectatorDelay', delay)} />
                  {label}
                </label>
              ))}
            </div>
          </fieldset>
        </div>
      </div>
    </div>
  );
}
