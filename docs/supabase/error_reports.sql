-- Tabela dos erros enviados pelo app durante o alfa fechado (#550).
--
-- Projeto do Supabase "easylobby" (plano grátis, sa-east-1). O app usa a
-- chave publicável, que entra como o papel `anon`. Com a RLS abaixo, esse
-- papel só INSERE: não lê, não muda e não apaga nenhuma linha. Quem lê é o
-- dono do projeto, pelo painel do Supabase.

create table public.error_reports (
  id bigint generated always as identity primary key,
  created_at timestamptz not null default now(),
  install_id uuid not null,
  app_version text not null check (char_length(app_version) <= 32),
  os text not null check (char_length(os) <= 32),
  level text not null check (level in ('ERROR', 'WARN')),
  target text not null check (char_length(target) <= 200),
  message text not null check (char_length(message) <= 4100)
);

comment on table public.error_reports is
  'Erros do app easylobby (#550). A chave publicável só insere.';

create index error_reports_created_at_idx on public.error_reports (created_at desc);

alter table public.error_reports enable row level security;

-- Nada além do INSERT para quem usa as chaves públicas.
revoke all on table public.error_reports from anon, authenticated;
grant insert (install_id, app_version, os, level, target, message)
  on table public.error_reports to anon;

create policy "app só insere erros"
  on public.error_reports
  for insert
  to anon
  with check (true);
