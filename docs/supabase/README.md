# Erros enviados pelo app (Supabase)

Durante o alfa fechado, o app instalado manda os erros do log para o projeto **easylobby** do Supabase (organização Aaru Online, plano grátis, região `sa-east-1`). Issue: #550.

## O que é enviado

Só os eventos de nível **erro** do `tracing`: erros do agente e do shell, erros da UI (`easylobby_ui`) e pânicos. Cada linha da tabela `error_reports` tem:

| Coluna | O que é |
| --- | --- |
| `created_at` | Hora em que chegou (UTC) |
| `install_id` | Id aleatório da instalação (`id-instalacao` na pasta de configuração do app) |
| `app_version`, `os` | Versão do easylobby e sistema |
| `level`, `target` | Nível e módulo que registrou o erro |
| `message` | Texto do erro, com a pasta do usuário trocada por `~` e cortado em 4.000 caracteres |

Valem as regras do log local (#537): nunca a senha do lockfile, o cabeçalho `Authorization`, a chave da Riot nem o texto do chat. Não vai nick, e-mail nem puuid.

Limites no app: lotes a cada 10 s, a mesma mensagem só uma vez por sessão e no máximo 100 erros por sessão. Build de desenvolvimento (`tauri dev`) não envia nada. O jogador desliga em **Configurações > Enviar erros para os devs** (o app grava o arquivo `enviar-erros-desligado` na pasta de configuração).

## Segurança

O app usa a chave **publicável** do projeto (`agent/src/error_report.rs`). Ela não é segredo: pela RLS de [`error_reports.sql`](error_reports.sql), o papel `anon` só tem `INSERT` nas colunas acima. Não lê, não muda e não apaga linhas. Só o dono do projeto lê, pelo painel.

Para testar com outro projeto, defina `EASYLOBBY_ERROR_REPORT_URL` e `EASYLOBBY_ERROR_REPORT_KEY` antes de abrir o app.

## Como ler os erros

No painel do Supabase, abra o projeto **easylobby** > **Table Editor** > `error_reports`, ou rode no **SQL Editor**:

```sql
-- Erros mais comuns da última semana e quantas instalações tiveram cada um
select target, message, count(*) as vezes, count(distinct install_id) as instalacoes,
       max(created_at) as ultimo, max(app_version) as versao
from error_reports
where created_at > now() - interval '7 days'
group by target, message
order by vezes desc;
```

## Custo e limpeza

O plano grátis tem 500 MB de banco; cada erro ocupa menos de 5 KB. Um projeto grátis sem uso por 7 dias é pausado pelo Supabase. Para apagar erros antigos:

```sql
delete from error_reports where created_at < now() - interval '30 days';
```
