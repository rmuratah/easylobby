import { describe, expect, it } from 'vitest';
import { parseMessage } from './protocol';
import { applyEvent } from './client';
import { makeState } from '../test/state';
import { testReadyCheck } from '../test/events';
import { readyCheckLabels, responseLabels } from './readyCheck';

describe('contrato ready check', () => {
  for (const state of Object.keys(readyCheckLabels)) {
    for (const playerResponse of Object.keys(responseLabels)) {
      it(`valida e aplica ${state}/${playerResponse}`, () => {
        const check = { ...testReadyCheck, state, playerResponse };
        const event = parseMessage(JSON.stringify({ type: 'readyCheck', readyCheck: check }));
        if (event?.type !== 'readyCheck') throw new Error('evento esperado');
        expect(applyEvent(makeState(), event).readyCheck).toEqual(check);
        expect(parseMessage(JSON.stringify({ type: 'hello', protocol: 1, agentVersion: 'test', state: makeState({ readyCheck: event.readyCheck }) }))).not.toBeNull();
      });
    }
  }
  it.each([{ state: 'novo' }, { state: 'toString' }, { playerResponse: 'novo' }, { playerResponse: 'toString' },
    { timer: null }, { duration: '13' }, { duration: null }])('rejeita %j em evento e hello', change => {
    const readyCheck = { ...testReadyCheck, ...change };
    expect(() => parseMessage(JSON.stringify({ type: 'readyCheck', readyCheck }))).toThrow();
    expect(() => parseMessage(JSON.stringify({ type: 'hello', protocol: 1, agentVersion: 'test', state: { ...makeState(), readyCheck } }))).toThrow();
  });
  it('descarta ready check quando a sessão LCU muda', () => {
    const state = makeState({ lcu: { state: 'connected', session: 1 }, readyCheck: testReadyCheck });
    expect(applyEvent(state, { type: 'lcu', status: { state: 'connected', session: 2 } }).readyCheck).toBeNull();
    expect(applyEvent(state, { type: 'lcu', status: { state: 'disconnected', reason: 'queda' } }).readyCheck).toBeNull();
  });
});
