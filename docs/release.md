# Como lançar uma versão

O instalador do Windows e o auto-update saem do workflow [Release](../.github/workflows/release.yml) (#35). Ele roda quando uma tag `vX.Y.Z` chega ao GitHub e publica a release no repositório público [rmuratah/easylobby-site](https://github.com/rmuratah/easylobby-site) (#567), porque este repositório vai ficar privado. O botão de download do site aponta para lá.

## Uma vez só: chave das atualizações

O app só instala uma atualização assinada pela chave do projeto. A parte pública fica em `desktop/src-tauri/tauri.conf.json` (`plugins.updater.pubkey`); a privada fica só com quem lança versões e nos secrets do repositório.

1. Gere a chave (pede uma senha):
   ```powershell
   pnpm --filter easylobby-desktop tauri signer generate -w "$env:USERPROFILE\.tauri\easylobby.key"
   ```
2. Copie o conteúdo de `easylobby.key.pub` para `plugins.updater.pubkey`.
3. Em **Settings > Secrets and variables > Actions**, crie `TAURI_SIGNING_PRIVATE_KEY` (conteúdo de `easylobby.key`) e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (a senha).
4. Guarde um backup da chave e da senha fora do PC. Sem elas, quem já instalou o app não recebe mais atualização.

## Uma vez só: token do easylobby-site

O `GITHUB_TOKEN` do workflow só escreve neste repositório. Para criar a release no `easylobby-site`:

1. No GitHub, em **Settings > Developer settings > Personal access tokens > Fine-grained tokens**, gere um token só para o repositório `rmuratah/easylobby-site`, com a permissão **Contents: Read and write**.
2. Em **Settings > Secrets and variables > Actions** deste repositório, crie `SITE_RELEASE_TOKEN` com esse token.
3. Anote a data de validade: com o token vencido, o workflow falha na etapa de publicar.

## A cada versão

1. Num PR, suba a versão nos três lugares, com o mesmo número:
   - `version` em `[workspace.package]` do `Cargo.toml` da raiz;
   - `version` em `desktop/src-tauri/tauri.conf.json`;
   - `version` em `desktop/package.json` e `ui/package.json`.
2. Depois do merge, crie uma tag anotada na `main`. A mensagem da tag vira as notas que o app mostra em Configurações > Atualizações:
   ```bash
   git switch main && git pull
   git tag -a v0.2.0 -m "Pós-jogo com resultado e placar."
   git push origin v0.2.0
   ```
3. O workflow confere se a tag bate com a versão do app, gera o instalador `easylobby_X.Y.Z_x64-setup.exe`, assina o pacote de atualização e cria uma release **rascunho** com o `latest.json` no `easylobby-site`.
4. Baixe o instalador do rascunho e teste num Windows. Se estiver bom, publique a release no `easylobby-site`. Só a release publicada (a mais recente que não é pré-release) chega aos jogadores.
5. Enquanto este repositório for público, o workflow também cria aqui um rascunho só com o `latest.json`, para quem instalou antes do #567 achar a atualização. Publique esse rascunho junto.

## Como o app atualiza

- Ao abrir e a cada 6 horas, o app lê `https://github.com/rmuratah/easylobby-site/releases/latest/download/latest.json` e confere a assinatura do pacote.
- Com versão nova, aparece um ponto no botão de Configurações e a seção **Atualizações** mostra a versão e as notas.
- **Atualizar e reiniciar** só fica liberado fora da fila, da partida encontrada, da seleção e da partida, porque o app fecha durante a instalação. O instalador roda sem perguntas (modo `passive`) e abre o easylobby de novo; o agente reconecta e fecha a UI oficial como sempre.
- O instalador é por usuário (`currentUser`): não pede administrador e instala em `%LOCALAPPDATA%\easylobby`.

## Ainda falta

- Assinar o instalador com certificado de código, para o Windows não barrar no SmartScreen e no Controle Inteligente de Aplicativos (#260). Até lá, o Windows mostra o aviso azul na primeira instalação.
