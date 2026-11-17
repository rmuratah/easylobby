import { useState, type ReactNode } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { featureLabels, problemText, unavailableFeature, unavailableFeatures } from '../agent/compatibility';
import type { Feature } from '../bindings/Feature';
import { Button, ErrorMessage } from './base';
import { LaunchUxButton } from './LaunchUxButton';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };

/**
 * Modo de compatibilidade (#36): avisa quais funções o agente desligou porque
 * o LoL mudou um endpoint delas, e oferece o client oficial. Fica escondido
 * com tudo funcionando; "Entendi" esconde até a lista mudar.
 */
export function CompatibilityBanner({ connection, send }: Props) {
  const items = unavailableFeatures(connection);
  const key = items.map(i => `${i.feature}:${i.path}:${i.problem}`).join('|');
  const [dismissed, setDismissed] = useState<string | null>(null);
  if (items.length === 0 || dismissed === key) return null;
  const names = items.map(i => featureLabels[i.feature]);
  return (
    <section role="alert" aria-label="Modo de compatibilidade"
      className="col-span-full flex flex-col gap-3 rounded-card border border-warn bg-surface card p-4">
      <div className="flex flex-col gap-1">
        <h2 className="text-body font-semibold text-ink">Algumas funções foram desligadas</h2>
        <p className="text-body text-ink">
          O LoL foi atualizado e mudou partes que o easylobby usa. Para não falhar sem aviso, desligamos
          só {names.length === 1 ? 'esta função' : 'estas funções'}: <strong>{joinNames(names)}</strong>.
          O resto continua funcionando. Para {names.length === 1 ? 'ela' : 'elas'}, use o client oficial até
          sair uma atualização do easylobby.
        </p>
      </div>
      <details className="text-caption text-muted">
        <summary className="cursor-pointer">Detalhes para reportar o problema</summary>
        <ul className="mt-2 flex flex-col gap-1">
          {items.map(i => (
            <li key={i.feature}>
              <span className="font-semibold">{featureLabels[i.feature]}</span>: {problemText(i)}{' '}
              <code className="break-all">{i.path}</code> ({i.detail})
            </li>
          ))}
        </ul>
      </details>
      <div className="flex flex-wrap items-center gap-2">
        <LaunchUxButton connection={connection} send={send} />
        <Button size="sm" variant="ghost" onClick={() => setDismissed(key)}>Entendi</Button>
      </div>
    </section>
  );
}

/**
 * Mostra `children` enquanto `feature` funciona. Desligada pelo modo de
 * compatibilidade, troca a tela por um aviso com o botão do client oficial
 * (ou por nada, com `quiet`).
 */
export function FeatureGate({ connection, send, feature, quiet = false, children }:
  Props & { feature: Feature; /** Desligada, não mostra nada (o aviso já está em outro lugar). */ quiet?: boolean; children: ReactNode }) {
  const item = unavailableFeature(connection, feature);
  if (!item) return children;
  if (quiet) return null;
  return (
    <ErrorMessage title={`${featureLabels[feature]}: função desligada`}
      description={<>{problemText(item)} Para não falhar sem aviso, o easylobby desligou esta função. Use o client oficial para continuar.</>}
      action={<LaunchUxButton connection={connection} send={send} />} />
  );
}

function joinNames(names: string[]): string {
  if (names.length <= 1) return names.join('');
  return `${names.slice(0, -1).join(', ')} e ${names.at(-1)}`;
}
