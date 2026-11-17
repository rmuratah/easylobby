import { afterEach, expect, it, vi } from 'vitest';
import { installErrorLogging, reportError, setErrorLog, type ErrorLog } from './errorLog';

function fakeLog() {
  const report = vi.fn<ErrorLog['report']>();
  const log: ErrorLog = { report, openFolder: () => Promise.resolve() };
  return Object.assign(log, { report });
}

afterEach(() => setErrorLog(undefined));

it('manda o nome, a mensagem e a pilha de um Error', () => {
  const log = fakeLog();
  setErrorLog(log);
  const error = new TypeError('x is undefined');
  reportError('tela', error);
  expect(log.report).toHaveBeenCalledWith('tela', 'TypeError: x is undefined', error.stack);
});

it('aceita texto e objetos', () => {
  const log = fakeLog();
  setErrorLog(log);
  reportError('ws', 'Conexão com o agente interrompida.');
  reportError('promessa rejeitada', { code: 1 });
  expect(log.report).toHaveBeenNthCalledWith(1, 'ws', 'Conexão com o agente interrompida.', undefined);
  expect(log.report).toHaveBeenNthCalledWith(2, 'promessa rejeitada', '{"code":1}', undefined);
});

it('não faz nada fora do app desktop', () => {
  setErrorLog(null);
  expect(() => reportError('tela', new Error('x'))).not.toThrow();
});

it('grava exceções e promessas rejeitadas da janela', () => {
  const log = fakeLog();
  setErrorLog(log);
  const target = new EventTarget() as Window;
  const uninstall = installErrorLogging(target);
  target.dispatchEvent(Object.assign(new Event('error'), { error: new Error('quebrou') }));
  target.dispatchEvent(Object.assign(new Event('unhandledrejection'), { reason: 'sem resposta' }));
  uninstall();
  target.dispatchEvent(Object.assign(new Event('error'), { error: new Error('depois') }));
  expect(log.report).toHaveBeenCalledTimes(2);
  expect(log.report).toHaveBeenNthCalledWith(1, 'exceção', 'Error: quebrou', expect.any(String));
  expect(log.report).toHaveBeenNthCalledWith(2, 'promessa rejeitada', 'sem resposta', undefined);
});
