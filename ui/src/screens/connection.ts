import type { ConnectionSnapshot } from '../agent/client';

/** Tom do indicador da barra: pronto, a caminho ou parado. */
export type ConnectionTone = 'ready' | 'waiting' | 'stopped';

export type ConnectionView = {
  /** Frase curta para a barra de título e leitores de tela. */
  label: string;
  tone: ConnectionTone;
  /** Tela cheia enquanto não dá para jogar; `null` quando está tudo pronto. */
  screen: null | {
    kind: 'connecting' | 'lolClosed' | 'error';
    title: string;
    description: string;
  };
};

/** Traduz o estado do agente e do LoL no que a UI mostra (#128). */
export function connectionView(connection: ConnectionSnapshot): ConnectionView {
  if (connection.status === 'incompatible') {
    return { label: 'Versões incompatíveis', tone: 'stopped', screen: { kind: 'error', title: 'Versões incompatíveis',
      description: 'O app e o agente são de versões diferentes. Atualize o easylobby e abra de novo.' } };
  }
  if (connection.status === 'reconnecting') {
    return { label: 'Reconectando ao agente…', tone: 'waiting', screen: { kind: 'connecting', title: 'Reconectando ao agente',
      description: 'O canal interno do easylobby caiu e está voltando sozinho. Se demorar, feche o app pela bandeja e abra de novo.' } };
  }
  if (!connection.state) {
    return { label: 'Conectando ao agente…', tone: 'waiting', screen: { kind: 'connecting', title: 'Conectando ao agente',
      description: 'Leva só um instante depois que o app abre.' } };
  }
  const lcu = connection.state.lcu;
  switch (lcu.state) {
    case 'starting':
      return { label: 'Aguardando o League of Legends…', tone: 'waiting', screen: { kind: 'connecting',
        title: 'Procurando o League of Legends', description: 'Leva alguns segundos depois que o client abre.' } };
    case 'retrying':
      if (lcu.clientNotRunning) {
        return { label: 'Abra o League of Legends para continuar', tone: 'stopped', screen: { kind: 'lolClosed',
          title: 'O League of Legends está fechado',
          description: 'Abra o jogo para ver lobby, fila e amigos aqui. O easylobby fecha a interface oficial sozinho depois.' } };
      }
      return { label: 'Tentando reconectar ao League of Legends…', tone: 'waiting', screen: { kind: 'connecting',
        title: 'Tentando reconectar ao League of Legends',
        description: 'O client está aberto, mas não respondeu. O easylobby tenta de novo sozinho.' } };
    case 'disconnected':
      return { label: 'Conexão com o League of Legends interrompida', tone: 'stopped', screen: { kind: 'error',
        title: 'Conexão com o League of Legends interrompida',
        description: 'O easylobby vai tentar de novo. Se o jogo travou, feche e abra o League of Legends.' } };
    case 'connected':
      if (!connection.state.summoner) {
        return { label: 'Aguardando login no League of Legends…', tone: 'waiting', screen: { kind: 'connecting',
          title: 'Aguardando o login', description: 'Entre na sua conta no client da Riot para continuar.' } };
      }
      return { label: 'Conectado ao League of Legends', tone: 'ready', screen: null };
  }
}
