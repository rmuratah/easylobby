# Testar lobby e escolha de fila no Windows (#17)

## Verificações automáticas

Na raiz do repositório, com Node e a versão de pnpm do `package.json` disponíveis:

```powershell
pnpm install --frozen-lockfile
pnpm test
pnpm lint
pnpm build:ui
```

Se a instalação for bloqueada pela política de idade mínima de uma dependência,
aguarde a liberação e repita; preserve o lockfile e a política.

## Com o League of Legends real

1. Remova a variável de mock da sessão PowerShell, se existir:
   `Remove-Item Env:EASYLOBBY_MOCK_LCU -ErrorAction SilentlyContinue`.
   Execute `pnpm dev:desktop` e abra o LoL. Antes de conectar/fazer login, a
   seção Lobby deve orientar a aguardar, sem permitir comandos.
2. Fora de partida, escolha Normal — Escolha alternada e clique **Criar lobby**.
   Confira a indicação de execução e que cliques repetidos não geram novas
   solicitações. Após a resposta, confira fila atual, quantidade de jogadores,
   seu Riot ID/nível e liderança.
3. Troque para Jogo Dinâmico, Ranqueada Solo/Duo, Ranqueada Flex e ARAM.
   A fila exibida deve vir do agente; se a LCU recusar uma fila, deve aparecer
   erro sem desconectar a UI. A fila anterior deve continuar visível.
   A capacidade deve acompanhar o lobby recebido (Solo/Duo: 2; demais: 5).
4. Clique **Sair do lobby**. A UI deve mostrar execução e depois lobby vazio e
   confirmação. Crie outro lobby para conferir que uma nova tentativa funciona.
5. Use a interface oficial para entrar no grupo de um amigo como membro:
   trocar fila deve ficar bloqueado, com explicação; sair continua disponível
   na fase Lobby. Outros membros sem nome disponível aparecem pelo ID de
   invocador. Se possível, transfira a liderança e confira a atualização.
6. Pela interface oficial, entre na busca ou inicie uma partida. Criar/trocar
   e sair devem ficar bloqueados durante busca, partida encontrada e seleção
   de campeões. Retorne à fase Lobby para recuperar as ações. Não há controles
   de cargos, entrar na fila ou ready check nesta entrega.
7. Feche/reabra o LoL e pare/reinicie o app/agente. Dados e confirmação antigos
   devem desaparecer e o lobby deve ser recebido novamente. Não deve haver
   reenvio automático de comandos. Se uma operação for interrompida ou expirar,
   confira o lobby antes de tentar novamente.
8. Confira também o perfil do invocador (#64) e o botão **Abrir client oficial**
   (#68), já integrados à main. O botão deve continuar funcionando durante as
   operações de lobby; todos os controles usam o mesmo `useAgent`.

A disponibilidade das filas e restrições da conta são decididas pelo LoL. A UI
mostra as cinco opções de `Queue`; uma opção presente no contrato pode ser
recusada pela LCU naquele momento.

## Sem LoL, com mock-lcu

Com Rust estável instalado, use três terminais na raiz:

```powershell
cargo run -p mock-lcu
```

```powershell
cargo run -p easylobby-agent --example canal_ws -- --mock
```

```powershell
pnpm dev:ui
```

Repita criação, troca das cinco filas e saída; confira o lobby e reinicie o
agente para testar reconexão. O replay do mock pode mudar a fase enquanto você
interage; nesse caso a UI deve bloquear as ações nas fases protegidas.

Para a integração automática com o agente e o mock reais (modo parado):

```powershell
cargo +stable build --locked -p mock-lcu
cargo +stable build --locked -p easylobby-agent --example canal_ws
$env:EASYLOBBY_INTEGRATION = '1'
pnpm --filter easylobby-ui exec vitest run src/agent/integration.test.ts
Remove-Item Env:EASYLOBBY_INTEGRATION
```

O teste percorre as cinco filas, confere o estado antes de cada resposta, sai do
lobby e verifica a recuperação após reiniciar o agente. Os testes React com
WebSocket simulado cobrem erros, timeout, duplicação, liderança, fases e respostas
tardias; eles não comprovam comportamento do LoL real.
