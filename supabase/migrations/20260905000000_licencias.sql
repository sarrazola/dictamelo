-- Licencias Pro y consumo. Solo la función de borde (rol de servicio) toca estas tablas:
-- RLS queda activo y sin políticas, así que ni la clave anónima ni un usuario final leen nada.

create table if not exists public.licenses (
  id              uuid primary key default gen_random_uuid(),
  -- SHA-256 de la clave de licencia. Nunca se guarda la clave en claro: si alguien
  -- obtuviera la base de datos, no podría usar las licencias de los clientes.
  key_hash        text        not null unique,
  instance_id     text,
  -- Último estado informado por Lemon Squeezy: active, expired, disabled, inactive...
  status          text        not null default 'unknown',
  -- Cuándo se revalidó por última vez contra Lemon Squeezy (para no llamar en cada petición).
  checked_at      timestamptz not null default now(),
  created_at      timestamptz not null default now()
);

comment on table public.licenses is 'Caché de licencias de Lemon Squeezy. La clave se guarda solo como hash.';

create table if not exists public.usage_events (
  id          bigserial   primary key,
  license_id  uuid        not null references public.licenses (id) on delete cascade,
  -- Segundos de audio procesados. Sirve para el tope mensual y para ver el costo real.
  seconds     numeric(10, 2) not null check (seconds >= 0),
  kind        text        not null default 'transcribe' check (kind in ('transcribe', 'cleanup')),
  created_at  timestamptz not null default now()
);

comment on table public.usage_events is 'Consumo por licencia. No guarda audio ni texto, solo duración.';

-- El tope mensual se consulta en cada petición: conviene el índice.
create index if not exists usage_events_license_created_idx
  on public.usage_events (license_id, created_at desc);

alter table public.licenses     enable row level security;
alter table public.usage_events enable row level security;

-- Segundos consumidos por una licencia en los últimos 30 días.
create or replace function public.usage_last_30_days(p_license uuid)
returns numeric
language sql
stable
security definer
set search_path = public
as $$
  select coalesce(sum(seconds), 0)
  from public.usage_events
  where license_id = p_license
    and created_at > now() - interval '30 days';
$$;
