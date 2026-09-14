create table video_intro_markers (
   video_id      integer primary key
      references videos ( id )
         on delete cascade,
   start_seconds double precision not null,
   end_seconds   double precision not null,
   source        text not null default 'detected',
   confidence    double precision not null default 1.0,
   created_at    timestamptz not null default now()
);