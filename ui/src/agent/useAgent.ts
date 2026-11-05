import { useEffect, useState, useSyncExternalStore } from 'react';
import { AgentClient } from './client';

export function useAgent() {
  const [client] = useState(() => new AgentClient());
  const connection = useSyncExternalStore(client.subscribe, client.getSnapshot);
  useEffect(() => {
    client.start();
    return client.stop;
  }, [client]);
  return { ...connection, send: client.send };
}
