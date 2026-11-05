/// <reference types="node" />
// @vitest-environment node
import { spawn, type ChildProcess } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { expect, it, vi } from 'vitest';
import { AgentClient } from './client';

// Usa os executáveis reais compilados pelo CI, sem substituir o WebSocket.
it.runIf(process.env.EASYLOBBY_INTEGRATION === '1')('conecta ao agente real com mock-lcu e recupera após reiniciar o agente', async () => {
  const root = resolve('..');
  const dir = await mkdtemp(join(tmpdir(), 'easylobby-ui-'));
  const lockfile = join(dir, 'lockfile');
  const ext = process.platform === 'win32' ? '.exe' : '';
  const children: ChildProcess[] = [];
  const errors: Error[] = [];
  const start = (binary: string, args: string[]) => {
    const child = spawn(join(root, 'target', 'debug', binary + ext), args, { cwd: root, stdio: 'ignore' });
    child.on('error', error => errors.push(error));
    children.push(child);
    return child;
  };
  const stop = (child: ChildProcess) => new Promise<void>(resolveExit => {
    if (!child.pid || child.exitCode !== null || child.signalCode !== null) return resolveExit();
    child.once('exit', () => resolveExit());
    child.kill();
  });
  const client = new AgentClient();
  const connected = () => vi.waitFor(() => {
    expect(errors).toEqual([]);
    expect(client.getSnapshot().state?.lcu.state).toBe('connected');
    expect(client.getSnapshot().state?.summoner?.summonerLevel).toBeGreaterThan(0);
  }, { timeout: 20000, interval: 100 });
  try {
    start('mock-lcu', ['--lockfile', lockfile, '--parado']);
    let agent = start(join('examples', 'canal_ws'), ['--lockfile', lockfile]);
    client.start();
    await connected();
    // Amigos e runas reais da fixture passam pela validação do protocolo.
    await vi.waitFor(() => {
      const state = client.getSnapshot().state;
      expect(state?.friends.length).toBeGreaterThan(0);
      expect(state?.runePages?.pages.length).toBeGreaterThan(0);
      expect(state?.runeCatalog?.styles.length).toBeGreaterThan(0);
    }, { timeout: 10000, interval: 100 });
    await expect(client.send({ type: 'ping' })).resolves.toEqual({ status: 'ok' });
    await expect(client.send({ type: 'getState' })).resolves.toEqual({ status: 'ok' });
    // O estado real deve chegar antes da resposta, inclusive ao trocar e sair.
    for (const queue of ['normalDraft', 'swiftplay', 'rankedSolo', 'rankedFlex', 'aram'] as const) {
      await expect(client.send({ type: 'createLobby', queue })).resolves.toEqual({ status: 'ok' });
      expect(client.getSnapshot().state?.lobby?.queue).toBe(queue);
      expect(client.getSnapshot().state?.lobby?.members.length).toBeGreaterThan(0);
      if (client.getSnapshot().state?.lobby?.showPositionSelector) {
        for (const [first, second] of [['middle', 'jungle'], ['fill', 'unselected'], ['unselected', 'unselected']] as const) {
          await expect(client.send({ type: 'setPositions', first, second })).resolves.toEqual({ status: 'ok' });
          const state = client.getSnapshot().state;
          const local = state?.lobby?.members.find(member => member.puuid === state.summoner?.puuid);
          expect(local?.firstPosition).toBe(first);
          expect(local?.secondPosition).toBe(second);
        }
        await expect(client.send({ type: 'setPositions', first: 'top', second: 'top' }))
          .resolves.toMatchObject({ status: 'error', code: 'invalidCommand' });
      }

    }
    await expect(client.send({ type: 'leaveLobby' })).resolves.toEqual({ status: 'ok' });
    expect(client.getSnapshot().state?.lobby).toBeNull();
    await stop(agent);
    await vi.waitFor(() => expect(client.getSnapshot().state).toBeNull(), { timeout: 5000 });
    agent = start(join('examples', 'canal_ws'), ['--lockfile', lockfile]);
    await connected();
    await expect(client.send({ type: 'ping' })).resolves.toEqual({ status: 'ok' });
  } finally {
    client.stop();
    await Promise.all(children.map(stop));
    await rm(dir, { recursive: true, force: true });
  }
}, 60000);
