import { useEffect, useId, useRef, useState, type DragEvent } from 'react';
import { createPortal } from 'react-dom';
import type { AgentClient } from '../agent/client';
import { LANES, moveLane, positionNames, type Lane } from '../agent/positions';
import { useCommand } from '../agent/useCommand';
import type { LobbyMember } from '../bindings/LobbyMember';
import type { Position } from '../bindings/Position';
import { Button } from './base';
import { cx } from './base/cx';
import { PositionIcon } from './PositionIcon';
import { ScarceMark } from './ScarceMark';

const focusRing = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';

/** Nome de "Qualquer posição" (`fill`) no Rift Clássico, como no client oficial. */
const ANY_POSITION = 'Qualquer posição';

/** Ordem das 5 rotas guardada no membro, ou `null` se ele ainda não definiu ou marcou "Qualquer posição". */
function laneOrder(order: Position[]): Lane[] | null {
  const lanes = order.filter((position): position is Lane => (LANES as readonly Position[]).includes(position));
  return lanes.length === LANES.length && new Set(lanes).size === LANES.length ? lanes : null;
}

/**
 * Preferências de posição de um membro no card do lobby do Rift Clássico
 * (#230): a 1ª em destaque e as outras quatro em ícones menores, na ordem.
 */
export function MemberPositionOrder({ member, name }: { member: LobbyMember; name: string }) {
  const any = member.positionOrder.length === 1 && member.positionOrder[0] === 'fill';
  const order = laneOrder(member.positionOrder);
  const [first, ...rest] = order ?? [];
  return (
    <div aria-label={`Preferências de posição de ${name}`} role="group" className="flex w-36 shrink-0 flex-col gap-2 @min-[52rem]:w-52">
      <p className="flex items-center gap-2 rounded-tile bg-accent-soft px-3 py-2 text-body font-medium text-accent-ink">
        <PositionIcon position={any ? 'fill' : first ?? 'unselected'} />
        <span className="truncate">{any ? ANY_POSITION : first ? positionNames[first] : 'Não definidas'}</span>{' '}
        {first && <span className="sr-only shrink-0 text-caption @min-[52rem]:not-sr-only @min-[52rem]:ml-auto">1ª preferência</span>}
      </p>
      {rest.length > 0 && (
        <ol aria-label="Da 2ª à 5ª preferência" className="flex items-center gap-1 px-1 text-muted">
          {rest.map((lane, index) => (
            <li key={lane} title={`${index + 2}ª: ${positionNames[lane]}`} className="flex items-center gap-0.5">
              <span aria-hidden="true" className="text-caption font-semibold">{index + 2}</span>
              <PositionIcon position={lane} size={16} />
              <span className="sr-only">{`${index + 2}ª: ${positionNames[lane]}`}</span>
            </li>
          ))}
        </ol>
      )}
    </div>
  );
}

type CardProps = {
  member: LobbyMember | undefined;
  /** Posições que faltam na fila agora (#410). */
  scarce: Position[];
  canEdit: boolean;
  send: AgentClient['send'];
};

/**
 * Card "Suas preferências de posição" do Rift Clássico (#230), embaixo da
 * lista de jogadores: resume a ordem salva e abre o pop-up para mudar.
 */
export function PositionOrderCard({ member, scarce, canEdit, send }: CardProps) {
  const [open, setOpen] = useState(false);
  const order = member ? laneOrder(member.positionOrder) : null;
  const any = !!member && member.positionOrder.length === 1 && member.positionOrder[0] === 'fill';
  return (
    <section aria-label="Suas preferências de posição"
      className="flex h-full min-w-0 flex-col gap-3 rounded-panel surface card p-4 shadow-soft animate-fade-up">
      <div className="flex flex-wrap items-center justify-between gap-3 px-1">
        <h3 className="text-section font-semibold">Suas preferências de posição</h3>
        <Button size="sm" variant="primary" disabled={!canEdit} onClick={() => setOpen(true)}>
          {order || any ? 'Mudar preferências' : 'Definir preferências'}
        </Button>
      </div>
      {any ? (
        <p className="flex items-center gap-2 px-1 text-body font-medium"><PositionIcon position="fill" />{ANY_POSITION}</p>
      ) : order ? (
        <ol aria-label="Sua ordem de preferência" className="flex flex-wrap gap-2 px-1">
          {order.map((lane, index) => (
            <li key={lane} className={cx('flex items-center gap-1.5 rounded-tile px-2.5 py-1.5 text-caption font-medium',
              index === 0 ? 'bg-accent-soft text-accent-ink' : 'bg-raised text-muted')}>
              <span className="font-semibold">{index + 1}</span>
              <PositionIcon position={lane} size={16} />
              {positionNames[lane]}
              {scarce.includes(lane) && <ScarceMark />}
            </li>
          ))}
        </ol>
      ) : (
        <p className="px-1 text-body text-muted">Ordene as 5 posições ou marque {ANY_POSITION} antes de buscar.</p>
      )}
      <p className="px-1 text-caption text-muted">
        Você recebe a Égide da Valentia ao completar uma partida como {ANY_POSITION} ou na 3ª à 5ª preferência.
      </p>
      {open && member && <PositionOrderDialog member={member} scarce={scarce} send={send} onClose={() => setOpen(false)} />}
    </section>
  );
}

/**
 * Pop-up "Defina sua preferência de posição" (#230): as 5 rotas numeradas de
 * 1 a 5, reordenadas arrastando ou pelas setas, e "Qualquer posição". Só
 * manda o comando ao clicar em Salvar.
 */
function PositionOrderDialog({ member, scarce, send, onClose }: {
  member: LobbyMember;
  scarce: Position[];
  send: AgentClient['send'];
  onClose: () => void;
}) {
  const titleId = useId();
  const firstRef = useRef<HTMLInputElement>(null);
  const [order, setOrder] = useState<Lane[]>(() => laneOrder(member.positionOrder) ?? [...LANES]);
  const [any, setAny] = useState(member.positionOrder.length === 1 && member.positionOrder[0] === 'fill');
  const [dragging, setDragging] = useState<number | null>(null);
  const { feedback, pending, run } = useCommand(send);

  useEffect(() => {
    firstRef.current?.focus();
    function onKey(event: KeyboardEvent) { if (event.key === 'Escape') onClose(); }
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [onClose]);

  async function save() {
    const ok = await run({ type: 'setPositionOrder', positions: any ? ['fill'] : order },
      { pending: 'Salvando preferências…', failure: 'salvar as preferências' });
    if (ok) onClose();
  }

  function onDrop(event: DragEvent, to: number) {
    event.preventDefault();
    if (dragging !== null) setOrder(current => moveLane(current, dragging, to));
    setDragging(null);
  }

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4 animate-fade-up"
      onPointerDown={event => { if (event.target === event.currentTarget) onClose(); }}>
      <div role="dialog" aria-modal="true" aria-labelledby={titleId}
        className="flex max-h-[calc(100vh-2rem)] w-full max-w-md flex-col gap-4 overflow-y-auto rounded-panel surface p-5 text-ink shadow-lift">
        <h2 id={titleId} className="text-section font-semibold">Defina sua preferência de posição</h2>
        <p className="text-caption text-muted">Arraste ou use as setas para ordenar da 1ª à 5ª preferência.</p>
        <ol aria-label="Ordem de preferência" className={cx('flex flex-col gap-2 transition-opacity', any && 'opacity-40')}>
          {order.map((lane, index) => (
            <li key={lane} draggable={!any && !pending}
              onDragStart={event => { event.dataTransfer.effectAllowed = 'move'; setDragging(index); }}
              onDragOver={event => { if (dragging !== null) event.preventDefault(); }}
              onDrop={event => onDrop(event, index)} onDragEnd={() => setDragging(null)}
              className={cx('flex items-center gap-3 rounded-tile px-3 py-2 text-body font-medium',
                index === 0 ? 'bg-accent-soft text-accent-ink' : 'bg-raised', !any && 'cursor-grab',
                dragging === index && 'ring-2 ring-accent')}>
              <span aria-hidden="true" className="w-4 text-center font-semibold">{index + 1}</span>
              <PositionIcon position={lane} />
              <span className="flex-1 truncate">
                <span className="sr-only">{index + 1}ª: </span>{positionNames[lane]}
                {scarce.includes(lane) && <> <ScarceMark className="ml-1 inline-block" /><span className="sr-only"> (prioritária)</span></>}
              </span>
              <button type="button" aria-label={`Subir ${positionNames[lane]}`} disabled={any || pending || index === 0}
                onClick={() => setOrder(current => moveLane(current, index, index - 1))}
                className={`flex size-7 items-center justify-center rounded-full text-muted enabled:hover:bg-surface enabled:hover:text-ink disabled:opacity-30 ${focusRing}`}>
                <span aria-hidden="true">▲</span>
              </button>
              <button type="button" aria-label={`Descer ${positionNames[lane]}`} disabled={any || pending || index === order.length - 1}
                onClick={() => setOrder(current => moveLane(current, index, index + 1))}
                className={`flex size-7 items-center justify-center rounded-full text-muted enabled:hover:bg-surface enabled:hover:text-ink disabled:opacity-30 ${focusRing}`}>
                <span aria-hidden="true">▼</span>
              </button>
            </li>
          ))}
        </ol>
        <label className="flex items-center gap-2 text-body font-medium">
          <input ref={firstRef} type="checkbox" checked={any} disabled={pending} onChange={event => setAny(event.target.checked)}
            className={`size-4 accent-accent ${focusRing}`} />
          <PositionIcon position="fill" />
          {ANY_POSITION}
        </label>
        <p className="text-caption text-muted">
          Você recebe a Égide da Valentia ao completar uma partida como {ANY_POSITION} ou na 3ª à 5ª preferência.
        </p>
        {feedback.status === 'error' && <p role="alert" className="text-caption text-danger">{feedback.message}</p>}
        <footer className="flex justify-end gap-2">
          <Button size="sm" variant="ghost" onClick={onClose}>Cancelar</Button>
          <Button size="sm" variant="primary" loading={pending} onClick={() => { void save(); }}>Salvar</Button>
        </footer>
      </div>
    </div>,
    document.body,
  );
}
